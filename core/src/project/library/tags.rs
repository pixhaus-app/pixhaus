//! Tag definitions for the project library.

use serde::{Deserialize, Serialize};

use crate::project::color::Rgba;
use crate::project::id::TagId;

/// Definition of a tag the user (or VLM auto-tagging) created.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TagDefinition {
    /// Stable identifier.
    pub id: TagId,

    /// Tag name. User-facing; must be non-empty in the editor but the
    /// data model itself does not enforce that.
    pub name: String,

    /// Optional accent color, used by the library tree to tint the chip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,

    /// `true` if created by VLM auto-tagging. Lets the UI display them
    /// differently and lets the user accept/reject them in batch.
    #[serde(default)]
    pub auto_generated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_with_color() {
        let tag = TagDefinition {
            id: TagId::new(3),
            name: "hero".to_owned(),
            color: Some(Rgba::opaque(255, 0, 255)),
            auto_generated: true,
        };
        let json = serde_json::to_string(&tag).unwrap();
        let back: TagDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, back);
    }

    #[test]
    fn round_trips_without_color() {
        let tag = TagDefinition {
            id: TagId::new(7),
            name: "wip".to_owned(),
            color: None,
            auto_generated: false,
        };
        let json = serde_json::to_string(&tag).unwrap();
        // An absent color is skipped entirely rather than written as null.
        assert!(!json.contains("color"), "absent color must not serialize: {json}");
        let back: TagDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, back);
    }

    #[test]
    fn old_blob_without_color_or_flag_deserializes() {
        let old = r#"{"id":1,"name":"prop"}"#;
        let tag: TagDefinition = serde_json::from_str(old).unwrap();
        assert_eq!(tag.id, TagId::new(1));
        assert_eq!(tag.name, "prop");
        assert_eq!(tag.color, None);
        assert!(!tag.auto_generated);
    }
}
