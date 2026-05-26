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

/// One fill-in-the-blank variable in a `PromptTemplate`.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptVariable {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default: String,
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
            }],
            default_style: Some(StyleId("s".into())),
            default_structure: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: PromptTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
