//! Built-in composition records. Source of truth migrated from the former
//! `reference_sheet::templates` module. Per spec section 8.
//!
//! The four legacy `CompositionTemplate` variants become built-in
//! `Structure`s; the negative-prompt clauses shared by all four move to a
//! single built-in Default `Style`. Migration-equivalence tests assert the
//! resolver reproduces the legacy layout prose, dimensions, and negatives.

use std::collections::BTreeMap;

use pixhaus_core::project::library::composition::{
    Dimensions, PanelRect, PanelSlot, PromptId, PromptTemplate, Structure, StructureId,
    StructureOutput, StructurePanel, Style, StyleId,
};

/// Default cascading baseline used when a project sets no `style_notes`.
pub const BUILTIN_DEFAULT_BASELINE: &str = "pixel art reference sheet";

/// Id of the built-in Default Style carrying the shared look negatives.
pub const STYLE_DEFAULT_ID: &str = "pixhaus.builtin.style.default";

/// The read-only registry of built-in composition records, loaded once at
/// startup. Project-tier records shadow these by id.
pub struct BuiltinLibrary {
    /// Built-in Structures keyed by id.
    pub structures: BTreeMap<StructureId, Structure>,
    /// Built-in Styles keyed by id.
    pub styles: BTreeMap<StyleId, Style>,
    /// Built-in saved Prompts keyed by id (none ship in v1).
    pub prompts: BTreeMap<PromptId, PromptTemplate>,
}

impl BuiltinLibrary {
    /// Constructs the built-in records. Pure — no I/O, cannot fail.
    #[must_use]
    pub fn load() -> Self {
        let mut structures = BTreeMap::new();
        for s in [character(), item(), tileset(), custom()] {
            structures.insert(s.id.clone(), s);
        }
        let mut styles = BTreeMap::new();
        let def = default_style();
        styles.insert(def.id.clone(), def);
        Self {
            structures,
            styles,
            prompts: BTreeMap::new(),
        }
    }
}

fn panel(
    label: &str,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    slot: PanelSlot,
    prose: &str,
) -> StructurePanel {
    StructurePanel {
        label: label.into(),
        rect: PanelRect { x, y, w, h },
        prose_fragment: prose.into(),
        slot,
    }
}

fn character() -> Structure {
    // Geometry copied verbatim from templates.rs::character_composition().
    // Views: 200x480 at x=i*200, y=0. Expressions: 256x192 at y=480.
    // Palette swatch: 1024x128 at y=672. Callouts: 512x320 at y=800.
    // Outfit: 256x384 at y=1120.
    let mut panels = Vec::new();
    let views = ["front", "side-left", "three-quarter", "side-right", "back"];
    for (i, label) in views.iter().enumerate() {
        panels.push(panel(
            label,
            u32::try_from(i).unwrap_or(0) * 200,
            0,
            200,
            480,
            PanelSlot::View,
            "five turnaround views in a horizontal strip across the top, \
             left-aligned starting at the left edge — front view, left side, \
             three-quarter view, right side, back view, each {panel_w} pixels \
             wide, {panel_h} pixels tall",
        ));
    }
    // Only the first view carries the shared turnaround clause; the rest carry
    // an empty fragment so the prose is not repeated five times. (Compose joins
    // non-empty fragments only.)
    for p in panels.iter_mut().skip(1) {
        p.prose_fragment.clear();
    }
    let exprs = ["neutral", "happy", "angry"];
    for (i, label) in exprs.iter().enumerate() {
        let prose = if i == 0 {
            "three facial expression close-ups side by side, left-aligned \
             starting at the left edge — neutral, happy, angry — each {panel_w} \
             pixels wide, {panel_h} pixels tall"
        } else {
            ""
        };
        panels.push(panel(
            label,
            u32::try_from(i).unwrap_or(0) * 256,
            480,
            256,
            192,
            PanelSlot::Expression,
            prose,
        ));
    }
    panels.push(panel(
        "palette",
        0,
        672,
        1024,
        128,
        PanelSlot::PaletteSwatch,
        "a horizontal palette swatch row showing all colours used, {panel_w} \
         pixels wide, {panel_h} pixels tall",
    ));
    for (i, label) in ["detail-1", "detail-2"].iter().enumerate() {
        let prose = if i == 0 {
            "two detail callout panels side by side, each {panel_w} pixels wide, \
             {panel_h} pixels tall"
        } else {
            ""
        };
        panels.push(panel(
            label,
            u32::try_from(i).unwrap_or(0) * 512,
            800,
            512,
            320,
            PanelSlot::Callout,
            prose,
        ));
    }
    panels.push(panel(
        "outfit-variant",
        0,
        1120,
        256,
        384,
        PanelSlot::Outfit,
        "one outfit-variant panel, {panel_w} pixels wide, {panel_h} pixels tall, \
         showing an alternate outfit or colour scheme. White background, clean \
         pixel-art lines, consistent scale across all views. Professional \
         sprite sheet format",
    ));
    Structure {
        id: StructureId("pixhaus.builtin.structure.character".into()),
        name: "Character".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions {
                width: 1024,
                height: 1536,
            },
            panels,
        },
        layout_negatives:
            "extra limbs, bad anatomy, duplicate characters, overlapping views, inconsistent scale"
                .into(),
    }
}

fn item() -> Structure {
    let mut panels = Vec::new();
    let views = [
        ("front", 0, 0),
        ("side-left", 512, 0),
        ("back", 0, 384),
        ("side-right", 512, 384),
    ];
    for (i, (label, x, y)) in views.iter().enumerate() {
        let prose = if i == 0 {
            "2×2 grid of orthographic views — top-left is front face, top-right \
             is left side, bottom-left is back face, bottom-right is right side, \
             each {panel_w}×{panel_h}"
        } else {
            ""
        };
        panels.push(panel(label, *x, *y, 512, 384, PanelSlot::View, prose));
    }
    panels.push(panel(
        "palette",
        0,
        768,
        1024,
        128,
        PanelSlot::PaletteSwatch,
        "a palette swatch row {panel_w}×{panel_h} pixels",
    ));
    for (i, label) in ["detail-1", "detail-2"].iter().enumerate() {
        let prose = if i == 0 {
            "two detail callout panels {panel_w}×{panel_h} each. White \
             background, consistent scale across all four views"
        } else {
            ""
        };
        panels.push(panel(
            label,
            u32::try_from(i).unwrap_or(0) * 512,
            896,
            512,
            128,
            PanelSlot::Callout,
            prose,
        ));
    }
    Structure {
        id: StructureId("pixhaus.builtin.structure.item".into()),
        name: "Item".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions {
                width: 1024,
                height: 1024,
            },
            panels,
        },
        layout_negatives: "floating elements, inconsistent scale across views".into(),
    }
}

fn tileset() -> Structure {
    // Per-band pixel dimensions interpolated from each rect, matching the
    // legacy tileset prompt's "(1024×256)" framing.
    let panels = vec![
        panel(
            "tile-primitives",
            0,
            0,
            1024,
            256,
            PanelSlot::View,
            "top row ({panel_w}×{panel_h}) shows the base tile primitives — flat \
             tile, corner variants, edge variants, in a grid",
        ),
        panel(
            "transition-variants",
            0,
            256,
            1024,
            384,
            PanelSlot::View,
            "middle band ({panel_w}×{panel_h}): transition tile variants and edge \
             blending rules",
        ),
        panel(
            "autotile-preview",
            0,
            640,
            1024,
            256,
            PanelSlot::View,
            "lower block ({panel_w}×{panel_h}): 3×3 autotile preview demonstrating \
             the autotile rule set",
        ),
        panel(
            "palette",
            0,
            896,
            1024,
            128,
            PanelSlot::PaletteSwatch,
            "bottom strip ({panel_w}×{panel_h}): palette swatch. White background, \
             grid-aligned, clean pixel art, consistent tile size throughout",
        ),
    ];
    Structure {
        id: StructureId("pixhaus.builtin.structure.tileset".into()),
        name: "Tileset".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions {
                width: 1024,
                height: 1024,
            },
            panels,
        },
        layout_negatives: "non-grid-aligned tiles, broken patterns, inconsistent tile size".into(),
    }
}

fn custom() -> Structure {
    let panels = vec![
        panel(
            "full-body",
            0,
            0,
            1024,
            896,
            PanelSlot::View,
            "full-body orthographic view centred in a {panel_w}×{panel_h} area",
        ),
        panel(
            "palette",
            0,
            896,
            1024,
            128,
            PanelSlot::PaletteSwatch,
            "palette swatch row at the bottom, {panel_w}×{panel_h} pixels. White background",
        ),
    ];
    Structure {
        id: StructureId("pixhaus.builtin.structure.custom".into()),
        name: "Custom".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions {
                width: 1024,
                height: 1024,
            },
            panels,
        },
        layout_negatives: String::new(),
    }
}

fn default_style() -> Style {
    Style {
        id: StyleId(STYLE_DEFAULT_ID.into()),
        name: "Default".into(),
        modifiers: String::new(),
        look_negatives:
            "blurry, low quality, watermark, text label, logo, cropped, photo realistic, 3d render"
                .into(),
        model_pref: None,
        quality: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::{ComposeRequest, compose};
    use std::collections::BTreeMap;

    fn structure<'a>(lib: &'a BuiltinLibrary, id: &str) -> &'a Structure {
        &lib.structures[&StructureId(id.into())]
    }

    fn compose_layout(lib: &BuiltinLibrary, id: &str) -> String {
        let empty = BTreeMap::new();
        let req = ComposeRequest {
            baseline: "",
            structure: structure(lib, id),
            style: None,
            prompt: None,
            variable_values: &empty,
            entity_info: &empty,
            inline_text: "",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        compose(&req).unwrap().positive
    }

    fn count_slot(s: &Structure, slot: PanelSlot) -> usize {
        let StructureOutput::Paneled { panels, .. } = &s.output else {
            return 0;
        };
        panels.iter().filter(|p| p.slot == slot).count()
    }

    fn canvas(s: &Structure) -> Dimensions {
        let StructureOutput::Paneled { canvas, .. } = &s.output else {
            panic!("expected paneled output")
        };
        *canvas
    }

    #[test]
    fn loads_four_structures_and_default_style() {
        let lib = BuiltinLibrary::load();
        assert_eq!(lib.structures.len(), 4);
        assert!(lib.styles.contains_key(&StyleId(STYLE_DEFAULT_ID.into())));
        assert!(lib.prompts.is_empty());
    }

    #[test]
    fn character_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let c = structure(&lib, "pixhaus.builtin.structure.character");
        assert_eq!(
            canvas(c),
            Dimensions {
                width: 1024,
                height: 1536
            }
        );
        // 5 views + 3 expressions + 1 palette + 2 callouts + 1 outfit = 12 panels.
        let StructureOutput::Paneled { panels, .. } = &c.output else {
            panic!()
        };
        assert_eq!(panels.len(), 12);
        assert_eq!(count_slot(c, PanelSlot::View), 5);
        assert_eq!(count_slot(c, PanelSlot::Expression), 3);
        assert_eq!(count_slot(c, PanelSlot::Callout), 2);
        assert_eq!(count_slot(c, PanelSlot::Outfit), 1);
        assert_eq!(count_slot(c, PanelSlot::PaletteSwatch), 1);
        let outfit = panels.iter().find(|p| p.slot == PanelSlot::Outfit).unwrap();
        assert_eq!(
            (outfit.rect.x, outfit.rect.y, outfit.rect.w, outfit.rect.h),
            (0, 1120, 256, 384)
        );
        let view_labels: Vec<&str> = panels
            .iter()
            .filter(|p| p.slot == PanelSlot::View)
            .map(|p| p.label.as_str())
            .collect();
        assert_eq!(
            view_labels,
            ["front", "side-left", "three-quarter", "side-right", "back"]
        );
    }

    #[test]
    fn item_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let s = structure(&lib, "pixhaus.builtin.structure.item");
        assert_eq!(
            canvas(s),
            Dimensions {
                width: 1024,
                height: 1024
            }
        );
        assert_eq!(count_slot(s, PanelSlot::View), 4);
        assert_eq!(count_slot(s, PanelSlot::Callout), 2);
        assert_eq!(count_slot(s, PanelSlot::Expression), 0);
        assert_eq!(count_slot(s, PanelSlot::PaletteSwatch), 1);
    }

    #[test]
    fn tileset_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let s = structure(&lib, "pixhaus.builtin.structure.tileset");
        assert_eq!(
            canvas(s),
            Dimensions {
                width: 1024,
                height: 1024
            }
        );
        assert_eq!(count_slot(s, PanelSlot::View), 3);
        assert_eq!(count_slot(s, PanelSlot::PaletteSwatch), 1);
        let StructureOutput::Paneled { panels, .. } = &s.output else {
            panic!()
        };
        let view_labels: Vec<&str> = panels
            .iter()
            .filter(|p| p.slot == PanelSlot::View)
            .map(|p| p.label.as_str())
            .collect();
        assert_eq!(
            view_labels,
            ["tile-primitives", "transition-variants", "autotile-preview"]
        );
    }

    #[test]
    fn custom_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let s = structure(&lib, "pixhaus.builtin.structure.custom");
        assert_eq!(
            canvas(s),
            Dimensions {
                width: 1024,
                height: 1024
            }
        );
        assert_eq!(count_slot(s, PanelSlot::View), 1);
        assert_eq!(count_slot(s, PanelSlot::PaletteSwatch), 1);
    }

    #[test]
    fn character_migration_preserves_all_layout_phrases() {
        let lib = BuiltinLibrary::load();
        let positive = compose_layout(&lib, "pixhaus.builtin.structure.character");
        for phrase in [
            "each 200 pixels wide, 480 pixels tall",
            "each 256 pixels wide, 192 pixels tall",
            "1024 pixels wide, 128 pixels tall",
            "each 512 pixels wide, 320 pixels tall",
            "256 pixels wide, 384 pixels tall",
            "Professional sprite sheet format",
        ] {
            assert!(positive.contains(phrase), "missing: {phrase}");
        }
    }

    #[test]
    fn item_migration_preserves_all_layout_phrases() {
        let lib = BuiltinLibrary::load();
        let positive = compose_layout(&lib, "pixhaus.builtin.structure.item");
        for phrase in ["512×384", "1024×128 pixels", "512×128 each"] {
            assert!(positive.contains(phrase), "missing: {phrase}");
        }
    }

    #[test]
    fn tileset_migration_preserves_all_layout_phrases() {
        let lib = BuiltinLibrary::load();
        let positive = compose_layout(&lib, "pixhaus.builtin.structure.tileset");
        for phrase in ["1024×256", "1024×384", "1024×128", "autotile"] {
            assert!(positive.contains(phrase), "missing: {phrase}");
        }
    }

    #[test]
    fn custom_migration_preserves_all_layout_phrases() {
        let lib = BuiltinLibrary::load();
        let positive = compose_layout(&lib, "pixhaus.builtin.structure.custom");
        for phrase in ["1024×896 area", "1024×128 pixels"] {
            assert!(positive.contains(phrase), "missing: {phrase}");
        }
    }

    #[test]
    fn migration_embeds_subject_via_inline_text() {
        let lib = BuiltinLibrary::load();
        let empty = BTreeMap::new();
        let req = ComposeRequest {
            baseline: "pixel art character model sheet",
            structure: structure(&lib, "pixhaus.builtin.structure.character"),
            style: None,
            prompt: None,
            variable_values: &empty,
            entity_info: &empty,
            inline_text: "GOLDEN_SUBJECT",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        assert!(out.positive.starts_with("pixel art character model sheet"));
        assert!(out.positive.contains("GOLDEN_SUBJECT"));
    }

    #[test]
    fn character_negatives_combine_with_default_style() {
        let lib = BuiltinLibrary::load();
        let style = &lib.styles[&StyleId(STYLE_DEFAULT_ID.into())];
        let empty = BTreeMap::new();
        let req = ComposeRequest {
            baseline: "",
            structure: structure(&lib, "pixhaus.builtin.structure.character"),
            style: Some(style),
            prompt: None,
            variable_values: &empty,
            entity_info: &empty,
            inline_text: "",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        // Legacy character negative, recombined from look + layout negatives.
        assert!(out.negative.contains("blurry, low quality, watermark"));
        assert!(
            out.negative
                .contains("overlapping views, inconsistent scale")
        );
    }
}
