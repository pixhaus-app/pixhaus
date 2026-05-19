//! Labelled-section prompt builder shared by verbs that need to wrap a
//! user prompt in an all-caps-heading constraint block.
//!
//! The shape — one or more `(HEADING, body)` sections followed by a
//! free-form payload — was lifted from `FalSprite`'s `buildSpritePrompt`
//! (`lib/fal.mjs:198-232`, MIT). The structure is the part worth porting:
//! image models follow stricter constraints when those constraints arrive
//! as discrete labelled sections rather than as a single dense paragraph.
//!
//! Verbs typically build a scaffold once and call [`PromptScaffold::render`]
//! with the per-invocation payload (character/choreography text, action
//! list, anchor description, etc.). Backends that prefer structured input
//! (JSON messages) can read the `sections` field directly instead of
//! consuming the rendered text.
//!
//! # Example
//!
//! ```
//! use pixhaus_ai::plugin::prompt_scaffold::PromptScaffold;
//!
//! let scaffold = PromptScaffold::new()
//!     .section("FORMAT", "A 4-by-4 grid of equally sized cells.")
//!     .section("FORBIDDEN", "No text, no UI, no watermarks.");
//! let prompt = scaffold.render("CHARACTER AND ANIMATION DIRECTION:\nbaby dragon");
//! assert!(prompt.contains("FORMAT:"));
//! assert!(prompt.contains("FORBIDDEN:"));
//! assert!(prompt.contains("baby dragon"));
//! ```

use std::fmt::{self, Display, Formatter};

/// A labelled-section prompt builder.
///
/// Sections are emitted in insertion order. Each section renders as
/// `"<HEADING>:\n<body>\n\n"`. The final payload is appended verbatim.
#[derive(Clone, Debug, Default)]
pub struct PromptScaffold {
    sections: Vec<(String, String)>,
}

impl PromptScaffold {
    /// Returns an empty scaffold.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a labelled section. Returns `self` for chaining.
    #[must_use]
    pub fn section(mut self, heading: impl Into<String>, body: impl Into<String>) -> Self {
        self.sections.push((heading.into(), body.into()));
        self
    }

    /// Returns a read-only view of the sections in insertion order.
    #[must_use]
    pub fn sections(&self) -> &[(String, String)] {
        &self.sections
    }

    /// Renders the scaffold followed by `payload`.
    ///
    /// Sections are separated by a blank line; the payload appears after
    /// the final section's trailing blank line. The output matches
    /// `FalSprite`'s `buildSpritePrompt` layout closely enough that an
    /// image backend trained on the upstream's traffic responds the same
    /// way.
    #[must_use]
    pub fn render(&self, payload: &str) -> String {
        let mut out = String::new();
        for (heading, body) in &self.sections {
            out.push_str(heading);
            out.push_str(":\n");
            out.push_str(body);
            out.push_str("\n\n");
        }
        out.push_str(payload);
        out
    }

    /// True when no sections have been added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// Number of sections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sections.len()
    }
}

impl Display for PromptScaffold {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scaffold_passes_payload_through() {
        let s = PromptScaffold::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.render("payload"), "payload");
    }

    #[test]
    fn single_section_is_capitalised_and_colon_terminated() {
        let s = PromptScaffold::new().section("FORMAT", "4x4 grid");
        let out = s.render("");
        assert!(out.starts_with("FORMAT:\n4x4 grid"));
    }

    #[test]
    fn sections_render_in_insertion_order() {
        let s = PromptScaffold::new()
            .section("ONE", "first")
            .section("TWO", "second")
            .section("THREE", "third");
        let out = s.render("end");
        let one = out.find("ONE:").unwrap();
        let two = out.find("TWO:").unwrap();
        let three = out.find("THREE:").unwrap();
        assert!(one < two);
        assert!(two < three);
        assert!(out.ends_with("end"));
    }

    #[test]
    fn sections_separated_by_blank_line() {
        let s = PromptScaffold::new()
            .section("A", "alpha")
            .section("B", "beta");
        let out = s.render("");
        assert!(out.contains("alpha\n\nB:\n"));
    }

    #[test]
    fn payload_appended_after_final_section() {
        let s = PromptScaffold::new().section("LEAD", "lead body");
        let out = s.render("payload tail");
        assert!(out.ends_with("payload tail"));
        assert!(out.contains("lead body\n\npayload tail"));
    }

    #[test]
    fn display_renders_without_payload() {
        let s = PromptScaffold::new().section("FOO", "bar");
        let out = format!("{s}");
        assert!(out.contains("FOO:\nbar"));
    }

    #[test]
    fn sections_view_matches_insertion() {
        let s = PromptScaffold::new().section("A", "1").section("B", "2");
        let view = s.sections();
        assert_eq!(view.len(), 2);
        assert_eq!(view[0].0, "A");
        assert_eq!(view[0].1, "1");
        assert_eq!(view[1].0, "B");
        assert_eq!(view[1].1, "2");
    }

    #[test]
    fn render_is_deterministic() {
        let s = PromptScaffold::new()
            .section("FORMAT", "grid")
            .section("FORBIDDEN", "no text");
        assert_eq!(s.render("payload"), s.render("payload"));
    }
}
