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

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use pixhaus_core::project::library::composition::{PromptId, PromptTemplate, Structure, StructureId, Style, StyleId};
use pixhaus_core::project::{EntityId, FrameIndex, IVec2, LayerId, Palette, ProjectMetadata, Rect, Size, Sprite, SpriteId};

use super::anchor::AnchorPayload;
use super::backend::InferenceBackend;
use crate::compose::builtins::BuiltinLibrary;

/// Project-tier composition records carried on a [`VerbContext`].
///
/// Owned (not borrowed) so the context stays `Clone + Serialize`. The
/// built-in registry is layered on at resolution time via
/// [`VerbContext::library_view`], not stored here — it is a pure
/// constructor with no per-project state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectCompositionLibrary {
    /// Project-tier Structures. Shadow built-ins by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structures: Vec<Structure>,
    /// Project-tier Styles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<Style>,
    /// Project-tier saved Prompts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<PromptTemplate>,
    /// The project's `style_notes` (`ProjectAi.style_notes`). Composing verbs
    /// use this as the prompt baseline in place of the built-in default, so the
    /// project's house style folds into every generation automatically.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,
}

impl ProjectCompositionLibrary {
    /// Returns `true` when no project-tier records are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.structures.is_empty() && self.styles.is_empty() && self.prompts.is_empty() && self.style_notes.is_empty()
    }
}

/// Borrowed, read-only resolution view layering project-tier records over
/// the built-in registry. Project records shadow built-ins by id.
///
/// The view borrows the project records from a [`VerbContext`] and owns a
/// freshly-loaded [`BuiltinLibrary`] (a pure, cheap constructor), so it can
/// be returned from [`VerbContext::library_view`] without a self-referential
/// borrow.
pub struct CompositionLibraryView<'a> {
    structures: &'a [Structure],
    styles: &'a [Style],
    prompts: &'a [PromptTemplate],
    builtins: BuiltinLibrary,
}

impl<'a> CompositionLibraryView<'a> {
    /// Constructs a view over the given project records and built-in registry.
    #[must_use]
    pub fn new(structures: &'a [Structure], styles: &'a [Style], prompts: &'a [PromptTemplate], builtins: BuiltinLibrary) -> Self {
        Self {
            structures,
            styles,
            prompts,
            builtins,
        }
    }

    /// Resolves a Structure id: project record first, then built-in.
    #[must_use]
    pub fn structure(&self, id: &StructureId) -> Option<&Structure> {
        self.structures.iter().find(|s| &s.id == id).or_else(|| self.builtins.structures.get(id))
    }

    /// Resolves a Style id: project record first, then built-in.
    #[must_use]
    pub fn style(&self, id: &StyleId) -> Option<&Style> {
        self.styles.iter().find(|s| &s.id == id).or_else(|| self.builtins.styles.get(id))
    }

    /// Resolves a Prompt id: project record first, then built-in.
    #[must_use]
    pub fn prompt(&self, id: &PromptId) -> Option<&PromptTemplate> {
        self.prompts.iter().find(|p| &p.id == id).or_else(|| self.builtins.prompts.get(id))
    }
}

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
    /// internally consistent. `bytes_per_pixel` must be one of the four
    /// supported pixel widths — `1` (indexed/alpha), `2` (grayscale +
    /// alpha), `3` (RGB), `4` (RGBA). Zero would let downstream code
    /// divide by it; values above 4 don't match any format the runtime
    /// hands out.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        if !matches!(self.bytes_per_pixel, 1..=4) {
            return false;
        }
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
///
/// Use [`VerbContextBuilder`] to construct a context from live editor
/// state — it is more readable than field assignment and enforces the
/// expected field order.
///
/// # Serialization
///
/// `VerbContext` serialises without the `backend` field (the field is
/// skipped). Deserialising a context always yields `backend: None`.
/// The runtime fills in `backend` on each invocation; the field is
/// never persisted.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// Library entity this invocation targets, if operating in library mode.
    ///
    /// The host sets this when invoking a verb on behalf of a specific library
    /// entity (e.g. auto-tag, cross-entity variant). Verbs that operate at the
    /// sprite level leave this `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_entity_id: Option<EntityId>,

    /// Style-anchor reference image for the target entity.
    ///
    /// Resolved by the host from the target sprite entity's embedded reference
    /// sheet before invocation. `None` when the entity has no sheet or the verb
    /// is not operating in library mode.
    ///
    /// Note: B10.3 introduced a richer [`Self::anchor`] field carrying
    /// the same upstream data plus palette, `LoRA` path, strength, and
    /// canonical hash. Both fields will coexist until a follow-up
    /// migrates B9.4's consumers (Variant text-edit) onto `anchor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_reference: Option<ReferenceImage>,

    /// Reference-sheet anchor for the entity being acted on, if the
    /// entity carries one.
    ///
    /// Resolved by the host when packaging the context: the host looks
    /// up the active sprite entity, builds an [`AnchorPayload`] from its
    /// embedded canonical reference sheet, and drops it here. `None`
    /// means "no anchor" — either the entity has no reference sheet or
    /// the verb is being invoked in a context where no entity is active
    /// (a generic palette generation, for instance).
    ///
    /// Image-generation verbs read `ctx.anchor.as_ref()` to derive
    /// `style_image`, palette constraints, the project-level `LoRA`
    /// path (B10.5 will introduce per-entity `LoRA`s), and IP-Adapter
    /// strength. Verbs that don't naturally use anchors (Critique,
    /// Conversational) ignore this field; the presence of an anchor
    /// never changes a verb's invocation requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<AnchorPayload>,

    /// Project-tier composition library (Structures, Styles, Prompts) for
    /// this invocation. Layered over the built-in registry by
    /// [`VerbContext::library_view`]. Empty for verbs that do not compose.
    #[serde(default, skip_serializing_if = "ProjectCompositionLibrary::is_empty")]
    pub composition_library: ProjectCompositionLibrary,
    /// Backend selected by the runtime for this invocation.
    ///
    /// Contract enforced by [`super::runtime::VerbRuntime::invoke`]:
    ///
    /// - When the verb's `required_capabilities` is empty (local CPU
    ///   only, like `EchoVerb`), this is `None`.
    /// - When the verb declares any required capabilities, this is
    ///   guaranteed to be `Some(_)` by the time the verb body runs.
    ///   Backend selection happens before spawn; if no registered
    ///   backend matches, `invoke` returns
    ///   [`super::error::VerbError::UnsupportedCapability`] or
    ///   [`super::error::VerbError::BackendUnavailable`] without ever
    ///   reaching the verb. So a verb that declared required
    ///   capabilities can `expect(...)` on this field without further
    ///   handling.
    ///
    /// Verbs that need a specific inference method downcast via
    /// [`super::backend::InferenceBackend::as_any`]:
    /// `ctx.backend.as_ref().and_then(|b| b.as_any().downcast_ref::<MyBackend>())`.
    /// That downcast lives in the verb, not in the protocol.
    #[serde(skip, default)]
    pub backend: Option<Arc<dyn InferenceBackend>>,
}

impl PartialEq for VerbContext {
    /// Two contexts are equal when their project state is equal. The
    /// `backend` field is excluded from comparison: it is runtime
    /// infrastructure, not part of the serialisable project snapshot.
    fn eq(&self, other: &Self) -> bool {
        self.project == other.project
            && self.sprite == other.sprite
            && self.active_sprite == other.active_sprite
            && self.active_layer == other.active_layer
            && self.active_frame == other.active_frame
            && self.active_palette == other.active_palette
            && self.selection == other.selection
            && self.references == other.references
            && self.style_refs == other.style_refs
            && self.library_entity_id == other.library_entity_id
            && self.anchor_reference == other.anchor_reference
            && self.anchor == other.anchor
            && self.composition_library == other.composition_library
    }
}

impl VerbContext {
    /// Constructs an empty context with no active state.
    ///
    /// Prefer [`VerbContext::builder`] in application code where several
    /// fields are being filled in; `empty` is useful in tests and in code
    /// that builds contexts incrementally.
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
            library_entity_id: None,
            anchor_reference: None,
            anchor: None,
            composition_library: ProjectCompositionLibrary::default(),
            backend: None,
        }
    }

    /// Returns a resolution view layering this context's project-tier
    /// composition records over a freshly-loaded built-in registry.
    #[must_use]
    pub fn library_view(&self) -> CompositionLibraryView<'_> {
        CompositionLibraryView::new(
            &self.composition_library.structures,
            &self.composition_library.styles,
            &self.composition_library.prompts,
            BuiltinLibrary::load(),
        )
    }

    /// Returns a [`VerbContextBuilder`] seeded with project metadata.
    #[must_use]
    pub fn builder(project: ProjectMetadata) -> VerbContextBuilder {
        VerbContextBuilder::new(project)
    }

    /// Returns the active sprite, or [`super::error::VerbError::MissingContext`]
    /// if the caller did not supply one. Verbs that *need* a sprite
    /// reach for this rather than handling `Option` themselves.
    pub fn require_sprite(&self) -> super::error::Result<&Sprite> {
        self.sprite.as_ref().ok_or(super::error::VerbError::MissingContext("active sprite"))
    }

    /// Returns the active sprite ID, or
    /// [`super::error::VerbError::MissingContext`] otherwise.
    pub fn require_sprite_id(&self) -> super::error::Result<SpriteId> {
        self.active_sprite.ok_or(super::error::VerbError::MissingContext("active sprite"))
    }

    /// Returns the active layer, or [`super::error::VerbError::MissingContext`]
    /// otherwise.
    pub fn require_active_layer(&self) -> super::error::Result<LayerId> {
        self.active_layer.ok_or(super::error::VerbError::MissingContext("active layer"))
    }

    /// Returns the active frame index, or
    /// [`super::error::VerbError::MissingContext`] otherwise.
    pub fn require_active_frame(&self) -> super::error::Result<FrameIndex> {
        self.active_frame.ok_or(super::error::VerbError::MissingContext("active frame"))
    }
}

/// Ergonomic builder for [`VerbContext`].
///
/// Preferred over direct field assignment in host code that packages
/// the current editor state for a verb invocation. Construct with
/// [`VerbContext::builder`].
///
/// # Example
///
/// ```no_run
/// use pixhaus_ai::plugin::{VerbContext, PixelData, ReferenceImage};
/// use pixhaus_core::project::{FrameIndex, LayerId, ProjectMetadata, SpriteId, IVec2};
///
/// # let metadata = ProjectMetadata {
/// #     name: "demo".into(), description: None, author: None,
/// #     created_at: 0, updated_at: 0, editor_version: "0".into(),
/// # };
/// let ctx = VerbContext::builder(metadata)
///     .with_active_sprite(SpriteId::new(1))
///     .with_active_layer(LayerId::new(3))
///     .with_active_frame(FrameIndex::new(2))
///     .add_reference(ReferenceImage {
///         origin: IVec2::new(0, 0),
///         pixels: PixelData::rgba8(1, 1, vec![0, 0, 0, 255]),
///         label: "Ref".into(),
///     })
///     .build();
/// ```
#[derive(Debug)]
pub struct VerbContextBuilder {
    ctx: VerbContext,
}

impl VerbContextBuilder {
    /// Constructs a builder seeded with project metadata. All optional
    /// fields default to `None` / empty.
    #[must_use]
    pub fn new(project: ProjectMetadata) -> Self {
        Self {
            ctx: VerbContext::empty(project),
        }
    }

    /// Sets the full sprite snapshot, and also sets `active_sprite`
    /// to the sprite's ID so callers do not need to set both.
    #[must_use]
    pub fn with_sprite(mut self, sprite: Sprite) -> Self {
        self.ctx.active_sprite = Some(sprite.id);
        self.ctx.sprite = Some(sprite);
        self
    }

    /// Sets only the active sprite ID without providing the full
    /// snapshot. Use [`Self::with_sprite`] when the full `Sprite` is
    /// available.
    #[must_use]
    pub fn with_active_sprite(mut self, id: SpriteId) -> Self {
        self.ctx.active_sprite = Some(id);
        self
    }

    /// Sets the active layer.
    #[must_use]
    pub fn with_active_layer(mut self, id: LayerId) -> Self {
        self.ctx.active_layer = Some(id);
        self
    }

    /// Sets the active frame.
    #[must_use]
    pub fn with_active_frame(mut self, frame: FrameIndex) -> Self {
        self.ctx.active_frame = Some(frame);
        self
    }

    /// Sets the active palette.
    #[must_use]
    pub fn with_active_palette(mut self, palette: Palette) -> Self {
        self.ctx.active_palette = Some(palette);
        self
    }

    /// Sets the selection bounds.
    #[must_use]
    pub fn with_selection(mut self, selection: Rect) -> Self {
        self.ctx.selection = Some(selection);
        self
    }

    /// Appends a reference image to the context.
    #[must_use]
    pub fn add_reference(mut self, reference: ReferenceImage) -> Self {
        self.ctx.references.push(reference);
        self
    }

    /// Appends a style reference to the context.
    #[must_use]
    pub fn add_style_ref(mut self, style_ref: StyleReference) -> Self {
        self.ctx.style_refs.push(style_ref);
        self
    }

    /// Sets the library entity this invocation targets.
    #[must_use]
    pub fn with_library_entity(mut self, entity_id: EntityId) -> Self {
        self.ctx.library_entity_id = Some(entity_id);
        self
    }

    /// Sets the style-anchor reference image for the target entity.
    #[must_use]
    pub fn with_anchor_reference(mut self, reference: ReferenceImage) -> Self {
        self.ctx.anchor_reference = Some(reference);
        self
    }

    /// Sets the reference-sheet anchor for this invocation.
    ///
    /// Hosts call this when the active sprite entity has an embedded
    /// canonical reference sheet; pass the [`AnchorPayload`] built from
    /// that sheet.
    #[must_use]
    pub fn with_anchor(mut self, anchor: AnchorPayload) -> Self {
        self.ctx.anchor = Some(anchor);
        self
    }

    /// Sets the project-tier composition library for this invocation.
    #[must_use]
    pub fn with_composition_library(mut self, library: ProjectCompositionLibrary) -> Self {
        self.ctx.composition_library = library;
        self
    }

    /// Finalises the builder and returns the [`VerbContext`].
    ///
    /// `backend` is left as `None`; the runtime fills it in during
    /// dispatch based on capability selection.
    #[must_use]
    pub fn build(self) -> VerbContext {
        self.ctx
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
        assert!(matches!(ctx.require_sprite(), Err(super::super::error::VerbError::MissingContext(_))));
    }

    #[test]
    fn require_active_layer_surfaces_missing() {
        let ctx = VerbContext::empty(metadata());
        let err = ctx.require_active_layer().unwrap_err();
        assert!(matches!(err, super::super::error::VerbError::MissingContext(_)));
    }

    #[test]
    fn pixel_data_round_trips_as_json() {
        let p = PixelData::rgba8(1, 1, vec![1, 2, 3, 4]);
        let json = serde_json::to_string(&p).unwrap();
        let back: PixelData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn builder_sets_all_fields() {
        use pixhaus_core::project::{IVec2, SpriteId};

        let ctx = VerbContext::builder(metadata())
            .with_active_sprite(SpriteId::new(7))
            .with_active_frame(FrameIndex::new(3))
            .add_reference(ReferenceImage {
                origin: IVec2::new(0, 0),
                pixels: PixelData::rgba8(1, 1, vec![0, 0, 0, 255]),
                label: "Ref".into(),
            })
            .build();

        assert_eq!(ctx.active_sprite, Some(SpriteId::new(7)));
        assert_eq!(ctx.active_frame, Some(FrameIndex::new(3)));
        assert_eq!(ctx.references.len(), 1);
        assert!(ctx.backend.is_none());
    }

    #[test]
    fn context_equality_ignores_backend() {
        // Two contexts with identical project state but one has a
        // backend attached; they should compare as equal.
        use crate::plugin::backend::InferenceBackend;
        use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};

        #[derive(Debug)]
        struct Stub;
        impl InferenceBackend for Stub {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn id(&self) -> &'static str {
                "stub"
            }
            fn capabilities(&self) -> BackendCapabilities {
                BackendCapabilities::empty()
            }
            fn cost_estimate(&self, _: BackendCapabilities) -> CostEstimate {
                CostEstimate::free()
            }
        }

        let mut ctx_a = VerbContext::empty(metadata());
        let ctx_b = VerbContext::empty(metadata());
        ctx_a.backend = Some(Arc::new(Stub));

        assert_eq!(ctx_a, ctx_b);
    }

    #[test]
    fn library_view_resolves_project_over_builtin() {
        use pixhaus_core::project::library::composition::{Structure, StructureId, StructureOutput};

        let project_struct = Structure {
            id: StructureId("pixhaus.builtin.structure.character".into()),
            name: "Shadowed".into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        };
        let ctx = VerbContext::builder(metadata())
            .with_composition_library(ProjectCompositionLibrary {
                structures: vec![project_struct],
                styles: Vec::new(),
                prompts: Vec::new(),
                style_notes: String::new(),
            })
            .build();
        let view = ctx.library_view();
        let resolved = view.structure(&StructureId("pixhaus.builtin.structure.character".into())).unwrap();
        assert_eq!(resolved.name, "Shadowed", "project record shadows built-in");
    }

    #[test]
    fn library_view_falls_back_to_builtin() {
        use pixhaus_core::project::library::composition::StructureId;

        let ctx = VerbContext::empty(metadata());
        let view = ctx.library_view();
        // No project records: a built-in id still resolves.
        assert!(view.structure(&StructureId("pixhaus.builtin.structure.item".into())).is_some());
        assert!(view.structure(&StructureId("nonexistent".into())).is_none());
    }

    #[test]
    fn library_view_resolves_style_over_builtin() {
        use pixhaus_core::project::library::composition::{Style, StyleId};

        let shadow = Style {
            id: StyleId("pixhaus.builtin.style.default".into()),
            name: "Shadowed".into(),
            modifiers: String::new(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        };
        let ctx = VerbContext::builder(metadata())
            .with_composition_library(ProjectCompositionLibrary {
                structures: Vec::new(),
                styles: vec![shadow],
                prompts: Vec::new(),
                style_notes: String::new(),
            })
            .build();
        let view = ctx.library_view();
        let resolved = view.style(&StyleId("pixhaus.builtin.style.default".into())).unwrap();
        assert_eq!(resolved.name, "Shadowed", "project style shadows built-in");
    }

    #[test]
    fn library_view_style_falls_back_to_builtin() {
        use pixhaus_core::project::library::composition::StyleId;

        let view_owner = VerbContext::empty(metadata());
        let view = view_owner.library_view();
        assert!(view.style(&StyleId("pixhaus.builtin.style.default".into())).is_some());
        assert!(view.style(&StyleId("nope".into())).is_none());
    }

    #[test]
    fn library_view_resolves_prompt_over_builtin() {
        use pixhaus_core::project::library::composition::{PromptId, PromptTemplate};

        let project_prompt = PromptTemplate {
            id: PromptId("project.prompt.hero".into()),
            name: "Hero".into(),
            text: "a {species} hero".into(),
            variables: Vec::new(),
            default_style: None,
            default_structure: None,
        };
        let ctx = VerbContext::builder(metadata())
            .with_composition_library(ProjectCompositionLibrary {
                structures: Vec::new(),
                styles: Vec::new(),
                prompts: vec![project_prompt],
                style_notes: String::new(),
            })
            .build();
        let view = ctx.library_view();
        let resolved = view.prompt(&PromptId("project.prompt.hero".into())).unwrap();
        assert_eq!(resolved.name, "Hero");
        // No built-in prompts ship, so an unknown id resolves to nothing.
        assert!(view.prompt(&PromptId("nope".into())).is_none());
    }

    #[test]
    fn context_serializes_without_backend() {
        use crate::plugin::backend::InferenceBackend;
        use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};

        #[derive(Debug)]
        struct Stub;
        impl InferenceBackend for Stub {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn id(&self) -> &'static str {
                "stub"
            }
            fn capabilities(&self) -> BackendCapabilities {
                BackendCapabilities::empty()
            }
            fn cost_estimate(&self, _: BackendCapabilities) -> CostEstimate {
                CostEstimate::free()
            }
        }

        let mut ctx = VerbContext::empty(metadata());
        ctx.backend = Some(Arc::new(Stub));

        let json = serde_json::to_string(&ctx).unwrap();
        // The backend field must not appear in the serialised form.
        assert!(!json.contains("backend"));

        let back: VerbContext = serde_json::from_str(&json).unwrap();
        assert!(back.backend.is_none());
    }
}
