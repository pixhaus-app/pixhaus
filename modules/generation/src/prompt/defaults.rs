//! The shipped Bit defaults.
//!
//! Bit (the Pixhaus mascot) and an idle loop are the working default until the
//! prompt-library system lands. These functions build the default instances the
//! builders consume; the prompt-library will swap these values, not the builders.

use super::MAGENTA_KEY_HEX;
use super::kb;
use super::types::{AnchorSpec, AnimationPrinciples, BodyPlan, CharacterIdentity, ColorAnchor, IdleSpec, StylePreset};

/// The default subject text the prompt box is seeded with (the canonical Bit
/// description). The user edits this; the edited text becomes the identity
/// description. Content, never an i18n key.
pub const BIT_DEFAULT_PROMPT: &str = "Bit, the Pixhaus mascot - a small retro robot with a boxy CRT/floppy-disk head, a glowing pixel-face screen showing its expression, a stubby antenna with a blinking pixel, chunky rounded limbs, friendly proportions.";

/// Bit's identity: the canonical description, neutral pose, and palette.
pub fn bit_identity() -> CharacterIdentity {
    CharacterIdentity {
        name: "Bit".to_owned(),
        description: BIT_DEFAULT_PROMPT.to_owned(),
        neutral_pose: "Standing idle, weight settled, a calm neutral pose.".to_owned(),
        palette: vec![
            ColorAnchor {
                label: "dark neutral".to_owned(),
                rgb: [24, 24, 32],
            },
            ColorAnchor {
                label: "cyan screen glow".to_owned(),
                rgb: [64, 200, 220],
            },
            ColorAnchor {
                label: "light highlight".to_owned(),
                rgb: [240, 240, 245],
            },
            ColorAnchor {
                label: "warm accent".to_owned(),
                rgb: [220, 90, 70],
            },
            ColorAnchor {
                label: "green accent".to_owned(),
                rgb: [120, 200, 90],
            },
        ],
        body_plan: BodyPlan::Biped,
    }
}

/// The default anchor composition: side-view, centred, on a flat magenta key.
pub fn default_anchor_spec() -> AnchorSpec {
    AnchorSpec {
        view_label: "2D side-view".to_owned(),
        facing: "RIGHT".to_owned(),
        height_fraction: (70, 85),
        magenta_hex: MAGENTA_KEY_HEX.to_owned(),
    }
}

/// The default idle sheet: a 4x2 grid (eight frames) at 12fps.
pub fn default_idle_spec() -> IdleSpec {
    IdleSpec {
        cols: 4,
        rows: 2,
        gutter_percent: 8,
        fps: 12,
        clip_name: "idle".to_owned(),
    }
}

/// The default clean game-sprite style.
pub fn default_style() -> StylePreset {
    StylePreset {
        line: kb::GAME_SPRITE_STYLE.to_owned(),
        frame_rate_feel: kb::GAME_SPRITE_FEEL.to_owned(),
    }
}

/// The default idle animation principles (breathing, overlap, negatives).
pub fn idle_principles() -> AnimationPrinciples {
    AnimationPrinciples {
        breathing: kb::BREATHING_OSCILLATION.to_owned(),
        secondary: kb::SECONDARY_OVERLAP.to_owned(),
        negatives: kb::IDLE_NEGATIVES.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_default_keys_on_the_canonical_magenta() {
        // The anchor builder reads this spec field while the idle builder reads the
        // const directly; lock the default to the const so both passes key on the
        // same colour and the chroma-key removal stays consistent across them.
        assert_eq!(default_anchor_spec().magenta_hex, MAGENTA_KEY_HEX);
        assert_eq!(default_anchor_spec().magenta_hex, "#FF00FF");
    }

    #[test]
    fn idle_default_is_the_shipped_eight_frame_grid() {
        let idle = default_idle_spec();
        assert_eq!(idle.cols, 4);
        assert_eq!(idle.rows, 2);
        assert_eq!(idle.frame_count(), 8, "the shipped default is a 4x2 eight-frame sheet");
    }

    #[test]
    fn bit_identity_carries_a_named_subject_and_a_palette() {
        let bit = bit_identity();
        assert_eq!(bit.name, "Bit");
        assert!(!bit.palette.is_empty(), "Bit ships with an explicit palette the prompt honours");
    }

    #[test]
    fn style_and_principle_prose_is_present() {
        // The builders splice these straight into the prompt; an empty line would
        // emit a blank section, so the shipped defaults must carry prose.
        assert!(!default_style().line.is_empty(), "the style line is emitted into the prompt");
        assert!(!idle_principles().breathing.is_empty(), "the breathing principle drives the idle loop");
    }
}
