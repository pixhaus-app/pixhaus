//! Verb output and effects.
//!
//! A verb returns a [`VerbOutput`] describing what *would* happen on
//! commit. The host displays it as a preview; the user accepts or
//! rejects; on accept the host walks [`VerbOutput::effects`] and
//! applies them through the undo stack (S05). Verbs do not mutate the
//! project directly — they describe edits.
//!
//! # Placeholder IDs
//!
//! Verbs cannot mint real `LayerId`, `FrameIndex`, or `PixelBufferId`
//! values: those come from the live editor state, which the verb does
//! not own. Effects use *placeholder* IDs that the host rewrites at
//! commit:
//!
//! - **Layers.** [`VerbEffect::AddLayer`] carries one new layer; the
//!   host assigns it a real `LayerId` and rewrites every cel in the
//!   effect that references the placeholder.
//! - **Pixel buffers.** Effects that create new pixel buffers carry a
//!   parallel [`NewPixelBuffer`] vector. Cels in the same effect use
//!   each buffer's `placeholder` field as their `PixelBufferId`; the
//!   host registers the bytes and rewrites the cel.
//! - **Frames.** [`VerbEffect::AddFrames`] places frames at indices
//!   relative to `after`; the host renumbers absolute indices.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use pixhaus_core::project::{
    Cel, Frame, FrameIndex, FrameTag, Layer, LayerId, Palette, PixelBufferId, Rect, Slice,
    SpriteId, Tileset,
};

use super::context::PixelData;

/// A pixel buffer the verb wants the host to register on commit.
///
/// `placeholder` is unique within the enclosing effect's buffer list.
/// Cels in the same effect reference the buffer by storing
/// `PixelBufferId(placeholder)` in their `CelData::Raster.buffer`
/// field; the host rewrites the reference to the real ID.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NewPixelBuffer {
    /// Effect-local handle the cels use to refer to this buffer.
    pub placeholder: PixelBufferId,
    /// The actual bytes.
    pub pixels: PixelData,
}

/// Severity of a critique finding.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CritiqueSeverity {
    /// Informational note — no action required.
    #[default]
    Info,
    /// Warning the artist should consider.
    Warning,
    /// Issue that almost certainly needs fixing.
    Error,
}

/// Category of a critique finding.
///
/// Matches the categories in the Critique verb brief (S29). New
/// categories are additive; verbs that produce findings the runtime
/// does not enumerate use [`Self::Other`].
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "label")]
pub enum CritiqueCategory {
    /// Pose continuity break between adjacent frames.
    PoseContinuity,
    /// Pixel uses a colour outside the active palette.
    PaletteViolation,
    /// Animation has a missing or implausibly-timed frame.
    MissingFrame,
    /// Pivot point drifts across frames in an animation.
    PivotDrift,
    /// Style differs from project references.
    StyleInconsistency,
    /// Verb-specific category.
    Other(String),
}

/// One finding produced by a critique verb.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CritiqueFinding {
    /// What kind of issue this is.
    pub category: CritiqueCategory,
    /// How serious.
    pub severity: CritiqueSeverity,
    /// One-sentence summary surfaced in the findings panel.
    pub summary: String,
    /// Optional frame to jump to when the user clicks the finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameIndex>,
    /// Optional layer to highlight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<LayerId>,
    /// Optional canvas region to highlight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Rect>,
}

/// One unit of work the host applies on commit.
///
/// Effects are intentionally coarse: enough granularity to round-trip
/// every built-in verb's output (S23–S36) without being so fine that
/// the host has to handle a per-pixel-edit case. Verbs whose output
/// does not fit a built-in variant use [`Self::Custom`] and namespace
/// the `kind` string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerbEffect {
    /// Add a new layer to the sprite, optionally with cels and pixel
    /// buffers. The layer's `id` is a placeholder; cels in `cels` use
    /// the same placeholder for `layer_id`.
    AddLayer {
        /// Sprite to add the layer to.
        sprite: SpriteId,
        /// New layer with a placeholder `id`.
        layer: Layer,
        /// Cels associated with the new layer. Each cel's `layer_id`
        /// equals `layer.id`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cels: Vec<Cel>,
        /// New pixel buffers referenced by `cels`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pixel_buffers: Vec<NewPixelBuffer>,
    },
    /// Add cels to existing layers. Cels reference *real* layer IDs
    /// (sourced from [`super::context::VerbContext`]).
    AddCels {
        /// Sprite to attach the cels to.
        sprite: SpriteId,
        /// New cels.
        cels: Vec<Cel>,
        /// New pixel buffers referenced by `cels`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pixel_buffers: Vec<NewPixelBuffer>,
    },
    /// Replace existing cels (matched by `(layer_id, frame_index)`).
    ReplaceCels {
        /// Sprite that owns the cels.
        sprite: SpriteId,
        /// Replacement cels.
        cels: Vec<Cel>,
        /// New pixel buffers referenced by `cels`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pixel_buffers: Vec<NewPixelBuffer>,
    },
    /// Append new frames to the timeline.
    AddFrames {
        /// Sprite that owns the timeline.
        sprite: SpriteId,
        /// Insert position. `None` prepends; `Some(i)` inserts after
        /// index `i`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<FrameIndex>,
        /// Frames in display order.
        frames: Vec<Frame>,
        /// Cels associated with the new frames. Their `frame_index`
        /// is relative to the start of `frames` (i.e. the first new
        /// frame is index `0`); the host renumbers on commit.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cels: Vec<Cel>,
        /// New pixel buffers referenced by `cels`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pixel_buffers: Vec<NewPixelBuffer>,
    },
    /// Add a frame tag.
    AddTag {
        /// Sprite that owns the timeline.
        sprite: SpriteId,
        /// New tag.
        tag: FrameTag,
    },
    /// Add a named slice.
    AddSlice {
        /// Sprite that owns the slices.
        sprite: SpriteId,
        /// New slice.
        slice: Slice,
    },
    /// Add a palette.
    AddPalette {
        /// Sprite that owns the palettes.
        sprite: SpriteId,
        /// New palette.
        palette: Palette,
    },
    /// Add a tileset, optionally with associated pixel data.
    AddTileset {
        /// Sprite that owns the tilesets.
        sprite: SpriteId,
        /// New tileset (its `id` is a placeholder).
        tileset: Tileset,
        /// New pixel buffers referenced by the tileset (e.g. the
        /// inline tile atlas).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pixel_buffers: Vec<NewPixelBuffer>,
    },
    /// Read-only finding. Used by the Critique verb.
    Critique {
        /// Findings, ordered as the verb wants them surfaced.
        findings: Vec<CritiqueFinding>,
    },
    /// Verb-specific effect. The `name` namespace mirrors
    /// [`super::descriptor::VerbId`]; the `payload` shape is owned by
    /// the verb that produced it.
    Custom {
        /// Verb-namespaced effect name (e.g. `"pixhaus.style.lora"`).
        name: String,
        /// Opaque payload.
        payload: serde_json::Value,
    },
}

/// Cost actually incurred by an invocation.
///
/// Returned alongside [`VerbOutput`]. The runtime surfaces this to the
/// UI so the user sees the real spend after the call settles, even
/// when the descriptor's [`super::descriptor::CostEstimate`] was off.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ActualCost {
    /// Wall-clock elapsed time.
    pub elapsed: Duration,
    /// USD cents charged. `0.0` for local / free verbs.
    pub usd_cents: f32,
    /// Backend identifier (e.g. `"anthropic.claude-sonnet-4-6"`).
    /// `None` for verbs that ran without a backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Input tokens consumed, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_input: Option<u32>,
    /// Output tokens generated, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_output: Option<u32>,
}

impl ActualCost {
    /// Zero-cost result for verbs that ran on the local CPU.
    #[must_use]
    pub const fn free(elapsed: Duration) -> Self {
        Self {
            elapsed,
            usd_cents: 0.0,
            backend: None,
            tokens_input: None,
            tokens_output: None,
        }
    }
}

/// What a verb produced, ready to become a preview.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerbOutput {
    /// One-line user-facing summary shown in the "Apply preview?"
    /// dialog (e.g. *"Add 4 in-between frames between idle.0 and
    /// idle.1"*).
    pub summary: String,
    /// Effects that will be applied on commit, in order.
    pub effects: Vec<VerbEffect>,
    /// Optional rendered thumbnail for the preview dialog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PixelData>,
    /// Actual cost reported by the verb.
    pub actual_cost: ActualCost,
    /// Verb-side notes (warnings, downgrades) the UI should surface
    /// alongside the preview.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{LayerId, SpriteId};

    #[test]
    fn add_layer_effect_round_trips() {
        let layer = Layer::raster(LayerId::new(0), "echo");
        let effect = VerbEffect::AddLayer {
            sprite: SpriteId::new(1),
            layer,
            cels: Vec::new(),
            pixel_buffers: Vec::new(),
        };
        let json = serde_json::to_string(&effect).unwrap();
        let back: VerbEffect = serde_json::from_str(&json).unwrap();
        assert_eq!(back, effect);
    }

    #[test]
    fn custom_effect_carries_namespace() {
        let effect = VerbEffect::Custom {
            name: "pixhaus.style.lora".into(),
            payload: serde_json::json!({"epochs": 30}),
        };
        let json = serde_json::to_string(&effect).unwrap();
        assert!(json.contains("custom"));
        assert!(json.contains("pixhaus.style.lora"));
    }

    #[test]
    fn critique_finding_serializes_compactly() {
        let f = CritiqueFinding {
            category: CritiqueCategory::PaletteViolation,
            severity: CritiqueSeverity::Warning,
            summary: "Pixel at (4, 4) outside palette".into(),
            frame: Some(FrameIndex::new(2)),
            layer: Some(LayerId::new(1)),
            region: None,
        };
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("warning"));
        assert!(!json.contains("region"));
    }

    #[test]
    fn actual_cost_free_drops_optionals() {
        let c = ActualCost::free(Duration::from_millis(5));
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("tokens_input"));
        assert!(!json.contains("backend"));
    }

    #[test]
    fn output_round_trips() {
        let out = VerbOutput {
            summary: "Echo the input".into(),
            effects: vec![],
            thumbnail: None,
            actual_cost: ActualCost::free(Duration::from_micros(10)),
            notes: vec![],
        };
        let bytes = rmp_serde::to_vec_named(&out).unwrap();
        let back: VerbOutput = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back, out);
    }
}
