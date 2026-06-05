//! Pure card-placement helpers for the responsive detail-card grid.
//!
//! The Codex entry-detail tabs (and any future card grid) lay their cards into 1,
//! 2, or 3 columns chosen by the available pane width, then distribute the card
//! indices across those columns with a fixed, order-preserving rule. Both helpers
//! here are pure - no egui, no state - so the placement decision is unit-testable
//! away from a live frame and the same call works in any region.
//!
//! Intended usage from a tab body:
//!
//! ```no_run
//! # use pixhaus_ui::widgets::{column_count, distribute};
//! # let cards: Vec<&str> = Vec::new();
//! # fn render_card(_i: usize, _ui: &mut egui::Ui) {}
//! # let mut ui: egui::Ui = unimplemented!();
//! let n = column_count(ui.available_width());
//! let cols_idx = distribute(cards.len(), n);
//! ui.columns(n, |cols| {
//!     for (c, idxs) in cols_idx.iter().enumerate() {
//!         for &i in idxs {
//!             render_card(i, &mut cols[c]);
//!         }
//!     }
//! });
//! ```
//!
//! Because each card body runs sequentially inside the `columns` closure, it can
//! borrow `&mut intents` / `&mut draft` - which a stored `Vec<closure>` could not.

/// At or above this width (in points) the grid uses three columns.
pub const THREE_COLUMN_MIN_WIDTH: f32 = 1040.0;

/// At or above this width (in points), and below [`THREE_COLUMN_MIN_WIDTH`], the
/// grid uses two columns. Below it, a single column.
pub const TWO_COLUMN_MIN_WIDTH: f32 = 640.0;

/// Pick the column count for a card grid from the available pane width.
///
/// Returns `3` at or above [`THREE_COLUMN_MIN_WIDTH`], `2` at or above
/// [`TWO_COLUMN_MIN_WIDTH`], and `1` below that. A non-finite or non-positive
/// width falls through to the single-column case, so a degenerate frame never
/// yields a wider grid than it can show.
#[must_use]
pub fn column_count(available_width: f32) -> usize {
    if available_width >= THREE_COLUMN_MIN_WIDTH {
        3
    } else if available_width >= TWO_COLUMN_MIN_WIDTH {
        2
    } else {
        1
    }
}

/// Assign `card_count` card indices to `columns` columns by an order-preserving
/// round-robin: card `i` lands in column `i % columns`, and the order within each
/// column matches the input order.
///
/// Always returns exactly `columns` inner vectors; some are empty when
/// `card_count < columns`. A `columns` of `0` is treated as `1` so the caller can
/// pass a raw [`column_count`] result without a divide-by-zero guard.
///
/// The helper encodes no caller's card list - it only round-robins indices. For a
/// worked breakpoint-by-breakpoint example see the `tests` module below.
#[must_use]
pub fn distribute(card_count: usize, columns: usize) -> Vec<Vec<usize>> {
    let columns = columns.max(1);
    let mut out = vec![Vec::new(); columns];
    for i in 0..card_count {
        out[i % columns].push(i);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_count_picks_one_below_the_two_column_breakpoint() {
        assert_eq!(column_count(0.0), 1);
        assert_eq!(column_count(500.0), 1);
        assert_eq!(column_count(TWO_COLUMN_MIN_WIDTH - 0.1), 1);
    }

    #[test]
    fn column_count_picks_two_between_the_breakpoints() {
        assert_eq!(column_count(TWO_COLUMN_MIN_WIDTH), 2);
        assert_eq!(column_count(640.0), 2);
        assert_eq!(column_count(1039.0), 2);
        assert_eq!(column_count(THREE_COLUMN_MIN_WIDTH - 0.1), 2);
    }

    #[test]
    fn column_count_picks_three_at_and_above_the_wide_breakpoint() {
        assert_eq!(column_count(THREE_COLUMN_MIN_WIDTH), 3);
        assert_eq!(column_count(1040.0), 3);
        assert_eq!(column_count(2000.0), 3);
    }

    #[test]
    fn column_count_handles_non_finite_width_as_single_column() {
        assert_eq!(column_count(f32::NAN), 1);
        assert_eq!(column_count(f32::NEG_INFINITY), 1);
        // Positive infinity is genuinely wide, so it reads as three columns.
        assert_eq!(column_count(f32::INFINITY), 3);
    }

    #[test]
    fn distribute_three_columns_round_robins_seven_cards() {
        assert_eq!(distribute(7, 3), vec![vec![0, 3, 6], vec![1, 4], vec![2, 5]]);
    }

    #[test]
    fn distribute_two_columns_splits_even_and_odd_in_order() {
        assert_eq!(distribute(7, 2), vec![vec![0, 2, 4, 6], vec![1, 3, 5]]);
    }

    #[test]
    fn distribute_one_column_keeps_natural_order() {
        assert_eq!(distribute(3, 1), vec![vec![0, 1, 2]]);
    }

    #[test]
    fn distribute_pads_empty_columns_when_cards_are_fewer() {
        assert_eq!(distribute(2, 3), vec![vec![0], vec![1], Vec::new()]);
    }

    #[test]
    fn distribute_treats_zero_columns_as_one() {
        assert_eq!(distribute(3, 0), vec![vec![0, 1, 2]]);
    }

    #[test]
    fn distribute_with_no_cards_returns_empty_columns() {
        // Annotate the empty inner vecs: with serde_json a dev-dependency, an unannotated
        // `Vec::new()` here is ambiguous (usize also impls PartialEq<serde_json::Value>).
        let empty: Vec<Vec<usize>> = vec![Vec::new(), Vec::new(), Vec::new()];
        assert_eq!(distribute(0, 3), empty);
    }

    #[test]
    fn distribute_always_returns_requested_column_count() {
        for columns in 1..=3 {
            assert_eq!(distribute(10, columns).len(), columns);
        }
    }
}
