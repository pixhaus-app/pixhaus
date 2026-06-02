//! The composable prompt types.
//!
//! These carry the structured knobs a prompt is built from - identity, palette,
//! composition, grid, timing - while the reusable prose lives in [`super::kb`]. They
//! are plain data (prompt content, never i18n keys); the shipped Bit values live in
//! [`super::defaults`]. When the prompt-library system lands it swaps these values,
//! not the builders.

/// One palette anchor: a human label plus its RGB. Content, never an i18n key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorAnchor {
    /// A short human label, e.g. "cyan screen glow".
    pub label: String,
    /// The straight RGB triple.
    pub rgb: [u8; 3],
}

/// Body topology, which drives the stance and composition vocabulary in a prompt.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum BodyPlan {
    /// Two legs, two arms - the default (Bit).
    #[default]
    Biped,
    /// Four legs.
    Quadruped,
    /// No feet; hovers in the frame.
    Floating,
}

/// A character's visual identity, kept free of medium adjectives so it restyles
/// cleanly across art modes.
#[derive(Clone, Debug)]
pub struct CharacterIdentity {
    /// The short name used in prose, e.g. "Bit".
    pub name: String,
    /// The one-paragraph identity description (the subject the user can edit).
    pub description: String,
    /// The neutral resting-pose phrase.
    pub neutral_pose: String,
    /// The palette anchors, emitted as an explicit colour list.
    pub palette: Vec<ColorAnchor>,
    /// The body topology.
    pub body_plan: BodyPlan,
}

/// Composition and chroma-key rules for the single anchor reference image.
#[derive(Clone, Debug)]
pub struct AnchorSpec {
    /// The view label, e.g. "2D side-view".
    pub view_label: String,
    /// The facing direction, e.g. "RIGHT".
    pub facing: String,
    /// The inclusive fraction-of-frame range the character fills, e.g. `(70, 85)`.
    pub height_fraction: (u8, u8),
    /// The flat chroma-key colour as a hex string, e.g. "#FF00FF".
    pub magenta_hex: String,
}

/// The idle-animation sheet spec: grid layout, gutter, and timing.
#[derive(Clone, Debug)]
pub struct IdleSpec {
    /// Sheet columns.
    pub cols: u32,
    /// Sheet rows.
    pub rows: u32,
    /// The flat-key margin to leave around each character, as a percent of the cell.
    pub gutter_percent: u8,
    /// Playback rate of the resulting clip, in frames per second.
    pub fps: u16,
    /// The clip name (e.g. "idle") - content, not an i18n key.
    pub clip_name: String,
}

impl IdleSpec {
    /// The number of frames the sheet holds (`cols * rows`).
    pub fn frame_count(&self) -> u32 {
        self.cols * self.rows
    }
}

/// A curated style line emitted into the prompt.
#[derive(Clone, Debug)]
pub struct StylePreset {
    /// The full style sentence.
    pub line: String,
    /// A short note on the motion feel.
    pub frame_rate_feel: String,
}

/// The animation-principle prose that shapes idle motion (distilled from the KB).
#[derive(Clone, Debug)]
pub struct AnimationPrinciples {
    /// The breathing-loop description.
    pub breathing: String,
    /// The secondary-motion / overlap description.
    pub secondary: String,
    /// The "what must not happen" line that guards drift and duplicates.
    pub negatives: String,
}
