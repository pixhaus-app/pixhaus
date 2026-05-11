//! Composition templates for the generate-reference-sheet verb.
//!
//! Each template defines expected sheet dimensions, structured prompt
//! engineering that augments the user's prompt, and the panel layout
//! (`SheetComposition`) written into every generated variant. Panel
//! rectangles are *intended* positions based on what the prompt
//! engineering asks the backend to produce — the actual placement
//! depends on the backend and model. B10.2 (iterate-reference-sheet)
//! enables panel-scoped correction after generation.

use pixhaus_core::project::{Rect, SheetComposition, SheetPanel};
use serde::{Deserialize, Serialize};

/// Four composition templates for reference sheet generation.
///
/// The template selection determines the sheet's panel layout, expected
/// dimensions, and the backend prompt engineering layer. Choosing the
/// right template ensures the backend produces a usable multi-panel
/// layout rather than a single free-composition image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionTemplate {
    /// Multi-view character model sheet.
    ///
    /// Five turnaround views (front, side-left, three-quarter, side-right,
    /// back), three facial expressions (neutral, happy, angry), a palette
    /// swatch row, and two detail callout slots. Layout: vertical strips,
    /// 1024×1536 px.
    Character,

    /// Multi-angle item or prop sheet.
    ///
    /// Four-angle turnaround (front, side-left, back, side-right), two
    /// detail callout panels, and a palette swatch. Layout: 2×2 view grid
    /// above a swatch row, 1024×1024 px.
    Item,

    /// Tileset primitive sheet.
    ///
    /// Tile primitives row, transition variants band, autotile preview
    /// block, and a palette swatch. 1024×1024 px.
    Tileset,

    /// Single full-body image with palette swatch.
    ///
    /// Simplest template: one centred orthographic view plus a palette
    /// swatch at the bottom. Falls back when no other template fits or
    /// when the user wants maximum compositional freedom. 1024×1024 px.
    Custom,
}

impl CompositionTemplate {
    /// Output sheet width in pixels.
    #[must_use]
    #[allow(clippy::unused_self)]
    pub const fn sheet_width(&self) -> u32 {
        1024
    }

    /// Output sheet height in pixels.
    #[must_use]
    pub const fn sheet_height(&self) -> u32 {
        match self {
            Self::Character => 1536,
            Self::Item | Self::Tileset | Self::Custom => 1024,
        }
    }

    /// Augments the user's prompt with layout instructions for this template.
    ///
    /// The return value instructs the backend where to place each panel so
    /// panel detection produces reliable coordinates. The user's description
    /// anchors the subject and style; the template text anchors the
    /// composition.
    #[must_use]
    pub fn build_prompt(&self, user_prompt: &str) -> String {
        match self {
            Self::Character => format!(
                "pixel art character model sheet, {user_prompt}. \
                 Layout: five turnaround views in a horizontal strip across \
                 the top — front view, left side, three-quarter view, right \
                 side, back view, each 200 pixels wide, 480 pixels tall. \
                 Below: three facial expression close-ups side by side — \
                 neutral, happy, angry — each 256 pixels wide, 192 pixels tall. \
                 Below that: a horizontal palette swatch row showing all \
                 colours used, 1024 pixels wide, 128 pixels tall. \
                 Bottom section: two detail callout panels side by side, \
                 each 512 pixels wide. \
                 White background, clean pixel-art lines, consistent scale \
                 across all views. Professional sprite sheet format."
            ),
            Self::Item => format!(
                "pixel art item reference sheet, {user_prompt}. \
                 Layout: 2×2 grid of orthographic views — top-left is front \
                 face (512×384), top-right is left side (512×384), \
                 bottom-left is back face (512×384), bottom-right is right \
                 side (512×384). \
                 Below the grid: a palette swatch row 1024×128 pixels. \
                 Bottom strip: two detail callout panels 512×128 each. \
                 White background, consistent scale across all four views."
            ),
            Self::Tileset => format!(
                "pixel art tileset reference sheet, {user_prompt}. \
                 Layout: top row (1024×256) shows the base tile primitives — \
                 flat tile, corner variants, edge variants, in a grid. \
                 Middle band (1024×384): transition tile variants and edge \
                 blending rules. \
                 Lower block (1024×256): 3×3 autotile preview demonstrating \
                 the autotile rule set. \
                 Bottom strip (1024×128): palette swatch. \
                 White background, grid-aligned, clean pixel art, \
                 consistent tile size throughout."
            ),
            Self::Custom => format!(
                "pixel art reference sheet, {user_prompt}. \
                 Full-body orthographic view centred in a 1024×896 area. \
                 Palette swatch row at the bottom, 1024×128 pixels. \
                 White background."
            ),
        }
    }

    /// Negative prompt additions for diffusion backends.
    ///
    /// The caller concatenates the user's own negative prompt (if any)
    /// after this string. Base clauses shared by all templates cover the
    /// most common failures; each template adds layout-specific exclusions.
    #[must_use]
    pub fn build_negative_prompt(&self) -> &'static str {
        match self {
            Self::Character => {
                "blurry, low quality, watermark, text label, logo, cropped, \
                 photo realistic, 3d render, extra limbs, bad anatomy, \
                 duplicate characters, overlapping views, inconsistent scale"
            }
            Self::Item => {
                "blurry, low quality, watermark, text label, logo, cropped, \
                 photo realistic, 3d render, floating elements, \
                 inconsistent scale across views"
            }
            Self::Tileset => {
                "blurry, low quality, watermark, text label, logo, \
                 photo realistic, 3d render, non-grid-aligned tiles, \
                 broken patterns, inconsistent tile size"
            }
            Self::Custom => {
                "blurry, low quality, watermark, text label, logo, cropped, \
                 photo realistic, 3d render, extra limbs, bad anatomy"
            }
        }
    }

    /// Returns the expected `SheetComposition` panel layout for this template.
    ///
    /// Coordinates are pixel positions within the generated sheet image.
    /// These reflect the layout the prompt engineering requests from the
    /// backend — actual placement may vary slightly. B10.2's panel-scoped
    /// iteration lets users correct misaligned panels without regenerating
    /// the whole sheet.
    #[must_use]
    pub fn composition(&self) -> SheetComposition {
        match self {
            Self::Character => character_composition(),
            Self::Item => item_composition(),
            Self::Tileset => tileset_composition(),
            Self::Custom => custom_composition(),
        }
    }
}

// ── Character layout — 1024×1536 ─────────────────────────────────────────────
//
// y=0..480    : five turnaround views, each 200×480
// y=480..672  : three expression panels, each 256×192
// y=672..800  : palette swatch, 1024×128
// y=800..1120 : two detail callouts, each 512×320
// y=1120..1536: margin / overflow

#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
fn character_composition() -> SheetComposition {
    let view_w = 200u32;
    let view_h = 480u32;
    let views = ["front", "side-left", "three-quarter", "side-right", "back"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| SheetPanel {
            region: Rect::from_xywh((i as u32 * view_w) as i32, 0, view_w, view_h),
            label: label.into(),
        })
        .collect();

    let expr_w = 256u32;
    let expr_h = 192u32;
    let expr_y = view_h as i32;
    let expressions = ["neutral", "happy", "angry"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| SheetPanel {
            region: Rect::from_xywh((i as u32 * expr_w) as i32, expr_y, expr_w, expr_h),
            label: label.into(),
        })
        .collect();

    let swatch_y = (view_h + expr_h) as i32;
    let palette_swatch = Some(Rect::from_xywh(0, swatch_y, 1024, 128));

    let callout_w = 512u32;
    let callout_h = 320u32;
    let callout_y = swatch_y + 128;
    let callouts = ["detail-1", "detail-2"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| SheetPanel {
            region: Rect::from_xywh(
                (i as u32 * callout_w) as i32,
                callout_y,
                callout_w,
                callout_h,
            ),
            label: label.into(),
        })
        .collect();

    SheetComposition {
        views,
        expressions,
        callouts,
        outfits: Vec::new(),
        palette_swatch,
    }
}

// ── Item layout — 1024×1024 ───────────────────────────────────────────────────
//
// y=0..384    : top-left quadrant — front (512×384)
// y=0..384    : top-right quadrant — side-left (512×384)
// y=384..768  : bottom-left quadrant — back (512×384)
// y=384..768  : bottom-right quadrant — side-right (512×384)
// y=768..896  : palette swatch (1024×128)
// y=896..1024 : two detail callouts, each 512×128

#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
fn item_composition() -> SheetComposition {
    let half_w = 512u32;
    let view_h = 384u32;
    let views = [
        ("front", 0i32, 0i32),
        ("side-left", half_w as i32, 0),
        ("back", 0, view_h as i32),
        ("side-right", half_w as i32, view_h as i32),
    ]
    .into_iter()
    .map(|(label, x, y)| SheetPanel {
        region: Rect::from_xywh(x, y, half_w, view_h),
        label: label.into(),
    })
    .collect();

    let palette_swatch = Some(Rect::from_xywh(0, (view_h * 2) as i32, 1024, 128));

    let callout_y = (view_h * 2 + 128) as i32;
    let callouts = ["detail-1", "detail-2"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| SheetPanel {
            region: Rect::from_xywh((i as u32 * half_w) as i32, callout_y, half_w, 128),
            label: label.into(),
        })
        .collect();

    SheetComposition {
        views,
        expressions: Vec::new(),
        callouts,
        outfits: Vec::new(),
        palette_swatch,
    }
}

// ── Tileset layout — 1024×1024 ────────────────────────────────────────────────
//
// y=0..256    : tile primitives row (1024×256)
// y=256..640  : transition variants band (1024×384)
// y=640..896  : autotile preview block (1024×256)
// y=896..1024 : palette swatch (1024×128)

fn tileset_composition() -> SheetComposition {
    let views = vec![
        SheetPanel {
            region: Rect::from_xywh(0, 0, 1024, 256),
            label: "tile-primitives".into(),
        },
        SheetPanel {
            region: Rect::from_xywh(0, 256, 1024, 384),
            label: "transition-variants".into(),
        },
        SheetPanel {
            region: Rect::from_xywh(0, 640, 1024, 256),
            label: "autotile-preview".into(),
        },
    ];

    SheetComposition {
        views,
        expressions: Vec::new(),
        callouts: Vec::new(),
        outfits: Vec::new(),
        palette_swatch: Some(Rect::from_xywh(0, 896, 1024, 128)),
    }
}

// ── Custom layout — 1024×1024 ─────────────────────────────────────────────────
//
// y=0..896    : full-body view (1024×896)
// y=896..1024 : palette swatch (1024×128)

fn custom_composition() -> SheetComposition {
    let views = vec![SheetPanel {
        region: Rect::from_xywh(0, 0, 1024, 896),
        label: "full-body".into(),
    }];

    SheetComposition {
        views,
        expressions: Vec::new(),
        callouts: Vec::new(),
        outfits: Vec::new(),
        palette_swatch: Some(Rect::from_xywh(0, 896, 1024, 128)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_sheet_is_1024x1536() {
        let t = CompositionTemplate::Character;
        assert_eq!(t.sheet_width(), 1024);
        assert_eq!(t.sheet_height(), 1536);
    }

    #[test]
    fn item_tileset_custom_are_1024x1024() {
        for t in [
            CompositionTemplate::Item,
            CompositionTemplate::Tileset,
            CompositionTemplate::Custom,
        ] {
            assert_eq!(t.sheet_width(), 1024);
            assert_eq!(t.sheet_height(), 1024);
        }
    }

    #[test]
    fn character_composition_has_five_views_and_three_expressions() {
        let comp = CompositionTemplate::Character.composition();
        assert_eq!(
            comp.views.len(),
            5,
            "character sheet has five turnaround views"
        );
        assert_eq!(
            comp.expressions.len(),
            3,
            "character sheet has three expressions"
        );
        assert_eq!(comp.callouts.len(), 2, "character sheet has two callouts");
        assert!(
            comp.palette_swatch.is_some(),
            "character sheet has palette swatch"
        );
    }

    #[test]
    fn character_view_labels_are_correct() {
        let comp = CompositionTemplate::Character.composition();
        let labels: Vec<&str> = comp.views.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(
            labels,
            ["front", "side-left", "three-quarter", "side-right", "back"]
        );
    }

    #[test]
    fn character_expression_labels_are_correct() {
        let comp = CompositionTemplate::Character.composition();
        let labels: Vec<&str> = comp.expressions.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, ["neutral", "happy", "angry"]);
    }

    #[test]
    fn item_composition_has_four_views_and_two_callouts() {
        let comp = CompositionTemplate::Item.composition();
        assert_eq!(comp.views.len(), 4);
        assert_eq!(comp.callouts.len(), 2);
        assert!(comp.expressions.is_empty());
        assert!(comp.palette_swatch.is_some());
    }

    #[test]
    fn tileset_composition_has_three_bands() {
        let comp = CompositionTemplate::Tileset.composition();
        assert_eq!(comp.views.len(), 3);
        let labels: Vec<&str> = comp.views.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(
            labels,
            ["tile-primitives", "transition-variants", "autotile-preview"]
        );
        assert!(comp.palette_swatch.is_some());
    }

    #[test]
    fn custom_composition_has_one_view() {
        let comp = CompositionTemplate::Custom.composition();
        assert_eq!(comp.views.len(), 1);
        assert_eq!(comp.views[0].label, "full-body");
        assert!(comp.palette_swatch.is_some());
    }

    #[test]
    fn build_prompt_embeds_user_text() {
        let t = CompositionTemplate::Character;
        let prompt = t.build_prompt("a blue wizard");
        assert!(
            prompt.contains("a blue wizard"),
            "user prompt is embedded in generated prompt"
        );
        assert!(
            prompt.contains("turnaround"),
            "character prompt mentions turnaround views"
        );
    }

    #[test]
    fn all_templates_have_nonempty_negative_prompts() {
        for t in [
            CompositionTemplate::Character,
            CompositionTemplate::Item,
            CompositionTemplate::Tileset,
            CompositionTemplate::Custom,
        ] {
            assert!(!t.build_negative_prompt().is_empty());
        }
    }

    #[test]
    fn template_serialises_as_snake_case() {
        let json = serde_json::to_string(&CompositionTemplate::Character).unwrap();
        assert_eq!(json, r#""character""#);
        let json = serde_json::to_string(&CompositionTemplate::Custom).unwrap();
        assert_eq!(json, r#""custom""#);
    }

    #[test]
    fn template_round_trips_via_serde() {
        for t in [
            CompositionTemplate::Character,
            CompositionTemplate::Item,
            CompositionTemplate::Tileset,
            CompositionTemplate::Custom,
        ] {
            let json = serde_json::to_string(&t).unwrap();
            let back: CompositionTemplate = serde_json::from_str(&json).unwrap();
            assert_eq!(t, back);
        }
    }
}
