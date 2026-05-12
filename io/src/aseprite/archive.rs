//! Translate between [`AsepriteDocument`] and [`PixhausArchive`].
//!
//! # Import — `document_to_archive`
//!
//! An Aseprite file is a single animated sprite. Frame tags become named
//! states inside a `Custom("Sprite")` entity. A file with no tags gets
//! one state named `"default"` that spans all frames.
//!
//! Layers, palettes, and slices from the document's structural frame
//! (frame 0) are shared across all states. Cels are partitioned by the
//! tag's frame range and re-indexed so each state starts at frame 0.
//!
//! Pixel bytes are extracted from the decompressed cel payloads and
//! placed into `PixhausArchive::buffers`; each raster cel gets one
//! `PixelBufferEntry` with a monotonically-minted `PixelBufferId`.
//!
//! # Export — `archive_to_document`
//!
//! Finds the active entity or the first `Custom`-kind entity with at
//! least one state and merges all states into a single Aseprite document:
//!
//! - Frame 0 contains structural chunks (layers, frame tags, palette,
//!   slices) plus the cels for the first frame of the first state.
//! - Subsequent frames hold the remaining cels in state-then-frame order.
//! - One `TagEntry` is emitted per state, covering its contiguous frame
//!   range in the merged timeline.
//!
//! # Export — `archive_to_per_state_documents`
//!
//! The B9 design default: each state becomes its own `.aseprite` file.
//! Each returned [`PerStateDocument`] has a `filename_stem` derived from
//! the state name and a single-state `AsepriteDocument` with no tag
//! chunk (one state per file makes tags unnecessary).
//!
//! Out-of-bounds linked cels (pointing to a frame index beyond the
//! state's frame count) produce a [`ConversionWarning::CrossStateLinkedCel`]
//! and are dropped; the affected frame has no cel for that layer.

use std::collections::HashMap;

use pixhaus_core::project::frame::Frame;
use pixhaus_core::project::id::{LayerId, PaletteId};
use pixhaus_core::project::library::{
    AiMetadata as LibAiMetadata, Entity, EntityContent as LibEntityContent,
    EntityDefaults as LibEntityDefaults, EntityKind as LibEntityKind,
    NamedSprite as LibNamedSprite,
};
use pixhaus_core::project::tileset::TilesetSource as CoreTilesetSource;
use pixhaus_core::project::{
    ActiveTarget, BlendMode, Cel, CelData, ColorMode, EntityId, FrameIndex, IVec2, Layer,
    LayerKind, NineSlice, Palette, PaletteEntry, PaletteFrameOverride, Pivot, Rect, Size, Slice,
    SliceId, SliceKey, Sprite, SpriteId, StateId, TileCell, TileFlags, TileIndex, TilemapData,
    Tileset, TilesetId, UserData,
};

use crate::error::{Error, Result};
use crate::pixhaus::{PixelBufferEntry, PixhausArchive};

use super::chunk::{
    CelChunk, CelChunkData, Chunk, ExternalFileEntry, ExternalFilesChunk, LayerChunk,
    LayerKindCode, NineSliceWire, PaletteChunk, PaletteEntryWire, PivotWire, SliceChunk,
    SliceKeyEntry, TagEntry, TagsChunk, TilesetChunk, TilesetSourceWire,
};
use super::document::{AsepriteDocument, ColorDepth, DocumentFrame, DocumentHeader};
use super::spec::{LAYER_FLAG_EDITABLE, LAYER_FLAG_GROUP_COLLAPSED, LAYER_FLAG_VISIBLE};

/// Non-fatal warnings produced during conversion.
///
/// `#[non_exhaustive]` lets downstream `match` arms remain open as new
/// variants arrive in future streams without a breaking change.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConversionWarning {
    /// A layer had an unrecognized Aseprite blend-mode code. The layer's
    /// blend mode was set to `Normal`; the original code is preserved
    /// in `LayerChunk::unknown_blend_code` for round-trip fidelity.
    UnknownBlendMode {
        /// Display name of the affected layer.
        layer_name: String,
        /// The unrecognized wire-level blend-mode code.
        code: u16,
    },
    /// A linked cel in `from_state` has a `source_frame` that points
    /// outside the state's frame range. The cel was dropped; the frame
    /// has no pixel data for that layer rather than a corrupt link.
    CrossStateLinkedCel {
        /// State being exported that contains the out-of-bounds link.
        from_state: StateId,
        /// State the frame index resolves to in the merged timeline, or
        /// the same as `from_state` when it cannot be determined.
        to_state: StateId,
    },
    /// An Aseprite tag's `from_frame` is >= the document's frame count.
    /// The tag was skipped; no state was created for it.
    InvalidTagRange {
        /// Display name of the skipped tag.
        tag_name: String,
        /// `from_frame` value from the tag.
        from: u16,
        /// `to_frame` value from the tag.
        to: u16,
        /// Number of frames in the document.
        frame_count: usize,
    },
    /// An external tileset chunk references a file ID not present in the
    /// document's `ExternalFiles` chunk. The tileset was imported with an
    /// empty path string; the warning lets callers surface the breakage.
    MissingExternalFileEntry {
        /// Tileset ID of the affected tileset.
        tileset_id: u32,
        /// The `external_file_id` that was not found.
        external_file_id: u32,
    },
}

/// One state's worth of Aseprite content, ready to encode into a
/// `.aseprite` file.
///
/// `filename_stem` is the state name sanitised for disk: ASCII-lowercase,
/// non-alphanumeric characters replaced with hyphens, consecutive hyphens
/// collapsed, leading/trailing hyphens trimmed. Falls back to `"state"`
/// when the result would otherwise be empty.
#[derive(Debug)]
pub struct PerStateDocument {
    /// Identity of the state in the source archive.
    pub state_id: StateId,
    /// Original state name, unsanitised.
    pub state_name: String,
    /// Filename-safe rendering of `state_name`.
    pub filename_stem: String,
    /// The translated Aseprite document for this state.
    pub document: AsepriteDocument,
    /// Non-fatal warnings raised while exporting this state.
    pub warnings: Vec<ConversionWarning>,
}

/// Result of converting an [`AsepriteDocument`] into a [`PixhausArchive`].
#[derive(Clone, Debug)]
pub struct ConvertedArchive {
    /// The translated archive.
    pub archive: PixhausArchive,
    /// Non-fatal warnings raised during conversion.
    pub warnings: Vec<ConversionWarning>,
}

// ── Mutable import state ──────────────────────────────────────────────────────

struct ImportCtx {
    /// Monotonic counter for minting `PixelBufferId`s.
    next_id: u32,
    /// Accumulated pixel buffers (raster cels + tilesets).
    buffers: Vec<PixelBufferEntry>,
}

impl ImportCtx {
    fn new() -> Self {
        Self {
            next_id: 1,
            buffers: Vec::new(),
        }
    }

    fn add_buffer(&mut self, width: u32, height: u32, stride: u32, pixels: Vec<u8>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.buffers.push(PixelBufferEntry {
            id,
            width,
            height,
            stride,
            pixels,
        });
        id
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Translate an [`AsepriteDocument`] into a [`PixhausArchive`].
///
/// Frame tags in the document become named states inside a single
/// `Custom("Sprite")` entity. A document with no tags gets one state
/// named `"default"` spanning all frames.
///
/// # Errors
///
/// - [`Error::NoFrames`] when the document has no frames.
/// - [`Error::UnknownCelLayer`] when a cel references a layer index
///   that does not exist in the document's layer list.
#[allow(clippy::too_many_lines)]
pub fn document_to_archive(
    doc: &AsepriteDocument,
    sprite_name: impl Into<String>,
) -> Result<ConvertedArchive> {
    if doc.frames.is_empty() {
        return Err(Error::NoFrames);
    }

    let sprite_name: String = sprite_name.into();
    let mut warnings: Vec<ConversionWarning> = Vec::new();
    let mut ctx = ImportCtx::new();

    let frame0 = &doc.frames[0];

    // ── Structural chunks from frame 0 ────────────────────────────────────────

    let layer_chunks: Vec<&LayerChunk> = frame0
        .chunks
        .iter()
        .filter_map(|c| {
            if let Chunk::Layer(l) = c {
                Some(l)
            } else {
                None
            }
        })
        .collect();

    let (layers, layer_ids) = build_layers(&layer_chunks, &mut warnings);

    let tags: Vec<&TagEntry> = frame0
        .chunks
        .iter()
        .find_map(|c| {
            if let Chunk::Tags(t) = c {
                Some(&t.tags)
            } else {
                None
            }
        })
        .map(|v| v.iter().collect())
        .unwrap_or_default();

    let base_palette = build_base_palette(frame0);

    let doc_slices = build_slices(frame0);

    let tilesets = build_tilesets(frame0, doc.header.color_depth, &mut ctx, &mut warnings);

    let transparent_index = matches!(doc.header.color_depth, ColorDepth::Indexed)
        .then_some(doc.header.transparent_index);

    let frame_count = doc.frames.len();

    // ── State ranges: one per tag, or one "default" covering all frames ───────

    let state_ranges: Vec<(String, usize, usize)> = if tags.is_empty() {
        vec![("default".into(), 0, frame_count - 1)]
    } else {
        let mut ranges = Vec::with_capacity(tags.len());
        for t in &tags {
            let from = t.from_frame as usize;
            if from >= frame_count {
                warnings.push(ConversionWarning::InvalidTagRange {
                    tag_name: t.name.clone(),
                    from: t.from_frame,
                    to: t.to_frame,
                    frame_count,
                });
                continue;
            }
            let to = (t.to_frame as usize).min(frame_count - 1);
            ranges.push((t.name.clone(), from, to));
        }
        if ranges.is_empty() {
            // All tags were invalid; fall back to a single default state.
            vec![("default".into(), 0, frame_count - 1)]
        } else {
            ranges
        }
    };

    // ── Build one NamedSprite per state ───────────────────────────────────────

    let mut named_sprites: Vec<LibNamedSprite> = Vec::new();

    for (state_idx, (state_name, from, to)) in state_ranges.iter().enumerate() {
        let state_frames: Vec<Frame> = doc.frames[*from..=*to]
            .iter()
            .map(|f| Frame {
                duration_ms: u32::from(f.duration_ms),
                user_data: UserData::default(),
            })
            .collect();

        let (cels, palette_overrides) =
            collect_state_cels(doc, *from, *to, &layer_ids, &mut ctx, &mut warnings)?;

        let slices = remap_slices(&doc_slices, *from, *to);
        let state_num = u32::try_from(state_idx + 1).unwrap_or(u32::MAX);

        named_sprites.push(LibNamedSprite {
            id: StateId::new(state_num),
            state_name: state_name.clone(),
            sprite: Sprite {
                id: SpriteId::new(state_num),
                name: format!("{sprite_name} / {state_name}"),
                canvas: Size::new(u32::from(doc.header.width), u32::from(doc.header.height)),
                color_mode: color_mode_from_depth(doc.header.color_depth),
                transparent_color_index: transparent_index,
                layers: layers.clone(),
                frames: state_frames,
                cels,
                palettes: base_palette.iter().cloned().collect(),
                palette_frame_overrides: palette_overrides,
                tilesets: tilesets.clone(),
                frame_tags: Vec::new(),
                animations: Vec::new(),
                slices,
                user_data: UserData::default(),
            },
            engine_tags: Vec::new(),
        });
    }

    // ── Assemble project ──────────────────────────────────────────────────────

    let mut project = pixhaus_core::project::Project::new("imported");
    project
        .library
        .entities
        .push(assemble_entity(sprite_name, named_sprites));

    Ok(ConvertedArchive {
        archive: PixhausArchive {
            project,
            buffers: ctx.buffers,
        },
        warnings,
    })
}

/// Translate a [`PixhausArchive`] into an [`AsepriteDocument`].
///
/// Selects the active entity when it is a `Custom`-kind entity with
/// states; otherwise the first such entity in the library. All states
/// are merged into a single document with one frame tag per state.
///
/// # Errors
///
/// - [`Error::NoSpriteEntityForExport`] when no exportable entity is
///   found in the library.
#[allow(clippy::too_many_lines)]
pub fn archive_to_document(archive: &PixhausArchive) -> Result<AsepriteDocument> {
    let states = find_sprite_states(archive)?;
    let first = &states[0].sprite;
    let canvas_w = u16::try_from(first.canvas.width).unwrap_or(u16::MAX);
    let canvas_h = u16::try_from(first.canvas.height).unwrap_or(u16::MAX);

    let color_depth = color_depth_from_mode(first.color_mode);

    // ── Compute merged frame ranges and tags ──────────────────────────────────

    let mut tag_entries: Vec<TagEntry> = Vec::new();
    let mut state_frame_offsets: Vec<u16> = Vec::new();
    let mut offset: u32 = 0;

    for state in states {
        let offset_u16 =
            u16::try_from(offset).map_err(|_| Error::FrameCountOverflow { total: offset })?;
        state_frame_offsets.push(offset_u16);

        let count = u32::try_from(state.sprite.frames.len()).unwrap_or(u32::MAX);
        let next_offset = offset
            .checked_add(count)
            .ok_or(Error::FrameCountOverflow { total: u32::MAX })?;
        if next_offset > u32::from(u16::MAX) + 1 {
            return Err(Error::FrameCountOverflow { total: next_offset });
        }

        let from_u16 = offset_u16;
        let to_u16 = u16::try_from(offset + count.saturating_sub(1)).map_err(|_| {
            Error::FrameCountOverflow {
                total: offset + count,
            }
        })?;

        tag_entries.push(TagEntry {
            from_frame: from_u16,
            to_frame: to_u16,
            loop_direction: 0,
            repeat: 0,
            name: state.state_name.clone(),
            deprecated_color: [0; 3],
        });
        offset = next_offset;
    }

    // ── Layer depths from first state ─────────────────────────────────────────

    let depths = compute_child_levels(&first.layers);

    // ── Frame 0: structural chunks + cels for frame 0 of state 0 ─────────────

    let first_duration = first
        .frames
        .first()
        .map_or(100, |f| u16::try_from(f.duration_ms).unwrap_or(u16::MAX));

    let mut frame0 = DocumentFrame::new(first_duration);

    for layer in &first.layers {
        let child_level = depths.get(&layer.id).copied().unwrap_or(0);
        frame0
            .chunks
            .push(Chunk::Layer(build_layer_chunk(layer, child_level)));
    }

    if !tag_entries.is_empty() {
        frame0
            .chunks
            .push(Chunk::Tags(TagsChunk { tags: tag_entries }));
    }

    if let Some(pal) = first.palettes.first() {
        frame0.chunks.push(palette_chunk_from(pal));
    }

    // Slices: merge across all states with per-state frame offsets.
    let states_ref: Vec<&LibNamedSprite> = states.iter().collect();
    for sc in merged_slice_chunks(&states_ref, &state_frame_offsets) {
        frame0.chunks.push(sc);
    }

    // Tileset chunks: ExternalFiles first, then one Tileset per entry.
    let (maybe_ef, tileset_chunks) =
        build_export_tileset_chunks(&first.tilesets, color_depth, archive);
    if let Some(ef) = maybe_ef {
        frame0.chunks.push(Chunk::ExternalFiles(ef));
    }
    for tc in tileset_chunks {
        frame0.chunks.push(Chunk::Tileset(tc));
    }

    let layer_map_0 = layer_index_map(&first.layers);
    append_cels(
        &mut frame0.chunks,
        &first.cels,
        FrameIndex::new(0),
        &layer_map_0,
        state_frame_offsets[0],
        archive,
    );

    // Build the document header with transparent index for indexed sprites.
    let mut header = DocumentHeader {
        color_depth,
        ..DocumentHeader::rgba(canvas_w, canvas_h)
    };
    if matches!(color_depth, ColorDepth::Indexed) {
        header.transparent_index = first.transparent_color_index.unwrap_or(0);
        header.color_count =
            u16::try_from(first.palettes.first().map_or(0, |p| p.colors.len())).unwrap_or(u16::MAX);
    }

    // Pre-build per-frame palette override chunks indexed by absolute frame.
    let mut palette_overrides: HashMap<usize, Vec<Chunk>> = HashMap::new();
    for (state_idx, state) in states.iter().enumerate() {
        let abs_base = usize::from(state_frame_offsets[state_idx]);
        for ov in &state.sprite.palette_frame_overrides {
            palette_overrides
                .entry(abs_base + ov.frame as usize)
                .or_default()
                .push(palette_chunk_from_entries(&ov.colors));
        }
    }

    // Emit any palette overrides that land on frame 0.
    if let Some(chunks) = palette_overrides.remove(&0) {
        frame0.chunks.extend(chunks);
    }

    let mut doc = AsepriteDocument {
        header,
        frames: vec![frame0],
    };

    // ── Remaining frames: state 0 frames 1.., then states 1..n all frames ────

    for (state_idx, state) in states.iter().enumerate() {
        let layer_map = layer_index_map(&state.sprite.layers);
        let frame_offset = state_frame_offsets[state_idx];
        let frame_local_start = usize::from(state_idx == 0);

        for frame_local in frame_local_start..state.sprite.frames.len() {
            let frame = &state.sprite.frames[frame_local];
            let mut doc_frame =
                DocumentFrame::new(u16::try_from(frame.duration_ms).unwrap_or(u16::MAX));

            append_cels(
                &mut doc_frame.chunks,
                &state.sprite.cels,
                FrameIndex::new(u32::try_from(frame_local).unwrap_or(u32::MAX)),
                &layer_map,
                frame_offset,
                archive,
            );

            let abs_frame = usize::from(frame_offset) + frame_local;
            if let Some(chunks) = palette_overrides.remove(&abs_frame) {
                doc_frame.chunks.extend(chunks);
            }

            doc.frames.push(doc_frame);
        }
    }

    Ok(doc)
}

/// Export each state in the first Custom-kind library entity as its own
/// [`AsepriteDocument`]. Returns one document per state, paired with the
/// state's id and a filename-safe rendering of the state name.
///
/// Picks the entity the same way [`archive_to_document`] does: the
/// active target's entity if it is a Custom-kind, else the first
/// Custom-kind entity in the library. Errors with
/// [`Error::NoSpriteEntityForExport`] when no Custom entity exists.
///
/// Each returned document is the same shape `archive_to_document` would
/// produce for that single state, minus the frame-tag metadata (there is
/// only one state per document, so tags are unnecessary).
///
/// Out-of-bounds linked cels — where `source_frame >= state.frames.len()`
/// — are dropped and recorded as
/// [`ConversionWarning::CrossStateLinkedCel`] in the per-state document's
/// `warnings` field.
///
/// # Errors
///
/// - [`Error::NoSpriteEntityForExport`] when no Custom-kind entity is
///   present in the library.
pub fn archive_to_per_state_documents(archive: &PixhausArchive) -> Result<Vec<PerStateDocument>> {
    let states = find_sprite_states(archive)?;

    let mut result: Vec<PerStateDocument> = Vec::with_capacity(states.len());

    for state in states {
        let color_depth = color_depth_from_mode(state.sprite.color_mode);
        let mut warnings: Vec<ConversionWarning> = Vec::new();
        let document = build_single_state_document(state, color_depth, archive, &mut warnings);

        result.push(PerStateDocument {
            state_id: state.id,
            state_name: state.state_name.clone(),
            filename_stem: sanitize_filename_stem(&state.state_name),
            document,
            warnings,
        });
    }

    Ok(result)
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Collect cels and per-frame palette overrides for a single state's frame range.
fn collect_state_cels(
    doc: &AsepriteDocument,
    from: usize,
    to: usize,
    layer_ids: &[LayerId],
    ctx: &mut ImportCtx,
    warnings: &mut Vec<ConversionWarning>,
) -> Result<(Vec<Cel>, Vec<PaletteFrameOverride>)> {
    let mut cels: Vec<Cel> = Vec::new();
    let mut palette_overrides: Vec<PaletteFrameOverride> = Vec::new();

    for (doc_frame_idx, doc_frame) in doc.frames.iter().enumerate() {
        if doc_frame_idx < from || doc_frame_idx > to {
            continue;
        }
        let state_frame = u32::try_from(doc_frame_idx - from).unwrap_or(u32::MAX);
        let frame_index = FrameIndex::new(state_frame);

        for chunk in &doc_frame.chunks {
            match chunk {
                Chunk::Cel(cc) => {
                    let layer_id =
                        *layer_ids
                            .get(cc.layer_index as usize)
                            .ok_or(Error::UnknownCelLayer {
                                layer: cc.layer_index,
                            })?;

                    // Detect cross-state linked cels: source frame outside [from, to].
                    if let CelChunkData::Linked {
                        frame: src_doc_frame,
                    } = &cc.data
                    {
                        let doc_src = *src_doc_frame as usize;
                        if doc_src < from || doc_src > to {
                            warnings.push(ConversionWarning::CrossStateLinkedCel {
                                from_state: StateId::new(0), // unknown at import time
                                to_state: StateId::new(0),
                            });
                            // Inline the source cel's pixels instead of creating a
                            // corrupt link. Look up the raster source in the document.
                            if let Some(inlined) = inline_cross_state_cel(
                                doc,
                                cc.layer_index,
                                doc_src,
                                layer_id,
                                frame_index,
                                doc.header.color_depth,
                                ctx,
                            ) {
                                cels.push(inlined);
                            }
                            continue;
                        }
                    }

                    cels.push(build_cel(
                        cc,
                        layer_id,
                        frame_index,
                        from,
                        doc.header.color_depth,
                        ctx,
                    ));
                }
                Chunk::Palette(pc) if doc_frame_idx > 0 => {
                    palette_overrides.push(PaletteFrameOverride {
                        frame: state_frame,
                        colors: palette_entries_from_chunk(pc),
                    });
                }
                _ => {}
            }
        }
    }

    Ok((cels, palette_overrides))
}

/// Resolve a cross-state linked cel by finding the source raster data in the
/// document and converting it to an inline raster cel.
///
/// If the source frame has no raster cel for `layer_index` (e.g. it is itself
/// a linked cel, or a tilemap cel), returns `None` and the frame is left blank
/// for that layer.
fn inline_cross_state_cel(
    doc: &AsepriteDocument,
    layer_index: u16,
    src_doc_frame: usize,
    layer_id: LayerId,
    frame_index: FrameIndex,
    depth: ColorDepth,
    ctx: &mut ImportCtx,
) -> Option<Cel> {
    let src_frame = doc.frames.get(src_doc_frame)?;
    let src_cel = src_frame.chunks.iter().find_map(|c| {
        if let Chunk::Cel(cc) = c {
            if cc.layer_index == layer_index {
                Some(cc)
            } else {
                None
            }
        } else {
            None
        }
    })?;

    match &src_cel.data {
        CelChunkData::Compressed {
            width,
            height,
            pixels,
        }
        | CelChunkData::Raw {
            width,
            height,
            pixels,
        } => {
            let w = u32::from(*width);
            let h = u32::from(*height);
            let bpp = depth.bytes_per_pixel();
            let stride = w * bpp;
            let buf_id = ctx.add_buffer(w, h, stride, pixels.clone());
            Some(Cel {
                layer_id,
                frame_index,
                position: IVec2::new(i32::from(src_cel.x), i32::from(src_cel.y)),
                opacity: src_cel.opacity,
                data: CelData::Raster {
                    buffer: pixhaus_core::project::PixelBufferId::new(buf_id),
                    size: Size::new(w, h),
                },
                user_data: UserData::default(),
            })
        }
        // Linked or tilemap source: cannot inline; leave the frame blank.
        _ => None,
    }
}

/// Build a `Custom("Sprite")` library entity from a set of named sprites.
fn assemble_entity(name: String, states: Vec<LibNamedSprite>) -> Entity {
    Entity {
        id: EntityId::new(1),
        kind: LibEntityKind::Custom("Sprite".into()),
        name,
        group_id: None,
        tags: Vec::new(),
        defaults: LibEntityDefaults::default(),
        content: LibEntityContent::Sprites { states },
        ai: LibAiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    }
}

/// Find the exportable sprite states from the archive.
///
/// Prefers the active entity when it is a Custom-kind sprite entity; falls
/// back to the first such entity in the library.
fn find_sprite_states(archive: &PixhausArchive) -> Result<&[LibNamedSprite]> {
    let active_id = match archive.project.active {
        ActiveTarget::State { entity_id, .. } => Some(entity_id),
        _ => None,
    };

    let entity = active_id
        .and_then(|eid| {
            archive
                .project
                .library
                .entities
                .iter()
                .find(|e| e.id == eid && is_sprite_entity(e))
        })
        .or_else(|| {
            archive
                .project
                .library
                .entities
                .iter()
                .find(|e| is_sprite_entity(e))
        })
        .ok_or(Error::NoSpriteEntityForExport)?;

    let LibEntityContent::Sprites { states } = &entity.content else {
        return Err(Error::NoSpriteEntityForExport);
    };

    if states.is_empty() {
        return Err(Error::NoSpriteEntityForExport);
    }

    Ok(states)
}

// ── Import helpers ────────────────────────────────────────────────────────────

fn is_sprite_entity(e: &Entity) -> bool {
    matches!(e.kind, LibEntityKind::Custom(_))
        && matches!(&e.content, LibEntityContent::Sprites { states } if !states.is_empty())
}

fn color_mode_from_depth(depth: ColorDepth) -> ColorMode {
    match depth {
        ColorDepth::Rgba => ColorMode::Rgba,
        ColorDepth::Grayscale => ColorMode::Grayscale,
        ColorDepth::Indexed => ColorMode::Indexed,
    }
}

fn build_layers(
    layer_chunks: &[&LayerChunk],
    warnings: &mut Vec<ConversionWarning>,
) -> (Vec<Layer>, Vec<LayerId>) {
    let mut layers: Vec<Layer> = Vec::with_capacity(layer_chunks.len());
    let mut layer_ids: Vec<LayerId> = Vec::with_capacity(layer_chunks.len());
    // parent_at_level[depth] = LayerId of the last layer seen at that depth
    let mut parent_at_level: Vec<LayerId> = Vec::new();

    for (i, lc) in layer_chunks.iter().enumerate() {
        let layer_id = LayerId::new(u32::try_from(i).unwrap_or(u32::MAX).saturating_add(1));
        layer_ids.push(layer_id);

        let level = lc.child_level as usize;

        let parent = if level == 0 {
            None
        } else {
            parent_at_level.get(level - 1).copied()
        };

        // Record this layer as the current at its depth level
        if parent_at_level.len() <= level {
            parent_at_level.resize(level + 1, layer_id);
        } else {
            parent_at_level.truncate(level + 1);
            parent_at_level[level] = layer_id;
        }

        let blend_mode = if let Some(code) = lc.unknown_blend_code {
            warnings.push(ConversionWarning::UnknownBlendMode {
                layer_name: lc.name.clone(),
                code,
            });
            BlendMode::Normal
        } else {
            lc.blend
        };

        let kind = match lc.kind {
            LayerKindCode::Normal => LayerKind::Raster,
            LayerKindCode::Group => LayerKind::Group {
                collapsed: lc.flags & LAYER_FLAG_GROUP_COLLAPSED != 0,
            },
            LayerKindCode::Tilemap => LayerKind::Tilemap {
                tileset: TilesetId::new(lc.tileset_index),
            },
        };

        layers.push(Layer {
            id: layer_id,
            name: lc.name.clone(),
            kind,
            blend_mode,
            opacity: lc.opacity,
            visible: lc.flags & LAYER_FLAG_VISIBLE != 0,
            locked: lc.flags & LAYER_FLAG_EDITABLE == 0,
            parent,
            user_data: UserData::default(),
        });
    }

    (layers, layer_ids)
}

fn build_base_palette(frame0: &DocumentFrame) -> Option<Palette> {
    let mut colors: Vec<PaletteEntry> = Vec::new();

    for chunk in &frame0.chunks {
        match chunk {
            Chunk::Palette(pc) => {
                let needed = pc.palette_size as usize;
                if colors.len() < needed {
                    colors.resize(
                        needed,
                        PaletteEntry::new(pixhaus_core::project::Rgba::transparent()),
                    );
                }
                for (i, entry) in pc.entries.iter().enumerate() {
                    let idx = pc.first_index as usize + i;
                    if idx < colors.len() {
                        colors[idx] = PaletteEntry {
                            color: entry.color,
                            name: entry.name.clone(),
                        };
                    }
                }
            }
            Chunk::OldPalette255(op) | Chunk::OldPalette63(op) if colors.is_empty() => {
                colors = op.colors.iter().map(|&c| PaletteEntry::new(c)).collect();
            }
            _ => {}
        }
    }

    if colors.is_empty() {
        None
    } else {
        Some(Palette {
            id: PaletteId::new(1),
            name: "imported".into(),
            colors,
            user_data: UserData::default(),
        })
    }
}

fn palette_entries_from_chunk(pc: &PaletteChunk) -> Vec<PaletteEntry> {
    pc.entries
        .iter()
        .map(|e| PaletteEntry {
            color: e.color,
            name: e.name.clone(),
        })
        .collect()
}

fn build_slices(frame0: &DocumentFrame) -> Vec<Slice> {
    let mut slices: Vec<Slice> = Vec::new();
    let mut slice_id: u32 = 1;

    for chunk in &frame0.chunks {
        if let Chunk::Slice(sc) = chunk {
            let keys: Vec<SliceKey> = sc
                .keys
                .iter()
                .map(|k| SliceKey {
                    frame: FrameIndex::new(k.frame),
                    bounds: Rect::from_xywh(k.x, k.y, k.width, k.height),
                    nine_slice: k.nine_slice.map(|ns| NineSlice {
                        center: Rect::from_xywh(ns.x, ns.y, ns.width, ns.height),
                    }),
                    pivot: k.pivot.map(|pv| Pivot {
                        offset: IVec2::new(pv.x, pv.y),
                    }),
                })
                .collect();

            slices.push(Slice {
                id: SliceId::new(slice_id),
                name: sc.name.clone(),
                keys,
                user_data: UserData::default(),
            });
            slice_id += 1;
        }
    }

    slices
}

fn build_tilesets(
    frame0: &DocumentFrame,
    depth: ColorDepth,
    ctx: &mut ImportCtx,
    warnings: &mut Vec<ConversionWarning>,
) -> Vec<Tileset> {
    // Build the external-file ID → path mapping from any ExternalFiles chunk.
    let external_files: HashMap<u32, String> = frame0
        .chunks
        .iter()
        .find_map(|c| {
            if let Chunk::ExternalFiles(ef) = c {
                Some(ef.entries.iter().map(|e| (e.id, e.name.clone())).collect())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let mut tilesets: Vec<Tileset> = Vec::new();

    for chunk in &frame0.chunks {
        if let Chunk::Tileset(tc) = chunk {
            let source = match &tc.source {
                TilesetSourceWire::Inline { pixels } => {
                    let bpp = depth.bytes_per_pixel();
                    let tile_w = u32::from(tc.tile_width);
                    let tile_h = u32::from(tc.tile_height);
                    let tile_count = tc.tile_count;

                    // Aseprite stores tiles as a vertical strip (width=tile_w,
                    // height=tile_h*tile_count). Pixhaus uses a horizontal strip
                    // (width=tile_w*tile_count, height=tile_h). Repack here.
                    let row_bytes = (tile_w * bpp) as usize;
                    let src_stride = row_bytes; // vertical strip: one tile wide
                    let dst_w = tile_w * tile_count;
                    let dst_stride = (dst_w * bpp) as usize;
                    let dst_h = tile_h;
                    let mut dst = vec![0u8; dst_stride * dst_h as usize];

                    for tile_idx in 0..tile_count as usize {
                        for row in 0..tile_h as usize {
                            let src_row_start = (tile_idx * tile_h as usize + row) * src_stride;
                            let dst_row_start = row * dst_stride + tile_idx * row_bytes;
                            dst[dst_row_start..dst_row_start + row_bytes]
                                .copy_from_slice(&pixels[src_row_start..src_row_start + row_bytes]);
                        }
                    }

                    let buf_id = ctx.add_buffer(
                        dst_w,
                        dst_h,
                        u32::try_from(dst_stride).unwrap_or(u32::MAX),
                        dst,
                    );
                    CoreTilesetSource::Inline {
                        buffer: pixhaus_core::project::PixelBufferId::new(buf_id),
                    }
                }
                TilesetSourceWire::External {
                    external_file_id, ..
                } => {
                    let path = if let Some(p) = external_files.get(external_file_id) {
                        p.clone()
                    } else {
                        warnings.push(ConversionWarning::MissingExternalFileEntry {
                            tileset_id: tc.tileset_id,
                            external_file_id: *external_file_id,
                        });
                        String::new()
                    };
                    CoreTilesetSource::External { path }
                }
            };

            tilesets.push(Tileset {
                id: TilesetId::new(tc.tileset_id),
                name: tc.name.clone(),
                tile_size: Size::new(u32::from(tc.tile_width), u32::from(tc.tile_height)),
                tile_count: tc.tile_count,
                base_index: tc.base_index,
                source,
                properties: Vec::new(),
                autotile: None,
                user_data: UserData::default(),
            });
        }
    }

    tilesets
}

fn build_cel(
    cc: &CelChunk,
    layer_id: LayerId,
    frame_index: FrameIndex,
    state_frame_start: usize,
    depth: ColorDepth,
    ctx: &mut ImportCtx,
) -> Cel {
    let position = IVec2::new(i32::from(cc.x), i32::from(cc.y));

    let data = match &cc.data {
        CelChunkData::Compressed {
            width,
            height,
            pixels,
        }
        | CelChunkData::Raw {
            width,
            height,
            pixels,
        } => {
            let w = u32::from(*width);
            let h = u32::from(*height);
            let bpp = depth.bytes_per_pixel();
            let stride = w * bpp;
            let buf_id = ctx.add_buffer(w, h, stride, pixels.clone());
            CelData::Raster {
                buffer: pixhaus_core::project::PixelBufferId::new(buf_id),
                size: Size::new(w, h),
            }
        }
        CelChunkData::Linked { frame } => {
            let doc_source = *frame as usize;
            let state_source = doc_source.saturating_sub(state_frame_start);
            CelData::Linked {
                source_frame: FrameIndex::new(u32::try_from(state_source).unwrap_or(u32::MAX)),
            }
        }
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
            let w = u32::from(*width);
            let h = u32::from(*height);
            let cells: Vec<TileCell> = tiles
                .iter()
                .map(|&entry| {
                    let tile_id = entry & tile_id_mask;
                    let mut flags = TileFlags::empty();
                    if entry & x_flip_mask != 0 {
                        flags = flags.union(TileFlags::FLIP_X);
                    }
                    if entry & y_flip_mask != 0 {
                        flags = flags.union(TileFlags::FLIP_Y);
                    }
                    if entry & diagonal_flip_mask != 0 {
                        flags = flags.union(TileFlags::FLIP_DIAGONAL);
                    }
                    TileCell {
                        index: TileIndex::new(tile_id),
                        flags,
                    }
                })
                .collect();
            CelData::Tilemap {
                data: TilemapData {
                    width: w,
                    height: h,
                    cells,
                },
            }
        }
    };

    Cel {
        layer_id,
        frame_index,
        position,
        opacity: cc.opacity,
        data,
        user_data: UserData::default(),
    }
}

/// Remap slice keys from document-level frame indices to state-local
/// frame indices.
///
/// Carries the key active at `from` forward as frame 0 of the state,
/// then includes any keys whose document frame index falls in
/// `(from, to]`, remapped to `doc_frame - from`.
fn remap_slices(slices: &[Slice], from: usize, to: usize) -> Vec<Slice> {
    slices
        .iter()
        .map(|slice| {
            // Find the key active at the state's start frame.
            let inherited = slice.keys.iter().rfind(|k| k.frame.get() as usize <= from);

            let mut keys: Vec<SliceKey> = Vec::new();

            if let Some(k) = inherited {
                keys.push(SliceKey {
                    frame: FrameIndex::new(0),
                    ..k.clone()
                });
            }

            for k in &slice.keys {
                let doc_frame = k.frame.get() as usize;
                if doc_frame > from && doc_frame <= to {
                    keys.push(SliceKey {
                        frame: FrameIndex::new(u32::try_from(doc_frame - from).unwrap_or(u32::MAX)),
                        ..k.clone()
                    });
                }
            }

            Slice {
                id: slice.id,
                name: slice.name.clone(),
                keys,
                user_data: slice.user_data.clone(),
            }
        })
        .collect()
}

// ── Per-state export helpers ──────────────────────────────────────────────────

/// Sanitise a state name into a filename-safe stem.
///
/// Rules:
/// - ASCII-lowercase.
/// - Any character outside `[A-Za-z0-9_-]` is replaced with `-`.
/// - Consecutive `-` are collapsed into one.
/// - Leading and trailing `-` are trimmed.
/// - If the result is empty, returns `"state"`.
///
/// Unicode code points that have no ASCII equivalent become `-`; they are
/// not transliterated.
fn sanitize_filename_stem(name: &str) -> String {
    // Replace non-ASCII and non-safe characters with '-'.
    let mut raw = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            raw.push(c.to_ascii_lowercase());
        } else {
            raw.push('-');
        }
    }

    // Collapse consecutive hyphens.
    let mut stem = String::with_capacity(raw.len());
    let mut prev_dash = false;
    for c in raw.chars() {
        if c == '-' {
            if !prev_dash {
                stem.push('-');
            }
            prev_dash = true;
        } else {
            stem.push(c);
            prev_dash = false;
        }
    }

    // Trim leading/trailing hyphens.
    let trimmed = stem.trim_matches('-');

    if trimmed.is_empty() {
        "state".into()
    } else {
        trimmed.to_owned()
    }
}

/// Build a single-state `AsepriteDocument` from one `NamedSprite`.
///
/// Produces the same structural layout as `archive_to_document` but for
/// a single state: frame 0 carries layers, palette, and slices (no tag
/// chunk); subsequent frames carry cels only. The `state_frame_offset` is
/// always 0 because there is no merged timeline.
///
/// Out-of-bounds linked cels are recorded in `warnings` and dropped.
fn build_single_state_document(
    state: &LibNamedSprite,
    color_depth: ColorDepth,
    archive: &PixhausArchive,
    warnings: &mut Vec<ConversionWarning>,
) -> AsepriteDocument {
    let canvas_w = u16::try_from(state.sprite.canvas.width).unwrap_or(u16::MAX);
    let canvas_h = u16::try_from(state.sprite.canvas.height).unwrap_or(u16::MAX);
    let depths = compute_child_levels(&state.sprite.layers);
    let layer_map = layer_index_map(&state.sprite.layers);
    let frame_count = state.sprite.frames.len();

    let first_duration = state
        .sprite
        .frames
        .first()
        .map_or(100, |f| u16::try_from(f.duration_ms).unwrap_or(u16::MAX));

    let mut frame0 = DocumentFrame::new(first_duration);

    for layer in &state.sprite.layers {
        let child_level = depths.get(&layer.id).copied().unwrap_or(0);
        frame0
            .chunks
            .push(Chunk::Layer(build_layer_chunk(layer, child_level)));
    }

    // No tag chunk — one state per document makes tags unnecessary.

    if let Some(pal) = state.sprite.palettes.first() {
        frame0.chunks.push(palette_chunk_from(pal));
    }

    for slice in &state.sprite.slices {
        frame0.chunks.push(slice_chunk_from(slice));
    }

    // Tileset chunks: ExternalFiles first, then one Tileset per entry.
    let (maybe_ef, tileset_chunks) =
        build_export_tileset_chunks(&state.sprite.tilesets, color_depth, archive);
    if let Some(ef) = maybe_ef {
        frame0.chunks.push(Chunk::ExternalFiles(ef));
    }
    for tc in tileset_chunks {
        frame0.chunks.push(Chunk::Tileset(tc));
    }

    append_cels_validated(
        &mut frame0.chunks,
        &state.sprite.cels,
        FrameIndex::new(0),
        &layer_map,
        frame_count,
        state.id,
        archive,
        warnings,
    );

    // Emit palette overrides that land on frame 0.
    for ov in &state.sprite.palette_frame_overrides {
        if ov.frame == 0 {
            frame0.chunks.push(palette_chunk_from_entries(&ov.colors));
        }
    }

    // Build the header with transparent index for indexed sprites.
    let mut header = DocumentHeader {
        color_depth,
        ..DocumentHeader::rgba(canvas_w, canvas_h)
    };
    if matches!(color_depth, ColorDepth::Indexed) {
        header.transparent_index = state.sprite.transparent_color_index.unwrap_or(0);
        header.color_count =
            u16::try_from(state.sprite.palettes.first().map_or(0, |p| p.colors.len()))
                .unwrap_or(u16::MAX);
    }

    let mut doc = AsepriteDocument {
        header,
        frames: vec![frame0],
    };

    for frame_local in 1..state.sprite.frames.len() {
        let frame = &state.sprite.frames[frame_local];
        let mut doc_frame =
            DocumentFrame::new(u16::try_from(frame.duration_ms).unwrap_or(u16::MAX));

        append_cels_validated(
            &mut doc_frame.chunks,
            &state.sprite.cels,
            FrameIndex::new(u32::try_from(frame_local).unwrap_or(u32::MAX)),
            &layer_map,
            frame_count,
            state.id,
            archive,
            warnings,
        );

        // Emit palette overrides for this frame.
        for ov in &state.sprite.palette_frame_overrides {
            if ov.frame == u32::try_from(frame_local).unwrap_or(u32::MAX) {
                doc_frame
                    .chunks
                    .push(palette_chunk_from_entries(&ov.colors));
            }
        }

        doc.frames.push(doc_frame);
    }

    doc
}

/// Append cel chunks for a specific frame, validating linked-cel bounds.
///
/// When a linked cel's `source_frame` is >= `state_frame_count`, the link
/// would reference a non-existent frame in the single-state document.
/// The cel is dropped and a [`ConversionWarning::CrossStateLinkedCel`] is
/// appended to `warnings` instead.
#[allow(clippy::too_many_arguments)]
fn append_cels_validated(
    chunks: &mut Vec<Chunk>,
    cels: &[Cel],
    frame_index: FrameIndex,
    layer_map: &HashMap<LayerId, u16>,
    state_frame_count: usize,
    state_id: StateId,
    archive: &PixhausArchive,
    warnings: &mut Vec<ConversionWarning>,
) {
    for cel in cels {
        if cel.frame_index != frame_index {
            continue;
        }
        let Some(&layer_idx) = layer_map.get(&cel.layer_id) else {
            continue;
        };
        if let CelData::Linked { source_frame } = &cel.data {
            if source_frame.get() as usize >= state_frame_count {
                warnings.push(ConversionWarning::CrossStateLinkedCel {
                    from_state: state_id,
                    // Cannot determine target state in per-state context.
                    to_state: state_id,
                });
                continue;
            }
        }
        // state_frame_offset is 0: single-state documents have no merged timeline.
        chunks.push(Chunk::Cel(cel_chunk_from(cel, layer_idx, 0, archive)));
    }
}

// ── Export helpers ────────────────────────────────────────────────────────────

/// Build slice chunks for a merged document, merging keys from all states.
///
/// Keys from each state are offset by that state's frame offset. When two
/// states have slices with the same `SliceId`, the first state's name and
/// flags take precedence and keys from later states are appended. This lets
/// a slice named "hitbox" track across all states without duplication.
fn merged_slice_chunks(states: &[&LibNamedSprite], state_frame_offsets: &[u16]) -> Vec<Chunk> {
    // Maintain insertion order separately from the hashmap.
    let mut order: Vec<SliceId> = Vec::new();
    let mut chunks: HashMap<SliceId, SliceChunk> = HashMap::new();

    for (state_idx, state) in states.iter().enumerate() {
        let frame_offset = u32::from(state_frame_offsets[state_idx]);
        for slice in &state.sprite.slices {
            let offset_keys: Vec<SliceKeyEntry> = slice
                .keys
                .iter()
                .map(|k| {
                    let abs_frame = k.frame.get().saturating_add(frame_offset);
                    SliceKeyEntry {
                        frame: abs_frame,
                        x: k.bounds.origin.x,
                        y: k.bounds.origin.y,
                        width: k.bounds.size.width,
                        height: k.bounds.size.height,
                        nine_slice: k.nine_slice.map(|ns| NineSliceWire {
                            x: ns.center.origin.x,
                            y: ns.center.origin.y,
                            width: ns.center.size.width,
                            height: ns.center.size.height,
                        }),
                        pivot: k.pivot.map(|pv| PivotWire {
                            x: pv.offset.x,
                            y: pv.offset.y,
                        }),
                    }
                })
                .collect();

            let sc = chunks.entry(slice.id).or_insert_with(|| {
                order.push(slice.id);
                SliceChunk {
                    name: slice.name.clone(),
                    keys: Vec::new(),
                    has_nine_slice: false,
                    has_pivot: false,
                }
            });
            sc.keys.extend(offset_keys);
            sc.has_nine_slice |= slice.keys.iter().any(|k| k.nine_slice.is_some());
            sc.has_pivot |= slice.keys.iter().any(|k| k.pivot.is_some());
        }
    }

    order
        .into_iter()
        .filter_map(|id| chunks.remove(&id))
        .map(Chunk::Slice)
        .collect()
}

/// Build the tileset export chunks (`ExternalFiles` + one `Tileset` per tileset).
///
/// For inline tilesets, repacks the Pixhaus horizontal strip back into the
/// Aseprite vertical strip format. For external tilesets, assigns sequential
/// external-file IDs and emits a single `ExternalFiles` chunk if needed.
///
/// Returns `(external_files_chunk, tileset_chunks)`.
fn build_export_tileset_chunks(
    tilesets: &[Tileset],
    color_depth: ColorDepth,
    archive: &PixhausArchive,
) -> (Option<ExternalFilesChunk>, Vec<TilesetChunk>) {
    let bpp = color_depth.bytes_per_pixel();
    let mut external_entries: Vec<ExternalFileEntry> = Vec::new();
    let mut tileset_chunks: Vec<TilesetChunk> = Vec::new();

    for tileset in tilesets {
        let tile_w = tileset.tile_size.width;
        let tile_h = tileset.tile_size.height;
        let tile_count = tileset.tile_count;

        let source = match &tileset.source {
            CoreTilesetSource::Inline { buffer } => {
                // Horizontal Pixhaus layout → vertical Aseprite layout.
                let pixels = archive
                    .buffer(*buffer)
                    .map_or(&[][..], |b| b.pixels.as_slice());

                let row_bytes = (tile_w * bpp) as usize;
                let src_stride = (tile_w * tile_count * bpp) as usize;
                let dst_len = (tile_w * bpp * tile_h * tile_count) as usize;
                let mut dst = vec![0u8; dst_len];

                for tile_idx in 0..tile_count as usize {
                    for row in 0..tile_h as usize {
                        let src_start = row * src_stride + tile_idx * row_bytes;
                        let dst_start = (tile_idx * tile_h as usize + row) * row_bytes;
                        if src_start + row_bytes <= pixels.len()
                            && dst_start + row_bytes <= dst.len()
                        {
                            dst[dst_start..dst_start + row_bytes]
                                .copy_from_slice(&pixels[src_start..src_start + row_bytes]);
                        }
                    }
                }

                TilesetSourceWire::Inline { pixels: dst }
            }
            CoreTilesetSource::External { path } => {
                let external_file_id = u32::try_from(external_entries.len()).unwrap_or(u32::MAX);
                external_entries.push(ExternalFileEntry {
                    id: external_file_id,
                    kind: 1, // tileset
                    name: path.clone(),
                });
                TilesetSourceWire::External {
                    external_file_id,
                    external_tileset_id: 0,
                }
            }
        };

        tileset_chunks.push(TilesetChunk {
            tileset_id: tileset.id.get(),
            flags: 0,
            tile_count,
            tile_width: u16::try_from(tile_w).unwrap_or(u16::MAX),
            tile_height: u16::try_from(tile_h).unwrap_or(u16::MAX),
            base_index: tileset.base_index,
            name: tileset.name.clone(),
            source,
        });
    }

    let ef = if external_entries.is_empty() {
        None
    } else {
        Some(ExternalFilesChunk {
            entries: external_entries,
        })
    };

    (ef, tileset_chunks)
}

/// Build a palette chunk from a slice of palette entries.
fn palette_chunk_from_entries(colors: &[PaletteEntry]) -> Chunk {
    let entries: Vec<PaletteEntryWire> = colors
        .iter()
        .map(|e| PaletteEntryWire {
            color: e.color,
            name: e.name.clone(),
        })
        .collect();
    let last = u32::try_from(entries.len().saturating_sub(1)).unwrap_or(u32::MAX);
    Chunk::Palette(PaletteChunk {
        palette_size: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        first_index: 0,
        last_index: last,
        entries,
    })
}

fn color_depth_from_mode(mode: ColorMode) -> ColorDepth {
    match mode {
        ColorMode::Rgba => ColorDepth::Rgba,
        ColorMode::Grayscale => ColorDepth::Grayscale,
        ColorMode::Indexed => ColorDepth::Indexed,
    }
}

/// Build a `child_level → depth` map for every layer in the sprite.
fn compute_child_levels(layers: &[Layer]) -> HashMap<LayerId, u16> {
    let parent_map: HashMap<LayerId, Option<LayerId>> =
        layers.iter().map(|l| (l.id, l.parent)).collect();

    let mut depths: HashMap<LayerId, u16> = HashMap::new();
    for layer in layers {
        depth_of(layer.id, &parent_map, &mut depths);
    }
    depths
}

fn depth_of(
    id: LayerId,
    parent_map: &HashMap<LayerId, Option<LayerId>>,
    cache: &mut HashMap<LayerId, u16>,
) -> u16 {
    if let Some(&d) = cache.get(&id) {
        return d;
    }
    let d = match parent_map.get(&id).copied().flatten() {
        None => 0,
        Some(parent_id) => depth_of(parent_id, parent_map, cache).saturating_add(1),
    };
    cache.insert(id, d);
    d
}

fn layer_index_map(layers: &[Layer]) -> HashMap<LayerId, u16> {
    layers
        .iter()
        .enumerate()
        .map(|(i, l)| (l.id, u16::try_from(i).unwrap_or(u16::MAX)))
        .collect()
}

fn build_layer_chunk(layer: &Layer, child_level: u16) -> LayerChunk {
    let (kind, tileset_index) = match &layer.kind {
        LayerKind::Raster | LayerKind::Reference { .. } => (LayerKindCode::Normal, 0),
        LayerKind::Group { .. } => (LayerKindCode::Group, 0),
        LayerKind::Tilemap { tileset } => (LayerKindCode::Tilemap, tileset.get()),
    };

    let mut flags: u16 = 0;
    if layer.visible {
        flags |= LAYER_FLAG_VISIBLE;
    }
    if !layer.locked {
        flags |= LAYER_FLAG_EDITABLE;
    }
    if matches!(layer.kind, LayerKind::Group { collapsed: true }) {
        flags |= LAYER_FLAG_GROUP_COLLAPSED;
    }

    LayerChunk {
        flags,
        kind,
        child_level,
        blend: layer.blend_mode,
        unknown_blend_code: None,
        opacity: layer.opacity,
        name: layer.name.clone(),
        tileset_index,
        uuid: None,
    }
}

fn palette_chunk_from(palette: &Palette) -> Chunk {
    let entries: Vec<PaletteEntryWire> = palette
        .colors
        .iter()
        .map(|e| PaletteEntryWire {
            color: e.color,
            name: e.name.clone(),
        })
        .collect();
    let last = u32::try_from(entries.len().saturating_sub(1)).unwrap_or(u32::MAX);
    Chunk::Palette(PaletteChunk {
        palette_size: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        first_index: 0,
        last_index: last,
        entries,
    })
}

fn slice_chunk_from(slice: &Slice) -> Chunk {
    let has_nine_slice = slice.keys.iter().any(|k| k.nine_slice.is_some());
    let has_pivot = slice.keys.iter().any(|k| k.pivot.is_some());

    let keys: Vec<SliceKeyEntry> = slice
        .keys
        .iter()
        .map(|k| SliceKeyEntry {
            frame: k.frame.get(),
            x: k.bounds.origin.x,
            y: k.bounds.origin.y,
            width: k.bounds.size.width,
            height: k.bounds.size.height,
            nine_slice: k.nine_slice.map(|ns| NineSliceWire {
                x: ns.center.origin.x,
                y: ns.center.origin.y,
                width: ns.center.size.width,
                height: ns.center.size.height,
            }),
            pivot: k.pivot.map(|pv| PivotWire {
                x: pv.offset.x,
                y: pv.offset.y,
            }),
        })
        .collect();

    Chunk::Slice(SliceChunk {
        name: slice.name.clone(),
        keys,
        has_nine_slice,
        has_pivot,
    })
}

/// Append cel chunks for a specific frame to `chunks`.
fn append_cels(
    chunks: &mut Vec<Chunk>,
    cels: &[Cel],
    frame_index: FrameIndex,
    layer_map: &HashMap<LayerId, u16>,
    state_frame_offset: u16,
    archive: &PixhausArchive,
) {
    for cel in cels {
        if cel.frame_index != frame_index {
            continue;
        }
        let Some(&layer_idx) = layer_map.get(&cel.layer_id) else {
            continue;
        };
        chunks.push(Chunk::Cel(cel_chunk_from(
            cel,
            layer_idx,
            state_frame_offset,
            archive,
        )));
    }
}

fn cel_chunk_from(
    cel: &Cel,
    layer_index: u16,
    state_frame_offset: u16,
    archive: &PixhausArchive,
) -> CelChunk {
    // Standard Aseprite tile-flip bitmasks.
    const TILE_ID_MASK: u32 = 0x1FFF_FFFF;
    const X_FLIP_MASK: u32 = 0x0800_0000;
    const Y_FLIP_MASK: u32 = 0x0400_0000;
    const DIAG_FLIP_MASK: u32 = 0x0200_0000;

    let data = match &cel.data {
        CelData::Raster { buffer, size } => {
            let pixels = archive
                .buffer(*buffer)
                .map(|b| b.pixels.clone())
                .unwrap_or_default();
            CelChunkData::Compressed {
                width: u16::try_from(size.width).unwrap_or(u16::MAX),
                height: u16::try_from(size.height).unwrap_or(u16::MAX),
                pixels,
            }
        }
        CelData::Linked { source_frame } => {
            let doc_frame =
                state_frame_offset + u16::try_from(source_frame.get()).unwrap_or(u16::MAX);
            CelChunkData::Linked { frame: doc_frame }
        }
        CelData::Tilemap { data } => {
            let tiles: Vec<u32> = data
                .cells
                .iter()
                .map(|cell| {
                    let mut entry = cell.index.get() & TILE_ID_MASK;
                    if cell.flags.contains(TileFlags::FLIP_X) {
                        entry |= X_FLIP_MASK;
                    }
                    if cell.flags.contains(TileFlags::FLIP_Y) {
                        entry |= Y_FLIP_MASK;
                    }
                    if cell.flags.contains(TileFlags::FLIP_DIAGONAL) {
                        entry |= DIAG_FLIP_MASK;
                    }
                    entry
                })
                .collect();
            CelChunkData::Tilemap {
                width: u16::try_from(data.width).unwrap_or(u16::MAX),
                height: u16::try_from(data.height).unwrap_or(u16::MAX),
                bits_per_tile: 32,
                tile_id_mask: TILE_ID_MASK,
                x_flip_mask: X_FLIP_MASK,
                y_flip_mask: Y_FLIP_MASK,
                diagonal_flip_mask: DIAG_FLIP_MASK,
                tiles,
            }
        }
    };

    CelChunk {
        layer_index,
        x: i16::try_from(cel.position.x).unwrap_or(i16::MAX),
        y: i16::try_from(cel.position.y).unwrap_or(i16::MAX),
        opacity: cel.opacity,
        z_index: 0,
        data,
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::library::EntityContent;
    use pixhaus_core::project::{BlendMode, EntityId, FrameIndex, SpriteId, StateId};

    use crate::aseprite::chunk::{
        CelChunk, CelChunkData, LayerChunk, LayerKindCode, TagEntry, TagsChunk, TilesetChunk,
        TilesetSourceWire,
    };
    use crate::aseprite::document::DocumentFrame;
    use crate::aseprite::spec::{LAYER_FLAG_EDITABLE, LAYER_FLAG_VISIBLE};
    use crate::pixhaus::PixhausArchive;

    fn rgba_layer_chunk(name: &str) -> LayerChunk {
        LayerChunk {
            flags: LAYER_FLAG_VISIBLE | LAYER_FLAG_EDITABLE,
            kind: LayerKindCode::Normal,
            child_level: 0,
            blend: BlendMode::Normal,
            unknown_blend_code: None,
            opacity: 255,
            name: name.into(),
            tileset_index: 0,
            uuid: None,
        }
    }

    fn compressed_cel(layer_index: u16, w: u16, h: u16) -> CelChunk {
        CelChunk {
            layer_index,
            x: 0,
            y: 0,
            opacity: 255,
            z_index: 0,
            data: CelChunkData::Compressed {
                width: w,
                height: h,
                pixels: vec![0u8; (w as usize) * (h as usize) * 4],
            },
        }
    }

    // ── Import ────────────────────────────────────────────────────────────────

    #[test]
    fn import_no_tags_creates_default_state() {
        let mut doc = AsepriteDocument::empty(8, 8);
        let mut f0 = DocumentFrame::new(100);
        f0.chunks.push(Chunk::Layer(rgba_layer_chunk("bg")));
        f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
        doc.frames.push(f0);

        let result = document_to_archive(&doc, "sprite").unwrap();
        let entities = &result.archive.project.library.entities;
        assert_eq!(entities.len(), 1);
        let EntityContent::Sprites { states } = &entities[0].content else {
            panic!()
        };
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].state_name, "default");
        assert_eq!(states[0].sprite.frames.len(), 1);
        assert_eq!(states[0].sprite.layers.len(), 1);
    }

    #[test]
    fn import_tags_become_states() {
        let mut doc = AsepriteDocument::empty(8, 8);
        let mut f0 = DocumentFrame::new(100);
        f0.chunks.push(Chunk::Layer(rgba_layer_chunk("bg")));
        f0.chunks.push(Chunk::Tags(TagsChunk {
            tags: vec![
                TagEntry {
                    from_frame: 0,
                    to_frame: 1,
                    loop_direction: 0,
                    repeat: 0,
                    name: "idle".into(),
                    deprecated_color: [0; 3],
                },
                TagEntry {
                    from_frame: 2,
                    to_frame: 3,
                    loop_direction: 0,
                    repeat: 0,
                    name: "walk".into(),
                    deprecated_color: [0; 3],
                },
            ],
        }));
        f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
        doc.frames.push(f0);

        // idle (0-1): frame 1 links to frame 0 (within idle's range).
        // walk (2-3): frame 2 is a raster; frame 3 links to frame 2 (within walk's range).
        let mut f1 = DocumentFrame::new(100);
        f1.chunks.push(Chunk::Cel(CelChunk {
            layer_index: 0,
            x: 0,
            y: 0,
            opacity: 255,
            z_index: 0,
            data: CelChunkData::Linked { frame: 0 },
        }));
        doc.frames.push(f1);

        let mut f2 = DocumentFrame::new(100);
        f2.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
        doc.frames.push(f2);

        let mut f3 = DocumentFrame::new(100);
        f3.chunks.push(Chunk::Cel(CelChunk {
            layer_index: 0,
            x: 0,
            y: 0,
            opacity: 255,
            z_index: 0,
            data: CelChunkData::Linked { frame: 2 }, // walk-internal
        }));
        doc.frames.push(f3);

        let result = document_to_archive(&doc, "hero").unwrap();
        assert!(result.warnings.is_empty());
        let entities = &result.archive.project.library.entities;
        assert_eq!(entities[0].name, "hero");
        let EntityContent::Sprites { states } = &entities[0].content else {
            panic!()
        };
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].state_name, "idle");
        assert_eq!(states[1].state_name, "walk");
        assert_eq!(states[0].sprite.frames.len(), 2);
        assert_eq!(states[1].sprite.frames.len(), 2);
    }

    #[test]
    fn import_empty_document_errors() {
        let doc = AsepriteDocument::empty(8, 8);
        assert!(matches!(
            document_to_archive(&doc, "x").unwrap_err(),
            Error::NoFrames
        ));
    }

    #[test]
    fn import_linked_cel_remapped_within_state() {
        // State "walk" starts at frame 2. A linked cel pointing at doc
        // frame 2 should become a link to state frame 0.
        let mut doc = AsepriteDocument::empty(8, 8);
        let mut f0 = DocumentFrame::new(100);
        f0.chunks.push(Chunk::Layer(rgba_layer_chunk("bg")));
        f0.chunks.push(Chunk::Tags(TagsChunk {
            tags: vec![
                TagEntry {
                    from_frame: 0,
                    to_frame: 0,
                    loop_direction: 0,
                    repeat: 0,
                    name: "idle".into(),
                    deprecated_color: [0; 3],
                },
                TagEntry {
                    from_frame: 1,
                    to_frame: 2,
                    loop_direction: 0,
                    repeat: 0,
                    name: "walk".into(),
                    deprecated_color: [0; 3],
                },
            ],
        }));
        f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
        doc.frames.push(f0);

        // Frame 1: walk frame 0
        let mut f1 = DocumentFrame::new(100);
        f1.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
        doc.frames.push(f1);

        // Frame 2: walk frame 1 — linked to frame 1 (walk frame 0)
        let mut f2 = DocumentFrame::new(100);
        f2.chunks.push(Chunk::Cel(CelChunk {
            layer_index: 0,
            x: 0,
            y: 0,
            opacity: 255,
            z_index: 0,
            data: CelChunkData::Linked { frame: 1 },
        }));
        doc.frames.push(f2);

        let result = document_to_archive(&doc, "hero").unwrap();
        let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
        else {
            panic!()
        };
        let walk = &states[1];
        let linked = walk.sprite.cels.iter().find(|c| {
            matches!(c.data, CelData::Linked { source_frame } if source_frame == FrameIndex::new(0))
        });
        assert!(linked.is_some(), "linked cel should point at state frame 0");
    }

    #[test]
    fn import_unknown_blend_mode_warns_and_uses_normal() {
        let mut doc = AsepriteDocument::empty(4, 4);
        let mut f0 = DocumentFrame::new(100);
        f0.chunks.push(Chunk::Layer(LayerChunk {
            flags: LAYER_FLAG_VISIBLE | LAYER_FLAG_EDITABLE,
            kind: LayerKindCode::Normal,
            child_level: 0,
            blend: BlendMode::Normal,
            unknown_blend_code: Some(99),
            opacity: 255,
            name: "weird".into(),
            tileset_index: 0,
            uuid: None,
        }));
        f0.chunks.push(Chunk::Cel(compressed_cel(0, 4, 4)));
        doc.frames.push(f0);

        let result = document_to_archive(&doc, "s").unwrap();
        assert_eq!(result.warnings.len(), 1);
        assert!(matches!(
            &result.warnings[0],
            ConversionWarning::UnknownBlendMode { code: 99, .. }
        ));
        let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
        else {
            panic!()
        };
        assert_eq!(states[0].sprite.layers[0].blend_mode, BlendMode::Normal);
    }

    // ── Export ────────────────────────────────────────────────────────────────

    fn make_two_state_project() -> pixhaus_core::project::Project {
        let layer = Layer::raster(LayerId::new(1), "bg");

        let frame = pixhaus_core::project::Frame {
            duration_ms: 100,
            user_data: UserData::default(),
        };

        let cel = Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            pixhaus_core::project::PixelBufferId::new(1),
            Size::new(8, 8),
        );

        let make_state = |name: &str, state_id: u32, sprite_id: u32| LibNamedSprite {
            id: StateId::new(state_id),
            state_name: name.into(),
            sprite: Sprite {
                id: SpriteId::new(sprite_id),
                name: format!("hero / {name}"),
                canvas: Size::new(8, 8),
                color_mode: ColorMode::Rgba,
                transparent_color_index: None,
                layers: vec![layer.clone()],
                frames: vec![frame.clone()],
                cels: vec![cel.clone()],
                palettes: Vec::new(),
                palette_frame_overrides: Vec::new(),
                tilesets: Vec::new(),
                frame_tags: Vec::new(),
                animations: Vec::new(),
                slices: Vec::new(),
                user_data: UserData::default(),
            },
            engine_tags: Vec::new(),
        };

        let entity = Entity {
            id: EntityId::new(1),
            kind: LibEntityKind::Custom("Character".into()),
            name: "hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: LibEntityDefaults::default(),
            content: LibEntityContent::Sprites {
                states: vec![make_state("idle", 1, 1), make_state("walk", 2, 2)],
            },
            ai: LibAiMetadata::default(),
            anchor_reference_id: None,
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        };

        let mut project = pixhaus_core::project::Project::new("test");
        project.library.entities.push(entity);
        project
    }

    #[test]
    fn export_two_states_produces_tags() {
        let project = make_two_state_project();
        let archive = PixhausArchive {
            project,
            buffers: vec![crate::pixhaus::PixelBufferEntry {
                id: 1,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 8 * 8 * 4],
            }],
        };

        let doc = archive_to_document(&archive).unwrap();

        // Two states → two frames (one per state, each with one frame)
        assert_eq!(doc.frames.len(), 2);

        let tags = doc.frames[0]
            .chunks
            .iter()
            .find_map(|c| {
                if let Chunk::Tags(t) = c {
                    Some(t)
                } else {
                    None
                }
            })
            .expect("frame 0 should have a tags chunk");

        assert_eq!(tags.tags.len(), 2);
        assert_eq!(tags.tags[0].name, "idle");
        assert_eq!(tags.tags[0].from_frame, 0);
        assert_eq!(tags.tags[0].to_frame, 0);
        assert_eq!(tags.tags[1].name, "walk");
        assert_eq!(tags.tags[1].from_frame, 1);
        assert_eq!(tags.tags[1].to_frame, 1);
    }

    #[test]
    fn export_empty_library_errors() {
        let project = pixhaus_core::project::Project::new("empty");
        let archive = PixhausArchive::new(project);
        assert!(matches!(
            archive_to_document(&archive).unwrap_err(),
            Error::NoSpriteEntityForExport
        ));
    }

    // ── sanitize_filename_stem ─────────────────────────────────────────────

    #[test]
    fn sanitize_spaces_become_hyphens() {
        assert_eq!(sanitize_filename_stem("idle walk"), "idle-walk");
    }

    #[test]
    fn sanitize_dots_stripped() {
        assert_eq!(sanitize_filename_stem("run.fast"), "run-fast");
    }

    #[test]
    fn sanitize_leading_trailing_hyphens_trimmed() {
        assert_eq!(sanitize_filename_stem("--idle--"), "idle");
    }

    #[test]
    fn sanitize_consecutive_hyphens_collapsed() {
        assert_eq!(sanitize_filename_stem("a  b"), "a-b");
    }

    #[test]
    fn sanitize_empty_falls_back_to_state() {
        assert_eq!(sanitize_filename_stem(""), "state");
        assert_eq!(sanitize_filename_stem("   "), "state");
        assert_eq!(sanitize_filename_stem("..."), "state");
    }

    #[test]
    fn sanitize_unicode_dropped() {
        // Non-ASCII chars are replaced with '-'; consecutive collapse applies.
        assert_eq!(sanitize_filename_stem("héro"), "h-ro");
        assert_eq!(sanitize_filename_stem("空白"), "state");
    }

    #[test]
    fn sanitize_underscores_preserved() {
        assert_eq!(sanitize_filename_stem("run_fast"), "run_fast");
    }

    #[test]
    fn export_layer_round_trips() {
        let project = make_two_state_project();
        let archive = PixhausArchive {
            project,
            buffers: vec![crate::pixhaus::PixelBufferEntry {
                id: 1,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 8 * 8 * 4],
            }],
        };
        let doc = archive_to_document(&archive).unwrap();

        let layers: Vec<_> = doc.frames[0]
            .chunks
            .iter()
            .filter_map(|c| {
                if let Chunk::Layer(l) = c {
                    Some(l)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].name, "bg");
        assert!(layers[0].flags & LAYER_FLAG_VISIBLE != 0);
        assert_eq!(layers[0].child_level, 0);
    }

    // ── Tilemap re-association ─────────────────────────────────────────────

    // Build a tileset chunk with RGBA inline pixels (Aseprite vertical-strip layout).
    // tile_count tiles, each tile_w x tile_h pixels.
    fn inline_tileset_chunk(
        tileset_id: u32,
        tile_count: u32,
        tile_w: u16,
        tile_h: u16,
    ) -> TilesetChunk {
        let bpp: usize = 4; // RGBA
        let strip_len = tile_w as usize * tile_h as usize * tile_count as usize * bpp;
        // Fill with a pattern so different tiles have distinct bytes.
        let pixels: Vec<u8> = (0..strip_len).map(|i| (i & 0xFF) as u8).collect();
        TilesetChunk {
            tileset_id,
            flags: 0,
            tile_count,
            tile_width: tile_w,
            tile_height: tile_h,
            base_index: 1,
            name: format!("tileset-{tileset_id}"),
            source: TilesetSourceWire::Inline { pixels },
        }
    }

    // Build a tilemap layer chunk referencing `tileset_id`.
    fn tilemap_layer_chunk(name: &str, tileset_index: u32) -> LayerChunk {
        LayerChunk {
            flags: LAYER_FLAG_VISIBLE | LAYER_FLAG_EDITABLE,
            kind: LayerKindCode::Tilemap,
            child_level: 0,
            blend: BlendMode::Normal,
            unknown_blend_code: None,
            opacity: 255,
            name: name.into(),
            tileset_index,
            uuid: None,
        }
    }

    // Build a tilemap cel chunk with an explicit tile-index list.
    fn tilemap_cel_chunk(
        layer_index: u16,
        grid_w: u16,
        grid_h: u16,
        tile_indices: Vec<u32>,
    ) -> CelChunk {
        CelChunk {
            layer_index,
            x: 0,
            y: 0,
            opacity: 255,
            z_index: 0,
            data: CelChunkData::Tilemap {
                width: grid_w,
                height: grid_h,
                bits_per_tile: 32,
                tile_id_mask: 0x1FFF_FFFF,
                x_flip_mask: 0x0800_0000,
                y_flip_mask: 0x0400_0000,
                diagonal_flip_mask: 0x0200_0000,
                tiles: tile_indices,
            },
        }
    }

    /// After `document_to_archive`, the tilemap layer's `tileset` ID must resolve
    /// to an entry in `sprite.tilesets`, and the tile cell indices must match
    /// what the document encoded. After a second `archive_to_document` →
    /// `document_to_archive` pass, both invariants must still hold and the tile
    /// indices must be identical to the first pass — this is the re-association
    /// that was previously un-exercised.
    #[test]
    fn tilemap_layer_references_imported_tileset_and_survives_round_trip() {
        // Construct a minimal document: one tileset (3 tiles, 2×2 px, RGBA),
        // one tilemap layer that references it, one frame with a 2×1 tilemap cel
        // whose cells use tile indices 1 and 2.
        const TILESET_ID: u32 = 0;
        const TILE_W: u16 = 2;
        const TILE_H: u16 = 2;
        const TILE_COUNT: u32 = 3; // empty tile (0) + two real tiles (1, 2)

        let mut doc = AsepriteDocument::empty(4, 2); // canvas: 2 tiles × 2 px wide, 1 tile × 2 px tall
        let mut f0 = DocumentFrame::new(100);
        f0.chunks.push(Chunk::Tileset(inline_tileset_chunk(
            TILESET_ID, TILE_COUNT, TILE_W, TILE_H,
        )));
        f0.chunks
            .push(Chunk::Layer(tilemap_layer_chunk("tiles", TILESET_ID)));
        f0.chunks
            .push(Chunk::Cel(tilemap_cel_chunk(0, 2, 1, vec![1, 2])));
        doc.frames.push(f0);

        // ── First import ──────────────────────────────────────────────────────
        let result1 = document_to_archive(&doc, "tilemap-test").unwrap();
        let EntityContent::Sprites { states: s1 } =
            &result1.archive.project.library.entities[0].content
        else {
            panic!("expected Sprites");
        };
        let sprite1 = &s1[0].sprite;

        // Tileset must be imported.
        assert_eq!(
            sprite1.tilesets.len(),
            1,
            "first import must have one tileset"
        );
        let tileset_id_1 = sprite1.tilesets[0].id;

        // Tilemap layer must reference an existing tileset.
        let layer1 = sprite1
            .layers
            .iter()
            .find(|l| matches!(&l.kind, LayerKind::Tilemap { .. }))
            .expect("first import must have a tilemap layer");
        let LayerKind::Tilemap { tileset: ref_id_1 } = &layer1.kind else {
            panic!("expected Tilemap kind");
        };
        assert_eq!(
            *ref_id_1, tileset_id_1,
            "tilemap layer must reference the imported tileset after first import"
        );

        // Tile indices must be [1, 2] as encoded.
        let cel1 = sprite1
            .cels
            .iter()
            .find(|c| matches!(&c.data, CelData::Tilemap { .. }))
            .expect("first import must have a tilemap cel");
        let CelData::Tilemap { data: td1 } = &cel1.data else {
            panic!("expected Tilemap cel data on first import");
        };
        assert_eq!(td1.cells[0].index.get(), 1, "first cell index must be 1");
        assert_eq!(td1.cells[1].index.get(), 2, "second cell index must be 2");

        // ── Round-trip: archive → document → re-import ────────────────────────
        let doc2 = archive_to_document(&result1.archive).unwrap();
        let result2 = document_to_archive(&doc2, "tilemap-test").unwrap();
        let EntityContent::Sprites { states: s2 } =
            &result2.archive.project.library.entities[0].content
        else {
            panic!("expected Sprites after round-trip");
        };
        let sprite2 = &s2[0].sprite;

        // Tileset count must be preserved.
        assert_eq!(
            sprite1.tilesets.len(),
            sprite2.tilesets.len(),
            "tileset count must survive round-trip"
        );

        // Tilemap layer after round-trip must reference a tileset that exists in the sprite.
        let layer2 = sprite2
            .layers
            .iter()
            .find(|l| matches!(&l.kind, LayerKind::Tilemap { .. }))
            .expect("round-trip must preserve the tilemap layer");
        let LayerKind::Tilemap { tileset: ref_id_2 } = &layer2.kind else {
            panic!("expected Tilemap kind after round-trip");
        };
        let tileset_exists = sprite2.tilesets.iter().any(|ts| ts.id == *ref_id_2);
        assert!(
            tileset_exists,
            "tilemap layer must reference a tileset that exists in sprite.tilesets after round-trip"
        );

        // Tile indices must be identical after the round-trip.
        let cel2 = sprite2
            .cels
            .iter()
            .find(|c| matches!(&c.data, CelData::Tilemap { .. }))
            .expect("round-trip must preserve the tilemap cel");
        let CelData::Tilemap { data: td2 } = &cel2.data else {
            panic!("expected Tilemap cel data after round-trip");
        };
        assert_eq!(
            td1.cells[0].index, td2.cells[0].index,
            "first tile index must be identical after round-trip"
        );
        assert_eq!(
            td1.cells[1].index, td2.cells[1].index,
            "second tile index must be identical after round-trip"
        );
    }
}
