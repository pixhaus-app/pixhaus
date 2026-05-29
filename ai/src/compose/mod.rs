//! Composition resolver. See docs/planning/work/prompt-style-structure-library.md section 6.

pub mod builtins;
pub mod variables;

use std::collections::BTreeMap;

use pixhaus_core::project::library::composition::{PanelSlot, PromptTemplate, Structure, StructureOutput, Style};
use pixhaus_core::project::{Rect, SheetComposition, SheetPanel};
use thiserror::Error;

use self::variables::{VarError, VarSource, substitute};

/// Error returned by [`compose`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComposeError {
    /// A prompt variable could not be resolved.
    #[error("variable: {0}")]
    Variable(#[from] VarError),
    /// A paneled structure declared no panels.
    #[error("paneled structure `{0}` has no panels")]
    EmptyPaneledStructure(String),
}

/// Canvas size returned alongside a composed prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canvas {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
}

/// Inputs to [`compose`]. Borrows everything; the caller owns the records.
pub struct ComposeRequest<'a> {
    /// Always-applied baseline layer (project style notes or the built-in default).
    pub baseline: &'a str,
    /// The picked layout contract.
    pub structure: &'a Structure,
    /// The picked look modifiers, if any.
    pub style: Option<&'a Style>,
    /// The picked saved prompt template, if any.
    pub prompt: Option<&'a PromptTemplate>,
    /// Explicit variable values supplied by the user.
    pub variable_values: &'a BTreeMap<String, String>,
    /// Entity metadata used as a fallback variable source.
    pub entity_info: &'a BTreeMap<String, String>,
    /// Free-typed prompt additions.
    pub inline_text: &'a str,
    /// Free-typed negative additions.
    pub inline_negatives: &'a str,
    /// Operation-specific trailing instruction, appended last when present.
    pub operation_hint: Option<&'a str>,
    /// App-built context fragments (background, references, grounding, `LoRA`).
    pub context_fragments: &'a [String],
}

/// Output of [`compose`].
pub struct ComposedPrompt {
    /// The assembled positive prompt.
    pub positive: String,
    /// The assembled negative prompt.
    pub negative: String,
    /// Panel slice geometry (empty for `Single` output).
    pub composition: SheetComposition,
    /// Canvas size for paneled output, `None` for `Single`.
    pub canvas: Option<Canvas>,
}

/// Joins non-empty, trimmed segments with `sep`.
fn join_nonempty(sep: &str, segments: &[String]) -> String {
    segments.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(sep)
}

fn structure_prose(structure: &Structure) -> Result<(Vec<String>, Option<Canvas>), ComposeError> {
    match &structure.output {
        StructureOutput::Single => Ok((Vec::new(), None)),
        StructureOutput::Paneled { canvas, panels } => {
            if panels.is_empty() {
                return Err(ComposeError::EmptyPaneledStructure(structure.id.0.clone()));
            }
            let mut prose = Vec::with_capacity(panels.len());
            for p in panels {
                let frag = p
                    .prose_fragment
                    .replace("{canvas_w}", &canvas.width.to_string())
                    .replace("{canvas_h}", &canvas.height.to_string())
                    .replace("{panel_w}", &p.rect.w.to_string())
                    .replace("{panel_h}", &p.rect.h.to_string())
                    .replace("{label}", &p.label);
                prose.push(frag);
            }
            Ok((
                prose,
                Some(Canvas {
                    width: canvas.width,
                    height: canvas.height,
                }),
            ))
        }
    }
}

/// Returns the panel slice geometry for a Structure, independent of any
/// prompt text. For consumers that only need the `SheetComposition`
/// rectangles (building test fixtures, seeding stored variants).
#[must_use]
pub fn composition_for(structure: &Structure) -> SheetComposition {
    build_composition(structure)
}

fn build_composition(structure: &Structure) -> SheetComposition {
    let StructureOutput::Paneled { panels, .. } = &structure.output else {
        return SheetComposition::default();
    };
    let mut comp = SheetComposition::default();
    for p in panels {
        let x = i32::try_from(p.rect.x).unwrap_or(i32::MAX);
        let y = i32::try_from(p.rect.y).unwrap_or(i32::MAX);
        let rect = Rect::from_xywh(x, y, p.rect.w, p.rect.h);
        let panel = SheetPanel {
            region: rect,
            label: p.label.clone(),
        };
        match p.slot {
            PanelSlot::View | PanelSlot::Generic => comp.views.push(panel),
            PanelSlot::Expression => comp.expressions.push(panel),
            PanelSlot::Callout => comp.callouts.push(panel),
            PanelSlot::Outfit => comp.outfits.push(panel),
            PanelSlot::PaletteSwatch => comp.palette_swatch = Some(rect),
        }
    }
    comp
}

/// Resolves a [`ComposeRequest`] into a positive/negative prompt plus slice geometry.
///
/// Pure and deterministic: same inputs produce the same bytes out.
pub fn compose(req: &ComposeRequest) -> Result<ComposedPrompt, ComposeError> {
    // Resolve prompt text with variable substitution (explicit -> entity info -> defaults).
    let prompt_text = match req.prompt {
        Some(p) => {
            let defaults: BTreeMap<String, String> = p
                .variables
                .iter()
                .filter(|v| !v.default.is_empty())
                .map(|v| (v.key.clone(), v.default.clone()))
                .collect();
            let sources: [&dyn VarSource; 3] = [req.variable_values, req.entity_info, &defaults];
            substitute(&p.text, &sources)?
        }
        None => String::new(),
    };

    let (layout_prose, canvas) = structure_prose(req.structure)?;
    let layout_joined = join_nonempty(". ", &layout_prose);

    let positive = join_nonempty(
        ". ",
        &[
            req.baseline.to_string(),
            req.style.map(|s| s.modifiers.clone()).unwrap_or_default(),
            layout_joined,
            prompt_text,
            join_nonempty(". ", req.context_fragments),
            req.operation_hint.unwrap_or("").to_string(),
            req.inline_text.to_string(),
        ],
    );

    let negative = join_nonempty(
        ", ",
        &[
            req.structure.layout_negatives.clone(),
            req.style.map(|s| s.look_negatives.clone()).unwrap_or_default(),
            req.inline_negatives.to_string(),
        ],
    );

    Ok(ComposedPrompt {
        positive,
        negative,
        composition: build_composition(req.structure),
        canvas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::library::composition::{ArtStyleKind, Dimensions, PanelRect, StructureId, StructurePanel, StyleId};

    fn paneled() -> Structure {
        Structure {
            id: StructureId("test.character".into()),
            name: "Character".into(),
            output: StructureOutput::Paneled {
                canvas: Dimensions { width: 1024, height: 480 },
                panels: vec![StructurePanel {
                    label: "front".into(),
                    rect: PanelRect { x: 0, y: 0, w: 200, h: 480 },
                    prose_fragment: "front view, {panel_w} by {panel_h}".into(),
                    slot: PanelSlot::View,
                }],
            },
            layout_negatives: "overlapping views".into(),
        }
    }

    fn empty_vars() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn interpolates_panel_dims_into_prose() {
        let s = paneled();
        let req = ComposeRequest {
            baseline: "pixel art",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        assert!(out.positive.contains("front view, 200 by 480"));
        assert_eq!(out.canvas, Some(Canvas { width: 1024, height: 480 }));
    }

    #[test]
    fn negatives_merge_structure_and_style() {
        let s = paneled();
        let style = Style {
            id: StyleId("st".into()),
            name: "S".into(),
            kind: ArtStyleKind::default(),
            modifiers: "16-bit".into(),
            look_negatives: "blurry".into(),
            model_pref: None,
            quality: None,
        };
        let req = ComposeRequest {
            baseline: "",
            structure: &s,
            style: Some(&style),
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "",
            inline_negatives: "watermark",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        assert_eq!(out.negative, "overlapping views, blurry, watermark");
        assert!(out.positive.starts_with("16-bit"));
    }

    #[test]
    fn single_output_has_empty_composition_and_no_canvas() {
        let s = Structure {
            id: StructureId("s".into()),
            name: "S".into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        };
        let req = ComposeRequest {
            baseline: "base",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "a sword",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        assert_eq!(out.positive, "base. a sword");
        assert!(out.composition.views.is_empty());
        assert!(out.canvas.is_none());
    }

    #[test]
    fn maps_panel_slots_to_composition_buckets() {
        let s = paneled();
        let out = build_composition(&s);
        assert_eq!(out.views.len(), 1);
        assert_eq!(out.views[0].label, "front");
    }

    #[test]
    fn empty_paneled_structure_errors() {
        let s = Structure {
            id: StructureId("bad".into()),
            name: "Bad".into(),
            output: StructureOutput::Paneled {
                canvas: Dimensions { width: 10, height: 10 },
                panels: vec![],
            },
            layout_negatives: String::new(),
        };
        let req = ComposeRequest {
            baseline: "",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        assert!(matches!(compose(&req), Err(ComposeError::EmptyPaneledStructure(_))));
    }

    #[test]
    fn positive_prompt_snapshot() {
        let s = paneled();
        let req = ComposeRequest {
            baseline: "pixel art character model sheet",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "a blue wizard",
            inline_negatives: "",
            operation_hint: Some("Preserve the character identity."),
            context_fragments: &["Background must be flat magenta.".into()],
        };
        let out = compose(&req).unwrap();
        insta::assert_snapshot!(out.positive);
    }
}
