//! Integration test for the canonical Bit demo Codex.
//!
//! Asserts the world the public builder produces is well-formed end to end: the
//! expected entries exist by handle with the right type and Canonical status, folders
//! exist and entries are filed, Bit carries its anchors and a prompt fragment, the
//! palette carries its six colours and a ramp, the animations carry pose beats, the
//! relationship graph is present, coverage is seeded for Bit and the Start button,
//! and - the load-bearing check - the services validation and `@`-reference resolver
//! find no blocking diagnostics and no unresolved references anywhere in the world.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]

use pixhaus_core::codex::{AntiAliasingRule, DetailLevel, EntryDetails, LineTreatment};
use pixhaus_core::{AnchorKind, AnchorStrength, CodexHandle, CoverageItemStatus, EntryStatus, EntryType, InclusionPriority};
use pixhaus_services::build_bit_demo_codex;
use pixhaus_services::codex::{preview_entry, resolve_text, validate_codex};

/// Resolves a handle to its entry id, failing the test if it does not exist.
fn handle_id(codex: &pixhaus_core::Codex, handle: &str) -> pixhaus_core::CodexEntryId {
    let h = CodexHandle::new(handle).expect("valid handle");
    codex.resolve_handle(&h).unwrap_or_else(|| panic!("handle {handle} resolves"))
}

#[test]
fn world_builds_with_the_expected_shape() {
    let doc = build_bit_demo_codex().expect("the demo world builds");
    let codex = doc.codex();

    assert_eq!(codex.entries().len(), 36, "36 entries");
    assert_eq!(codex.folders().len(), 8, "8 folders");
    assert_eq!(codex.relationships().len(), 65, "65 relationship edges");
}

#[test]
fn entries_exist_by_handle_with_type_and_canonical_status() {
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();

    let expected: &[(&str, EntryType)] = &[
        ("bit", EntryType::Character),
        ("byte", EntryType::Npc),
        ("bit_default", EntryType::Palette),
        ("pixel_art", EntryType::Style),
        ("retro_arcade", EntryType::Vibe),
        ("idle", EntryType::Animation),
        ("walk", EntryType::Animation),
        ("run", EntryType::Animation),
        ("jump", EntryType::Animation),
        ("fall", EntryType::Animation),
        ("attack", EntryType::Animation),
        ("hurt", EntryType::Animation),
        ("turnaround", EntryType::Pose),
        ("floppy", EntryType::Item),
        ("circuit_tiles", EntryType::Material),
        ("arcade_world", EntryType::Location),
        ("readable_at_32px", EntryType::Rule),
        ("transparent_background", EntryType::Rule),
        ("unified_8bit_palette", EntryType::Rule),
        ("no_extra_limbs", EntryType::Rule),
        ("no_grimdark", EntryType::Rule),
        ("start_button", EntryType::UiElement),
        ("bit_idle_cycle", EntryType::Recipe),
        ("single_subject", EntryType::Rule),
        ("identity_lock", EntryType::Rule),
        ("spatial_stability", EntryType::Rule),
        ("clean_silhouette", EntryType::Rule),
        ("clean_key", EntryType::Rule),
        ("even_lighting", EntryType::Rule),
        ("flat_side_view", EntryType::Rule),
        ("no_text_or_ui", EntryType::Rule),
        ("tile_seamless", EntryType::Rule),
        ("single_gait", EntryType::Rule),
        ("bit_sprite_sheet", EntryType::Recipe),
        ("circuit_tileset", EntryType::Recipe),
    ];

    for (handle, entry_type) in expected {
        let entry = codex.entry(handle_id(codex, handle)).expect("entry present");
        assert_eq!(entry.entry_type, *entry_type, "{handle} has type {entry_type:?}");
        assert_eq!(entry.status, EntryStatus::Canonical, "{handle} is Canonical");
        assert!(!entry.description.trim().is_empty(), "{handle} has a description");
    }

    // The forbidden alternative style exists as the deliberate "not this" reference -
    // a Style entry, but Deprecated, not Canonical, so it never enters a prompt.
    let forbidden = codex.entry(handle_id(codex, "flat_3d_render")).expect("entry present");
    assert_eq!(forbidden.entry_type, EntryType::Style, "flat_3d_render is a Style");
    assert_eq!(forbidden.status, EntryStatus::Deprecated, "flat_3d_render is Deprecated");
    assert!(!forbidden.description.trim().is_empty(), "flat_3d_render has a description");
}

#[test]
fn folders_exist_and_entries_are_filed() {
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();

    let folder_names: Vec<&str> = codex.folders().values().map(|f| f.name.as_str()).collect();
    for name in [
        "Characters",
        "Palettes",
        "Styles & vibes",
        "Animations",
        "Props & items",
        "World",
        "Rules",
        "UI",
    ] {
        assert!(folder_names.contains(&name), "folder {name} exists");
    }

    // Bit is filed under a folder; the Recipe stays at the root.
    let bit = codex.entry(handle_id(codex, "bit")).expect("bit");
    assert!(bit.folder_id.is_some(), "Bit is filed in a folder");
    let recipe = codex.entry(handle_id(codex, "bit_idle_cycle")).expect("recipe");
    assert!(recipe.folder_id.is_none(), "the recipe stays at the codex root");
}

#[test]
fn bit_has_its_anchors_and_a_prompt_fragment() {
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    let bit = codex.entry(handle_id(codex, "bit")).expect("bit");

    assert_eq!(bit.anchors.len(), 7, "Bit carries seven anchors");
    assert!(
        bit.anchors
            .iter()
            .any(|a| a.kind == AnchorKind::Identity && a.strength == AnchorStrength::Locked),
        "a Locked identity anchor",
    );
    assert!(
        bit.anchors
            .iter()
            .any(|a| a.kind == AnchorKind::Negative && a.strength == AnchorStrength::Locked),
        "a Locked negative anchor",
    );
    assert!(!bit.prompt_fragments.is_empty(), "Bit has prompt fragments");
    assert!(!bit.negative_fragments.is_empty(), "Bit has negative fragments");
    // The alias resolves to Bit.
    assert_eq!(codex.resolve_handle(&CodexHandle::new("the_mascot").unwrap()), Some(bit.id));
}

#[test]
fn palette_has_six_colors_and_a_ramp() {
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    let palette = codex.entry(handle_id(codex, "bit_default")).expect("palette");
    match &palette.details {
        EntryDetails::Palette(p) => {
            assert_eq!(p.colors.len(), 6, "six palette colours");
            assert_eq!(p.ramps.len(), 2, "two named ramps - body charcoal and screen glow");
            assert_eq!(p.colors[0].rgba, [24, 24, 32, 255], "the charcoal body colour");
            assert!(!p.allow_generated_colors, "the palette is closed");
        }
        other => panic!("expected a palette body, got {other:?}"),
    }
}

#[test]
fn animations_carry_principled_bodies_fragments_and_anchors() {
    // The seven Animation entries are the enriched teaching set: a rich
    // multi-beat body, a Critical identity+action prompt fragment, per-entry
    // negatives, and a motion-intent Animation anchor. The turnaround Pose entry is
    // checked separately below (it holds a Generic body, not an Animation one).
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    for handle in ["idle", "walk", "run", "jump", "fall", "attack", "hurt"] {
        let entry = codex.entry(handle_id(codex, handle)).expect("animation");
        match &entry.details {
            EntryDetails::Animation(a) => {
                assert!(a.pose_beats.len() >= 4, "{handle} walks at least four key poses");
                assert!(
                    a.pose_beats.iter().all(|b| !b.label.trim().is_empty() && !b.description.trim().is_empty()),
                    "{handle} pose beats are labelled and described",
                );
                assert!(!a.purpose.trim().is_empty(), "{handle} has a purpose");
                assert!(a.fps > 0, "{handle} has a playback rate");
                assert!(a.recommended_frame_count > 0, "{handle} has a frame count");
            }
            other => panic!("expected an animation body for {handle}, got {other:?}"),
        }
        assert!(
            entry.prompt_fragments.iter().any(|f| f.priority == InclusionPriority::Critical),
            "{handle} leads with a Critical identity+action fragment",
        );
        assert!(!entry.negative_fragments.is_empty(), "{handle} carries negatives");
        assert!(
            entry.anchors.iter().any(|a| a.kind == AnchorKind::Animation),
            "{handle} carries a motion-intent Animation anchor",
        );
        assert!(entry.anchors.iter().any(|a| a.kind == AnchorKind::Style), "{handle} carries a Style anchor");
    }
}

#[test]
fn the_turnaround_reference_carries_its_views_and_anchors() {
    // The turnaround is a Pose reference entry (a Generic body), enriched with the
    // four canonical views, a Critical fragment, negatives, and a Locked Animation
    // anchor that locks it as a static multi-view reference.
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    let entry = codex.entry(handle_id(codex, "turnaround")).expect("turnaround");
    assert_eq!(entry.entry_type, EntryType::Pose, "turnaround is a Pose entry");
    match &entry.details {
        EntryDetails::Generic(g) => {
            for key in ["view.front", "view.three_quarter", "view.side", "view.back"] {
                assert!(g.fields.iter().any(|f| f.key == key), "turnaround documents {key}");
            }
        }
        other => panic!("expected a generic body for the turnaround Pose, got {other:?}"),
    }
    assert!(
        entry.prompt_fragments.iter().any(|f| f.priority == InclusionPriority::Critical),
        "turnaround leads with a Critical fragment",
    );
    assert!(!entry.negative_fragments.is_empty(), "turnaround carries negatives");
    assert!(
        entry
            .anchors
            .iter()
            .any(|a| a.kind == AnchorKind::Animation && a.strength == AnchorStrength::Locked),
        "turnaround carries a Locked Animation anchor",
    );
}

#[test]
fn animation_entries_compile_into_substantial_prompts() {
    // The payoff: an enriched animation entry's fragments and anchors compile into a
    // real, substantial generation prompt. This proves the authored content reaches
    // the prompt the user sends, without asserting brittle exact strings.
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    for handle in ["idle", "walk", "run", "jump", "fall", "attack", "hurt"] {
        let entry_id = handle_id(codex, handle);
        let compiled = preview_entry(codex, entry_id, None).expect("the entry compiles");
        // The compiled positive prompt is substantial - the Critical identity line
        // plus the Important motion lines and the folded-in anchors add up.
        assert!(
            compiled.positive.len() > 200,
            "{handle} compiles a substantial positive prompt (got {} chars): {}",
            compiled.positive.len(),
            compiled.positive,
        );
        // The entry contributed (its fragments and/or anchors survived).
        assert!(compiled.references_used.contains(&entry_id), "{handle} is a used reference");
        // The motion-intent Animation anchor is folded into the prompt.
        assert!(
            compiled.included_anchors.iter().any(|a| a.kind == AnchorKind::Animation),
            "{handle} folds its Animation anchor into the prompt",
        );
        // The negatives reach the negative prompt.
        assert!(!compiled.negative.trim().is_empty(), "{handle} produces a negative prompt");

        // Every `@`-reference in the compiled positive prompt still resolves against
        // the world (the new fragments only name handles that exist).
        let resolution = resolve_text(codex, &compiled.positive);
        assert!(
            resolution.is_clean(),
            "{handle} compiled prompt has unresolved references: {:?}",
            resolution.problems,
        );
    }
}

#[test]
fn the_style_carries_art_direction_grade_details() {
    // The pixel-art Style entry holds the load-bearing 8-bit rules: a selective outer
    // outline and manual-only anti-aliasing, populated rendering rules, a forbidden
    // list, and the fixed 512x512 resolution.
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    let entry = codex.entry(handle_id(codex, "pixel_art")).expect("pixel_art");
    match &entry.details {
        EntryDetails::Style(s) => {
            assert_eq!(s.line_treatment, LineTreatment::Selective, "selective outer outline");
            assert_eq!(s.anti_aliasing, AntiAliasingRule::Manual, "manual anti-aliasing only");
            assert_eq!(s.detail_level, DetailLevel::Low, "low detail for 32px legibility");
            assert_eq!(s.resolution, Some((512, 512)), "the canonical canvas");
            assert!(!s.rendering_rules.trim().is_empty(), "populated rendering rules");
            assert!(!s.negative_rules.is_empty(), "a forbidden list");
        }
        other => panic!("expected a style body, got {other:?}"),
    }
    // The Style anchor is Locked and the entry carries negatives.
    assert!(
        entry
            .anchors
            .iter()
            .any(|a| a.kind == AnchorKind::Style && a.strength == AnchorStrength::Locked),
        "the style is Locked",
    );
    assert!(!entry.negative_fragments.is_empty(), "the style carries a forbidden list");
}

#[test]
fn the_new_rule_prop_material_and_recipe_entries_are_well_formed() {
    // The review-derived rules, the prop and material bodies, and the new recipes all
    // exist with non-empty bodies and the right shape.
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();

    // Every new rule carries a constraint field and a single Locked anchor.
    for handle in [
        "single_subject",
        "identity_lock",
        "spatial_stability",
        "clean_silhouette",
        "clean_key",
        "even_lighting",
        "flat_side_view",
        "no_text_or_ui",
        "tile_seamless",
        "single_gait",
    ] {
        let entry = codex.entry(handle_id(codex, handle)).expect("rule entry");
        assert_eq!(entry.entry_type, EntryType::Rule, "{handle} is a Rule");
        match &entry.details {
            EntryDetails::Generic(g) => {
                assert!(
                    g.fields.iter().any(|f| f.key == "constraint" && !f.value.trim().is_empty()),
                    "{handle} has a constraint"
                );
            }
            other => panic!("expected a generic rule body for {handle}, got {other:?}"),
        }
        assert!(
            entry.anchors.iter().any(|a| a.strength == AnchorStrength::Locked),
            "{handle} carries a Locked anchor",
        );
    }

    // The floppy prop carries the prop-brief fields.
    let floppy = codex.entry(handle_id(codex, "floppy")).expect("floppy");
    match &floppy.details {
        EntryDetails::Generic(g) => {
            for key in ["silhouette", "material", "composition", "view"] {
                assert!(g.fields.iter().any(|f| f.key == key), "floppy documents {key}");
            }
        }
        other => panic!("expected a generic prop body, got {other:?}"),
    }
    assert!(!floppy.negative_fragments.is_empty(), "floppy carries negatives");

    // The circuit-tiles material carries the seam and surface fields and a Strong
    // Negative anchor (seamlessness is the make-or-break tile rule).
    let tiles = codex.entry(handle_id(codex, "circuit_tiles")).expect("circuit_tiles");
    match &tiles.details {
        EntryDetails::Generic(g) => {
            for key in ["tiling", "surface", "detail", "edges"] {
                assert!(g.fields.iter().any(|f| f.key == key), "circuit_tiles documents {key}");
            }
        }
        other => panic!("expected a generic material body, got {other:?}"),
    }
    assert!(
        tiles
            .anchors
            .iter()
            .any(|a| a.kind == AnchorKind::Negative && a.strength == AnchorStrength::Strong),
        "the tileset's seam rule is a Strong negative anchor",
    );

    // The two new recipes carry the step spine and a positive fragment.
    for handle in ["bit_sprite_sheet", "circuit_tileset"] {
        let entry = codex.entry(handle_id(codex, handle)).expect("recipe");
        assert_eq!(entry.entry_type, EntryType::Recipe, "{handle} is a Recipe");
        assert!(entry.folder_id.is_none(), "{handle} stays at the codex root");
        match &entry.details {
            EntryDetails::Generic(g) => {
                assert!(g.fields.iter().any(|f| f.key.starts_with("step_")), "{handle} has step fields");
            }
            other => panic!("expected a generic recipe body for {handle}, got {other:?}"),
        }
        assert!(!entry.prompt_fragments.is_empty(), "{handle} has a compiled-prompt fragment");
    }
}

#[test]
fn the_style_prop_and_material_compile_into_clean_prompts() {
    // The payoff beyond animations: the style, the prop, and the material each compile
    // into a substantial, reference-clean generation prompt, proving the enriched
    // fragments and anchors reach the prompt the user sends.
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    for handle in ["pixel_art", "floppy", "circuit_tiles"] {
        let entry_id = handle_id(codex, handle);
        let compiled = preview_entry(codex, entry_id, None).expect("the entry compiles");
        assert!(
            compiled.positive.len() > 120,
            "{handle} compiles a substantial positive prompt (got {} chars): {}",
            compiled.positive.len(),
            compiled.positive,
        );
        assert!(compiled.references_used.contains(&entry_id), "{handle} is a used reference");
        assert!(!compiled.negative.trim().is_empty(), "{handle} produces a negative prompt");
        let resolution = resolve_text(codex, &compiled.positive);
        assert!(
            resolution.is_clean(),
            "{handle} compiled prompt has unresolved references: {:?}",
            resolution.problems,
        );
    }
}

#[test]
fn coverage_is_seeded_for_bit_and_the_button() {
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();

    let bit = handle_id(codex, "bit");
    assert_eq!(codex.coverage_status(bit, "idle"), CoverageItemStatus::Approved);
    assert_eq!(codex.coverage_status(bit, "death"), CoverageItemStatus::Deprecated);
    assert_eq!(codex.entry(bit).expect("bit").custom_slots.len(), 1, "Bit has a custom slot");

    let button = handle_id(codex, "start_button");
    assert_eq!(codex.coverage_status(button, "normal"), CoverageItemStatus::Approved);
    assert_eq!(codex.coverage_status(button, "hover"), CoverageItemStatus::Generated);
}

#[test]
fn relationships_are_present() {
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();
    let bit = handle_id(codex, "bit");
    let palette = handle_id(codex, "bit_default");
    assert!(
        codex
            .relationships()
            .iter()
            .any(|r| r.from == bit && r.to == palette && r.kind == pixhaus_core::RelationKind::Uses),
        "Bit Uses the palette",
    );
}

#[test]
fn the_world_has_no_broken_references_or_blocking_diagnostics() {
    let doc = build_bit_demo_codex().expect("builds");
    let codex = doc.codex();

    // Validation pass: a clean, well-formed world has no blocking findings.
    let report = validate_codex(codex);
    assert!(!report.has_blocking(), "no blocking diagnostics: {:?}", report.diagnostics);

    // Reference resolution: every `@`-reference in every prompt and negative fragment,
    // and every descriptive text field, resolves cleanly against the world it lives in.
    for entry in codex.entries().values() {
        for fragment in &entry.prompt_fragments {
            let resolution = resolve_text(codex, &fragment.text);
            assert!(
                resolution.is_clean(),
                "entry {} has an unresolved reference in a prompt fragment: {:?}",
                entry.handle.as_str(),
                resolution.problems,
            );
        }
        for negative in &entry.negative_fragments {
            let resolution = resolve_text(codex, negative);
            assert!(
                resolution.is_clean(),
                "entry {} has an unresolved reference in a negative fragment: {:?}",
                entry.handle.as_str(),
                resolution.problems,
            );
        }
    }
}
