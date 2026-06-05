//! Type-specific entry bodies: [`EntryDetails`] (bible 4).
//!
//! Every [`CodexEntry`](crate::codex::CodexEntry) shares a header; the body varies by
//! type. This pass gives rich bodies to Character, Palette, Style, and Animation —
//! the types Generate leans on hardest — and a [`Generic`](EntryDetails::Generic) body
//! to the rest. These are stable data fields, never display strings.

use serde::{Deserialize, Serialize};

use crate::codex::handle::CodexHandle;

/// The role a color plays in a palette (bible 4.7).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ColorRole {
    /// Darkest tones.
    Shadow,
    /// Base tones.
    Midtone,
    /// Lightest tones.
    Highlight,
    /// Outline / edge color.
    Outline,
    /// Skin tones.
    Skin,
    /// Cloth and fabric.
    Cloth,
    /// Metal surfaces.
    Metal,
    /// Magic glow / emissive.
    MagicGlow,
    /// Danger signalling.
    Danger,
    /// Healing signalling.
    Healing,
    /// UI accent.
    UiAccent,
}

/// One color in a palette: an RGBA quad with its role and lock state (bible 4.7).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PaletteColor {
    /// Straight RGBA, one byte per channel.
    pub rgba: [u8; 4],
    /// The role this color plays.
    pub role: ColorRole,
    /// Whether this color is locked against change (bible 11.6 palette lock, per-color).
    pub locked: bool,
    /// Whether generation may drop this color (an optional, not a required, color).
    pub optional: bool,
}

impl PaletteColor {
    /// A required, unlocked color of `role`.
    pub fn new(rgba: [u8; 4], role: ColorRole) -> Self {
        Self {
            rgba,
            role,
            locked: false,
            optional: false,
        }
    }
}

/// A named shading ramp: an ordered run of indices into a palette's color list.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PaletteRamp {
    /// The ramp's name (project content).
    pub name: String,
    /// Indices into the owning palette's `colors`, dark-to-light.
    pub color_indices: Vec<usize>,
}

/// How precise the line treatment of a style is (bible 4.8).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum LineTreatment {
    /// No explicit outline.
    None,
    /// A clean, single-weight outline.
    #[default]
    Clean,
    /// A heavy, bold outline.
    Bold,
    /// Selective outlines (e.g. outer only).
    Selective,
}

/// How much detail a style carries (bible 4.8).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum DetailLevel {
    /// Flat, minimal detail.
    Minimal,
    /// Low detail.
    Low,
    /// A balanced amount of detail.
    #[default]
    Medium,
    /// High detail.
    High,
}

/// Whether a style permits anti-aliasing (bible 4.8).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum AntiAliasingRule {
    /// No anti-aliasing (hard pixel edges).
    #[default]
    None,
    /// Manual, hand-placed anti-aliasing only.
    Manual,
    /// Anti-aliasing is allowed freely.
    Allowed,
}

/// How an animation loops (bible 4.13).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum LoopBehavior {
    /// Loops continuously.
    #[default]
    Loop,
    /// Plays once and holds the last frame.
    Once,
    /// Plays forward then backward.
    PingPong,
}

/// Rich body for a Character entry (bible 4.1).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CharacterDetails {
    /// Proportion notes (e.g. head-to-body ratio).
    pub proportions: String,
    /// Silhouette notes that keep the shape recognizable.
    pub silhouette_notes: String,
    /// Handle of the palette this character uses, if any.
    pub palette_ref: Option<CodexHandle>,
    /// Style handles this character may be rendered in.
    pub allowed_styles: Vec<CodexHandle>,
    /// Traits or styles generation must avoid for this character.
    pub forbidden_styles: Vec<CodexHandle>,
    /// Animation handles this character is expected to have (its animation set).
    pub animation_set: Vec<CodexHandle>,
}

/// Rich body for a Palette entry (bible 4.7).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct PaletteDetails {
    /// The palette's colors, with roles and lock state.
    pub colors: Vec<PaletteColor>,
    /// Named shading ramps over the colors.
    pub ramps: Vec<PaletteRamp>,
    /// Whether generation may add colors outside this palette.
    pub allow_generated_colors: bool,
}

/// Rich body for a Style entry (bible 4.8).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct StyleDetails {
    /// Free-text rendering rules.
    pub rendering_rules: String,
    /// How outlines are treated.
    pub line_treatment: LineTreatment,
    /// How much detail the style carries.
    pub detail_level: DetailLevel,
    /// Whether anti-aliasing is permitted.
    pub anti_aliasing: AntiAliasingRule,
    /// Target resolution in pixels, `(width, height)`, if the style fixes one.
    pub resolution: Option<(u32, u32)>,
    /// Negative rules: what this style forbids.
    pub negative_rules: Vec<String>,
}

/// One beat in an animation: a named pose the motion passes through (bible 4.13/4.14).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PoseBeat {
    /// The beat's label (project content), e.g. "windup", "apex".
    pub label: String,
    /// What the body is doing at this beat.
    pub description: String,
}

/// Rich body for an Animation entry (bible 4.13).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AnimationDetails {
    /// What the animation is for (its gameplay purpose).
    pub purpose: String,
    /// How it loops.
    pub loop_behavior: LoopBehavior,
    /// Recommended frame count.
    pub recommended_frame_count: u32,
    /// Recommended playback rate in frames per second.
    pub fps: u16,
    /// The pose beats the motion passes through, in order.
    pub pose_beats: Vec<PoseBeat>,
    /// Handles of characters this animation is compatible with.
    pub character_compatibility: Vec<CodexHandle>,
}

/// One field of a generic entry: a labelled value the prompt compiler may use.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GenericField {
    /// The field's stable key (e.g. `species`, `rarity`); not a display label.
    pub key: String,
    /// The field's value (project content).
    pub value: String,
}

/// A generic, key/value body for entry types without rich fields this pass.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct GenericDetails {
    /// The entry's fields, in author order.
    pub fields: Vec<GenericField>,
}

/// The type-specific body of a [`CodexEntry`](crate::codex::CodexEntry).
///
/// The header carries everything shared; this enum carries what differs by type. The
/// variant must agree with the entry's [`EntryType`](crate::codex::EntryType); the
/// model does not enforce that pairing this pass — the commands that build entries do.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EntryDetails {
    /// A character body.
    Character(CharacterDetails),
    /// A palette body.
    Palette(PaletteDetails),
    /// A style body.
    Style(StyleDetails),
    /// An animation body.
    Animation(AnimationDetails),
    /// A generic key/value body for every other type.
    Generic(GenericDetails),
}

impl EntryDetails {
    /// The default body for `entry_type`: a rich empty body for the four rich types,
    /// a generic body otherwise.
    pub fn default_for(entry_type: crate::codex::EntryType) -> Self {
        use crate::codex::EntryType;
        match entry_type {
            EntryType::Character => EntryDetails::Character(CharacterDetails::default()),
            EntryType::Palette => EntryDetails::Palette(PaletteDetails::default()),
            EntryType::Style => EntryDetails::Style(StyleDetails::default()),
            EntryType::Animation => EntryDetails::Animation(AnimationDetails::default()),
            _ => EntryDetails::Generic(GenericDetails::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::EntryType;

    #[test]
    fn default_for_picks_rich_body_for_rich_types() {
        assert!(matches!(EntryDetails::default_for(EntryType::Character), EntryDetails::Character(_)));
        assert!(matches!(EntryDetails::default_for(EntryType::Palette), EntryDetails::Palette(_)));
        assert!(matches!(EntryDetails::default_for(EntryType::Style), EntryDetails::Style(_)));
        assert!(matches!(EntryDetails::default_for(EntryType::Animation), EntryDetails::Animation(_)));
    }

    #[test]
    fn default_for_picks_generic_body_otherwise() {
        for t in [EntryType::Prop, EntryType::Rule, EntryType::Biome, EntryType::Vfx] {
            assert!(matches!(EntryDetails::default_for(t), EntryDetails::Generic(_)));
        }
    }

    #[test]
    fn palette_color_defaults_required_unlocked() {
        let c = PaletteColor::new([10, 20, 30, 255], ColorRole::Shadow);
        assert!(!c.locked);
        assert!(!c.optional);
        assert_eq!(c.role, ColorRole::Shadow);
    }
}
