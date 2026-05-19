//! CHARACTER and CHOREOGRAPHY prompt assets for the animated-sprite-sheet
//! verb.
//!
//! The two `.txt` files in `prompts/` are adapted from `FalSprite`
//! (<https://github.com/lovisdotio/falsprite>), MIT-licensed.
//! Copyright (c) 2026 lovisdotio. See `prompts/LICENSE.txt` for the full
//! license text and `NOTICES.md` at the repo root for the project-wide
//! attribution surface.
//!
//! The only adaptation is the `{grid_word}` template marker, which
//! replaces the upstream's runtime `NUM_WORDS[gridSize]` interpolation.
//! Section structure, wording, and the leg-alternation / no-numbers rules
//! are unchanged so backends respond the way upstream's traffic primes
//! them to.

/// Maximum grid size accepted by the verb. The upstream supports 2..=6 and
/// we keep the same range so the word-substitution table stays small and
/// the image-model output remains tractable.
pub const MAX_GRID_SIZE: u8 = 6;

/// Minimum grid size accepted by the verb. 1×1 collapses to a single
/// still — the animation flow constraints become meaningless — so the
/// floor is 2.
pub const MIN_GRID_SIZE: u8 = 2;

/// Default grid size when the user does not specify one. Matches the
/// upstream's `gridSize = 4` default.
pub const DEFAULT_GRID_SIZE: u8 = 4;

/// CHARACTER × CHOREOGRAPHY system prompt template. The `{grid_word}`
/// marker is replaced with the lowercase English word for the grid size
/// (`"two"`, `"three"`, ... `"six"`) via [`fill_grid_word`].
pub const REWRITE_SYSTEM_PROMPT_TEMPLATE: &str = include_str!("prompts/rewrite_system.txt");

/// STRICT TECHNICAL REQUIREMENTS scaffold for the image backend. Same
/// `{grid_word}` marker conventions as [`REWRITE_SYSTEM_PROMPT_TEMPLATE`].
pub const SPRITE_CONSTRAINT_TEMPLATE: &str = include_str!("prompts/sprite_constraints.txt");

/// Returns the lowercase English word for `grid_size` in the upstream's
/// vocabulary. Falls back to `"four"` for out-of-range inputs so the
/// prompt always reads naturally even when an unexpected value sneaks
/// through.
#[must_use]
pub fn grid_word(grid_size: u8) -> &'static str {
    match grid_size {
        2 => "two",
        3 => "three",
        5 => "five",
        6 => "six",
        // `4` is the upstream default and the documented fallback for
        // anything out of range. Both arms render `"four"`.
        _ => "four",
    }
}

/// Substitutes every `{grid_word}` marker in `template` with the
/// lowercase English word for `grid_size`.
#[must_use]
pub fn fill_grid_word(template: &str, grid_size: u8) -> String {
    template.replace("{grid_word}", grid_word(grid_size))
}

/// Builds the system prompt the LLM sees when rewriting the user's
/// character concept into CHARACTER + CHOREOGRAPHY sections.
#[must_use]
pub fn rewrite_system_prompt(grid_size: u8) -> String {
    fill_grid_word(REWRITE_SYSTEM_PROMPT_TEMPLATE, grid_size)
}

/// Builds the technical-requirements scaffold (without the
/// character/choreography payload).
///
/// Callers append the LLM-rewritten character + choreography text after
/// the rendered scaffold; see [`crate::verbs::animated_sprite_sheet::build_sprite_prompt`].
#[must_use]
pub fn sprite_constraint_scaffold(grid_size: u8) -> String {
    fill_grid_word(SPRITE_CONSTRAINT_TEMPLATE, grid_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_word_covers_supported_range() {
        assert_eq!(grid_word(2), "two");
        assert_eq!(grid_word(3), "three");
        assert_eq!(grid_word(4), "four");
        assert_eq!(grid_word(5), "five");
        assert_eq!(grid_word(6), "six");
    }

    #[test]
    fn grid_word_falls_back_for_out_of_range() {
        assert_eq!(grid_word(0), "four");
        assert_eq!(grid_word(1), "four");
        assert_eq!(grid_word(7), "four");
        assert_eq!(grid_word(255), "four");
    }

    #[test]
    fn fill_grid_word_replaces_all_markers() {
        let template = "{grid_word}-by-{grid_word} grid of {grid_word} beats";
        assert_eq!(
            fill_grid_word(template, 4),
            "four-by-four grid of four beats"
        );
        assert_eq!(fill_grid_word(template, 2), "two-by-two grid of two beats");
    }

    #[test]
    fn fill_grid_word_no_markers_returns_input() {
        assert_eq!(fill_grid_word("no markers here", 4), "no markers here");
    }

    #[test]
    fn rewrite_prompt_contains_required_sections() {
        let p = rewrite_system_prompt(4);
        assert!(p.contains("CHARACTER:"));
        assert!(p.contains("CHOREOGRAPHY:"));
        assert!(p.contains("RULES:"));
        assert!(p.contains("four-beat"));
        assert!(!p.contains("{grid_word}"));
    }

    #[test]
    fn sprite_scaffold_contains_required_sections() {
        let p = sprite_constraint_scaffold(4);
        assert!(p.contains("STRICT TECHNICAL REQUIREMENTS"));
        assert!(p.contains("FORMAT:"));
        assert!(p.contains("FORBIDDEN:"));
        assert!(p.contains("CONSISTENCY:"));
        assert!(p.contains("ANIMATION FLOW:"));
        assert!(p.contains("MOTION QUALITY:"));
        assert!(p.contains("CHARACTER AND ANIMATION DIRECTION:"));
        assert!(p.contains("four-by-four"));
        assert!(!p.contains("{grid_word}"));
    }

    #[test]
    fn grid_size_constants_are_consistent() {
        const _: () = {
            assert!(MIN_GRID_SIZE >= 2);
            assert!(MAX_GRID_SIZE >= MIN_GRID_SIZE);
            assert!(MAX_GRID_SIZE <= 6);
            assert!(DEFAULT_GRID_SIZE >= MIN_GRID_SIZE);
            assert!(DEFAULT_GRID_SIZE <= MAX_GRID_SIZE);
        };
    }

    #[test]
    fn prompt_assets_are_non_empty() {
        assert!(!REWRITE_SYSTEM_PROMPT_TEMPLATE.trim().is_empty());
        assert!(!SPRITE_CONSTRAINT_TEMPLATE.trim().is_empty());
    }
}
