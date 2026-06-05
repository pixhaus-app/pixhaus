//! The prompt builder: assemble anchor and idle-animation prompts from data.
//!
//! Two pure functions turn a [`CharacterIdentity`] and the specs into the final
//! prompt strings a provider sends. The text fuses the proven anchor-then-sheet
//! scaffold, the Bit identity, and the animation knowledge base (via [`kb`]). The
//! output is content - it flows to a provider and into saved provenance - never an
//! i18n key. Bit and an idle loop are the shipped defaults ([`defaults`]); the
//! future prompt-library swaps the data, not these builders.

pub mod defaults;
pub mod kb;
pub mod types;

pub use types::{AnchorSpec, AnimationPrinciples, BodyPlan, CharacterIdentity, ColorAnchor, IdleSpec, StylePreset};

/// The flat chroma-key colour the whole pipeline keys on. The prompt asks for it and
/// `pixhaus_mod_providers::chroma_key_magenta` removes it.
pub const MAGENTA_KEY_HEX: &str = "#FF00FF";

/// Builds the anchor prompt: one neutral character-reference image on a flat key.
pub fn build_anchor_prompt(identity: &CharacterIdentity, anchor: &AnchorSpec) -> String {
    let (lo, hi) = anchor.height_fraction;
    let sections = [
        format!(
            "CHARACTER REFERENCE IMAGE - a single {} game creature, used downstream as a visual anchor.",
            anchor.view_label
        ),
        format!("CHARACTER:\n{}", identity.description),
        palette_block(&identity.palette),
        format!(
            "POSE:\n{} Shown {}, facing {}. {}",
            identity.neutral_pose,
            anchor.view_label,
            anchor.facing,
            stance_clause(identity.body_plan)
        ),
        format!("COMPOSITION:\n{}", composition_clause(identity.body_plan, lo, hi)),
        magenta_block(&anchor.magenta_hex),
    ];
    sections.join("\n\n")
}

/// Builds the idle-animation sheet prompt: one image, a grid of breathing-loop frames.
pub fn build_idle_prompt(identity: &CharacterIdentity, idle: &IdleSpec, style: &StylePreset, principles: &AnimationPrinciples) -> String {
    let count = idle.frame_count();
    let sections = [
        format!(
            "IDLE ANIMATION SPRITE SHEET - ONE image, a {} columns by {} rows grid ({count} frames total), read left to right, top to bottom.",
            idle.cols, idle.rows
        ),
        format!(
            "CHARACTER:\n{} Keep the outfit, colours, and silhouette identical in every cell - only the pose changes.",
            identity.description
        ),
        format!("ACTION - subtle breathing idle loop:\n{}", principles.breathing),
        format!("SECONDARY MOTION:\n{}", principles.secondary),
        format!(
            "TIMING:\n{}fps. {count} frames make one seamless loop - frame {count} returns to the exact pose of frame 1 so the loop is continuous.",
            idle.fps
        ),
        format!(
            "ZERO TOLERANCE ON DRIFT:\nEvery cell shows exactly one character at the exact same size. The head sits at the same height in every cell (it may move only a few pixels for breathing) and the feet rest on the same baseline in all cells. Same baseline, same eye level, same scale, same horizontal centre. Leave a flat-key gutter of about {}% around the character in each cell. {}",
            idle.gutter_percent, principles.negatives
        ),
        format!("PER-CELL POSES (column, row - row-major):\n{}", idle_frame_lines(idle)),
        magenta_block(MAGENTA_KEY_HEX),
        format!("STYLE:\n{} {}", style.line, style.frame_rate_feel),
    ];
    sections.join("\n\n")
}

/// The palette as an explicit colour list the model should honour.
fn palette_block(palette: &[ColorAnchor]) -> String {
    let mut lines = vec!["PALETTE (use these colours):".to_owned()];
    lines.extend(palette.iter().map(|c| format!("- {} ({}, {}, {})", c.label, c.rgb[0], c.rgb[1], c.rgb[2])));
    lines.join("\n")
}

/// The flat chroma-key contract for the background.
fn magenta_block(hex: &str) -> String {
    format!(
        "BACKGROUND - CHROMA KEY:\nEvery pixel outside the character must be perfectly flat solid pure magenta, exactly {hex}. No gradient, halo, anti-aliasing, or shadow - every non-character pixel is pure magenta. The character's own pixels must avoid pure magenta so the silhouette keys cleanly."
    )
}

/// The stance clause for the anchor pose, gated by body plan.
fn stance_clause(body_plan: BodyPlan) -> &'static str {
    match body_plan {
        BodyPlan::Biped => "Feet flat on the ground, arms hanging naturally at the sides.",
        BodyPlan::Quadruped => "All four feet flat on the ground, weight evenly distributed.",
        BodyPlan::Floating => "Hovering in place, with a small gap below so it reads as airborne.",
    }
}

/// The composition clause, gated by body plan (a floating body has no baseline).
fn composition_clause(body_plan: BodyPlan, lo: u8, hi: u8) -> String {
    match body_plan {
        BodyPlan::Floating => {
            format!(
                "Center the character. It occupies about {lo}-{hi}% of the frame and floats near the middle with flat-key margins all around, the full character visible."
            )
        }
        BodyPlan::Biped | BodyPlan::Quadruped => {
            format!(
                "Center the character horizontally. It occupies about {lo}-{hi}% of the frame height, feet near the bottom edge and head near the top edge, the full character visible."
            )
        }
    }
}

/// The per-cell pose lines, one per frame, in row-major order.
fn idle_frame_lines(idle: &IdleSpec) -> String {
    let count = idle.frame_count();
    (0..count)
        .map(|i| {
            let col = i % idle.cols;
            let row = i / idle.cols;
            format!("FRAME {} (column {col}, row {row}): {}.", i + 1, breathing_phase(i, count))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The breath-cycle phrase for frame `index` of `count`, mapped through one inhale
/// and exhale so the last frame returns to the first.
fn breathing_phase(index: u32, count: u32) -> &'static str {
    const PHASES: [&str; 8] = [
        "rest pose, chest neutral, just before the inhale",
        "chest beginning to rise, knees softening",
        "chest near the top of the inhale, the body at its tallest",
        "top of the breath held, the antenna lagging slightly upward",
        "chest beginning to fall, settling down",
        "chest mid-exhale, knees flexing a little more",
        "near the bottom of the exhale, the body at its lowest",
        "returning to rest, matching frame 1 so the loop is seamless",
    ];
    // Endpoint-inclusive: frame 0 maps to the first phase and the last frame to the
    // last (the loop-back), for any frame count, not just eight.
    let bucket = if count <= 1 {
        0
    } else {
        (index as usize * (PHASES.len() - 1)) / (count as usize - 1)
    };
    PHASES[bucket.min(PHASES.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_anchor_spec_keys_on_the_canonical_magenta() {
        // The anchor builder reads the spec field, the idle builder reads the const;
        // lock the spec default to the const so the two passes can never key on
        // different colours.
        assert_eq!(defaults::default_anchor_spec().magenta_hex, MAGENTA_KEY_HEX);
    }

    #[test]
    fn anchor_prompt_carries_identity_palette_and_magenta_key() {
        let prompt = build_anchor_prompt(&defaults::bit_identity(), &defaults::default_anchor_spec());
        assert!(prompt.contains("Bit, the Pixhaus mascot"), "the identity description is present");
        assert!(prompt.contains("#FF00FF"), "the chroma key is named");
        assert!(prompt.contains("cyan screen glow (64, 200, 220)"), "a palette anchor is listed with its rgb");
        assert!(prompt.contains("facing RIGHT"), "the facing is stated");
        assert!(prompt.contains("70-85%"), "the height fraction is stated");
        assert!(prompt.contains("Feet flat on the ground"), "a biped stance clause is used");
    }

    #[test]
    fn idle_prompt_states_grid_loop_and_per_cell_poses() {
        let prompt = build_idle_prompt(
            &defaults::bit_identity(),
            &defaults::default_idle_spec(),
            &defaults::default_style(),
            &defaults::idle_principles(),
        );
        assert!(
            prompt.contains("4 columns by 2 rows grid (8 frames total)"),
            "the grid and frame count are stated"
        );
        assert!(prompt.contains("12fps"), "the playback rate is stated");
        assert!(prompt.contains("FRAME 1 (column 0, row 0)"), "the first cell is labelled row-major");
        assert!(prompt.contains("FRAME 5 (column 0, row 1)"), "the fifth cell wraps to the second row");
        assert!(prompt.contains("FRAME 8 (column 3, row 1)"), "the last cell is the bottom-right");
        assert!(prompt.contains("seamless loop"), "the loop is called out");
        assert!(prompt.contains("duplicate characters"), "the negatives guard duplicates");
        assert!(prompt.contains("#FF00FF"), "the chroma key is named");
    }

    #[test]
    fn idle_prompt_has_one_line_per_frame() {
        let idle = defaults::default_idle_spec();
        let lines = idle_frame_lines(&idle);
        assert_eq!(lines.lines().count(), idle.frame_count() as usize, "one pose line per frame");
        assert!(lines.lines().last().unwrap().contains("seamless"), "the final frame loops back");
    }

    #[test]
    fn builder_uses_the_edited_description_not_a_fixed_one() {
        // The prompt box overrides the identity description; the builder must use it.
        let mut identity = defaults::bit_identity();
        identity.description = "A tiny copper automaton with one glowing eye.".to_owned();
        let prompt = build_anchor_prompt(&identity, &defaults::default_anchor_spec());
        assert!(prompt.contains("tiny copper automaton"), "the edited subject flows into the prompt");
    }

    #[test]
    fn breathing_phase_scales_to_frame_count() {
        // Generalises beyond eight frames without panicking or duplicating the loop end.
        assert_eq!(breathing_phase(0, 4), "rest pose, chest neutral, just before the inhale");
        assert!(breathing_phase(3, 4).contains("seamless"), "the last frame of any count loops back");
    }
}
