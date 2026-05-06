//! Verb: Extend (multi-direction) — S25.
//!
//! [`ExtendVerb`] generates directional views of a sprite from a single
//! source frame. Given a front-facing (south) sprite, it produces west,
//! north, and east views (4-direction) or all eight compass directions
//! (8-direction), matching the source's pixel style.
//!
//! # Protocol
//!
//! The verb declares `IMAGE_GENERATION | VIEW_SYNTHESIS` capabilities.
//! The runtime selects a backend that satisfies both (typically Replicate
//! with a TripoSR-class model, or Stability with style-conditioned
//! generation) and attaches it to `ctx.backend`. The verb calls
//! [`crate::plugin::backend::InferenceBackend::as_view_synthesis`] to
//! recover the calling interface without coupling to a specific adapter.
//!
//! # Effects
//!
//! The verb produces one [`VerbEffect::AddLayer`] per generated direction.
//! Each layer carries a single cel at the active frame containing the
//! synthesised pixels. Layer names follow the pattern
//! `"{layer_name} – {Direction}"` (e.g. `"Extend – West"`).
//!
//! # Documentation
//!
//! See `docs/verbs/extend.md` for the user-facing guide.

use std::time::Duration;

use async_trait::async_trait;
use pixhaus_core::project::{Cel, FrameIndex, Layer, LayerId, PixelBufferId, Size};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;
use crate::plugin::view_synthesis::{Direction, DirectionalViewRequest};

/// Stable identifier for the built-in Extend verb.
pub const EXTEND_VERB_ID: &str = "pixhaus.builtin.extend";

/// Which directions to generate, relative to the source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectionSet {
    /// South, West, North, East (cardinal directions, 4 views total
    /// including the source).
    FourDirection,
    /// All eight compass directions (4 cardinal + 4 diagonal).
    EightDirection,
    /// Explicit direction list. Duplicate entries are silently
    /// de-duplicated. The source direction, if present, is skipped —
    /// the verb never re-generates the frame the user already has.
    Custom {
        /// Directions to synthesise. De-duplicated; source excluded.
        directions: Vec<Direction>,
    },
}

impl DirectionSet {
    /// Returns the directions to synthesise, excluding `source`.
    ///
    /// For `FourDirection` and `EightDirection`, all standard directions
    /// except `source` are returned in the canonical display order.
    /// For `Custom`, the list is de-duplicated and `source` is removed.
    #[must_use]
    pub fn target_directions(&self, source: Direction) -> Vec<Direction> {
        let all = match self {
            Self::FourDirection => Direction::four(),
            Self::EightDirection => Direction::eight(),
            Self::Custom { directions } => {
                let mut seen = std::collections::HashSet::new();
                directions
                    .iter()
                    .filter(|d| seen.insert(**d))
                    .copied()
                    .collect()
            }
        };
        all.into_iter().filter(|d| *d != source).collect()
    }
}

/// Inputs for [`ExtendVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtendInputs {
    /// Source sprite pixels — the frame the user wants to extend from.
    /// Must be well-formed RGBA8 data.
    pub source: PixelData,
    /// Which direction the source sprite is currently facing.
    /// Defaults to `south` (front-facing).
    #[serde(default = "default_source_direction")]
    pub source_direction: Direction,
    /// Which directions to generate.
    #[serde(default = "default_direction_set")]
    pub direction_set: DirectionSet,
    /// Style fidelity 0.0–1.0. Higher values produce views that adhere
    /// more strictly to the source's colour palette and pixel style;
    /// lower values give the model more creative latitude.
    ///
    /// Defaults to `0.8`.
    #[serde(default = "default_style_intensity")]
    pub style_intensity: f32,
    /// Base name for generated layers. Defaults to `"Extend"`.
    /// Each layer is named `"{layer_name} – {direction}"`.
    #[serde(default)]
    pub layer_name: Option<String>,
}

fn default_source_direction() -> Direction {
    Direction::South
}

fn default_direction_set() -> DirectionSet {
    DirectionSet::FourDirection
}

fn default_style_intensity() -> f32 {
    0.8
}

/// Generates multi-direction sprite views from a single source frame.
///
/// See [`EXTEND_VERB_ID`] and `docs/verbs/extend.md`.
#[derive(Debug)]
pub struct ExtendVerb {
    descriptor: VerbDescriptor,
}

impl ExtendVerb {
    /// Constructs the verb.
    #[must_use]
    // `serde_json::json!` calls `Result::unwrap` on infallible internal
    // builders. The workspace disallows `unwrap` everywhere, so we exempt
    // this constructor rather than hand-rolling the schema `Value::Object`.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "object",
                    "description": "Source sprite pixels (current direction)",
                    "properties": {
                        "width":          {"type": "integer", "minimum": 1},
                        "height":         {"type": "integer", "minimum": 1},
                        "bytes_per_pixel":{"type": "integer"},
                        "stride":         {"type": "integer"},
                        "bytes":          {"type": "array", "items": {"type": "integer"}}
                    },
                    "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                },
                "source_direction": {
                    "type": "string",
                    "enum": [
                        "south", "south_west", "west", "north_west",
                        "north", "north_east", "east", "south_east"
                    ],
                    "default": "south"
                },
                "direction_set": {
                    "description": "Which directions to generate",
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": {"kind": {"const": "four_direction"}},
                            "required": ["kind"]
                        },
                        {
                            "type": "object",
                            "properties": {"kind": {"const": "eight_direction"}},
                            "required": ["kind"]
                        },
                        {
                            "type": "object",
                            "properties": {
                                "kind":       {"const": "custom"},
                                "directions": {"type": "array", "items": {"type": "string"}}
                            },
                            "required": ["kind", "directions"]
                        }
                    ],
                    "default": {"kind": "four_direction"}
                },
                "style_intensity": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.8,
                    "description": "Style fidelity 0.0–1.0 (higher = closer to source style)"
                },
                "layer_name": {
                    "type": ["string", "null"],
                    "description": "Base name for generated layers (default: Extend)"
                }
            },
            "required": ["source"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(EXTEND_VERB_ID),
                display_name: "Extend".into(),
                description: "Generate multi-direction sprite views from a single source frame"
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::IMAGE_GENERATION
                    .union(BackendCapabilities::VIEW_SYNTHESIS),
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddLayer],
                cost_estimate: CostEstimate {
                    // Four directions at ~15 s per backend call.
                    typical_latency: Duration::from_secs(60),
                    // Eight directions at ~60 s each on a loaded backend.
                    max_latency: Duration::from_secs(480),
                    // ~5 ¢ per direction via Replicate, 4 directions.
                    typical_usd_cents: 20.0,
                    // 8 directions, high-res model.
                    max_usd_cents: 80.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/extend.md".into()),
            },
        }
    }
}

impl Default for ExtendVerb {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates that `inputs` satisfy the Extend verb's preconditions.
fn validate_extend_inputs(inputs: &VerbInputs) -> Result<()> {
    let parsed: ExtendInputs = inputs.deserialize()?;
    if !parsed.source.is_well_formed() {
        return Err(VerbError::Schema(
            "extend: source pixel buffer dimensions and byte count are inconsistent".into(),
        ));
    }
    // The verb encodes `source` as PNG before sending to the backend
    // and assumes 4 bytes per pixel throughout. Reject indexed or
    // grayscale inputs at the boundary so failures surface as a
    // typed Schema error rather than a panic deeper inside the
    // PNG encode path.
    if parsed.source.bytes_per_pixel != 4 {
        return Err(VerbError::Schema(format!(
            "extend: source must be RGBA8 (bytes_per_pixel == 4); got {}",
            parsed.source.bytes_per_pixel,
        )));
    }
    if !(0.0..=1.0).contains(&parsed.style_intensity) {
        return Err(VerbError::Schema(format!(
            "extend: style_intensity {} is outside [0.0, 1.0]",
            parsed.style_intensity
        )));
    }
    if let DirectionSet::Custom { ref directions } = parsed.direction_set {
        if directions.is_empty() {
            return Err(VerbError::Schema(
                "extend: custom direction set must contain at least one direction".into(),
            ));
        }
    }
    Ok(())
}

/// Builds an `AddLayer` effect for one synthesised direction.
///
/// Placeholder IDs are local to this effect; the host rewrites them at
/// commit. Each `AddLayer` has its own independent placeholder namespace,
/// so `LayerId::new(0)` and `PixelBufferId::new(0)` are safe to reuse
/// across effects.
fn build_add_layer_effect(
    pixels: PixelData,
    target_dir: Direction,
    active_frame: FrameIndex,
    layer_base: &str,
    sprite: pixhaus_core::project::SpriteId,
) -> VerbEffect {
    let placeholder_layer = LayerId::new(0);
    let placeholder_buffer = PixelBufferId::new(0);
    let layer_name = format!("{layer_base} \u{2013} {target_dir}");
    let layer = Layer::raster(placeholder_layer, &layer_name);
    let cel = Cel::raster(
        placeholder_layer,
        active_frame,
        placeholder_buffer,
        Size::new(pixels.width, pixels.height),
    );
    VerbEffect::AddLayer {
        sprite,
        layer,
        cels: vec![cel],
        pixel_buffers: vec![NewPixelBuffer {
            placeholder: placeholder_buffer,
            pixels,
        }],
    }
}

#[async_trait]
impl Verb for ExtendVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        validate_extend_inputs(inputs)
    }

    // Progress fractions are display-only; direction counts are ≤ 8.
    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = std::time::Instant::now();
        let inputs: ExtendInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;
        let active_frame = ctx.active_frame.unwrap_or(FrameIndex::new(0));
        let backend_arc = ctx
            .backend
            .as_ref()
            .ok_or(VerbError::MissingContext("view synthesis backend"))?;
        let view_backend = backend_arc.as_view_synthesis().ok_or_else(|| {
            VerbError::Backend(
                "selected backend does not implement view synthesis; \
                 configure a Replicate or Stability backend"
                    .into(),
            )
        })?;

        let target_dirs = inputs
            .direction_set
            .target_directions(inputs.source_direction);

        if target_dirs.is_empty() {
            return Err(VerbError::Schema(
                "extend: no directions to generate after excluding the source direction".into(),
            ));
        }

        let layer_base = inputs.layer_name.as_deref().unwrap_or("Extend").to_owned();
        let canvas_size = inputs.source.size();

        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend_arc.id().to_owned()),
            })
            .await;

        let total = target_dirs.len() as f32;
        let mut effects = Vec::with_capacity(target_dirs.len());
        let mut succeeded_dirs: Vec<Direction> = Vec::with_capacity(target_dirs.len());
        let mut notes = Vec::new();

        for (idx, &target_dir) in target_dirs.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(VerbError::Cancelled);
            }

            let n = target_dirs.len();
            // Use (idx + 1) so the bar actually reaches 1.0 on the last
            // iteration. The previous (idx / total) capped at (N-1)/N.
            progress
                .step(
                    Some((idx + 1) as f32 / total),
                    format!("generating {target_dir} view ({}/{})", idx + 1, n),
                )
                .await;

            let request = DirectionalViewRequest {
                source: inputs.source.clone(),
                source_direction: inputs.source_direction,
                target_direction: target_dir,
                style_intensity: inputs.style_intensity,
            };

            let pixels = match view_backend
                .generate_directional_view(request, cancel.clone())
                .await
            {
                Ok(p) => p,
                Err(VerbError::Cancelled) => return Err(VerbError::Cancelled),
                Err(e) => {
                    notes.push(format!(
                        "Warning: could not generate {target_dir} view: {e}"
                    ));
                    continue;
                }
            };

            effects.push(build_add_layer_effect(
                pixels,
                target_dir,
                active_frame,
                &layer_base,
                sprite_id,
            ));
            succeeded_dirs.push(target_dir);
        }

        if effects.is_empty() {
            return Err(VerbError::Backend(
                "extend: all direction synthesis calls failed; see verb notes".into(),
            ));
        }

        // Track *which* directions actually succeeded rather than slicing
        // target_dirs by effects.len() — a mid-iteration failure followed
        // by a later success would mis-label results otherwise. Reference
        // the canvas as `WxH` so the summary identifies the sprite size,
        // not just one axis.
        let summary = format!(
            "Add {} direction view{} ({}×{}px): {}",
            effects.len(),
            if effects.len() == 1 { "" } else { "s" },
            canvas_size.width,
            canvas_size.height,
            succeeded_dirs
                .iter()
                .map(|d| d.display_name())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let thumbnail = if let Some(VerbEffect::AddLayer { pixel_buffers, .. }) = effects.first() {
            pixel_buffers.first().map(|b| b.pixels.clone())
        } else {
            None
        };

        Ok(VerbOutput {
            summary,
            effects,
            thumbnail,
            actual_cost: ActualCost::free(started.elapsed().max(Duration::from_micros(1))),
            notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::backend::InferenceBackend;
    use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
    use crate::plugin::runtime::VerbRuntime;
    use crate::plugin::view_synthesis::ViewSynthesisBackend;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    // ── Fixture helpers ───────────────────────────────────────────────────

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "extend-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn ctx_with_sprite() -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_frame = Some(FrameIndex::new(0));
        ctx.backend = Some(std::sync::Arc::new(FakeViewSynthesisBackend {
            fill: [0x80, 0x40, 0xc0, 0xff],
        }));
        ctx
    }

    fn rgba8_4x4() -> PixelData {
        PixelData::rgba8(4, 4, vec![0xffu8; 4 * 4 * 4])
    }

    fn four_dir_inputs() -> VerbInputs {
        VerbInputs::from_struct(&ExtendInputs {
            source: rgba8_4x4(),
            source_direction: Direction::South,
            direction_set: DirectionSet::FourDirection,
            style_intensity: 0.8,
            layer_name: None,
        })
        .unwrap()
    }

    // ── Fake backend ─────────────────────────────────────────────────────

    /// Test-only backend that returns solid-colour frames for every
    /// direction synthesis call.
    #[derive(Debug)]
    struct FakeViewSynthesisBackend {
        fill: [u8; 4],
    }

    impl InferenceBackend for FakeViewSynthesisBackend {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn id(&self) -> &'static str {
            "fake.view_synthesis"
        }
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::VIEW_SYNTHESIS)
        }
        fn cost_estimate(&self, _: BackendCapabilities) -> CostEstimate {
            CostEstimate::free()
        }
        fn as_view_synthesis(&self) -> Option<&dyn ViewSynthesisBackend> {
            Some(self)
        }
    }

    #[async_trait]
    impl ViewSynthesisBackend for FakeViewSynthesisBackend {
        async fn generate_directional_view(
            &self,
            request: DirectionalViewRequest,
            _cancel: CancellationToken,
        ) -> Result<PixelData> {
            let w = request.source.width;
            let h = request.source.height;
            let bytes = self
                .fill
                .iter()
                .cycle()
                .take((w * h * 4) as usize)
                .copied()
                .collect();
            Ok(PixelData::rgba8(w, h, bytes))
        }
    }

    // ── Unit tests ────────────────────────────────────────────────────────

    #[test]
    fn descriptor_requires_image_gen_and_view_synthesis() {
        let verb = ExtendVerb::new();
        let caps = verb.descriptor().required_capabilities;
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::VIEW_SYNTHESIS));
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }

    #[test]
    fn validate_rejects_malformed_source() {
        let verb = ExtendVerb::new();
        let inputs = VerbInputs::from_struct(&ExtendInputs {
            source: PixelData {
                width: 4,
                height: 4,
                bytes_per_pixel: 4,
                stride: 16,
                bytes: vec![0; 8], // too few bytes
            },
            source_direction: Direction::South,
            direction_set: DirectionSet::FourDirection,
            style_intensity: 0.8,
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_non_rgba8_source() {
        // Indexed (1bpp) input must be rejected — the verb encodes
        // source as a PNG and the synthesis prompt assumes RGBA pixels.
        let verb = ExtendVerb::new();
        let inputs = VerbInputs::from_struct(&ExtendInputs {
            source: PixelData {
                width: 4,
                height: 4,
                bytes_per_pixel: 1,
                stride: 4,
                bytes: vec![0; 16],
            },
            source_direction: Direction::South,
            direction_set: DirectionSet::FourDirection,
            style_intensity: 0.8,
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_out_of_range_style_intensity() {
        let verb = ExtendVerb::new();
        let inputs = VerbInputs::from_struct(&ExtendInputs {
            source: rgba8_4x4(),
            source_direction: Direction::South,
            direction_set: DirectionSet::FourDirection,
            style_intensity: 1.5, // out of range
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_empty_custom_direction_set() {
        let verb = ExtendVerb::new();
        let inputs = VerbInputs::from_struct(&ExtendInputs {
            source: rgba8_4x4(),
            source_direction: Direction::South,
            direction_set: DirectionSet::Custom { directions: vec![] },
            style_intensity: 0.8,
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let verb = ExtendVerb::new();
        assert!(verb.validate(&four_dir_inputs()).is_ok());
    }

    #[test]
    fn direction_set_excludes_source() {
        let dirs = DirectionSet::FourDirection.target_directions(Direction::South);
        assert!(!dirs.contains(&Direction::South));
        assert_eq!(dirs.len(), 3); // W, N, E
    }

    #[test]
    fn direction_set_eight_excludes_source() {
        let dirs = DirectionSet::EightDirection.target_directions(Direction::North);
        assert!(!dirs.contains(&Direction::North));
        assert_eq!(dirs.len(), 7);
    }

    #[test]
    fn custom_direction_set_deduplicates() {
        let dirs = DirectionSet::Custom {
            directions: vec![Direction::West, Direction::West, Direction::East],
        }
        .target_directions(Direction::South);
        assert_eq!(dirs.len(), 2);
        assert!(dirs.contains(&Direction::West));
        assert!(dirs.contains(&Direction::East));
    }

    #[tokio::test]
    async fn four_direction_produces_three_add_layer_effects() {
        let verb = ExtendVerb::new();
        let output = verb
            .invoke(
                ctx_with_sprite(),
                four_dir_inputs(),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        // Source is South, 4-direction set: W, N, E are generated.
        assert_eq!(output.effects.len(), 3);
        for effect in &output.effects {
            match effect {
                VerbEffect::AddLayer {
                    layer,
                    cels,
                    pixel_buffers,
                    ..
                } => {
                    assert!(layer.name.starts_with("Extend \u{2013}"));
                    assert_eq!(cels.len(), 1);
                    assert_eq!(pixel_buffers.len(), 1);
                    assert_eq!(pixel_buffers[0].pixels.width, 4);
                    assert_eq!(pixel_buffers[0].pixels.height, 4);
                }
                other => panic!("expected AddLayer, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn eight_direction_produces_seven_effects() {
        let verb = ExtendVerb::new();
        let inputs = VerbInputs::from_struct(&ExtendInputs {
            source: rgba8_4x4(),
            source_direction: Direction::South,
            direction_set: DirectionSet::EightDirection,
            style_intensity: 0.5,
            layer_name: Some("Walk".into()),
        })
        .unwrap();

        let output = verb
            .invoke(
                ctx_with_sprite(),
                inputs,
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(output.effects.len(), 7);
        // All layer names use the custom base name.
        for effect in &output.effects {
            if let VerbEffect::AddLayer { layer, .. } = effect {
                assert!(
                    layer.name.starts_with("Walk \u{2013}"),
                    "layer name: {}",
                    layer.name
                );
            }
        }
    }

    #[tokio::test]
    async fn extend_requires_active_sprite() {
        let verb = ExtendVerb::new();
        let mut ctx = VerbContext::empty(metadata());
        ctx.backend = Some(std::sync::Arc::new(FakeViewSynthesisBackend {
            fill: [0; 4],
        }));

        let res = verb
            .invoke(
                ctx,
                four_dir_inputs(),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active sprite"))
        ));
    }

    #[tokio::test]
    async fn extend_fails_without_view_synthesis_backend() {
        /// Backend that advertises `VIEW_SYNTHESIS` but does NOT implement
        /// the sub-trait (`as_view_synthesis` returns None).
        #[derive(Debug)]
        struct NoViewSynthesisImpl;
        impl InferenceBackend for NoViewSynthesisImpl {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn id(&self) -> &'static str {
                "stub.no_view_synthesis"
            }
            fn capabilities(&self) -> BackendCapabilities {
                BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::VIEW_SYNTHESIS)
            }
            fn cost_estimate(&self, _: BackendCapabilities) -> CostEstimate {
                CostEstimate::free()
            }
            // as_view_synthesis returns None (default)
        }

        let verb = ExtendVerb::new();
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.backend = Some(std::sync::Arc::new(NoViewSynthesisImpl));

        let res = verb
            .invoke(
                ctx,
                four_dir_inputs(),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(res, Err(VerbError::Backend(_))));
    }

    #[tokio::test]
    async fn cancel_mid_generation_returns_cancelled() {
        let verb = ExtendVerb::new();
        let cancel = CancellationToken::new();
        cancel.cancel(); // pre-cancel before invoke

        let res = verb
            .invoke(
                ctx_with_sprite(),
                four_dir_inputs(),
                VerbProgress::discard(),
                cancel,
            )
            .await;
        assert!(matches!(res, Err(VerbError::Cancelled)));
    }

    #[tokio::test]
    async fn thumbnail_matches_first_generated_direction() {
        let verb = ExtendVerb::new();
        let output = verb
            .invoke(
                ctx_with_sprite(),
                four_dir_inputs(),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        // Thumbnail must be present and the correct size.
        let thumb = output.thumbnail.unwrap();
        assert_eq!(thumb.width, 4);
        assert_eq!(thumb.height, 4);
        assert!(thumb.is_well_formed());
    }

    #[tokio::test]
    async fn runs_through_verb_runtime() {
        let runtime = VerbRuntime::new();
        runtime.register(ExtendVerb::new()).unwrap();

        // Register the fake backend.
        runtime
            .register_backend(
                FakeViewSynthesisBackend {
                    fill: [0x10, 0x20, 0x30, 0xff],
                },
                10,
            )
            .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(EXTEND_VERB_ID),
                ctx_with_sprite(),
                four_dir_inputs(),
            )
            .unwrap();

        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.verb.as_str(), EXTEND_VERB_ID);
        assert_eq!(preview.output.effects.len(), 3);
    }
}
