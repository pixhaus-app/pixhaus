//! Verb context: what the runtime hands to every verb at invocation.
//!
//! A [`VerbContext`] is a read-only snapshot. Verbs read from it; they
//! never mutate the project directly. Side effects flow back as
//! [`super::output::VerbEffect`]s, which the host applies through the
//! undo system after the user accepts the preview.
//!
//! Snapshots are passed by value because verbs may run on a worker
//! thread (`tokio::task::spawn_blocking`) where shared references would
//! force `'static` bounds. Cloning is cheap relative to the cost of any
//! useful verb (a sprite is bytes of structured data, not pixel
//! buffers).

use serde::{Deserialize, Serialize};

use pixhaus_core::project::{
    FrameIndex, IVec2, LayerId, Palette, ProjectMetadata, Rect, Size, Sprite, SpriteId,
};

/// Raw pixel bytes carried in or out of a verb.
///
/// Pixel buffers are not part of the on-disk project model — the model
/// references them by [`pixhaus_core::project::PixelBufferId`]. Verbs
/// produce *new* pixel data and consume *reference* pixel data, both of
/// which need to cross the protocol boundary. `PixelData` is the
/// inline carrier: layout-explicit bytes plus the metadata to interpret
/// them.
///
/// The host materialises inbound pixel data into a buffer-registry
/// entry on commit, returning a real `PixelBufferId` for the cel that
/// references it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PixelData {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per pixel — `4` for RGBA8, `1` for indexed.
    pub bytes_per_pixel: u8,
    /// Bytes per row, including any padding. Must be `>= width *
    /// bytes_per_pixel`. Allows the producer to keep SIMD-friendly
    /// alignment without forcing a copy.
    pub stride: u32,
    /// Raw bytes. `bytes.len()` must equal `stride * height`.
    pub bytes: Vec<u8>,
}

impl PixelData {
    /// Tightly-packed RGBA8 image with no row padding.
    #[must_use]
    pub fn rgba8(width: u32, height: u32, bytes: Vec<u8>) -> Self {
        Self {
            width,
            height,
            bytes_per_pixel: 4,
            stride: width.saturating_mul(4),
            bytes,
        }
    }

    /// Width and height as a [`Size`].
    #[must_use]
    pub const fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Returns `true` if the byte count, stride, and dimensions are
    /// internally consistent.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let row_min = u64::from(self.width) * u64::from(self.bytes_per_pixel);
        let stride = u64::from(self.stride);
        let height = u64::from(self.height);
        stride >= row_min && self.bytes.len() as u64 == stride * height
    }
}

/// A reference image visible to the runtime.
///
/// Reference layers in the project model carry a `PixelBufferId`
/// handle; the runtime resolves the handle to bytes when packaging the
/// verb context. The `origin` mirrors
/// [`pixhaus_core::project::LayerKind::Reference::origin`] so verbs can
/// reason about placement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReferenceImage {
    /// Top-left placement of the reference image in canvas
    /// coordinates.
    pub origin: IVec2,
    /// The reference image as raw bytes.
    pub pixels: PixelData,
    /// User-facing label, copied from the source layer's `name` field.
    pub label: String,
}

/// Project-level style reference.
///
/// Style references are richer than reference images: they may point
/// at trained models (`LoRA` outputs from S30), tagged style sheets, or
/// external palettes. Verbs that need style conditioning consume them
/// as opaque blobs — only the verb knows how to feed a particular
/// style reference into its backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StyleReference {
    /// In-project palette referenced by name.
    Palette {
        /// Display name of the palette.
        name: String,
        /// Inline copy of the palette.
        palette: Palette,
    },
    /// Pixel-art style sheet — a single image cropped to representative
    /// content.
    StyleSheet {
        /// User-facing label.
        label: String,
        /// The style sheet itself.
        pixels: PixelData,
    },
    /// Trained style model. The bytes are opaque to the runtime; verbs
    /// that consume this kind own the format.
    TrainedModel {
        /// Verb-namespaced model identifier (e.g.
        /// `"pixhaus.style.lora"`). Names the format the bytes carry.
        model_id: String,
        /// Display name shown in the style picker.
        label: String,
        /// Opaque model bytes.
        bytes: Vec<u8>,
    },
}

/// Read-only snapshot the runtime hands to every verb.
///
/// Constructed by the host from the live project state. Verbs may mine
/// any field, including the full [`Sprite`], to inform their backend
/// calls. Mutations come back as [`super::output::VerbEffect`]s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerbContext {
    /// Project metadata snapshot.
    pub project: ProjectMetadata,
    /// The sprite currently being edited, if any. Contains the full
    /// layer stack, frame timeline, palettes, and tilesets so verbs
    /// can read whatever they need without round-tripping.
    pub sprite: Option<Sprite>,
    /// Active sprite ID, mirrored on the context for the common case
    /// of "I just need the ID to attach an effect".
    pub active_sprite: Option<SpriteId>,
    /// Currently-edited layer.
    pub active_layer: Option<LayerId>,
    /// Currently-displayed frame.
    pub active_frame: Option<FrameIndex>,
    /// Active palette resolved from
    /// [`pixhaus_core::project::CanvasState`]. May differ from the
    /// sprite's first palette when the user has switched context.
    pub active_palette: Option<Palette>,
    /// Selection bounds, if any. `None` means "no selection — operate
    /// on the whole canvas".
    pub selection: Option<Rect>,
    /// Reference images visible at invocation time.
    pub references: Vec<ReferenceImage>,
    /// Project-level style references the user (or a prior style-
    /// learning verb) configured.
    pub style_refs: Vec<StyleReference>,
}

impl VerbContext {
    /// Constructs an empty context with no active selection. Useful in
    /// tests and as a starting point for context-builder code.
    #[must_use]
    pub fn empty(project: ProjectMetadata) -> Self {
        Self {
            project,
            sprite: None,
            active_sprite: None,
            active_layer: None,
            active_frame: None,
            active_palette: None,
            selection: None,
            references: Vec::new(),
            style_refs: Vec::new(),
        }
    }

    /// Returns the active sprite, or [`super::error::VerbError::MissingContext`]
    /// if the caller did not supply one. Verbs that *need* a sprite
    /// reach for this rather than handling `Option` themselves.
    pub fn require_sprite(&self) -> super::error::Result<&Sprite> {
        self.sprite
            .as_ref()
            .ok_or(super::error::VerbError::MissingContext("active sprite"))
    }

    /// Returns the active sprite ID, or
    /// [`super::error::VerbError::MissingContext`] otherwise.
    pub fn require_sprite_id(&self) -> super::error::Result<SpriteId> {
        self.active_sprite
            .ok_or(super::error::VerbError::MissingContext("active sprite"))
    }

    /// Returns the active layer, or [`super::error::VerbError::MissingContext`]
    /// otherwise.
    pub fn require_active_layer(&self) -> super::error::Result<LayerId> {
        self.active_layer
            .ok_or(super::error::VerbError::MissingContext("active layer"))
    }

    /// Returns the active frame index, or
    /// [`super::error::VerbError::MissingContext`] otherwise.
    pub fn require_active_frame(&self) -> super::error::Result<FrameIndex> {
        self.active_frame
            .ok_or(super::error::VerbError::MissingContext("active frame"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "ctx-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0.1.0".into(),
        }
    }

    #[test]
    fn rgba8_constructor_packs_stride() {
        let p = PixelData::rgba8(2, 2, vec![0; 16]);
        assert_eq!(p.stride, 8);
        assert_eq!(p.bytes_per_pixel, 4);
        assert_eq!(p.size(), Size::new(2, 2));
        assert!(p.is_well_formed());
    }

    #[test]
    fn malformed_pixel_data_detected() {
        let too_few = PixelData {
            width: 4,
            height: 4,
            bytes_per_pixel: 4,
            stride: 16,
            bytes: vec![0; 32],
        };
        assert!(!too_few.is_well_formed());
        let stride_too_small = PixelData {
            width: 4,
            height: 4,
            bytes_per_pixel: 4,
            stride: 8,
            bytes: vec![0; 32],
        };
        assert!(!stride_too_small.is_well_formed());
    }

    #[test]
    fn empty_context_has_no_sprite() {
        let ctx = VerbContext::empty(metadata());
        assert!(ctx.sprite.is_none());
        assert!(ctx.references.is_empty());
        assert!(matches!(
            ctx.require_sprite(),
            Err(super::super::error::VerbError::MissingContext(_))
        ));
    }

    #[test]
    fn require_active_layer_surfaces_missing() {
        let ctx = VerbContext::empty(metadata());
        let err = ctx.require_active_layer().unwrap_err();
        assert!(matches!(
            err,
            super::super::error::VerbError::MissingContext(_)
        ));
    }

    #[test]
    fn pixel_data_round_trips_as_json() {
        let p = PixelData::rgba8(1, 1, vec![1, 2, 3, 4]);
        let json = serde_json::to_string(&p).unwrap();
        let back: PixelData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}
