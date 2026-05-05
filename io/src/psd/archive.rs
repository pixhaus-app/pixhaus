//! Conversion from a parsed [`psd::Psd`] document into a [`PixhausArchive`].
//!
//! PSD import is read-only: this module never writes PSD files.
//! The conversion is intentionally lossy for features Pixhaus v1 does not
//! model: clipping masks, adjustment layers, smart objects. Non-fatal losses
//! are collected as [`ConversionWarning`]s returned alongside the archive.

use std::collections::HashMap;

use pixhaus_core::project::{
    BrushState, CanvasState, Cel, CelData, ColorMode, FeatureFlags, Frame, FrameIndex, IVec2,
    Layer, LayerId, LayerKind, PixelBufferId, Project, ProjectMetadata, SchemaVersion,
    SelectionState, Size, Sprite, SpriteId, UserData,
};

use crate::error::{Error, Result};
use crate::pixhaus::{PixelBufferEntry, PixhausArchive};

use super::spec::blend_mode_from_psd_debug;

/// Non-fatal issue encountered while converting a PSD document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConversionWarning {
    /// The PSD file uses more than 8 bits per channel. The `psd` crate
    /// converts to 8-bit RGBA automatically; precision is lost on import.
    HighBitDepthDownsampled {
        /// Bits per channel declared in the file header (16 or 32).
        bits: u8,
    },
    /// A PSD blend mode has no direct Pixhaus equivalent. The layer
    /// falls back to `BlendMode::Normal`.
    UnsupportedBlendMode {
        /// Name of the layer or group that carries the blend mode.
        layer_name: String,
        /// Debug representation of the original PSD blend mode variant.
        psd_mode: String,
    },
    /// A layer carries a clipping mask. Pixhaus v1 does not model clipping
    /// masks during import; the layer's pixel data is included without the
    /// mask applied.
    ClippingMaskIgnored {
        /// Name of the affected layer.
        layer_name: String,
    },
    /// A pixel layer has zero width or height and contributes no pixel
    /// data. The layer itself is still present in the layer list but has
    /// no associated cel.
    EmptyLayerSkipped {
        /// Name of the empty layer.
        layer_name: String,
    },
    /// The PSD file uses a non-RGB color mode. The `psd` crate converts
    /// channels to RGBA internally; results may differ from Photoshop's
    /// own display.
    NonRgbColorModeConverted {
        /// Human-readable name of the source color mode.
        mode: String,
    },
}

/// Result of converting a [`psd::Psd`] into a [`PixhausArchive`].
#[derive(Debug)]
pub struct ConvertedArchive {
    /// The translated archive ready for further encoding or editing.
    pub archive: PixhausArchive,
    /// Non-fatal warnings raised during conversion.
    pub warnings: Vec<ConversionWarning>,
}

/// Convert a parsed [`psd::Psd`] document into a [`PixhausArchive`].
///
/// `name` is used as the sprite name in the resulting project. PSD files are
/// always treated as single-frame; the resulting sprite has exactly one frame.
///
/// Groups are emitted before pixel layers in the Pixhaus layer list. The
/// parent-child relationships are correct; the visual stacking order within
/// the same parent may differ from the PSD source when groups and pixel layers
/// are interleaved at the same level.
///
/// # Errors
///
/// Returns [`Error::PsdParse`] when the color mode cannot be converted
/// (CMYK, Indexed, Lab, Multichannel) or when the bit depth is not 8, 16, or
/// 32 bits per channel.
#[allow(clippy::too_many_lines)]
pub fn document_to_archive(psd: &psd::Psd, name: &str) -> Result<ConvertedArchive> {
    let mut warnings: Vec<ConversionWarning> = Vec::new();

    // Warn on high bit depth — the psd crate always outputs 8-bit RGBA.
    match psd.depth() {
        psd::PsdDepth::Eight => {}
        psd::PsdDepth::Sixteen => {
            warnings.push(ConversionWarning::HighBitDepthDownsampled { bits: 16 });
        }
        psd::PsdDepth::ThirtyTwo => {
            warnings.push(ConversionWarning::HighBitDepthDownsampled { bits: 32 });
        }
        other @ psd::PsdDepth::One => {
            return Err(Error::PsdParse(format!("unsupported bit depth: {other:?}")));
        }
    }

    // Reject colour modes that cannot be approximated as RGBA.
    match psd.color_mode() {
        psd::ColorMode::Rgb => {}
        psd::ColorMode::Grayscale => {
            warnings.push(ConversionWarning::NonRgbColorModeConverted {
                mode: "Grayscale".into(),
            });
        }
        other => {
            return Err(Error::PsdParse(format!(
                "unsupported color mode: {other:?} — only RGB and Grayscale are supported"
            )));
        }
    }

    let canvas_w = psd.width();
    let canvas_h = psd.height();

    // Assign Pixhaus LayerIds to PSD groups first so pixel layers that
    // reference a parent group can resolve the id during their own pass.
    let mut next_layer_id: u32 = 1;
    let mut next_buffer_id: u32 = 1;
    let mut group_psd_id_to_layer_id: HashMap<u32, LayerId> = HashMap::new();

    for &psd_group_id in psd.group_ids_in_order() {
        let id = LayerId::new(next_layer_id);
        next_layer_id += 1;
        group_psd_id_to_layer_id.insert(psd_group_id, id);
    }

    let mut layers: Vec<Layer> = Vec::new();
    let mut cels: Vec<Cel> = Vec::new();
    let mut buffers: Vec<PixelBufferEntry> = Vec::new();

    // PSD's group_ids_in_order() returns top-to-bottom; Pixhaus stores
    // layers bottom-to-top (index 0 is the bottommost). Reverse so the
    // visual stack order survives the import.
    for &psd_group_id in psd.group_ids_in_order().iter().rev() {
        let Some(group) = psd.groups().get(&psd_group_id) else {
            continue;
        };
        let layer_id = group_psd_id_to_layer_id[&psd_group_id];
        let parent = group
            .parent_id()
            .and_then(|pid| group_psd_id_to_layer_id.get(&pid).copied());

        let blend_debug = format!("{:?}", group.blend_mode());
        let (blend_mode, had_unknown) = blend_mode_from_psd_debug(&blend_debug);
        if had_unknown {
            warnings.push(ConversionWarning::UnsupportedBlendMode {
                layer_name: group.name().to_string(),
                psd_mode: blend_debug,
            });
        }

        layers.push(Layer {
            id: layer_id,
            name: group.name().to_string(),
            kind: LayerKind::Group { collapsed: false },
            blend_mode,
            opacity: group.opacity(),
            visible: group.visible(),
            locked: false,
            parent,
            user_data: UserData::default(),
        });
    }

    // Emit pixel layers. psd.layers() is ordered top-to-bottom visually;
    // reverse for Pixhaus's bottom-to-top convention (see groups loop above).
    for psd_layer in psd.layers().iter().rev() {
        let layer_id = LayerId::new(next_layer_id);
        next_layer_id += 1;

        let parent = psd_layer
            .parent_id()
            .and_then(|pid| group_psd_id_to_layer_id.get(&pid).copied());

        let blend_debug = format!("{:?}", psd_layer.blend_mode());
        let (blend_mode, had_unknown) = blend_mode_from_psd_debug(&blend_debug);
        if had_unknown {
            warnings.push(ConversionWarning::UnsupportedBlendMode {
                layer_name: psd_layer.name().to_string(),
                psd_mode: blend_debug,
            });
        }

        if psd_layer.is_clipping_mask() {
            warnings.push(ConversionWarning::ClippingMaskIgnored {
                layer_name: psd_layer.name().to_string(),
            });
        }

        layers.push(Layer {
            id: layer_id,
            name: psd_layer.name().to_string(),
            kind: LayerKind::Raster,
            blend_mode,
            opacity: psd_layer.opacity(),
            visible: psd_layer.visible(),
            locked: false,
            parent,
            user_data: UserData::default(),
        });

        let w = u32::from(psd_layer.width());
        let h = u32::from(psd_layer.height());

        if w == 0 || h == 0 {
            warnings.push(ConversionWarning::EmptyLayerSkipped {
                layer_name: psd_layer.name().to_string(),
            });
            continue;
        }

        let pixels = psd_layer.rgba();
        let buf_id = PixelBufferId::new(next_buffer_id);
        next_buffer_id += 1;

        buffers.push(PixelBufferEntry {
            id: buf_id.get(),
            width: w,
            height: h,
            stride: w * 4,
            pixels,
        });

        cels.push(Cel {
            layer_id,
            frame_index: FrameIndex::new(0),
            position: IVec2::new(psd_layer.layer_left(), psd_layer.layer_top()),
            opacity: 255,
            data: CelData::Raster {
                buffer: buf_id,
                size: Size::new(w, h),
            },
            user_data: UserData::default(),
        });
    }

    let sprite = Sprite {
        id: SpriteId::new(1),
        name: name.to_string(),
        canvas: Size::new(canvas_w, canvas_h),
        color_mode: ColorMode::Rgba,
        transparent_color_index: None,
        layers,
        frames: vec![Frame {
            duration_ms: 100,
            user_data: UserData::default(),
        }],
        cels,
        palettes: Vec::new(),
        palette_frame_overrides: Vec::new(),
        tilesets: Vec::new(),
        frame_tags: Vec::new(),
        animations: Vec::new(),
        slices: Vec::new(),
        user_data: UserData::default(),
    };

    let project = Project {
        schema_version: SchemaVersion::current(),
        feature_flags: FeatureFlags::empty(),
        metadata: ProjectMetadata {
            name: sprite.name.clone(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: env!("CARGO_PKG_VERSION").into(),
        },
        sprites: vec![sprite],
        canvas: CanvasState::default(),
        brush: BrushState::default(),
        selection: SelectionState::default(),
    };

    Ok(ConvertedArchive {
        archive: PixhausArchive { project, buffers },
        warnings,
    })
}
