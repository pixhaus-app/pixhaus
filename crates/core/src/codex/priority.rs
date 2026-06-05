//! Field inclusion priority for the prompt compiler (bible 7.5, 11.9).
//!
//! Prompt-relevant fields carry a priority so the compiler can drop low-value
//! content first when the context budget is tight. The model stores the priority;
//! the compiler in the layers above does the budgeting.

use serde::{Deserialize, Serialize};

/// How strongly a field should be kept when the prompt compiler trims to fit the
/// context budget (bible 11.9). Higher priority is kept longer.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum InclusionPriority {
    /// Always include; dropping it changes the result's identity.
    Critical,
    /// Include unless the budget forces a cut.
    Important,
    /// Include when there is room.
    #[default]
    Normal,
    /// Include only when budget is plentiful.
    Optional,
    /// Never put in a prompt (e.g. internal notes).
    NeverInPrompt,
}

/// A prompt fragment paired with its inclusion priority.
///
/// The text is project content the prompt compiler may emit; the priority governs
/// whether it survives a budget trim.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PromptFragment {
    /// The fragment text, in plain words.
    pub text: String,
    /// How strongly to keep it under a tight budget.
    pub priority: InclusionPriority,
}

impl PromptFragment {
    /// A fragment carrying `text` at `priority`.
    pub fn new(text: impl Into<String>, priority: InclusionPriority) -> Self {
        Self { text: text.into(), priority }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_orders_critical_first() {
        assert!(InclusionPriority::Critical < InclusionPriority::Normal);
        assert!(InclusionPriority::Normal < InclusionPriority::NeverInPrompt);
        assert_eq!(InclusionPriority::default(), InclusionPriority::Normal);
    }

    #[test]
    fn fragment_holds_text_and_priority() {
        let f = PromptFragment::new("round head", InclusionPriority::Critical);
        assert_eq!(f.text, "round head");
        assert_eq!(f.priority, InclusionPriority::Critical);
    }
}
