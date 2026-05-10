//! Translation between [`AsepriteDocument`] and [`PixhausArchive`].
//!
//! This is the lossy-direction code: features Aseprite carries that
//! Pixhaus does not model are dropped here, with warnings collected so
//! the editor can surface them to the artist. Going the other way is
//! lossless for everything in `docs/aseprite-compat.md`'s "read+write"
//! column; lossy for anything in "ignored".
//!
//! Two axes of conversion:
//! 1. Document → Archive: pure read path. Reader produces a Document,
//!    the editor calls [`document_to_archive`].
//! 2. Archive → Document: pure write path. Editor calls
//!    [`archive_to_document`], hands the result to [`super::write::encode`].

use std::collections::HashMap;

use pixhaus_core::project::{
    BrushState, CanvasState, Cel, CelData, ColorMode, FeatureFlags, Frame, FrameIndex, FrameRange,
    FrameTag, IVec2, Layer, LayerId, LayerKind, LoopDirection, NineSlice, Palette, PaletteEntry,
    PaletteFrameOverride, PaletteId, Pivot, PixelBufferId, Project, ProjectMetadata, Rect,
    SchemaVersion, SelectionState, Size, Slice, SliceId, SliceKey, Sprite, SpriteId, TileCell,
    TileFlags, TileIndex, TilemapData, Tileset, TilesetId, TilesetSource, UserData,
};

use crate::error::{Error, Result};
use crate::pixhaus::{PixelBufferEntry, PixhausArchive};

use super::chunk::{
    CelChunk, CelChunkData, Chunk, LayerChunk, LayerKindCode, NineSliceWire, PaletteChunk,
    PaletteEntryWire, PivotWire, SliceChunk, SliceKeyEntry, TagEntry, TagsChunk, TilesetChunk,
    TilesetSourceWire, UserDataChunk,
};
use super::document::{AsepriteDocument, ColorDepth, DocumentFrame, DocumentHeader};
use super::spec::{
    LAYER_FLAG_EDITABLE, LAYER_FLAG_GROUP_COLLAPSED, LAYER_FLAG_REFERENCE, LAYER_FLAG_VISIBLE,
};

/// Non-fatal warnings produced during conversion.
///
/// Each variant carries enough context for the editor to map back to
/// the affected entity so a user can investigate. The warnings list is
/// produced *alongside* the converted value — never as a fatal failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConversionWarning {
    /// An ICC color profile chunk was encountered. Pixhaus operates
    /// display-referred in sRGB; the profile is dropped.
    ColorProfileDiscarded,
    /// A user-data chunk's properties map was dropped. The entity's
    /// text and color survived.
    UserDataPropertiesDiscarded,
    /// A pixel-ratio other than 1:1 was declared in the file header.
    NonSquarePixelRatio {
        /// Pixel-ratio numerator (width).
        width: u8,
        /// Pixel-ratio denominator (height).
        height: u8,
    },
    /// A cel was encountered with non-zero z-index. Pixhaus uses layer
    /// ordering only.
    CelZIndexDropped {
        /// Original z-index on the cel.
        value: i16,
    },
    /// A tileset was declared with auto-flip flags set. Pixhaus does
    /// not implement auto-flip matching in v1.
    TilesetAutoFlipIgnored {
        /// The tileset name as it appears in the file.
        name: String,
    },
    /// An external tileset reference was encountered. Pixhaus inlines
    /// the tileset on save; the original file link is lost.
    ExternalTilesetInlined {
        /// The tileset name.
        name: String,
    },
    /// A layer chunk carried a 16-byte UUID. Pixhaus does not preserve
    /// per-layer UUIDs.
    LayerUuidDropped,
    /// A legacy 0x0004 / 0x0011 palette chunk appeared. The reader
    /// captures the colors but the data model carries them through the
    /// modern 0x2019 chunk only.
    LegacyPaletteEncountered,
    /// A blend-mode code outside the 0–18 range defined by the spec was
    /// encountered. The layer falls back to `BlendMode::Normal`. Aseprite
    /// extensions sometimes register custom blend modes here; round-
    /// tripping a file with an unknown code drops it.
    UnknownBlendMode {
        /// Raw blend-mode code from the layer chunk.
        code: u16,
    },
    /// One or more `UserData` chunks followed a `Palette` chunk.
    /// Aseprite uses these to attach metadata to individual palette
    /// entries; Pixhaus does not model per-entry palette user-data, so
    /// they are dropped rather than leaking onto whichever chunk
    /// preceded the palette. Emitted at most once per conversion.
    PaletteEntryUserDataDropped,
    /// An external-file tileset reference could not be resolved because
    /// the referenced file was not bundled with the document. The
    /// tileset is imported with no pixel data — callers should re-link
    /// the source after import.
    ExternalTilesetUnresolved {
        /// The external file path or name as it appears in the file.
        path: String,
    },
}

/// Result of converting a [`AsepriteDocument`] into a [`PixhausArchive`].
#[derive(Clone, Debug)]
pub struct ConvertedArchive {
    /// The translated archive.
    pub archive: PixhausArchive,
    /// Warnings raised during conversion.
    pub warnings: Vec<ConversionWarning>,
}

#[derive(Copy, Clone, Debug)]
enum UserDataTarget {
    /// No previous chunk owns the next `UserData`. Subsequent
    /// `UserData` chunks are dropped (Pixhaus doesn't model orphan
    /// user-data).
    None,
    /// A leading frame-0 `UserData` attaches to the sprite itself per
    /// the Aseprite spec. Initialised at the start of frame 0 and
    /// cleared after the first chunk that takes ownership.
    Sprite,
    Layer(usize),
    Cel(usize),
    /// Tag user-data is index-walked: each consecutive `UserData`
    /// chunk after a `Tags` chunk attaches to the *next* tag
    /// (`Tag(i)`, `Tag(i+1)`, ...) until the chunk run ends.
    Tag(usize),
    Slice(usize),
    Tileset(usize),
    /// Palette user-data drops on the floor: Aseprite supports per-
    /// entry user-data, but Pixhaus doesn't model it. Emit a warning
    /// the first time we drop one, then ignore the rest in the run.
    PaletteEntry,
}

/// Translate an [`AsepriteDocument`] into a [`PixhausArchive`], gathering
/// warnings for fields the data model doesn't preserve.
///
/// # Errors
///
/// - [`Error::InvalidLayerHierarchy`] when a layer chunk's child level
///   exceeds the depth of the layers seen so far.
/// - [`Error::UnknownCelLayer`] when a cel chunk references a layer
///   index that hasn't appeared yet.
#[allow(clippy::too_many_lines)]
pub fn document_to_archive(
    doc: &AsepriteDocument,
    sprite_name: impl Into<String>,
) -> Result<ConvertedArchive> {
    let mut warnings = Vec::new();
    if (doc.header.pixel_width != doc.header.pixel_height)
        || doc.header.pixel_width == 0
        || doc.header.pixel_height == 0
    {
        warnings.push(ConversionWarning::NonSquarePixelRatio {
            width: doc.header.pixel_width,
            height: doc.header.pixel_height,
        });
    }

    let color_mode = match doc.header.color_depth {
        ColorDepth::Rgba => ColorMode::Rgba,
        ColorDepth::Grayscale => ColorMode::Grayscale,
        ColorDepth::Indexed => ColorMode::Indexed,
    };

    let mut layers: Vec<Layer> = Vec::new();
    let mut layer_ids: Vec<LayerId> = Vec::new();
    let mut parent_stack: Vec<LayerId> = Vec::new();
    let mut tilesets: Vec<Tileset> = Vec::new();
    let mut tileset_id_by_aseprite_id: HashMap<u32, TilesetId> = HashMap::new();
    let mut frame_tags: Vec<FrameTag> = Vec::new();
    let mut base_palette: Vec<PaletteEntry> = Vec::new();
    let mut palette_frame_overrides: Vec<PaletteFrameOverride> = Vec::new();
    let mut slices: Vec<Slice> = Vec::new();
    let mut frames: Vec<Frame> = doc.frames.iter().map(frame_from_doc).collect();
    let mut cels: Vec<Cel> = Vec::new();
    let mut buffers: Vec<PixelBufferEntry> = Vec::new();
    let mut next_buffer_id: u32 = 1;
    let mut next_layer_id: u32 = 1;
    let mut next_tileset_id: u32 = 1;
    let mut next_slice_id: u32 = 1;

    // Pre-pass: materialise every Tileset chunk before the main loop so
    // tilemap layers can resolve their tileset references regardless of
    // chunk order in the source file. Aseprite emits tilesets and the
    // layers that reference them in arbitrary order, and a single linear
    // pass would silently fall back to TilesetId(0) whenever a layer
    // chunk arrived first.
    let mut tileset_ordinal_by_wire: HashMap<u32, usize> = HashMap::new();
    for frame in &doc.frames {
        for chunk in &frame.chunks {
            if let Chunk::Tileset(c) = chunk {
                if tileset_ordinal_by_wire.contains_key(&c.tileset_id) {
                    // Multiple Tileset chunks for the same wire id are
                    // not part of the spec; keep the first occurrence.
                    continue;
                }
                let id = TilesetId::new(next_tileset_id);
                next_tileset_id += 1;
                tileset_id_by_aseprite_id.insert(c.tileset_id, id);
                tileset_ordinal_by_wire.insert(c.tileset_id, tilesets.len());
                let tileset = tileset_from_chunk(
                    c,
                    id,
                    &mut buffers,
                    &mut next_buffer_id,
                    doc.header.color_depth,
                    &mut warnings,
                );
                tilesets.push(tileset);
            }
        }
    }

    let mut last_target = UserDataTarget::None;
    let mut sprite_user_data = UserData::default();
    let mut palette_user_data_warned = false;

    for (frame_index, frame) in doc.frames.iter().enumerate() {
        let frame_idx = FrameIndex::new(u32::try_from(frame_index).unwrap_or(u32::MAX));
        // The first chunk of frame 0 owns the sprite's UserData per the
        // Aseprite spec. Initialise the target so a leading UserData
        // chunk lands on the sprite instead of being dropped.
        if frame_index == 0 {
            last_target = UserDataTarget::Sprite;
        }
        for chunk in &frame.chunks {
            match chunk {
                Chunk::Layer(c) => {
                    let id = LayerId::new(next_layer_id);
                    next_layer_id += 1;
                    if c.uuid.is_some() {
                        warnings.push(ConversionWarning::LayerUuidDropped);
                    }
                    if let Some(code) = c.unknown_blend_code {
                        warnings.push(ConversionWarning::UnknownBlendMode { code });
                    }
                    let parent = derive_parent(&parent_stack, c.child_level)?;
                    parent_stack.truncate(c.child_level as usize);
                    parent_stack.push(id);
                    let layer = layer_from_chunk(c, id, parent, &tileset_id_by_aseprite_id);
                    layer_ids.push(id);
                    layers.push(layer);
                    last_target = UserDataTarget::Layer(layers.len() - 1);
                }
                Chunk::Cel(c) => {
                    let cel = cel_from_chunk(
                        c,
                        frame_idx,
                        &layer_ids,
                        &mut buffers,
                        &mut next_buffer_id,
                        doc.header.color_depth,
                        &mut warnings,
                    )?;
                    cels.push(cel);
                    last_target = UserDataTarget::Cel(cels.len() - 1);
                }
                Chunk::Tags(c) => {
                    let start = frame_tags.len();
                    frame_tags.extend(c.tags.iter().map(tag_from_chunk));
                    if c.tags.is_empty() {
                        last_target = UserDataTarget::None;
                    } else {
                        last_target = UserDataTarget::Tag(start);
                    }
                }
                Chunk::Palette(c) => {
                    let entries: Vec<PaletteEntry> =
                        c.entries.iter().map(palette_entry_from_wire).collect();
                    apply_palette_chunk(
                        frame_index,
                        c.first_index as usize,
                        c.last_index as usize,
                        c.palette_size as usize,
                        entries,
                        &mut base_palette,
                        &mut palette_frame_overrides,
                    );
                    // Aseprite per-entry palette user-data isn't modelled
                    // by Pixhaus; switch to the dropped-with-warning
                    // target so subsequent UserData chunks in the run
                    // don't latch onto whichever target preceded this
                    // palette chunk.
                    last_target = UserDataTarget::PaletteEntry;
                }
                Chunk::OldPalette255(_) | Chunk::OldPalette63(_) => {
                    warnings.push(ConversionWarning::LegacyPaletteEncountered);
                }
                Chunk::Slice(c) => {
                    slices.push(slice_from_chunk(c, SliceId::new(next_slice_id)));
                    next_slice_id += 1;
                    last_target = UserDataTarget::Slice(slices.len() - 1);
                }
                Chunk::Tileset(c) => {
                    // The chunk itself was materialised in the pre-pass;
                    // point user-data at the matching tileset by its
                    // wire id so any UserData chunk that follows lands
                    // on the right Pixhaus `Tileset`.
                    if let Some(&idx) = tileset_ordinal_by_wire.get(&c.tileset_id) {
                        last_target = UserDataTarget::Tileset(idx);
                    } else {
                        last_target = UserDataTarget::None;
                    }
                }
                Chunk::ColorProfile(_) => {
                    warnings.push(ConversionWarning::ColorProfileDiscarded);
                }
                Chunk::UserData(c) => {
                    if c.had_properties {
                        warnings.push(ConversionWarning::UserDataPropertiesDiscarded);
                    }
                    let user_data = user_data_from_chunk(c);
                    last_target = apply_user_data_to_target(
                        last_target,
                        user_data,
                        &mut sprite_user_data,
                        &mut layers,
                        &mut cels,
                        &mut frame_tags,
                        &mut slices,
                        &mut tilesets,
                        &mut palette_user_data_warned,
                        &mut warnings,
                    );
                }
                Chunk::ExternalFiles(_) | Chunk::Unknown { .. } => {
                    // External-files entries are read implicitly by the
                    // tileset chunk. Unknown chunks are dropped — a
                    // lossless writer would restore them, but Pixhaus
                    // exposes only modelled chunks back to the wire.
                }
            }
        }
    }

    // Reference layers carry their image on the layer itself, not as a
    // generic cel. Aseprite still ships the pixel data via a cel chunk
    // (because the wire format has no other way to deliver it), so we
    // pull the matching cel back out and seat its buffer + position on
    // the `LayerKind::Reference` value. Without this fixup the layer
    // reads as a transparent placeholder and round-tripping loses the
    // reference image.
    let layer_id_to_pos: HashMap<LayerId, usize> =
        layers.iter().enumerate().map(|(i, l)| (l.id, i)).collect();
    cels.retain(|cel| {
        let Some(&pos) = layer_id_to_pos.get(&cel.layer_id) else {
            return true;
        };
        if !matches!(layers[pos].kind, LayerKind::Reference { .. }) {
            return true;
        }
        if let CelData::Raster { buffer, .. } = &cel.data {
            layers[pos].kind = LayerKind::Reference {
                image: *buffer,
                origin: cel.position,
            };
        }
        false
    });

    let palette = if base_palette.is_empty() {
        Vec::new()
    } else {
        vec![Palette {
            id: PaletteId::new(1),
            name: "default".into(),
            colors: base_palette,
            user_data: UserData::default(),
        }]
    };

    let canvas = Size::new(u32::from(doc.header.width), u32::from(doc.header.height));

    let mut feature_flags = FeatureFlags::empty();
    if !tilesets.is_empty()
        || layers
            .iter()
            .any(|l| matches!(l.kind, LayerKind::Tilemap { .. }))
    {
        feature_flags = feature_flags.union(FeatureFlags::TILEMAPS);
    }
    if layers
        .iter()
        .any(|l| matches!(l.kind, LayerKind::Reference { .. }))
    {
        feature_flags = feature_flags.union(FeatureFlags::REFERENCES);
    }
    if !slices.is_empty() {
        feature_flags = feature_flags.union(FeatureFlags::SLICES);
    }

    let transparent_color_index = if matches!(color_mode, ColorMode::Indexed) {
        Some(doc.header.transparent_index)
    } else {
        None
    };

    let sprite = Sprite {
        id: SpriteId::new(1),
        name: sprite_name.into(),
        canvas,
        color_mode,
        transparent_color_index,
        layers,
        frames: std::mem::take(&mut frames),
        cels,
        palettes: palette,
        palette_frame_overrides,
        tilesets,
        frame_tags,
        animations: Vec::new(),
        slices,
        user_data: sprite_user_data,
    };

    let project = Project {
        schema_version: SchemaVersion::current(),
        feature_flags,
        metadata: ProjectMetadata {
            name: sprite.name.clone(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: env!("CARGO_PKG_VERSION").into(),
        },
        sprites: vec![sprite],
        library: pixhaus_core::project::Library::default(),
        canvas: CanvasState::default(),
        brush: BrushState::default(),
        selection: SelectionState::default(),
        active: pixhaus_core::project::ActiveTarget::None,
    };

    Ok(ConvertedArchive {
        archive: PixhausArchive { project, buffers },
        warnings,
    })
}

fn derive_parent(stack: &[LayerId], child_level: u16) -> Result<Option<LayerId>> {
    let depth = child_level as usize;
    if depth == 0 {
        return Ok(None);
    }
    if depth > stack.len() {
        return Err(Error::InvalidLayerHierarchy { child: child_level });
    }
    Ok(stack.get(depth - 1).copied())
}

/// Applies a `UserData` chunk to the target the previous chunk
/// announced and returns the target that should own the *next*
/// `UserData` chunk.
///
/// Most targets ([`UserDataTarget::Layer`], [`UserDataTarget::Cel`],
/// [`UserDataTarget::Slice`], [`UserDataTarget::Tileset`],
/// [`UserDataTarget::Sprite`]) consume one chunk and then transition to
/// [`UserDataTarget::None`] — back-to-back `UserData` chunks would
/// otherwise overwrite the same target. [`UserDataTarget::Tag`] walks
/// forward through the tag run, mirroring the spec's "`Tag1` →
/// `UserData`, `Tag2` → `UserData`, ..." sequencing.
/// [`UserDataTarget::PaletteEntry`] drops the chunk (Pixhaus has no
/// per-entry palette user-data) and stays in that state so an entire
/// palette user-data run is dropped without leaking onto whichever
/// chunk preceded the palette.
#[allow(clippy::too_many_arguments)]
fn apply_user_data_to_target(
    target: UserDataTarget,
    user_data: UserData,
    sprite_user_data: &mut UserData,
    layers: &mut [Layer],
    cels: &mut [Cel],
    tags: &mut [FrameTag],
    slices: &mut [Slice],
    tilesets: &mut [Tileset],
    palette_user_data_warned: &mut bool,
    warnings: &mut Vec<ConversionWarning>,
) -> UserDataTarget {
    match target {
        UserDataTarget::None => UserDataTarget::None,
        UserDataTarget::Sprite => {
            *sprite_user_data = user_data;
            UserDataTarget::None
        }
        UserDataTarget::Layer(idx) => {
            if let Some(layer) = layers.get_mut(idx) {
                layer.user_data = user_data;
            }
            UserDataTarget::None
        }
        UserDataTarget::Cel(idx) => {
            if let Some(cel) = cels.get_mut(idx) {
                cel.user_data = user_data;
            }
            UserDataTarget::None
        }
        UserDataTarget::Tag(idx) => {
            if let Some(tag) = tags.get_mut(idx) {
                tag.user_data = user_data;
            }
            UserDataTarget::Tag(idx + 1)
        }
        UserDataTarget::Slice(idx) => {
            if let Some(slice) = slices.get_mut(idx) {
                slice.user_data = user_data;
            }
            UserDataTarget::None
        }
        UserDataTarget::Tileset(idx) => {
            if let Some(tileset) = tilesets.get_mut(idx) {
                tileset.user_data = user_data;
            }
            UserDataTarget::None
        }
        UserDataTarget::PaletteEntry => {
            if !*palette_user_data_warned {
                warnings.push(ConversionWarning::PaletteEntryUserDataDropped);
                *palette_user_data_warned = true;
            }
            UserDataTarget::PaletteEntry
        }
    }
}

/// Merges a single Aseprite palette chunk into the running base palette
/// (frame 0) or per-frame override list (any other frame).
///
/// Aseprite palette chunks declare a `[first_index, last_index]` slice
/// rather than a complete palette: a writer that updates only entries
/// 5..=7 in frame 0 emits one chunk with `first_index=5, last_index=7,
/// entries=[c5, c6, c7]`. Replacing the whole palette with `entries`
/// clobbers the surrounding colours; instead resize the destination to
/// `palette_size` and overwrite the slice in place.
fn apply_palette_chunk(
    frame_index: usize,
    first_index: usize,
    last_index: usize,
    palette_size: usize,
    entries: Vec<PaletteEntry>,
    base_palette: &mut Vec<PaletteEntry>,
    palette_frame_overrides: &mut Vec<PaletteFrameOverride>,
) {
    if frame_index == 0 {
        merge_palette_slice(base_palette, first_index, last_index, palette_size, entries);
    } else {
        let frame_no = u32::try_from(frame_index).unwrap_or(u32::MAX);
        // The override list keeps a complete palette per frame, so seed
        // it from the base palette and then overlay this chunk's slice.
        if let Some(existing) = palette_frame_overrides
            .iter_mut()
            .find(|p| p.frame == frame_no)
        {
            merge_palette_slice(
                &mut existing.colors,
                first_index,
                last_index,
                palette_size,
                entries,
            );
        } else {
            let mut colors = base_palette.clone();
            merge_palette_slice(&mut colors, first_index, last_index, palette_size, entries);
            palette_frame_overrides.push(PaletteFrameOverride {
                frame: frame_no,
                colors,
            });
        }
    }
}

fn merge_palette_slice(
    target: &mut Vec<PaletteEntry>,
    first_index: usize,
    last_index: usize,
    palette_size: usize,
    entries: Vec<PaletteEntry>,
) {
    let needed_len = palette_size
        .max(last_index.saturating_add(1))
        .max(first_index.saturating_add(entries.len()));
    if target.len() < needed_len {
        target.resize(
            needed_len,
            PaletteEntry::new(pixhaus_core::project::Rgba::new(0, 0, 0, 0)),
        );
    }
    for (offset, entry) in entries.into_iter().enumerate() {
        let dst = first_index.saturating_add(offset);
        if let Some(slot) = target.get_mut(dst) {
            *slot = entry;
        }
    }
}

fn layer_from_chunk(
    c: &LayerChunk,
    id: LayerId,
    parent: Option<LayerId>,
    tileset_lookup: &HashMap<u32, TilesetId>,
) -> Layer {
    let visible = c.flags & LAYER_FLAG_VISIBLE != 0;
    let editable = c.flags & LAYER_FLAG_EDITABLE != 0;
    let kind = match c.kind {
        LayerKindCode::Normal => {
            if c.flags & LAYER_FLAG_REFERENCE != 0 {
                LayerKind::Reference {
                    image: PixelBufferId::new(0),
                    origin: IVec2::zero(),
                }
            } else {
                LayerKind::Raster
            }
        }
        LayerKindCode::Group => LayerKind::Group {
            collapsed: c.flags & LAYER_FLAG_GROUP_COLLAPSED != 0,
        },
        LayerKindCode::Tilemap => {
            let id = tileset_lookup
                .get(&c.tileset_index)
                .copied()
                .unwrap_or(TilesetId::new(0));
            LayerKind::Tilemap { tileset: id }
        }
    };
    Layer {
        id,
        name: c.name.clone(),
        kind,
        blend_mode: c.blend,
        opacity: c.opacity,
        visible,
        locked: !editable,
        parent,
        user_data: UserData::default(),
    }
}

fn cel_from_chunk(
    c: &CelChunk,
    frame: FrameIndex,
    layer_ids: &[LayerId],
    buffers: &mut Vec<PixelBufferEntry>,
    next_buffer_id: &mut u32,
    color_depth: ColorDepth,
    warnings: &mut Vec<ConversionWarning>,
) -> Result<Cel> {
    let layer_id =
        layer_ids
            .get(usize::from(c.layer_index))
            .copied()
            .ok_or(Error::UnknownCelLayer {
                layer: c.layer_index,
            })?;
    if c.z_index != 0 {
        warnings.push(ConversionWarning::CelZIndexDropped { value: c.z_index });
    }
    let position = IVec2::new(i32::from(c.x), i32::from(c.y));
    let data = match &c.data {
        CelChunkData::Raw {
            width,
            height,
            pixels,
        }
        | CelChunkData::Compressed {
            width,
            height,
            pixels,
        } => {
            let id = PixelBufferId::new(*next_buffer_id);
            *next_buffer_id += 1;
            buffers.push(PixelBufferEntry {
                id: id.get(),
                width: u32::from(*width),
                height: u32::from(*height),
                stride: u32::from(*width) * color_depth.bytes_per_pixel(),
                pixels: pixels.clone(),
            });
            CelData::Raster {
                buffer: id,
                size: Size::new(u32::from(*width), u32::from(*height)),
            }
        }
        CelChunkData::Linked { frame } => CelData::Linked {
            source_frame: FrameIndex::new(u32::from(*frame)),
        },
        CelChunkData::Tilemap {
            width,
            height,
            tile_id_mask,
            x_flip_mask,
            y_flip_mask,
            diagonal_flip_mask,
            tiles,
            ..
        } => {
            let mut data = TilemapData::empty(u32::from(*width), u32::from(*height));
            let id_shift = tile_id_mask.trailing_zeros();
            for (cell, raw) in data.cells.iter_mut().zip(tiles.iter().copied()) {
                let id = (raw & tile_id_mask) >> id_shift;
                let mut flags = TileFlags::empty();
                if raw & x_flip_mask != 0 {
                    flags = flags.union(TileFlags::FLIP_X);
                }
                if raw & y_flip_mask != 0 {
                    flags = flags.union(TileFlags::FLIP_Y);
                }
                if raw & diagonal_flip_mask != 0 {
                    flags = flags.union(TileFlags::FLIP_DIAGONAL);
                }
                *cell = TileCell {
                    index: TileIndex::new(id),
                    flags,
                };
            }
            CelData::Tilemap { data }
        }
    };
    Ok(Cel {
        layer_id,
        frame_index: frame,
        position,
        opacity: c.opacity,
        data,
        user_data: UserData::default(),
    })
}

fn frame_from_doc(frame: &DocumentFrame) -> Frame {
    Frame {
        duration_ms: u32::from(frame.duration_ms),
        user_data: UserData::default(),
    }
}

fn tag_from_chunk(c: &TagEntry) -> FrameTag {
    let direction = match c.loop_direction {
        1 => LoopDirection::Reverse,
        2 => LoopDirection::PingPong,
        3 => LoopDirection::PingPongReverse,
        _ => LoopDirection::Forward,
    };
    FrameTag {
        name: c.name.clone(),
        range: FrameRange::new(
            FrameIndex::new(u32::from(c.from_frame)),
            FrameIndex::new(u32::from(c.to_frame)),
        ),
        loop_direction: direction,
        repeat: c.repeat,
        user_data: UserData::default(),
    }
}

fn palette_entry_from_wire(c: &PaletteEntryWire) -> PaletteEntry {
    PaletteEntry {
        color: c.color,
        name: c.name.clone(),
    }
}

fn slice_from_chunk(c: &SliceChunk, id: SliceId) -> Slice {
    Slice {
        id,
        name: c.name.clone(),
        keys: c.keys.iter().map(slice_key_from_wire).collect(),
        user_data: UserData::default(),
    }
}

fn slice_key_from_wire(c: &SliceKeyEntry) -> SliceKey {
    SliceKey {
        frame: FrameIndex::new(c.frame),
        bounds: Rect::from_xywh(c.x, c.y, c.width, c.height),
        nine_slice: c.nine_slice.map(|n| NineSlice {
            center: Rect::from_xywh(n.x, n.y, n.width, n.height),
        }),
        pivot: c.pivot.map(|p| Pivot {
            offset: IVec2::new(p.x, p.y),
        }),
    }
}

fn tileset_from_chunk(
    c: &TilesetChunk,
    id: TilesetId,
    buffers: &mut Vec<PixelBufferEntry>,
    next_buffer_id: &mut u32,
    color_depth: ColorDepth,
    warnings: &mut Vec<ConversionWarning>,
) -> Tileset {
    if c.flags & 0b0011_1000 != 0 {
        warnings.push(ConversionWarning::TilesetAutoFlipIgnored {
            name: c.name.clone(),
        });
    }
    let source = match &c.source {
        TilesetSourceWire::External {
            external_file_id,
            external_tileset_id,
        } => {
            // External tilesets reference pixels that live in a sibling
            // file; resolving that file isn't yet wired through the
            // codec. The tileset is imported with an empty buffer so
            // the document still loads — surface a clear warning so the
            // caller can re-link the source rather than silently shipping
            // a transparent tilesheet.
            warnings.push(ConversionWarning::ExternalTilesetUnresolved {
                path: format!(
                    "external_file_id={external_file_id}, external_tileset_id={external_tileset_id}"
                ),
            });
            let buf_id = PixelBufferId::new(*next_buffer_id);
            *next_buffer_id += 1;
            buffers.push(PixelBufferEntry {
                id: buf_id.get(),
                width: u32::from(c.tile_width),
                height: u32::from(c.tile_height) * c.tile_count,
                stride: u32::from(c.tile_width) * color_depth.bytes_per_pixel(),
                pixels: Vec::new(),
            });
            TilesetSource::Inline { buffer: buf_id }
        }
        TilesetSourceWire::Inline { pixels } => {
            let buf_id = PixelBufferId::new(*next_buffer_id);
            *next_buffer_id += 1;
            buffers.push(PixelBufferEntry {
                id: buf_id.get(),
                width: u32::from(c.tile_width),
                height: u32::from(c.tile_height) * c.tile_count,
                stride: u32::from(c.tile_width) * color_depth.bytes_per_pixel(),
                pixels: pixels.clone(),
            });
            TilesetSource::Inline { buffer: buf_id }
        }
    };
    Tileset {
        id,
        name: c.name.clone(),
        tile_size: Size::new(u32::from(c.tile_width), u32::from(c.tile_height)),
        tile_count: c.tile_count,
        base_index: c.base_index,
        source,
        properties: Vec::new(),
        autotile: None,
        user_data: UserData::default(),
    }
}

fn user_data_from_chunk(c: &UserDataChunk) -> UserData {
    UserData {
        text: c.text.clone(),
        color: c.color,
    }
}

/// Translate a [`PixhausArchive`] into an [`AsepriteDocument`].
///
/// The first sprite drives the document; extra sprites are ignored.
/// Animations beyond frame tags are dropped. The encoder
/// ([`super::write::encode`]) is the layer that surfaces wire-level
/// validation errors (e.g. inline-tileset pixel-byte mismatches), so
/// this function is infallible.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn archive_to_document(archive: &PixhausArchive) -> AsepriteDocument {
    let Some(sprite) = archive.project.sprites.first() else {
        return AsepriteDocument::empty(0, 0);
    };
    let buffer_lookup: HashMap<u32, &PixelBufferEntry> =
        archive.buffers.iter().map(|b| (b.id, b)).collect();

    let color_depth = match sprite.color_mode {
        ColorMode::Rgba => ColorDepth::Rgba,
        ColorMode::Grayscale => ColorDepth::Grayscale,
        ColorMode::Indexed => ColorDepth::Indexed,
    };

    let mut header = DocumentHeader::rgba(
        u16::try_from(sprite.canvas.width).unwrap_or(u16::MAX),
        u16::try_from(sprite.canvas.height).unwrap_or(u16::MAX),
    );
    header.color_depth = color_depth;
    header.color_count =
        u16::try_from(sprite.palettes.first().map_or(0, |p| p.colors.len())).unwrap_or(u16::MAX);
    header.transparent_index = sprite.transparent_color_index.unwrap_or(0);

    let frame_durations: Vec<u16> = if sprite.frames.is_empty() {
        vec![100]
    } else {
        sprite
            .frames
            .iter()
            .map(|f| u16::try_from(f.duration_ms).unwrap_or(u16::MAX))
            .collect()
    };

    let mut frames: Vec<DocumentFrame> = frame_durations
        .iter()
        .map(|d| DocumentFrame::new(*d))
        .collect();

    let layer_index: HashMap<LayerId, u16> = sprite
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| (l.id, u16::try_from(i).unwrap_or(u16::MAX)))
        .collect();

    // Layer chunks reference a tileset by the *wire id* the matching
    // Tileset chunk emits (`TilesetChunk.tileset_id`), not its ordinal
    // position in the tileset list. Keying the lookup by ordinal worked
    // only when the Pixhaus `TilesetId` values happened to be `0..n-1`;
    // any project that re-orders tilesets or reuses ids breaks otherwise.
    let tileset_index: HashMap<TilesetId, u32> =
        sprite.tilesets.iter().map(|t| (t.id, t.id.get())).collect();

    if let Some(frame0) = frames.first_mut() {
        // Sprite-level user-data must be the first UserData chunk in
        // frame 0 — Aseprite's convention is "the first chunk owns the
        // sprite". Emitting it ahead of the layer/cel chunks below keeps
        // it from being attached to the wrong target on read.
        if !sprite.user_data.is_empty() {
            frame0
                .chunks
                .push(Chunk::UserData(user_data_to_chunk(&sprite.user_data)));
        }
        let mut child_levels: HashMap<LayerId, u16> = HashMap::new();
        for layer in &sprite.layers {
            let level = compute_child_level(layer, &sprite.layers, &mut child_levels);
            frame0
                .chunks
                .push(Chunk::Layer(layer_to_chunk(layer, level, &tileset_index)));
            if !layer.user_data.is_empty() {
                frame0
                    .chunks
                    .push(Chunk::UserData(user_data_to_chunk(&layer.user_data)));
            }
        }
    }

    // Reference-layer images live on `LayerKind::Reference` rather than
    // in `sprite.cels`. Synthesise a cel chunk on frame 0 for each one
    // so the wire format actually carries the reference image; without
    // this the layer round-trips as transparent.
    let synthesized_reference_cels: Vec<Cel> = sprite
        .layers
        .iter()
        .filter_map(|l| match &l.kind {
            LayerKind::Reference { image, origin } => {
                let size = buffer_lookup
                    .get(&image.get())
                    .map_or(Size::new(0, 0), |b| Size::new(b.width, b.height));
                Some(Cel {
                    layer_id: l.id,
                    frame_index: FrameIndex::new(0),
                    position: *origin,
                    opacity: 255,
                    data: CelData::Raster {
                        buffer: *image,
                        size,
                    },
                    user_data: UserData::default(),
                })
            }
            _ => None,
        })
        .collect();

    let mut cels_by_frame: Vec<Vec<&Cel>> = vec![Vec::new(); frames.len()];
    for cel in &sprite.cels {
        let idx = cel.frame_index.get() as usize;
        if let Some(bucket) = cels_by_frame.get_mut(idx) {
            bucket.push(cel);
        }
    }
    for synth in &synthesized_reference_cels {
        if let Some(bucket) = cels_by_frame.first_mut() {
            bucket.push(synth);
        }
    }
    for (frame_idx, bucket) in cels_by_frame.iter().enumerate() {
        let Some(frame) = frames.get_mut(frame_idx) else {
            continue;
        };
        for cel in bucket {
            let chunk = cel_to_chunk(cel, &layer_index, &buffer_lookup, color_depth);
            frame.chunks.push(Chunk::Cel(chunk));
            if !cel.user_data.is_empty() {
                frame
                    .chunks
                    .push(Chunk::UserData(user_data_to_chunk(&cel.user_data)));
            }
        }
    }

    if let Some(frame0) = frames.first_mut() {
        if !sprite.frame_tags.is_empty() {
            frame0
                .chunks
                .push(Chunk::Tags(tags_to_chunk(&sprite.frame_tags)));
            for tag in &sprite.frame_tags {
                if !tag.user_data.is_empty() {
                    frame0
                        .chunks
                        .push(Chunk::UserData(user_data_to_chunk(&tag.user_data)));
                }
            }
        }
        if let Some(palette) = sprite.palettes.first()
            && !palette.colors.is_empty()
        {
            // An empty palette would serialize with `last_index = 0`
            // and `entries = []`; the reader uses last_index to allocate
            // one entry and then hits EOF trying to fill it. Skip the
            // chunk entirely when there are no colours to emit.
            frame0
                .chunks
                .push(Chunk::Palette(palette_to_chunk(palette)));
        }
        for slice in &sprite.slices {
            frame0.chunks.push(Chunk::Slice(slice_to_chunk(slice)));
            if !slice.user_data.is_empty() {
                frame0
                    .chunks
                    .push(Chunk::UserData(user_data_to_chunk(&slice.user_data)));
            }
        }
        for tileset in &sprite.tilesets {
            frame0.chunks.push(Chunk::Tileset(tileset_to_chunk(
                tileset,
                &buffer_lookup,
                color_depth,
            )));
            if !tileset.user_data.is_empty() {
                frame0
                    .chunks
                    .push(Chunk::UserData(user_data_to_chunk(&tileset.user_data)));
            }
        }
    }

    for override_entry in &sprite.palette_frame_overrides {
        let frame_idx = override_entry.frame as usize;
        if frame_idx == 0 {
            // Frame-0 palette state is carried by sprite.palettes; an
            // override pinned at frame 0 would duplicate the chunk.
            continue;
        }
        if override_entry.colors.is_empty() {
            continue;
        }
        let Some(frame) = frames.get_mut(frame_idx) else {
            continue;
        };
        frame.chunks.push(Chunk::Palette(palette_chunk_from_entries(
            &override_entry.colors,
        )));
    }

    AsepriteDocument { header, frames }
}

fn compute_child_level(layer: &Layer, layers: &[Layer], cache: &mut HashMap<LayerId, u16>) -> u16 {
    if let Some(level) = cache.get(&layer.id) {
        return *level;
    }
    let level = match layer.parent {
        None => 0,
        Some(parent_id) => match layers.iter().find(|l| l.id == parent_id) {
            Some(parent) => compute_child_level(parent, layers, cache).saturating_add(1),
            None => 0,
        },
    };
    cache.insert(layer.id, level);
    level
}

fn layer_to_chunk(
    layer: &Layer,
    child_level: u16,
    tileset_index: &HashMap<TilesetId, u32>,
) -> LayerChunk {
    let mut flags: u16 = 0;
    if layer.visible {
        flags |= LAYER_FLAG_VISIBLE;
    }
    if !layer.locked {
        flags |= LAYER_FLAG_EDITABLE;
    }
    let (kind, tileset_idx) = match layer.kind {
        LayerKind::Raster => (LayerKindCode::Normal, 0),
        LayerKind::Group { collapsed } => {
            if collapsed {
                flags |= LAYER_FLAG_GROUP_COLLAPSED;
            }
            (LayerKindCode::Group, 0)
        }
        LayerKind::Tilemap { tileset } => (
            LayerKindCode::Tilemap,
            tileset_index.get(&tileset).copied().unwrap_or(0),
        ),
        LayerKind::Reference { .. } => {
            flags |= LAYER_FLAG_REFERENCE;
            (LayerKindCode::Normal, 0)
        }
    };
    LayerChunk {
        flags,
        kind,
        child_level,
        blend: layer.blend_mode,
        unknown_blend_code: None,
        opacity: layer.opacity,
        name: layer.name.clone(),
        tileset_index: tileset_idx,
        uuid: None,
    }
}

fn cel_to_chunk(
    cel: &Cel,
    layer_index: &HashMap<LayerId, u16>,
    buffers: &HashMap<u32, &PixelBufferEntry>,
    color_depth: ColorDepth,
) -> CelChunk {
    let layer = layer_index.get(&cel.layer_id).copied().unwrap_or(0);
    let data = match &cel.data {
        CelData::Raster { buffer, size } => {
            let pixels = buffers
                .get(&buffer.get())
                .map(|entry| pixel_bytes_for_aseprite(entry, *size, color_depth))
                .unwrap_or_default();
            CelChunkData::Compressed {
                width: u16::try_from(size.width).unwrap_or(u16::MAX),
                height: u16::try_from(size.height).unwrap_or(u16::MAX),
                pixels,
            }
        }
        CelData::Linked { source_frame } => CelChunkData::Linked {
            frame: u16::try_from(source_frame.get()).unwrap_or(u16::MAX),
        },
        CelData::Tilemap { data } => {
            let tile_id_mask: u32 = 0x1FFF_FFFF;
            let x_flip_mask: u32 = 0x2000_0000;
            let y_flip_mask: u32 = 0x4000_0000;
            let diagonal_flip_mask: u32 = 0x8000_0000;
            let mut tiles = Vec::with_capacity(data.cells.len());
            for cell in &data.cells {
                let mut raw = cell.index.get() & tile_id_mask;
                if cell.flags.contains(TileFlags::FLIP_X) {
                    raw |= x_flip_mask;
                }
                if cell.flags.contains(TileFlags::FLIP_Y) {
                    raw |= y_flip_mask;
                }
                if cell.flags.contains(TileFlags::FLIP_DIAGONAL) {
                    raw |= diagonal_flip_mask;
                }
                tiles.push(raw);
            }
            CelChunkData::Tilemap {
                width: u16::try_from(data.width).unwrap_or(u16::MAX),
                height: u16::try_from(data.height).unwrap_or(u16::MAX),
                bits_per_tile: 32,
                tile_id_mask,
                x_flip_mask,
                y_flip_mask,
                diagonal_flip_mask,
                tiles,
            }
        }
    };
    CelChunk {
        layer_index: layer,
        x: i16::try_from(cel.position.x).unwrap_or(i16::MIN),
        y: i16::try_from(cel.position.y).unwrap_or(i16::MIN),
        opacity: cel.opacity,
        z_index: 0,
        data,
    }
}

fn pixel_bytes_for_aseprite(
    entry: &PixelBufferEntry,
    size: Size,
    color_depth: ColorDepth,
) -> Vec<u8> {
    let bpp = color_depth.bytes_per_pixel() as usize;
    let row = size.width as usize * bpp;
    let stride = entry.stride as usize;
    if stride == row && entry.pixels.len() == row * size.height as usize {
        return entry.pixels.clone();
    }
    let mut out = Vec::with_capacity(row * size.height as usize);
    for y in 0..size.height as usize {
        let start = y * stride;
        let end = start + row;
        if let Some(slice) = entry.pixels.get(start..end) {
            out.extend_from_slice(slice);
        } else {
            out.resize(out.len() + row, 0);
        }
    }
    out
}

fn tags_to_chunk(tags: &[FrameTag]) -> TagsChunk {
    TagsChunk {
        tags: tags
            .iter()
            .map(|t| TagEntry {
                from_frame: u16::try_from(t.range.start.get()).unwrap_or(u16::MAX),
                to_frame: u16::try_from(t.range.end.get()).unwrap_or(u16::MAX),
                loop_direction: match t.loop_direction {
                    LoopDirection::Forward => 0,
                    LoopDirection::Reverse => 1,
                    LoopDirection::PingPong => 2,
                    LoopDirection::PingPongReverse => 3,
                },
                repeat: t.repeat,
                name: t.name.clone(),
                deprecated_color: [0, 0, 0],
            })
            .collect(),
    }
}

fn palette_to_chunk(palette: &Palette) -> PaletteChunk {
    palette_chunk_from_entries(&palette.colors)
}

fn palette_chunk_from_entries(colors: &[PaletteEntry]) -> PaletteChunk {
    let last_index = colors.len().saturating_sub(1);
    PaletteChunk {
        palette_size: u32::try_from(colors.len()).unwrap_or(u32::MAX),
        first_index: 0,
        last_index: u32::try_from(last_index).unwrap_or(u32::MAX),
        entries: colors
            .iter()
            .map(|e| PaletteEntryWire {
                color: e.color,
                name: e.name.clone(),
            })
            .collect(),
    }
}

fn slice_to_chunk(slice: &Slice) -> SliceChunk {
    let has_nine_slice = slice.keys.iter().any(|k| k.nine_slice.is_some());
    let has_pivot = slice.keys.iter().any(|k| k.pivot.is_some());
    SliceChunk {
        name: slice.name.clone(),
        has_nine_slice,
        has_pivot,
        keys: slice
            .keys
            .iter()
            .map(|k| SliceKeyEntry {
                frame: k.frame.get(),
                x: k.bounds.origin.x,
                y: k.bounds.origin.y,
                width: k.bounds.size.width,
                height: k.bounds.size.height,
                nine_slice: k.nine_slice.map(|n| NineSliceWire {
                    x: n.center.origin.x,
                    y: n.center.origin.y,
                    width: n.center.size.width,
                    height: n.center.size.height,
                }),
                pivot: k.pivot.map(|p| PivotWire {
                    x: p.offset.x,
                    y: p.offset.y,
                }),
            })
            .collect(),
    }
}

fn tileset_to_chunk(
    tileset: &Tileset,
    buffers: &HashMap<u32, &PixelBufferEntry>,
    color_depth: ColorDepth,
) -> TilesetChunk {
    let pixels = match &tileset.source {
        TilesetSource::Inline { buffer } => buffers
            .get(&buffer.get())
            .map(|entry| {
                pixel_bytes_for_aseprite(
                    entry,
                    Size::new(
                        tileset.tile_size.width,
                        tileset.tile_size.height * tileset.tile_count,
                    ),
                    color_depth,
                )
            })
            .unwrap_or_default(),
        TilesetSource::External { .. } => Vec::new(),
    };
    TilesetChunk {
        tileset_id: tileset.id.get(),
        flags: 0,
        tile_count: tileset.tile_count,
        tile_width: u16::try_from(tileset.tile_size.width).unwrap_or(u16::MAX),
        tile_height: u16::try_from(tileset.tile_size.height).unwrap_or(u16::MAX),
        base_index: tileset.base_index,
        name: tileset.name.clone(),
        source: TilesetSourceWire::Inline { pixels },
    }
}

fn user_data_to_chunk(data: &UserData) -> UserDataChunk {
    UserDataChunk {
        text: data.text.clone(),
        color: data.color,
        had_properties: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_archive_to_document_yields_empty() {
        let archive = PixhausArchive::new(Project::new("test"));
        let doc = archive_to_document(&archive);
        assert!(doc.frames.is_empty());
    }

    #[test]
    fn document_to_archive_preserves_canvas_size() {
        let mut doc = AsepriteDocument::empty(64, 32);
        doc.frames.push(DocumentFrame::new(100));
        let converted = document_to_archive(&doc, "test").unwrap();
        let sprite = converted.archive.project.sprites.first().unwrap();
        assert_eq!(sprite.canvas, Size::new(64, 32));
    }

    #[test]
    fn unknown_blend_mode_surfaces_warning() {
        // Build a one-frame document carrying a layer chunk whose
        // unknown_blend_code is set: the archive layer must surface
        // ConversionWarning::UnknownBlendMode and fall back to Normal.
        let mut doc = AsepriteDocument::empty(8, 8);
        doc.frames.push(DocumentFrame::new(100));
        doc.frames[0].chunks.push(Chunk::Layer(LayerChunk {
            flags: LAYER_FLAG_VISIBLE | LAYER_FLAG_EDITABLE,
            kind: LayerKindCode::Normal,
            child_level: 0,
            blend: pixhaus_core::project::BlendMode::Normal,
            unknown_blend_code: Some(99),
            opacity: 255,
            name: "main".into(),
            tileset_index: 0,
            uuid: None,
        }));
        let converted = document_to_archive(&doc, "x").unwrap();
        assert!(matches!(
            converted.warnings.first(),
            Some(ConversionWarning::UnknownBlendMode { code: 99 })
        ));
    }

    #[test]
    fn indexed_color_mode_pulls_transparent_index_from_header() {
        let mut doc = AsepriteDocument::empty(8, 8);
        doc.header.color_depth = ColorDepth::Indexed;
        doc.header.transparent_index = 7;
        doc.frames.push(DocumentFrame::new(100));
        let converted = document_to_archive(&doc, "indexed").unwrap();
        let sprite = converted.archive.project.sprites.first().unwrap();
        assert_eq!(sprite.color_mode, ColorMode::Indexed);
        assert_eq!(sprite.transparent_color_index, Some(7));
    }

    #[test]
    fn rgba_color_mode_leaves_transparent_index_none() {
        let mut doc = AsepriteDocument::empty(8, 8);
        doc.header.transparent_index = 4; // header carries it; we ignore in RGBA
        doc.frames.push(DocumentFrame::new(100));
        let converted = document_to_archive(&doc, "rgba").unwrap();
        let sprite = converted.archive.project.sprites.first().unwrap();
        assert_eq!(sprite.color_mode, ColorMode::Rgba);
        assert!(sprite.transparent_color_index.is_none());
    }

    #[test]
    fn per_frame_palette_chunks_become_overrides() {
        use pixhaus_core::project::Rgba;

        // Three frames; frame 0 declares the base palette, frame 1
        // swaps swatch 1 — the archive must capture the override
        // separately so it can be re-emitted on write.
        let mut doc = AsepriteDocument::empty(8, 8);
        doc.frames.push(DocumentFrame::new(100));
        doc.frames.push(DocumentFrame::new(100));
        doc.frames.push(DocumentFrame::new(100));
        doc.frames[0].chunks.push(Chunk::Palette(PaletteChunk {
            palette_size: 2,
            first_index: 0,
            last_index: 1,
            entries: vec![
                PaletteEntryWire {
                    color: Rgba::transparent(),
                    name: None,
                },
                PaletteEntryWire {
                    color: Rgba::opaque(10, 20, 30),
                    name: None,
                },
            ],
        }));
        doc.frames[1].chunks.push(Chunk::Palette(PaletteChunk {
            palette_size: 2,
            first_index: 0,
            last_index: 1,
            entries: vec![
                PaletteEntryWire {
                    color: Rgba::transparent(),
                    name: None,
                },
                PaletteEntryWire {
                    color: Rgba::opaque(99, 99, 99),
                    name: None,
                },
            ],
        }));
        let converted = document_to_archive(&doc, "p").unwrap();
        let sprite = converted.archive.project.sprites.first().unwrap();
        assert_eq!(sprite.palettes.len(), 1);
        assert_eq!(sprite.palettes[0].colors[1].color, Rgba::opaque(10, 20, 30));
        assert_eq!(sprite.palette_frame_overrides.len(), 1);
        assert_eq!(sprite.palette_frame_overrides[0].frame, 1);
        assert_eq!(
            sprite.palette_frame_overrides[0].colors[1].color,
            Rgba::opaque(99, 99, 99)
        );
    }

    #[test]
    fn palette_overrides_round_trip_to_document() {
        use pixhaus_core::project::{PaletteEntry as CorePaletteEntry, Rgba};

        // Build a sprite carrying a base palette + one frame-1 override,
        // run it through archive_to_document, and check the document
        // carries one Palette chunk in frame 0 and one in frame 1.
        let mut sprite =
            pixhaus_core::project::Sprite::empty(SpriteId::new(1), "p", Size::new(8, 8));
        sprite.frames = vec![
            pixhaus_core::project::Frame::default(),
            pixhaus_core::project::Frame::default(),
            pixhaus_core::project::Frame::default(),
        ];
        sprite.palettes = vec![Palette {
            id: PaletteId::new(1),
            name: "default".into(),
            colors: vec![
                CorePaletteEntry::new(Rgba::transparent()),
                CorePaletteEntry::new(Rgba::opaque(10, 20, 30)),
            ],
            user_data: UserData::default(),
        }];
        sprite.palette_frame_overrides = vec![PaletteFrameOverride {
            frame: 1,
            colors: vec![
                CorePaletteEntry::new(Rgba::transparent()),
                CorePaletteEntry::new(Rgba::opaque(99, 99, 99)),
            ],
        }];
        let mut project = Project::new("p");
        project.sprites = vec![sprite];
        let archive = PixhausArchive {
            project,
            buffers: Vec::new(),
        };
        let doc = archive_to_document(&archive);
        let frame0_palette_count = doc.frames[0]
            .chunks
            .iter()
            .filter(|c| matches!(c, Chunk::Palette(_)))
            .count();
        let frame1_palette_count = doc.frames[1]
            .chunks
            .iter()
            .filter(|c| matches!(c, Chunk::Palette(_)))
            .count();
        let frame2_palette_count = doc.frames[2]
            .chunks
            .iter()
            .filter(|c| matches!(c, Chunk::Palette(_)))
            .count();
        assert_eq!(frame0_palette_count, 1);
        assert_eq!(frame1_palette_count, 1);
        assert_eq!(frame2_palette_count, 0);
    }

    #[test]
    fn tileset_base_index_round_trips() {
        // Read side: a tileset chunk with base_index = 5 produces a
        // Tileset model with base_index = 5.
        let mut doc = AsepriteDocument::empty(8, 8);
        doc.frames.push(DocumentFrame::new(100));
        doc.frames[0].chunks.push(Chunk::Tileset(TilesetChunk {
            tileset_id: 1,
            flags: 0,
            tile_count: 1,
            tile_width: 4,
            tile_height: 4,
            base_index: 5,
            name: "decals".into(),
            source: TilesetSourceWire::Inline {
                pixels: vec![0u8; 4 * 4 * 4],
            },
        }));
        let converted = document_to_archive(&doc, "t").unwrap();
        let sprite = converted.archive.project.sprites.first().unwrap();
        assert_eq!(sprite.tilesets[0].base_index, 5);

        // Write side: archive_to_document carries base_index back through.
        let doc_back = archive_to_document(&converted.archive);
        let chunk = doc_back.frames[0].chunks.iter().find_map(|c| match c {
            Chunk::Tileset(t) => Some(t.clone()),
            _ => None,
        });
        assert_eq!(chunk.map(|c| c.base_index), Some(5));
    }

    #[test]
    fn transparent_index_round_trips_through_archive_to_document() {
        // Build an indexed sprite with transparent_color_index=3, run it
        // through archive_to_document, and verify the header carries 3.
        let mut sprite =
            pixhaus_core::project::Sprite::empty(SpriteId::new(1), "ix", Size::new(8, 8));
        sprite.color_mode = ColorMode::Indexed;
        sprite.transparent_color_index = Some(3);
        let mut project = Project::new("ix");
        project.sprites = vec![sprite];
        let archive = PixhausArchive {
            project,
            buffers: Vec::new(),
        };
        let doc = archive_to_document(&archive);
        assert_eq!(doc.header.color_depth, ColorDepth::Indexed);
        assert_eq!(doc.header.transparent_index, 3);
    }

    #[test]
    fn child_level_walks_parent_chain() {
        let mut layers = vec![
            Layer {
                id: LayerId::new(1),
                name: "root".into(),
                kind: LayerKind::Group { collapsed: false },
                blend_mode: pixhaus_core::project::BlendMode::Normal,
                opacity: 255,
                visible: true,
                locked: false,
                parent: None,
                user_data: UserData::default(),
            },
            Layer {
                id: LayerId::new(2),
                name: "child".into(),
                kind: LayerKind::Group { collapsed: false },
                blend_mode: pixhaus_core::project::BlendMode::Normal,
                opacity: 255,
                visible: true,
                locked: false,
                parent: Some(LayerId::new(1)),
                user_data: UserData::default(),
            },
            Layer {
                id: LayerId::new(3),
                name: "leaf".into(),
                kind: LayerKind::Raster,
                blend_mode: pixhaus_core::project::BlendMode::Normal,
                opacity: 255,
                visible: true,
                locked: false,
                parent: Some(LayerId::new(2)),
                user_data: UserData::default(),
            },
        ];
        let mut cache = HashMap::new();
        assert_eq!(compute_child_level(&layers[0], &layers, &mut cache), 0);
        assert_eq!(compute_child_level(&layers[1], &layers, &mut cache), 1);
        assert_eq!(compute_child_level(&layers[2], &layers, &mut cache), 2);
        let _ = layers.pop();
    }
}
