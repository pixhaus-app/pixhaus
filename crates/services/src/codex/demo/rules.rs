//! The project rules: the constraint entries the whole world is checked against.
//!
//! Split from the parent: the five original constraints plus the review-derived rules
//! adapted from sprite- and tile-review criteria. Shared command helpers come from
//! `super`; this file is called by
//! [`detail_entries`](super::entries::detail_entries) in entry order.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is exactly what this section touches - the shared command
// helpers and the `core` types its specs name.
use super::{
    AnchorKind, AnchorStrength, BuildError, Document, EntryStatus, Handles, InclusionPriority, anchor, delta, frag, fragments, generic, id, status, update,
};

/// One project rule: handle, constraint field value, and its single anchor.
struct RuleSpec {
    handle: &'static str,
    description: &'static str,
    constraint: &'static str,
    anchor_kind: AnchorKind,
    anchor_statement: &'static str,
    /// An optional positive fragment (only a couple of rules carry one).
    fragment: Option<&'static str>,
}

/// Fills in the project rules: the five original constraints plus the review-derived
/// rules adapted from sprite- and tile-review criteria. Each is Canonical, carries a
/// one-field generic body and a single Locked anchor, and a couple add a prompt
/// fragment. One long, flat table on purpose - the rules are data with no branching.
#[allow(clippy::too_many_lines)]
pub(super) fn detail_rules(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    let specs = [
        RuleSpec {
            handle: "readable_at_32px",
            description: "Every sprite's silhouette must read clearly at 32px.",
            constraint: "silhouette must read at 32px",
            anchor_kind: AnchorKind::Scale,
            anchor_statement: "Every sprite must hold a clear, recognizable silhouette at 32px.",
            fragment: None,
        },
        RuleSpec {
            handle: "transparent_background",
            description: "Sprites are generated and exported on a transparent background.",
            constraint: "transparent background for all sprites",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "No baked-in background behind a sprite; the background stays transparent.",
            fragment: None,
        },
        RuleSpec {
            handle: "unified_8bit_palette",
            description: "The whole world shares one crisp 8-bit palette - the Bit Default Palette.",
            constraint: "all assets use the Bit Default 8-bit palette",
            anchor_kind: AnchorKind::Palette,
            anchor_statement: "Every asset uses the Bit Default 6-colour 8-bit palette; no off-palette colour.",
            fragment: Some("unified crisp 8-bit palette across the whole world, @palette.bit_default"),
        },
        RuleSpec {
            handle: "no_extra_limbs",
            description: "Bit has exactly two arms, two legs, one antenna, and no mouth.",
            constraint: "no extra limbs, no mouth on Bit",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "Bit never has extra limbs, a mouth, or sharp teeth.",
            fragment: None,
        },
        RuleSpec {
            handle: "no_grimdark",
            description: "The Bit world stays friendly and optimistic - no dark, gritty, or grimdark tone.",
            constraint: "friendly, optimistic; never grimdark",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "No grimdark, horror, or dystopian tone anywhere in the Bit world.",
            fragment: None,
        },
        RuleSpec {
            handle: "single_subject",
            description: "Exactly one Bit per frame - no twin, clone, mirror, ghost, or second figure.",
            constraint: "one subject per frame; no duplicates",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "Never more than one character in a cell - no twin, clone, mirror, reflection, ghost echo, or stray second figure.",
            fragment: None,
        },
        RuleSpec {
            handle: "identity_lock",
            description: "Bit stays on-model across every frame and every asset - same head, screen, antenna, palette, and proportions; nothing appears or disappears.",
            constraint: "on-model identity across all frames; no flicker",
            anchor_kind: AnchorKind::Identity,
            anchor_statement: "Every frame and every asset is unmistakably the same Bit: same head shape, same pixel-face screen, same single antenna, same palette and proportions; no part appears or disappears between frames.",
            fragment: None,
        },
        RuleSpec {
            handle: "spatial_stability",
            description: "Same scale, same baseline, same horizontal centre - locomotion animates in place, not sliding across the frame.",
            constraint: "consistent scale and baseline; animate in place",
            anchor_kind: AnchorKind::Scale,
            anchor_statement: "Keep one scale, one baseline, and one horizontal centre across a sheet; ground motion happens in place (on an invisible treadmill), with the airborne frames the only exception.",
            fragment: None,
        },
        RuleSpec {
            handle: "clean_silhouette",
            description: "Read by value, not by outline - keep a clean, closed silhouette at 32px with limbs clear of the body.",
            constraint: "clean readable silhouette at 32px; read by value",
            anchor_kind: AnchorKind::Visual,
            anchor_statement: "Interior form reads by value, not by interior line; the outer silhouette stays clean and closed and reads at 32px, with overlapping limbs separated by a near/far value split and a dark edge.",
            fragment: None,
        },
        RuleSpec {
            handle: "clean_key",
            description: "Crisp keyed silhouette - no halo, fringe, or stray pixels, and transparent gaps inside the silhouette too.",
            constraint: "clean transparent key; no halo or fringe",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "Everything that is not the subject is fully transparent, including the gaps inside the silhouette; no key-colour halo, no colour fringe, no stray pixels, no drawn ground line.",
            fragment: None,
        },
        RuleSpec {
            handle: "even_lighting",
            description: "Even, flat lighting everywhere - no directional cast shadow, rim light, spotlight, or vignette.",
            constraint: "even flat lighting; no directional shadow",
            anchor_kind: AnchorKind::Style,
            anchor_statement: "Light every asset evenly and flatly with at most a soft CRT bloom; no directional cast shadow, no rim light, no centre spotlight, no vignette.",
            fragment: None,
        },
        RuleSpec {
            handle: "flat_side_view",
            description: "Flat 2D side or front view - not 3D, isometric, or top-down (except tiles, which are top-down); no perspective.",
            constraint: "flat 2D view; no perspective or isometric",
            anchor_kind: AnchorKind::Style,
            anchor_statement: "Render flat in 2D - side or front view for characters and props, top-down for floor tiles; never 3D, isometric, or perspective.",
            fragment: None,
        },
        RuleSpec {
            handle: "no_text_or_ui",
            description: "No text, labels, frame numbers, health bars, watermarks, or signatures in the art.",
            constraint: "no text, ui, or watermark contamination",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "No text, labels, frame numbers, health bars, UI chrome, watermarks, or signatures anywhere in the generated art.",
            fragment: None,
        },
        RuleSpec {
            handle: "tile_seamless",
            description: "Tiles read as one continuous surface - top matches bottom, left matches right, even density, no grid line or hero blob.",
            constraint: "seamless tiling; even density; no hero feature",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "A tile body reads as one continuous surface: its top edge matches its bottom and its left matches its right, detail density is uniform everywhere, and there is no visible grid line and no repeating hero feature the eye snaps to.",
            fragment: Some("seamless tiling on all four edges, uniform detail density, no visible grid line or hero feature, @material.circuit_tiles"),
        },
        RuleSpec {
            handle: "single_gait",
            description: "A whole locomotion sheet is one cycle at one energy - the bottom row continues the top, not a second, calmer animation.",
            constraint: "one continuous gait across the sheet",
            anchor_kind: AnchorKind::Animation,
            anchor_statement: "A locomotion sheet is a single continuous cycle at one energy level; the bottom row continues the top row's gait - never split into two different animations.",
            fragment: None,
        },
    ];

    for spec in specs {
        let entry = id(handles, spec.handle)?;
        update(doc, entry, delta(spec.description, "", "", &["rule", "constraint"]))?;
        generic(doc, entry, &[("constraint", spec.constraint)])?;
        anchor(doc, entry, spec.anchor_kind, AnchorStrength::Locked, spec.anchor_statement)?;
        if let Some(text) = spec.fragment {
            fragments(doc, entry, vec![frag(text, InclusionPriority::Important)])?;
        }
        status(doc, entry, EntryStatus::Canonical)?;
    }
    Ok(())
}
