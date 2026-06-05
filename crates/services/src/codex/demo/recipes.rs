//! The forbidden-style reference and the reusable generation recipes.
//!
//! Split from the parent: the explicitly forbidden `flat_3d_render` style (the
//! concrete "not this") and the two spec-driven recipe entries beyond the idle cycle.
//! Shared command helpers come from `super`; this file is called by
//! [`detail_entries`](super::entries::detail_entries) after the hand-authored pass.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is exactly what this section touches.
use super::{
    AnchorKind, AnchorStrength, AntiAliasingRule, BuildError, Command, DetailLevel, Document, EntryStatus, Handles, InclusionPriority, LineTreatment,
    SetStyleDetails, StyleDetails, anchor, delta, frag, fragments, generic, id, status, update,
};

/// Fills in `flat_3d_render`: the explicitly forbidden alternative style, kept as a
/// concrete "not this" reference. It is Deprecated so the resolver suggests
/// `pixel_art` as its replacement, carries a single Negative anchor, and has no
/// positive fragments so it can never enter a prompt.
pub(super) fn detail_forbidden_style(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    let forbidden = id(handles, "flat_3d_render")?;
    update(
        doc,
        forbidden,
        delta(
            "A smooth 3D-render look - soft gradients, ambient-occlusion shading, anti-aliased edges. The opposite of the house style; kept only as the forbidden reference so 'not this' is concrete.",
            "",
            "Soft shaded volumes, gradient fills, blurred edges, baked lighting - everything @style.pixel_art forbids.",
            &["style", "forbidden", "reference"],
        ),
    )?;
    let body = StyleDetails {
        rendering_rules: "Smooth gradient shading, ambient occlusion, anti-aliased edges, baked directional lighting.".to_owned(),
        line_treatment: LineTreatment::None,
        detail_level: DetailLevel::High,
        anti_aliasing: AntiAliasingRule::Allowed,
        resolution: None,
        negative_rules: vec![],
    };
    let mut details = SetStyleDetails::new(forbidden, body);
    details.apply(doc)?;
    anchor(
        doc,
        forbidden,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "Never render Bit's world this way; this is the explicitly forbidden style, not an option.",
    )?;
    // Deprecated last: it still resolves so `forbidden_styles` and relationships point
    // at it, and the resolver offers `pixel_art` as the suggested replacement.
    status(doc, forbidden, EntryStatus::Deprecated)?;
    Ok(())
}

/// One reusable generation recipe beyond the idle cycle: a handle, header text, the
/// generic step spine, one positive fragment, and a single anchor.
struct RecipeSpec {
    handle: &'static str,
    description: &'static str,
    visual: &'static str,
    tags: &'static [&'static str],
    /// The ordered `(key, value)` step fields of the recipe body.
    steps: &'static [(&'static str, &'static str)],
    /// The single compiled-prompt fragment, at Normal priority.
    fragment: &'static str,
    /// The recipe's single anchor: kind, strength, statement.
    anchor: (AnchorKind, AnchorStrength, &'static str),
}

/// Fills in the two reusable recipes: a character sprite-sheet recipe and a tileset
/// recipe. Both stay at the codex root (no folder), like `bit_idle_cycle`. Each is the
/// anchor-then-skin / lock-then-paint spine distilled into a recipe entry.
pub(super) fn detail_new_recipes(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    let specs = [
        RecipeSpec {
            handle: "bit_sprite_sheet",
            description: "The reusable recipe for any Bit action sprite sheet: anchor the identity, author the pose table, skin onto it, align, then review.",
            visual: "",
            tags: &["recipe", "sprite-sheet", "character", "workflow"],
            steps: &[
                ("step_1_anchor", "Neutral Bit on a flat key from @pose.turnaround as the on-model anchor."),
                (
                    "step_2_pose_table",
                    "Pick the action's pose beats from the matching animation entry as the pose map.",
                ),
                (
                    "step_3_skin",
                    "Render the sheet with the anchor attached; lowest temperature for identity-critical multi-cell work.",
                ),
                ("step_4_normalize", "Same baseline, same scale, in-place; key to transparent."),
                ("step_5_review", "Check against the Rules folder."),
                ("canvas", "512x512 cells"),
            ],
            fragment: "generate a @character.bit action sheet in @style.pixel_art using @palette.bit_default, every cell matched to the @pose.turnaround anchor, same scale and baseline, transparent background, checked against @rule.single_subject and @rule.identity_lock",
            anchor: (
                AnchorKind::Animation,
                AnchorStrength::Normal,
                "Anchor-then-skin: lock Bit's identity from the turnaround, render every cell against it, normalize to one baseline and scale.",
            ),
        },
        RecipeSpec {
            handle: "circuit_tileset",
            description: "The reusable recipe for the Circuit Tiles set: lock the palette across the set, paint the body for seamless tiling, then review the edges.",
            visual: "",
            tags: &["recipe", "tileset", "material", "workflow"],
            steps: &[
                ("step_1_lock", "Lock @palette.bit_default across the whole set so no tile drifts."),
                (
                    "step_2_paint",
                    "Paint the body tile for seamless tiling: top matches bottom, left matches right, even detail, no hero chip.",
                ),
                ("step_3_edges", "Heal the seams and check edge-cap consistency."),
                ("step_4_review", "Check against @rule.tile_seamless and @rule.even_lighting."),
                ("temperature", "moderate - lower than free generation, higher than a character sheet"),
            ],
            fragment: "generate a @material.circuit_tiles set in @style.pixel_art using @palette.bit_default, seamless on all four edges, even detail with no hero chip, checked against @rule.tile_seamless",
            anchor: (
                AnchorKind::Style,
                AnchorStrength::Normal,
                "Lock the palette across the set, paint for seamlessness, heal the seams, review the edges.",
            ),
        },
    ];

    for spec in specs {
        let entry = id(handles, spec.handle)?;
        update(doc, entry, delta(spec.description, "", spec.visual, spec.tags))?;
        generic(doc, entry, spec.steps)?;
        let (kind, strength, statement) = spec.anchor;
        anchor(doc, entry, kind, strength, statement)?;
        fragments(doc, entry, vec![frag(spec.fragment, InclusionPriority::Normal)])?;
        status(doc, entry, EntryStatus::Canonical)?;
    }
    Ok(())
}
