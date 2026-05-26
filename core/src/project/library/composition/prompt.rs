//! Saved request template with variable placeholders.

use serde::{Deserialize, Serialize};

use super::{StructureId, StyleId};

/// Stable id for a `PromptTemplate`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PromptId(pub String);

/// Saved generation request with variable placeholders.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: PromptId,
    pub name: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<PromptVariable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_style: Option<StyleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_structure: Option<StructureId>,
}

/// One variable in a `PromptTemplate`.
///
/// A variable is `{key}` substituted into the template text. `control` shapes
/// the editor widget the cockpit renders for it and how a value turns into a
/// string; substitution itself stays string-valued, so `compose` is unaffected.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PromptVariable {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default: String,
    #[serde(default, skip_serializing_if = "VarControl::is_text")]
    pub control: VarControl,
}

/// The editor widget a [`PromptVariable`] renders as in the cockpit.
///
/// `#[serde(default)]` on the field means a 4.2-era variable with no `control`
/// key loads as [`VarControl::Text`], so old projects and built-ins open
/// unchanged. The control only shapes the widget and how a chosen value renders
/// to a string — `substitute`/`compose` stay string-based.
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VarControl {
    /// A free-text field — the back-compatible default.
    #[default]
    Text,
    /// A dropdown of fixed choices.
    Select { choices: Vec<String> },
    /// A numeric dial rendered as a slider/drag value.
    Number { min: f64, max: f64, step: f64 },
    /// A colour swatch; the value renders as a hex string.
    Color,
    /// A choice list the generator samples from at random each run — the
    /// serendipity control. Behaves like [`VarControl::Select`] in the editor
    /// but invites a fresh pick per generation.
    Wildcard { choices: Vec<String> },
}

impl VarControl {
    /// Whether this is the default [`VarControl::Text`]. Used as the
    /// `skip_serializing_if` predicate so the common case adds no bytes.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, VarControl::Text)
    }

    /// The choices a `Select` or `Wildcard` offers, or an empty slice.
    #[must_use]
    pub fn choices(&self) -> &[String] {
        match self {
            VarControl::Select { choices } | VarControl::Wildcard { choices } => choices,
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_round_trips() {
        let p = PromptTemplate {
            id: PromptId("p1".into()),
            name: "Warrior".into(),
            text: "a {species} warrior".into(),
            variables: vec![PromptVariable {
                key: "species".into(),
                label: "Species".into(),
                default: "human".into(),
                control: VarControl::Text,
            }],
            default_style: Some(StyleId("s".into())),
            default_structure: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: PromptTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn absent_control_defaults_to_text() {
        // A 4.2-era variable serialized without a `control` key must load as
        // `Text` so old projects open unchanged.
        let legacy = r#"{"key":"species","label":"Species","default":"human"}"#;
        let var: PromptVariable = serde_json::from_str(legacy).unwrap();
        assert_eq!(var.control, VarControl::Text);
    }

    #[test]
    fn text_control_is_not_serialized() {
        // The default (Text) control is skipped so old projects round-trip
        // byte-identical and files stay small.
        let var = PromptVariable {
            key: "k".into(),
            label: "L".into(),
            default: String::new(),
            control: VarControl::Text,
        };
        let json = serde_json::to_string(&var).unwrap();
        assert!(!json.contains("control"), "Text control must be skipped: {json}");
    }

    #[test]
    fn typed_controls_round_trip() {
        for control in [
            VarControl::Text,
            VarControl::Select {
                choices: vec!["a".into(), "b".into()],
            },
            VarControl::Number {
                min: 0.0,
                max: 10.0,
                step: 0.5,
            },
            VarControl::Color,
            VarControl::Wildcard {
                choices: vec!["red".into(), "blue".into()],
            },
        ] {
            let var = PromptVariable {
                key: "k".into(),
                label: "L".into(),
                default: "d".into(),
                control: control.clone(),
            };
            let json = serde_json::to_string(&var).unwrap();
            let back: PromptVariable = serde_json::from_str(&json).unwrap();
            assert_eq!(back.control, control, "round trip for {control:?}");
        }
    }
}
