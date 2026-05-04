//! Verb descriptor: the static contract every verb publishes.
//!
//! A descriptor is everything the runtime, UI, and plugin loader need
//! to know about a verb *without invoking it*: identity, schema, cost,
//! and the backend capabilities the verb requires. Descriptors are
//! cheap to clone and serialise; the IPC layer (B4) will surface them
//! to the UI for menu rendering and form generation.

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Stable identifier for a verb.
///
/// IDs are reverse-DNS-style strings; built-in verbs use the prefix
/// `pixhaus.builtin.`, and third-party plugins use the publisher's
/// prefix (`com.example.myplugin.foo`). The format is enforced by
/// convention rather than a parser — keeping the ID a transparent
/// `String` lets the IPC layer round-trip it without bespoke serde.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerbId(String);

impl VerbId {
    /// Constructs a verb ID from a string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the wrapped ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerbId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for VerbId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for VerbId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Bitfield of inference capabilities a verb may require.
///
/// Stored as a single `u32` so descriptors round-trip as compact JSON
/// numbers and so verbs can declare "needs vision-language *and*
/// image-edit" in one expression. The runtime checks the configured
/// backend(s) against the verb's required set before invocation; a
/// missing capability surfaces as
/// [`super::error::VerbError::UnsupportedCapability`].
///
/// Adding a new capability is additive (existing values keep their bit
/// position). Repurposing or removing a bit is a breaking change for
/// every plugin that declared it.
#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BackendCapabilities(pub u32);

impl BackendCapabilities {
    /// Generates plain text. The most common LLM capability.
    pub const TEXT_GENERATION: Self = Self(1 << 0);

    /// Reads images and produces text (e.g. Claude vision, GPT-5
    /// vision). Used by Critique and Conversational editing.
    pub const VISION_LANGUAGE: Self = Self(1 << 1);

    /// Generates images from text. Used by tileset-from-description and
    /// the Continue/Inbetween verbs.
    pub const IMAGE_GENERATION: Self = Self(1 << 2);

    /// Edits an existing image given a mask or instruction. Used by
    /// Variant and Sketch finishing.
    pub const IMAGE_EDIT: Self = Self(1 << 3);

    /// Inpaints regions of an image given a mask. Strict subset of
    /// `IMAGE_EDIT` for backends that expose it separately.
    pub const IMAGE_INPAINT: Self = Self(1 << 4);

    /// Generates video frames or interpolates between key frames. Used
    /// by Inbetween and Continue.
    pub const FRAME_INTERPOLATION: Self = Self(1 << 5);

    /// Estimates 2D or 3D pose from an image. Used by Motion-from-video.
    pub const POSE_ESTIMATION: Self = Self(1 << 6);

    /// Segments an image into regions. Used by Auto-mesh-deformation.
    pub const SEGMENTATION: Self = Self(1 << 7);

    /// Analyses audio (beat detection, lip-sync features). Used by
    /// Audio-driven timing.
    pub const AUDIO_ANALYSIS: Self = Self(1 << 8);

    /// Trains a small style model (LoRA-class). Used by Project style
    /// learning.
    pub const STYLE_TRAINING: Self = Self(1 << 9);

    /// Supports tool-use / function-calling protocols. Used by
    /// Conversational editing.
    pub const TOOL_USE: Self = Self(1 << 10);

    /// Produces vector embeddings for similarity search.
    pub const EMBEDDINGS: Self = Self(1 << 11);

    /// Synthesises novel views of a single subject. Used by Extend.
    pub const VIEW_SYNTHESIS: Self = Self(1 << 12);

    /// The empty set — the verb runs entirely on local CPU and needs
    /// no inference backend. Used by classical-only verbs (Cleanup's
    /// fast path) and the [`super::echo::EchoVerb`] reference plugin.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns the union of two sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the intersection of two sets.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns `true` if every bit in `other` is also set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` when no capability is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Coarse latency / cost expectations a verb advertises in advance.
///
/// The numbers are best-effort estimates from the verb author. The
/// runtime returns the actual cost via
/// [`super::output::ActualCost`] after invocation. The UI shows the
/// estimate before the user clicks "Run" so cloud-backed verbs don't
/// surprise anyone.
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Typical wall-clock duration. `Duration::ZERO` if the verb is
    /// effectively instantaneous on the local CPU.
    pub typical_latency: Duration,
    /// Worst-case wall-clock duration the user might see.
    pub max_latency: Duration,
    /// Typical cost in USD cents. `0.0` for free / local verbs.
    pub typical_usd_cents: f32,
    /// Worst-case cost in USD cents.
    pub max_usd_cents: f32,
}

impl CostEstimate {
    /// Constant-size, free-of-charge estimate for verbs that run on
    /// the local CPU and finish in microseconds.
    #[must_use]
    pub const fn free() -> Self {
        Self {
            typical_latency: Duration::ZERO,
            max_latency: Duration::ZERO,
            typical_usd_cents: 0.0,
            max_usd_cents: 0.0,
        }
    }
}

/// Coarse classification of effects a verb produces.
///
/// Listed in [`VerbDescriptor::output_kinds`] so the UI can route a
/// verb to the correct preview pane (e.g. a Critique verb is shown in
/// the findings panel rather than the canvas) without inspecting the
/// effect payload. Strings inside [`Self::Custom`] are namespaced the
/// same way as [`VerbId`]s.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "name")]
pub enum EffectKind {
    /// Adds a new layer (and optionally cels) to the active sprite.
    AddLayer,
    /// Adds one or more cels to existing layers.
    AddCels,
    /// Replaces existing cel pixel data.
    ReplaceCels,
    /// Appends new frames to the timeline.
    AddFrames,
    /// Adds a frame tag (named range).
    AddTag,
    /// Adds a slice (named rectangular region).
    AddSlice,
    /// Adds a palette to the active sprite.
    AddPalette,
    /// Adds a tileset, possibly with new tiles.
    AddTileset,
    /// Read-only finding (no edit). Used by Critique.
    Critique,
    /// Verb-specific effect kind. The string is namespaced like a
    /// [`VerbId`]; e.g. `"pixhaus.builtin.style_learning.model"`.
    Custom(String),
}

/// Static metadata every verb publishes.
///
/// The descriptor is the contract between a verb and the rest of
/// Pixhaus. Once registered, its identity (`id`) cannot change without
/// breaking calls; everything else is free to evolve as the
/// registration protocol bumps a minor version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerbDescriptor {
    /// Stable identifier — see [`VerbId`].
    pub id: VerbId,
    /// Human-readable name shown in the command palette.
    pub display_name: String,
    /// One-sentence description rendered in tooltips and the verb
    /// inspector.
    pub description: String,
    /// Semantic version of this verb's implementation. Independent of
    /// the protocol's own version.
    pub version: String,
    /// Backend capabilities the verb requires. Empty means the verb is
    /// self-contained.
    pub required_capabilities: BackendCapabilities,
    /// JSON Schema describing the accepted input shape. The runtime
    /// does not validate against it (verbs implement [`super::verb::Verb::validate`]
    /// for that); the schema exists so the UI can build forms.
    pub input_schema: serde_json::Value,
    /// JSON Schema describing the verb's `VerbOutput.effects` shape.
    /// Optional — verbs whose effects are trivially the standard
    /// [`super::output::VerbEffect`] enum can leave this `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    /// Effect kinds the verb produces. Used by the UI to route preview
    /// rendering before the verb runs.
    pub output_kinds: Vec<EffectKind>,
    /// Cost / latency expectations for the UI to surface.
    pub cost_estimate: CostEstimate,
    /// `true` if the verb may emit progress events while running.
    pub streaming: bool,
    /// `true` if the verb honours its cancellation token.
    pub cancellable: bool,
    /// Optional URL to user-facing documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_id_round_trips_as_string() {
        let id = VerbId::new("pixhaus.builtin.echo");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"pixhaus.builtin.echo\"");
        let back: VerbId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn verb_id_display_matches_string() {
        let id = VerbId::new("foo.bar");
        assert_eq!(format!("{id}"), "foo.bar");
        assert_eq!(id.as_str(), "foo.bar");
    }

    #[test]
    fn capabilities_union_and_contain() {
        let caps =
            BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::FRAME_INTERPOLATION);
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::FRAME_INTERPOLATION));
        assert!(!caps.contains(BackendCapabilities::VISION_LANGUAGE));
        assert!(!caps.is_empty());
    }

    #[test]
    fn empty_capabilities_serializes_as_zero() {
        let caps = BackendCapabilities::empty();
        let json = serde_json::to_string(&caps).unwrap();
        assert_eq!(json, "0");
        assert!(caps.is_empty());
    }

    #[test]
    fn cost_free_is_all_zero() {
        let c = CostEstimate::free();
        assert_eq!(c.typical_usd_cents, 0.0);
        assert_eq!(c.typical_latency, Duration::ZERO);
    }

    #[test]
    fn descriptor_round_trips_as_json() {
        let d = VerbDescriptor {
            id: VerbId::new("pixhaus.builtin.echo"),
            display_name: "Echo".into(),
            description: "Echo input as a layer".into(),
            version: "0.1.0".into(),
            required_capabilities: BackendCapabilities::empty(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            output_kinds: vec![EffectKind::AddLayer],
            cost_estimate: CostEstimate::free(),
            streaming: false,
            cancellable: true,
            documentation_url: None,
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: VerbDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn effect_kind_custom_serializes_with_name() {
        let k = EffectKind::Custom("pixhaus.style.lora".into());
        let json = serde_json::to_string(&k).unwrap();
        assert!(json.contains("custom"));
        assert!(json.contains("pixhaus.style.lora"));
    }
}
