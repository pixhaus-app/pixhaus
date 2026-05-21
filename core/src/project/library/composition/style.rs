//! Reusable look modifiers — the artist's main library primitive.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::project::library::ai::{ModelId, Quality};

/// Stable id for a Style.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StyleId(pub String);

/// Reusable look modifier record.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Style {
    pub id: StyleId,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modifiers: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub look_negatives: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_pref: Option<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_round_trips_minimal() {
        let s = Style {
            id: StyleId("test.style".into()),
            name: "SNES".into(),
            modifiers: "16-bit palette".into(),
            look_negatives: "blurry".into(),
            model_pref: None,
            quality: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Style = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn empty_optionals_are_skipped() {
        let s = Style {
            id: StyleId("x".into()),
            name: "x".into(),
            modifiers: String::new(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"id":"x","name":"x"}"#);
    }
}
