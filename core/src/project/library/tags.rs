//! Tag definitions for the project library.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::project::color::Rgba;
use crate::project::id::TagId;

/// Definition of a tag the user (or VLM auto-tagging) created.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
