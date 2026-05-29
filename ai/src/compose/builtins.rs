//! Built-in composition records. Source of truth migrated from the former
//! `reference_sheet::templates` module. Per spec section 8.
//!
//! The four legacy `CompositionTemplate` variants become built-in
//! `Structure`s; the negative-prompt clauses shared by all four move to a
//! single built-in Default `Style`. Migration-equivalence tests assert the
//! resolver reproduces the legacy layout prose, dimensions, and negatives.

use std::collections::BTreeMap;

use pixhaus_core::project::library::composition::{
    ArtStyleKind, Dimensions, PanelRect, PanelSlot, PromptId, PromptTemplate, PromptVariable, Structure, StructureId, StructureOutput, StructurePanel,
    Style, StyleId, VarControl,
};

/// Default cascading baseline used when a project sets no `style_notes` and no
/// art style is resolved. Equal to the pixel-art baseline, since pixel art is
/// the default kind; prefer [`baseline_for`] when a kind is known.
pub const BUILTIN_DEFAULT_BASELINE: &str = "pixel art reference sheet";

/// The cascading prompt baseline for an art-style kind. Replaces the single
/// hardcoded pixel-art baseline so a non-pixel style no longer prefixes
/// "pixel art" onto every prompt. Pure; the default kind returns
/// [`BUILTIN_DEFAULT_BASELINE`].
#[must_use]
pub fn baseline_for(kind: ArtStyleKind) -> &'static str {
    match kind {
        ArtStyleKind::PixelArt => BUILTIN_DEFAULT_BASELINE,
        ArtStyleKind::RetroPixel => "retro pixel art reference sheet",
        ArtStyleKind::PixelInspired => "pixel-inspired reference sheet",
        ArtStyleKind::CleanHd => "high-detail reference sheet",
        ArtStyleKind::MapStyle => "top-down map reference sheet",
    }
}

/// Id of the built-in Default Style carrying the shared look negatives.
pub const STYLE_DEFAULT_ID: &str = "pixhaus.builtin.style.default";

/// Id of the built-in pixel-art Style — the default look, carrying the pixel
/// adjectives stripped from structure prose.
pub const STYLE_PIXEL_ART_ID: &str = "pixhaus.builtin.style.pixel_art";

/// Id of the built-in retro-pixel Style.
pub const STYLE_RETRO_PIXEL_ID: &str = "pixhaus.builtin.style.retro_pixel";

/// Id of the built-in pixel-inspired Style.
pub const STYLE_PIXEL_INSPIRED_ID: &str = "pixhaus.builtin.style.pixel_inspired";

/// Id of the built-in clean-HD Style.
pub const STYLE_CLEAN_HD_ID: &str = "pixhaus.builtin.style.clean_hd";

/// Id of the built-in map-style Style.
pub const STYLE_MAP_STYLE_ID: &str = "pixhaus.builtin.style.map_style";

/// Id of the built-in free-form Single structure — one image, no panels. The
/// cockpit's default output so the artist is never forced into a paneled sheet
/// to make one image.
pub const STRUCTURE_SINGLE_ID: &str = "pixhaus.builtin.structure.single";

/// The read-only registry of built-in composition records, loaded once at
/// startup. Project-tier records shadow these by id.
pub struct BuiltinLibrary {
    /// Built-in Structures keyed by id.
    pub structures: BTreeMap<StructureId, Structure>,
    /// Built-in Styles keyed by id.
    pub styles: BTreeMap<StyleId, Style>,
    /// Built-in example Prompts keyed by id (Bit, the mascot, and its world).
    pub prompts: BTreeMap<PromptId, PromptTemplate>,
}

impl BuiltinLibrary {
    /// Constructs the built-in records. Pure — no I/O, cannot fail.
    #[must_use]
    pub fn load() -> Self {
        let mut structures = BTreeMap::new();
        for s in [single(), character(), item(), tileset(), custom()] {
            structures.insert(s.id.clone(), s);
        }
        let mut styles = BTreeMap::new();
        let def = default_style();
        styles.insert(def.id.clone(), def);
        for s in art_styles() {
            styles.insert(s.id.clone(), s);
        }
        let mut prompts = BTreeMap::new();
        for p in example_prompts() {
            prompts.insert(p.id.clone(), p);
        }
        Self { structures, styles, prompts }
    }
}

/// Containment + identity-scale-lock + no-border + no-text rules appended once
/// to the lead prose fragment of every paneled built-in structure. Universal —
/// applies to every art style, pixel or clean-HD — so it lives in the structure,
/// not in any Style. Carries no `{var}` placeholders, so appending it before
/// substitution leaves the lead fragment's `{panel_w}`/`{panel_h}` clause intact.
const PANEL_DISCIPLINE: &str = "keep the entire subject inside its own cell with \
    equal empty margin on all four sides, never letting limbs, weapons, or effects \
    cross into a neighbouring cell; render every cell at an identical bounding box \
    and identical pixel scale; cells are separated by background only — no drawn \
    cell borders, dividing lines, gridlines, or gutters; no text, labels, captions, \
    numbers, or watermarks anywhere on the sheet";

/// Structure-level negatives carrying the same rules, so they hold even when no
/// Style is picked — compose folds `look_negatives` only when a style is `Some`,
/// so the no-border/no-text rule must live on the structure to always apply.
const PANEL_NEG: &str = "drawn cell borders, dividing lines, gridlines, gutters, \
    panel frames, text, labels, captions, page numbers, subject bleeding across \
    cell edges, inconsistent cell scale";

/// Appends the shared [`PANEL_DISCIPLINE`] clause to a structure's lead prose
/// fragment. Compose joins fragments with ". ", so appending to the lead emits
/// the clause exactly once per sheet.
fn lead(base: &str) -> String {
    format!("{base} {PANEL_DISCIPLINE}")
}

/// Appends the shared [`PANEL_NEG`] rule to a structure's domain `layout_negatives`,
/// joining with ", " and dropping a leading separator when the domain part is empty
/// (so `custom()`'s formerly-empty negatives do not start with a bare comma).
fn with_panel_neg(domain: &str) -> String {
    let domain = domain.trim();
    if domain.is_empty() {
        PANEL_NEG.to_owned()
    } else {
        format!("{domain}, {PANEL_NEG}")
    }
}

fn panel(label: &str, x: u32, y: u32, w: u32, h: u32, slot: PanelSlot, prose: &str) -> StructurePanel {
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
    // Append the universal panel-discipline clause to the lead fragment so it
    // rides the sheet exactly once.
    if let Some(first) = panels.first_mut() {
        first.prose_fragment = lead(&first.prose_fragment);
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
        panels.push(panel(label, u32::try_from(i).unwrap_or(0) * 256, 480, 256, 192, PanelSlot::Expression, prose));
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
        panels.push(panel(label, u32::try_from(i).unwrap_or(0) * 512, 800, 512, 320, PanelSlot::Callout, prose));
    }
    panels.push(panel(
        "outfit-variant",
        0,
        1120,
        256,
        384,
        PanelSlot::Outfit,
        "one outfit-variant panel, {panel_w} pixels wide, {panel_h} pixels tall, \
         showing an alternate outfit or colour scheme. White background, \
         consistent scale across all views. Professional \
         sprite sheet format",
    ));
    Structure {
        id: StructureId("pixhaus.builtin.structure.character".into()),
        name: "Character".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions { width: 1024, height: 1536 },
            panels,
        },
        layout_negatives: with_panel_neg("extra limbs, bad anatomy, duplicate characters, overlapping views, inconsistent scale"),
    }
}

fn item() -> Structure {
    let mut panels = Vec::new();
    let views = [("front", 0, 0), ("side-left", 512, 0), ("back", 0, 384), ("side-right", 512, 384)];
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
        panels.push(panel(label, u32::try_from(i).unwrap_or(0) * 512, 896, 512, 128, PanelSlot::Callout, prose));
    }
    if let Some(first) = panels.first_mut() {
        first.prose_fragment = lead(&first.prose_fragment);
    }
    Structure {
        id: StructureId("pixhaus.builtin.structure.item".into()),
        name: "Item".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions { width: 1024, height: 1024 },
            panels,
        },
        layout_negatives: with_panel_neg("floating elements, inconsistent scale across views"),
    }
}

fn tileset() -> Structure {
    // Per-band pixel dimensions interpolated from each rect, matching the
    // legacy tileset prompt's "(1024×256)" framing.
    let mut panels = vec![
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
             grid-aligned, consistent tile size throughout",
        ),
    ];
    if let Some(first) = panels.first_mut() {
        first.prose_fragment = lead(&first.prose_fragment);
    }
    Structure {
        id: StructureId("pixhaus.builtin.structure.tileset".into()),
        name: "Tileset".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions { width: 1024, height: 1024 },
            panels,
        },
        layout_negatives: with_panel_neg("non-grid-aligned tiles, broken patterns, inconsistent tile size"),
    }
}

fn custom() -> Structure {
    let mut panels = vec![
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
    if let Some(first) = panels.first_mut() {
        first.prose_fragment = lead(&first.prose_fragment);
    }
    Structure {
        id: StructureId("pixhaus.builtin.structure.custom".into()),
        name: "Custom".into(),
        output: StructureOutput::Paneled {
            canvas: Dimensions { width: 1024, height: 1024 },
            panels,
        },
        // custom() previously had empty negatives; the shared rule supplies them.
        layout_negatives: with_panel_neg(""),
    }
}

fn single() -> Structure {
    // Free-composition output: no panels, so the resolver emits no layout prose
    // and the verb falls back to its default 1024² canvas. The artist's subject
    // text and the project baseline carry the whole prompt.
    Structure {
        id: StructureId(STRUCTURE_SINGLE_ID.into()),
        name: "Single image".into(),
        output: StructureOutput::Single,
        layout_negatives: String::new(),
    }
}

/// Look negatives shared by every built-in Style — the legacy default-Style
/// negatives. Photo/3d negatives only fit pixel-class looks, so the clean-HD
/// Style trims them rather than fighting its own prompt.
const SHARED_LOOK_NEGATIVES: &str = "blurry, low quality, watermark, text label, logo, cropped, photo realistic, 3d render";

fn default_style() -> Style {
    Style {
        id: StyleId(STYLE_DEFAULT_ID.into()),
        name: "Default".into(),
        kind: ArtStyleKind::PixelArt,
        modifiers: String::new(),
        look_negatives: SHARED_LOOK_NEGATIVES.into(),
        model_pref: None,
        quality: None,
    }
}

/// The art-style taxonomy as built-in Styles, one per [`ArtStyleKind`].
///
/// Each carries its kind, look-appropriate modifiers, and look negatives, with
/// a distinct sortable name so it surfaces in the picker (dedup is by id; two
/// styles sharing a name would sort unstably). The pixel-art Style holds the
/// pixel adjectives stripped from the style-agnostic structure prose, so the
/// pixel look appears only when a pixel-class Style is selected.
fn art_styles() -> Vec<Style> {
    vec![
        Style {
            id: StyleId(STYLE_PIXEL_ART_ID.into()),
            name: "Pixel art".into(),
            kind: ArtStyleKind::PixelArt,
            modifiers: "clean pixel art, crisp pixel-art lines, limited palette".into(),
            look_negatives: SHARED_LOOK_NEGATIVES.into(),
            model_pref: None,
            quality: None,
        },
        Style {
            id: StyleId(STYLE_RETRO_PIXEL_ID.into()),
            name: "Retro pixel".into(),
            kind: ArtStyleKind::RetroPixel,
            modifiers: "lo-fi retro pixel art, chunky pixels, tight 8-bit palette".into(),
            look_negatives: SHARED_LOOK_NEGATIVES.into(),
            model_pref: None,
            quality: None,
        },
        Style {
            id: StyleId(STYLE_PIXEL_INSPIRED_ID.into()),
            name: "Pixel inspired".into(),
            kind: ArtStyleKind::PixelInspired,
            modifiers: "pixel-inspired art, hand-drawn shading with a pixel feel".into(),
            look_negatives: SHARED_LOOK_NEGATIVES.into(),
            model_pref: None,
            quality: None,
        },
        Style {
            id: StyleId(STYLE_CLEAN_HD_ID.into()),
            name: "Clean HD".into(),
            kind: ArtStyleKind::CleanHd,
            modifiers: "clean high-detail rendering, smooth shading, crisp edges".into(),
            look_negatives: "blurry, low quality, watermark, text label, logo, cropped".into(),
            model_pref: None,
            quality: None,
        },
        Style {
            id: StyleId(STYLE_MAP_STYLE_ID.into()),
            name: "Map style".into(),
            kind: ArtStyleKind::MapStyle,
            modifiers: "top-down map art, seamless tiling, even lighting".into(),
            look_negatives: SHARED_LOOK_NEGATIVES.into(),
            model_pref: None,
            quality: None,
        },
    ]
}

/// Built-in example prompts featuring Bit, the Pixhaus mascot, and its world.
///
/// These seed the prompt picker so a fresh project has worked examples to start
/// from and a default character to run through the reference-sheet → animation
/// pipeline. Each points at the built-in structure that frames it best. Plain
/// text, no variables — they double as copy a new user can read and tweak.
fn example_prompts() -> Vec<PromptTemplate> {
    let character = StructureId("pixhaus.builtin.structure.character".into());
    let item = StructureId("pixhaus.builtin.structure.item".into());
    let tileset = StructureId("pixhaus.builtin.structure.tileset".into());
    vec![
        PromptTemplate {
            id: PromptId("pixhaus.builtin.prompt.bit".into()),
            name: "Bit — mascot".into(),
            text: "Bit, the Pixhaus mascot — a small retro robot with a boxy CRT/floppy-disk \
                   head, a glowing pixel-face screen showing its expression, a stubby antenna \
                   with a blinking pixel, chunky rounded limbs, friendly proportions, crisp \
                   8-bit palette."
                .into(),
            variables: Vec::new(),
            default_style: None,
            default_structure: Some(character.clone()),
        },
        PromptTemplate {
            id: PromptId("pixhaus.builtin.prompt.byte".into()),
            name: "Byte — companion bot".into(),
            text: "Byte, Bit's companion — a small floating drone bot with a single round \
                   glowing lens-eye, a little spinning propeller on top, slim arms, the same \
                   crisp 8-bit palette as Bit."
                .into(),
            variables: Vec::new(),
            default_style: None,
            default_structure: Some(character),
        },
        PromptTemplate {
            id: PromptId("pixhaus.builtin.prompt.floppy".into()),
            name: "Floppy — power-up".into(),
            text: "A retro floppy-disk power-up from Bit's world — a chunky 3.5-inch floppy \
                   disk with a glowing label and a pixel shine, 8-bit palette."
                .into(),
            variables: Vec::new(),
            default_style: None,
            default_structure: Some(item),
        },
        PromptTemplate {
            id: PromptId("pixhaus.builtin.prompt.circuit_tiles".into()),
            name: "Circuit-grid — tileset".into(),
            text: "A top-down circuit-board floor tileset for Bit's world — blueprint-grid \
                   lines, solder traces, glowing node junctions, seamless edges, 8-bit palette."
                .into(),
            variables: Vec::new(),
            default_style: None,
            default_structure: Some(tileset),
        },
        // A parametric example whose variables exercise every typed control, so
        // the cockpit's creative dials have something real to drive out of the box.
        PromptTemplate {
            id: PromptId("pixhaus.builtin.prompt.creature".into()),
            name: "Creature — parametric".into(),
            text: "a {size} {species} creature, {mood}, in a {palette} colour palette, crisp pixel art".into(),
            variables: vec![
                PromptVariable {
                    key: "species".into(),
                    label: "Species".into(),
                    default: "slime".into(),
                    control: VarControl::Select {
                        choices: ["slime", "golem", "wisp", "beetle", "fox"].iter().map(|s| (*s).to_owned()).collect(),
                    },
                },
                PromptVariable {
                    key: "size".into(),
                    label: "Size".into(),
                    default: "small".into(),
                    control: VarControl::Select {
                        choices: ["tiny", "small", "hulking"].iter().map(|s| (*s).to_owned()).collect(),
                    },
                },
                PromptVariable {
                    key: "mood".into(),
                    label: "Mood".into(),
                    default: "curious".into(),
                    control: VarControl::Text,
                },
                PromptVariable {
                    key: "palette".into(),
                    label: "Palette".into(),
                    default: "emerald".into(),
                    control: VarControl::Wildcard {
                        choices: ["emerald", "ember", "cobalt", "amethyst", "bone"].iter().map(|s| (*s).to_owned()).collect(),
                    },
                },
            ],
            default_style: None,
            default_structure: Some(StructureId(STRUCTURE_SINGLE_ID.into())),
        },
    ]
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

    /// Composes with no style picked, so the negative folds the structure's
    /// `layout_negatives` only — proving the panel-discipline rule holds without
    /// a Style, since compose folds `look_negatives` only when `style` is `Some`.
    fn compose_negative_no_style(lib: &BuiltinLibrary, id: &str) -> String {
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
        compose(&req).unwrap().negative
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
    fn loads_structures_and_default_style() {
        let lib = BuiltinLibrary::load();
        // Single (free-form) plus the four paneled structures.
        assert_eq!(lib.structures.len(), 5);
        assert!(lib.structures.contains_key(&StructureId(STRUCTURE_SINGLE_ID.into())));
        let single = &lib.structures[&StructureId(STRUCTURE_SINGLE_ID.into())];
        assert!(matches!(single.output, StructureOutput::Single), "single structure has no panels");
        assert!(lib.styles.contains_key(&StyleId(STYLE_DEFAULT_ID.into())));
        // One built-in Style per art-style kind so every look surfaces in the picker.
        for id in [
            STYLE_PIXEL_ART_ID,
            STYLE_RETRO_PIXEL_ID,
            STYLE_PIXEL_INSPIRED_ID,
            STYLE_CLEAN_HD_ID,
            STYLE_MAP_STYLE_ID,
        ] {
            assert!(lib.styles.contains_key(&StyleId(id.into())), "missing built-in style {id}");
        }
        // Seeded example prompts — Bit's world plus one parametric example.
        assert_eq!(lib.prompts.len(), 5);
        for id in [
            "pixhaus.builtin.prompt.bit",
            "pixhaus.builtin.prompt.byte",
            "pixhaus.builtin.prompt.floppy",
            "pixhaus.builtin.prompt.circuit_tiles",
            "pixhaus.builtin.prompt.creature",
        ] {
            assert!(lib.prompts.contains_key(&PromptId(id.into())));
        }
        // The parametric example exercises typed controls (Select + Wildcard).
        let creature = &lib.prompts[&PromptId("pixhaus.builtin.prompt.creature".into())];
        assert_eq!(creature.variables.len(), 4);
        assert!(creature.variables.iter().any(|v| matches!(v.control, VarControl::Wildcard { .. })));
        // Every example points at a real built-in structure.
        for p in lib.prompts.values() {
            let s = p.default_structure.as_ref().expect("example prompt names a structure");
            assert!(lib.structures.contains_key(s));
        }
    }

    #[test]
    fn character_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let c = structure(&lib, "pixhaus.builtin.structure.character");
        assert_eq!(canvas(c), Dimensions { width: 1024, height: 1536 });
        // 5 views + 3 expressions + 1 palette + 2 callouts + 1 outfit = 12 panels.
        let StructureOutput::Paneled { panels, .. } = &c.output else { panic!() };
        assert_eq!(panels.len(), 12);
        assert_eq!(count_slot(c, PanelSlot::View), 5);
        assert_eq!(count_slot(c, PanelSlot::Expression), 3);
        assert_eq!(count_slot(c, PanelSlot::Callout), 2);
        assert_eq!(count_slot(c, PanelSlot::Outfit), 1);
        assert_eq!(count_slot(c, PanelSlot::PaletteSwatch), 1);
        let outfit = panels.iter().find(|p| p.slot == PanelSlot::Outfit).unwrap();
        assert_eq!((outfit.rect.x, outfit.rect.y, outfit.rect.w, outfit.rect.h), (0, 1120, 256, 384));
        let view_labels: Vec<&str> = panels.iter().filter(|p| p.slot == PanelSlot::View).map(|p| p.label.as_str()).collect();
        assert_eq!(view_labels, ["front", "side-left", "three-quarter", "side-right", "back"]);
    }

    #[test]
    fn item_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let s = structure(&lib, "pixhaus.builtin.structure.item");
        assert_eq!(canvas(s), Dimensions { width: 1024, height: 1024 });
        assert_eq!(count_slot(s, PanelSlot::View), 4);
        assert_eq!(count_slot(s, PanelSlot::Callout), 2);
        assert_eq!(count_slot(s, PanelSlot::Expression), 0);
        assert_eq!(count_slot(s, PanelSlot::PaletteSwatch), 1);
    }

    #[test]
    fn tileset_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let s = structure(&lib, "pixhaus.builtin.structure.tileset");
        assert_eq!(canvas(s), Dimensions { width: 1024, height: 1024 });
        assert_eq!(count_slot(s, PanelSlot::View), 3);
        assert_eq!(count_slot(s, PanelSlot::PaletteSwatch), 1);
        let StructureOutput::Paneled { panels, .. } = &s.output else { panic!() };
        let view_labels: Vec<&str> = panels.iter().filter(|p| p.slot == PanelSlot::View).map(|p| p.label.as_str()).collect();
        assert_eq!(view_labels, ["tile-primitives", "transition-variants", "autotile-preview"]);
    }

    #[test]
    fn custom_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let s = structure(&lib, "pixhaus.builtin.structure.custom");
        assert_eq!(canvas(s), Dimensions { width: 1024, height: 1024 });
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

    mod baseline_for {
        use super::*;
        use rstest::rstest;

        #[rstest]
        #[case::pixel_art(ArtStyleKind::PixelArt, "pixel art reference sheet")]
        #[case::retro_pixel(ArtStyleKind::RetroPixel, "retro pixel art reference sheet")]
        #[case::pixel_inspired(ArtStyleKind::PixelInspired, "pixel-inspired reference sheet")]
        #[case::clean_hd(ArtStyleKind::CleanHd, "high-detail reference sheet")]
        #[case::map_style(ArtStyleKind::MapStyle, "top-down map reference sheet")]
        fn returns_kind_specific_baseline(#[case] kind: ArtStyleKind, #[case] expected: &str) {
            assert_eq!(super::super::baseline_for(kind), expected);
        }

        #[test]
        fn pixel_art_baseline_matches_the_legacy_constant() {
            // The legacy default baseline was the pixel-art string; the default
            // kind must still produce it so unstyled generation is unchanged.
            assert_eq!(super::super::baseline_for(ArtStyleKind::default()), BUILTIN_DEFAULT_BASELINE);
        }
    }

    #[test]
    fn art_styles_have_distinct_sortable_names() {
        let lib = BuiltinLibrary::load();
        let names: Vec<&str> = lib.styles.values().map(|s| s.name.as_str()).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "built-in style names must be distinct: {names:?}");
    }

    #[test]
    fn art_styles_carry_their_kind() {
        let lib = BuiltinLibrary::load();
        for (id, kind) in [
            (STYLE_PIXEL_ART_ID, ArtStyleKind::PixelArt),
            (STYLE_RETRO_PIXEL_ID, ArtStyleKind::RetroPixel),
            (STYLE_PIXEL_INSPIRED_ID, ArtStyleKind::PixelInspired),
            (STYLE_CLEAN_HD_ID, ArtStyleKind::CleanHd),
            (STYLE_MAP_STYLE_ID, ArtStyleKind::MapStyle),
        ] {
            let style = &lib.styles[&StyleId(id.into())];
            assert_eq!(style.kind, kind, "style {id} must carry kind {kind:?}");
        }
    }

    #[test]
    fn pixel_art_style_modifiers_carry_the_pixel_adjectives() {
        // The pixel adjectives stripped from structure prose live in the
        // pixel-art Style's modifiers so they appear only when it is selected.
        let lib = BuiltinLibrary::load();
        let style = &lib.styles[&StyleId(STYLE_PIXEL_ART_ID.into())];
        assert!(style.modifiers.contains("pixel art"), "pixel-art modifiers must carry the pixel adjective: {}", style.modifiers);
    }

    #[test]
    fn clean_hd_style_modifiers_are_not_pixel() {
        let lib = BuiltinLibrary::load();
        let style = &lib.styles[&StyleId(STYLE_CLEAN_HD_ID.into())];
        assert!(!style.modifiers.to_lowercase().contains("pixel"), "clean-HD modifiers must not mention pixel: {}", style.modifiers);
    }

    #[test]
    fn character_structure_prose_no_longer_carries_pixel_adjective() {
        // The pixel-art adjective moved from the structure into the pixel-art
        // Style, so the style-agnostic layout prose drops it. The "N pixels
        // wide" dimension unit is a different token and must survive.
        let lib = BuiltinLibrary::load();
        let positive = compose_layout(&lib, "pixhaus.builtin.structure.character");
        assert!(!positive.contains("pixel-art"), "character layout prose must not carry the pixel-art adjective: {positive}");
        assert!(!positive.contains("pixel art"), "character layout prose must not carry the pixel-art adjective: {positive}");
        // The migration-asserted dimension prose still uses the "pixels" unit.
        assert!(positive.contains("200 pixels wide"), "dimension unit prose must survive: {positive}");
    }

    #[test]
    fn tileset_structure_prose_no_longer_carries_pixel_adjective() {
        let lib = BuiltinLibrary::load();
        let positive = compose_layout(&lib, "pixhaus.builtin.structure.tileset");
        assert!(!positive.contains("pixel art"), "tileset layout prose must not carry the pixel-art adjective: {positive}");
        assert!(!positive.contains("pixel-art"), "tileset layout prose must not carry the pixel-art adjective: {positive}");
    }

    #[test]
    fn character_layout_prose_snapshot() {
        let lib = BuiltinLibrary::load();
        insta::assert_snapshot!(compose_layout(&lib, "pixhaus.builtin.structure.character"));
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
        assert!(out.negative.contains("overlapping views, inconsistent scale"));
    }

    // ── Panel discipline: containment + no-border/no-text + identity/scale-lock ──

    mod panel_discipline {
        use super::*;
        use rstest::rstest;

        const CHARACTER: &str = "pixhaus.builtin.structure.character";
        const ITEM: &str = "pixhaus.builtin.structure.item";
        const TILESET: &str = "pixhaus.builtin.structure.tileset";
        const CUSTOM: &str = "pixhaus.builtin.structure.custom";

        /// Every paneled built-in carries the shared containment + scale-lock
        /// clause in its positive prose, so a downstream fixed-rect crop lands on
        /// a contained, identically-scaled subject.
        #[rstest]
        #[case::character(CHARACTER)]
        #[case::item(ITEM)]
        #[case::tileset(TILESET)]
        #[case::custom(CUSTOM)]
        fn paneled_structures_emit_containment_clause(#[case] id: &str) {
            let lib = BuiltinLibrary::load();
            let positive = compose_layout(&lib, id);
            assert!(positive.contains("equal empty margin on all four sides"), "{id} missing containment margin clause: {positive}");
            assert!(
                positive.contains("identical bounding box and identical pixel scale"),
                "{id} missing identity/scale-lock clause: {positive}"
            );
            assert!(positive.contains("no drawn cell borders"), "{id} missing no-border clause: {positive}");
        }

        /// The clause rides on the lead fragment, and compose joins fragments
        /// with ". ", so it appears once per sheet — not once per panel.
        #[rstest]
        #[case::character(CHARACTER)]
        #[case::item(ITEM)]
        #[case::tileset(TILESET)]
        #[case::custom(CUSTOM)]
        fn containment_clause_appears_exactly_once(#[case] id: &str) {
            let lib = BuiltinLibrary::load();
            let positive = compose_layout(&lib, id);
            let count = positive.matches("equal empty margin on all four sides").count();
            assert_eq!(count, 1, "{id} must carry the containment clause exactly once, got {count}: {positive}");
        }

        /// Free composition is excluded: a single-image structure must not be
        /// over-locked with per-cell containment prose.
        #[test]
        fn single_structure_has_no_containment_clause() {
            let lib = BuiltinLibrary::load();
            let positive = compose_layout(&lib, STRUCTURE_SINGLE_ID);
            assert!(
                !positive.contains("equal empty margin on all four sides"),
                "single (free-form) structure must not carry the containment clause: {positive}"
            );
        }

        /// The negatives carry the no-border / no-text rule even with no Style
        /// picked, because compose folds `look_negatives` only when `style` is
        /// `Some` — the rule must live on the structure to always hold.
        #[rstest]
        #[case::character(CHARACTER)]
        #[case::item(ITEM)]
        #[case::tileset(TILESET)]
        #[case::custom(CUSTOM)]
        fn paneled_structures_negatives_forbid_borders_and_text(#[case] id: &str) {
            let lib = BuiltinLibrary::load();
            let negative = compose_negative_no_style(&lib, id);
            assert!(negative.contains("drawn cell borders"), "{id} negatives must forbid drawn cell borders: {negative}");
            assert!(negative.contains("text, labels"), "{id} negatives must forbid text/labels: {negative}");
        }

        /// The shared negative is appended to each structure's domain negatives,
        /// never replacing them — additive, so the migration tests stay green.
        #[test]
        fn character_negatives_keep_domain_phrase() {
            let lib = BuiltinLibrary::load();
            let negative = compose_negative_no_style(&lib, CHARACTER);
            assert!(negative.contains("overlapping views, inconsistent scale"), "character domain negatives must survive: {negative}");
            assert!(negative.contains("drawn cell borders"), "shared panel negatives must be present: {negative}");
        }

        /// Custom had empty domain negatives, so the shared rule must not leave
        /// a leading bare comma after concatenation.
        #[test]
        fn custom_negatives_do_not_start_with_a_bare_comma() {
            let lib = BuiltinLibrary::load();
            let negative = compose_negative_no_style(&lib, CUSTOM);
            assert!(!negative.starts_with(','), "negatives must not start with a bare comma: {negative}");
            assert!(negative.contains("drawn cell borders"), "custom must still carry the shared panel negatives: {negative}");
        }

        /// Single keeps empty `layout_negatives`; with no style its negative is
        /// empty, so the discipline rule does not leak onto free composition.
        #[test]
        fn single_structure_negatives_stay_empty() {
            let lib = BuiltinLibrary::load();
            let negative = compose_negative_no_style(&lib, STRUCTURE_SINGLE_ID);
            assert!(negative.is_empty(), "single (free-form) negatives must stay empty: {negative}");
        }
    }

    #[test]
    fn item_layout_prose_snapshot() {
        let lib = BuiltinLibrary::load();
        insta::assert_snapshot!(compose_layout(&lib, "pixhaus.builtin.structure.item"));
    }

    #[test]
    fn tileset_layout_prose_snapshot() {
        let lib = BuiltinLibrary::load();
        insta::assert_snapshot!(compose_layout(&lib, "pixhaus.builtin.structure.tileset"));
    }

    #[test]
    fn custom_layout_prose_snapshot() {
        let lib = BuiltinLibrary::load();
        insta::assert_snapshot!(compose_layout(&lib, "pixhaus.builtin.structure.custom"));
    }
}
