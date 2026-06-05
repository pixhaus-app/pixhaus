//! Coverage: tracking the assets a project needs (bible 15).
//!
//! Coverage makes the Codex production-oriented. A [`CoverageTemplate`] names the
//! slots an entry should fill (idle, walk, run, ...); applying it to an entry seeds a
//! per-slot [`CoverageItemStatus`] the artist drives from missing to approved. Slot
//! keys are stable ids; labels are a [`CoverageLabel`] — a localization key for the
//! built-in presets, a literal for user-authored slots — never display text resolved
//! here. Templates are project-level and reusable: each carries a stable
//! [`CoverageTemplateId`], an entry applies them by id, and the layers above resolve
//! the labels.

use serde::{Deserialize, Serialize};

/// Stable identifier for a [`CoverageTemplate`] within one [`Codex`](crate::codex::Codex).
///
/// Like every model id, this is a monotonic `u64` newtype minted from an
/// [`IdCounter`](crate::ids::IdCounter) the [`Codex`](crate::codex::Codex) owns.
/// Entries reference templates by this id, so renaming a template never breaks the
/// reference.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoverageTemplateId(pub u64);

/// A slot's display label: a localization key for built-in presets, or a literal for
/// user-authored slots.
///
/// `core` never localizes, so it stores the label as data and lets the UI resolve it:
/// a [`Key`](CoverageLabel::Key) goes through `tr()`, a [`Literal`](CoverageLabel::Literal)
/// renders as-is (and is never passed to `tr()`, so a user label never spams the
/// missing-key log).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CoverageLabel {
    /// A localization key resolved through the i18n service in the layers above.
    Key(String),
    /// Literal project content, rendered as-is and never localized.
    Literal(String),
}

/// One slot in a coverage template: a stable key and an editable label.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CoverageSlot {
    /// Stable slot key (e.g. `idle`, `walk`); used to key coverage state. Set once on
    /// creation, never changed by a rename, so coverage status cells survive.
    pub key: String,
    /// The slot's display label; the layers above resolve it.
    pub label: CoverageLabel,
}

impl CoverageSlot {
    /// A slot with stable `key` and a localization-key `label` (a built-in preset slot).
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: CoverageLabel::Key(label.into()),
        }
    }

    /// A slot with stable `key` and a literal `text` label (a user-authored slot).
    pub fn custom(key: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: CoverageLabel::Literal(text.into()),
        }
    }
}

/// A named set of coverage slots an entry should fill (bible 15.2).
///
/// A template is project-level and reusable: it carries a stable [`CoverageTemplateId`]
/// an entry references, plus a project-content `name` for display. The presets returned
/// by the built-in constructors carry a sentinel id ([`CoverageTemplate::UNASSIGNED`])
/// until the registering command mints the real one.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CoverageTemplate {
    /// Stable id; entries reference this, never the name.
    pub id: CoverageTemplateId,
    /// Display name (project content; may change without breaking references).
    pub name: String,
    /// The slots this template requires, in order.
    pub slots: Vec<CoverageSlot>,
}

impl CoverageTemplate {
    /// The id a freshly built template carries before a command mints its real id.
    pub const UNASSIGNED: CoverageTemplateId = CoverageTemplateId(u64::MAX);

    /// A template named `name` with `slots`, carrying the [`UNASSIGNED`](Self::UNASSIGNED)
    /// id until a command registers it.
    pub fn new(name: impl Into<String>, slots: Vec<CoverageSlot>) -> Self {
        Self {
            id: Self::UNASSIGNED,
            name: name.into(),
            slots,
        }
    }

    /// The built-in platformer-character template: idle, walk, run, jump, fall, land,
    /// attack, hurt, death (bible 15.2). Labels are localization keys under
    /// `codex.coverage.slot.*`. The id is [`UNASSIGNED`](Self::UNASSIGNED) until
    /// registered.
    pub fn platformer_character() -> Self {
        let slots = ["idle", "walk", "run", "jump", "fall", "land", "attack", "hurt", "death"]
            .iter()
            .map(|k| CoverageSlot::new(*k, format!("codex.coverage.slot.{k}")))
            .collect();
        Self::new("platformer_character", slots)
    }

    /// The built-in top-down 4-direction template: idle, walk up/down/left/right,
    /// attack. Labels are localization keys under `codex.coverage.slot.*`.
    pub fn top_down_four_direction() -> Self {
        let slots = ["idle", "walk_up", "walk_down", "walk_left", "walk_right", "attack"]
            .iter()
            .map(|k| CoverageSlot::new(*k, format!("codex.coverage.slot.{k}")))
            .collect();
        Self::new("top_down_four_direction", slots)
    }

    /// The built-in UI-button-states template: normal, hover, pressed, disabled. Labels
    /// are localization keys under `codex.coverage.slot.*`.
    pub fn ui_button_states() -> Self {
        let slots = ["normal", "hover", "pressed", "disabled"]
            .iter()
            .map(|k| CoverageSlot::new(*k, format!("codex.coverage.slot.{k}")))
            .collect();
        Self::new("ui_button_states", slots)
    }
}

/// The state of one coverage item for one entry (bible 15.3).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum CoverageItemStatus {
    /// No asset exists yet.
    #[default]
    Missing,
    /// A draft asset exists.
    Draft,
    /// An asset was generated, not yet reviewed.
    Generated,
    /// An asset exists and needs review.
    NeedsReview,
    /// An asset is approved.
    Approved,
    /// An asset was finalized by hand.
    ManuallyFinalized,
    /// The slot was retired.
    Deprecated,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_template_id_compares_by_value() {
        assert_eq!(CoverageTemplateId(3), CoverageTemplateId(3));
        assert!(CoverageTemplateId(1) < CoverageTemplateId(2));
    }

    #[test]
    fn slot_new_is_a_key_label() {
        let s = CoverageSlot::new("idle", "codex.coverage.slot.idle");
        assert_eq!(s.key, "idle");
        assert_eq!(s.label, CoverageLabel::Key("codex.coverage.slot.idle".to_owned()));
    }

    #[test]
    fn slot_custom_is_a_literal_label() {
        let s = CoverageSlot::custom("crouch", "Crouch");
        assert_eq!(s.key, "crouch");
        assert_eq!(s.label, CoverageLabel::Literal("Crouch".to_owned()));
    }

    #[test]
    fn new_template_carries_unassigned_id() {
        let t = CoverageTemplate::new("custom", vec![CoverageSlot::custom("a", "A")]);
        assert_eq!(t.id, CoverageTemplate::UNASSIGNED);
        assert_eq!(t.name, "custom");
    }

    #[test]
    fn platformer_template_has_the_nine_slots() {
        let t = CoverageTemplate::platformer_character();
        assert_eq!(t.name, "platformer_character");
        let keys: Vec<&str> = t.slots.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["idle", "walk", "run", "jump", "fall", "land", "attack", "hurt", "death"]);
        assert_eq!(t.slots[0].label, CoverageLabel::Key("codex.coverage.slot.idle".to_owned()));
    }

    #[test]
    fn top_down_template_has_the_six_slots() {
        let t = CoverageTemplate::top_down_four_direction();
        assert_eq!(t.name, "top_down_four_direction");
        let keys: Vec<&str> = t.slots.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["idle", "walk_up", "walk_down", "walk_left", "walk_right", "attack"]);
        assert_eq!(t.slots[1].label, CoverageLabel::Key("codex.coverage.slot.walk_up".to_owned()));
    }

    #[test]
    fn ui_button_template_has_the_four_slots() {
        let t = CoverageTemplate::ui_button_states();
        assert_eq!(t.name, "ui_button_states");
        let keys: Vec<&str> = t.slots.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["normal", "hover", "pressed", "disabled"]);
    }

    #[test]
    fn item_status_defaults_missing() {
        assert_eq!(CoverageItemStatus::default(), CoverageItemStatus::Missing);
    }
}
