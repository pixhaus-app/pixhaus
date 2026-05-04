//! Palette operations: swap, cycle, ramp, and nearest-color lookup.

use crate::project::color::Rgba;

use super::space::oklab_mix;

// ── Nearest color ────────────────────────────────────────────────────────────

/// Returns the index of the entry whose RGB values are nearest to `target`,
/// ignoring alpha on both sides.
///
/// Generic over any iterator yielding `Rgba` so callers like
/// `Palette::nearest_index` can pass `self.colors.iter().map(|e| e.color)`
/// without first allocating a `Vec<Rgba>`. Uses squared Euclidean distance
/// in sRGB space. Returns `None` if the iterator yields no items.
pub fn nearest_color_index<I>(palette: I, target: Rgba) -> Option<usize>
where
    I: IntoIterator<Item = Rgba>,
{
    palette
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let dr = i32::from(c.r) - i32::from(target.r);
            let dg = i32::from(c.g) - i32::from(target.g);
            let db = i32::from(c.b) - i32::from(target.b);
            (i, dr * dr + dg * dg + db * db)
        })
        .min_by_key(|&(_, dist)| dist)
        .map(|(i, _)| i)
}

// ── Palette swap ─────────────────────────────────────────────────────────────

/// Replaces every occurrence of `from` (compared by all four channels) with
/// `to` in `colors`.
pub fn palette_swap(colors: &mut [Rgba], from: Rgba, to: Rgba) {
    for c in colors {
        if *c == from {
            *c = to;
        }
    }
}

// ── Color ramp ───────────────────────────────────────────────────────────────

/// Generates a ramp of `steps` colors interpolated from `start` to `end` in
/// Oklab space.
///
/// - `steps = 1` returns `[start]`.
/// - `steps = 2` returns `[start, end]`.
/// - Larger values insert perceptually uniform intermediate colors.
/// - Returns an empty `Vec` if `steps == 0`.
pub fn color_ramp(start: Rgba, end: Rgba, steps: usize) -> Vec<Rgba> {
    match steps {
        0 => Vec::new(),
        1 => vec![start],
        2 => vec![start, end],
        #[allow(clippy::cast_precision_loss)]
        n => (0..n)
            .map(|i| {
                let t = i as f32 / (n - 1) as f32;
                oklab_mix(start, end, t)
            })
            .collect(),
    }
}

// ── Color cycling ────────────────────────────────────────────────────────────

/// Returns a copy of `entries` with the slice `[first..=last]` shifted by
/// `offset` positions.
///
/// Positive `offset` shifts entries toward higher indices (forward); negative
/// shifts toward lower indices (backward). Entries outside the range are
/// copied unchanged.
///
/// If `first > last`, either index is out of bounds, or `offset == 0`,
/// the function returns a copy of `entries` unchanged. It does not panic
/// on bad inputs.
pub fn palette_cycle(entries: &[Rgba], first: usize, last: usize, offset: isize) -> Vec<Rgba> {
    let mut out = entries.to_vec();
    if first > last || last >= entries.len() || offset == 0 {
        return out;
    }
    let range_len = last - first + 1;
    // range_len is at most entries.len() which is always < isize::MAX in practice
    #[allow(clippy::cast_possible_wrap)]
    let offset = offset.rem_euclid(range_len as isize) as usize;
    // Rotate the sub-slice in-place: [first..=last] shifted by `offset`
    // A forward shift of 1 means: old[first] goes to out[last], old[first+1] goes to out[first], etc.
    // i.e., the sub-slice is rotated right by `offset`.
    out[first..=last].rotate_right(offset);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }
    fn blue() -> Rgba {
        Rgba::opaque(0, 0, 255)
    }
    fn green() -> Rgba {
        Rgba::opaque(0, 255, 0)
    }

    // ── nearest_color_index ──────────────────────────────────────────────────

    #[test]
    fn nearest_exact_match() {
        let palette = [red(), blue(), green()];
        assert_eq!(nearest_color_index(palette.iter().copied(), red()), Some(0));
        assert_eq!(
            nearest_color_index(palette.iter().copied(), blue()),
            Some(1)
        );
        assert_eq!(
            nearest_color_index(palette.iter().copied(), green()),
            Some(2)
        );
    }

    #[test]
    fn nearest_picks_closest_by_distance() {
        let palette = [Rgba::opaque(0, 0, 0), Rgba::opaque(200, 0, 0)];
        let target = Rgba::opaque(180, 0, 0);
        assert_eq!(
            nearest_color_index(palette.iter().copied(), target),
            Some(1)
        );
    }

    #[test]
    fn nearest_empty_palette_returns_none() {
        assert_eq!(nearest_color_index(std::iter::empty(), red()), None);
    }

    // ── palette_swap ─────────────────────────────────────────────────────────

    #[test]
    fn swap_replaces_all_matches() {
        let mut colors = vec![red(), blue(), red()];
        palette_swap(&mut colors, red(), green());
        assert_eq!(colors, vec![green(), blue(), green()]);
    }

    #[test]
    fn swap_no_match_is_noop() {
        let original = vec![red(), blue()];
        let mut colors = original.clone();
        palette_swap(&mut colors, green(), red());
        assert_eq!(colors, original);
    }

    #[test]
    fn swap_compares_all_channels() {
        // Colors differing only in alpha must not match a solid color
        let opaque_red = red();
        let semi_red = Rgba::new(255, 0, 0, 128);
        let mut colors = vec![opaque_red, semi_red];
        palette_swap(&mut colors, opaque_red, green());
        assert_eq!(colors[0], green());
        assert_eq!(colors[1], semi_red); // not replaced
    }

    // ── color_ramp ───────────────────────────────────────────────────────────

    #[test]
    fn ramp_zero_steps_empty() {
        assert!(color_ramp(red(), blue(), 0).is_empty());
    }

    #[test]
    fn ramp_one_step_is_start() {
        assert_eq!(color_ramp(red(), blue(), 1), vec![red()]);
    }

    #[test]
    fn ramp_two_steps_is_endpoints() {
        let r = color_ramp(red(), blue(), 2);
        assert_eq!(r[0], red());
        assert_eq!(r[1], blue());
    }

    #[test]
    fn ramp_n_steps_correct_length() {
        let r = color_ramp(red(), blue(), 5);
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn ramp_endpoints_match_input() {
        let r = color_ramp(red(), blue(), 7);
        assert_eq!(r[0], red());
        assert_eq!(*r.last().unwrap(), blue());
    }

    #[test]
    fn ramp_is_monotone_in_oklab() {
        // For a red → blue ramp the green channel should stay low throughout
        let r = color_ramp(red(), blue(), 5);
        for c in &r {
            assert!(c.g < 100, "unexpected green spike: {c:?}");
        }
    }

    // ── palette_cycle ────────────────────────────────────────────────────────

    #[test]
    fn cycle_forward_by_one() {
        let entries = vec![red(), blue(), green()];
        let cycled = palette_cycle(&entries, 0, 2, 1);
        // rotate_right(1): [red, blue, green] → [green, red, blue]
        assert_eq!(cycled, vec![green(), red(), blue()]);
    }

    #[test]
    fn cycle_backward_by_one() {
        let entries = vec![red(), blue(), green()];
        let cycled = palette_cycle(&entries, 0, 2, -1);
        // rotate_left by 1: [red, blue, green] → [blue, green, red]
        assert_eq!(cycled, vec![blue(), green(), red()]);
    }

    #[test]
    fn cycle_zero_offset_is_noop() {
        let entries = vec![red(), blue(), green()];
        let cycled = palette_cycle(&entries, 0, 2, 0);
        assert_eq!(cycled, entries);
    }

    #[test]
    fn cycle_wraps_full_range() {
        let entries = vec![red(), blue(), green()];
        let cycled = palette_cycle(&entries, 0, 2, 3);
        // rotate_right(3) on len-3 slice = no change
        assert_eq!(cycled, entries);
    }

    #[test]
    fn cycle_affects_only_range() {
        // palette_cycle on a sub-range leaves entries outside unchanged
        let entries = vec![red(), blue(), green(), Rgba::opaque(255, 255, 0)];
        let cycled = palette_cycle(&entries, 1, 2, 1);
        // [1..=2] = [blue, green] → rotate_right(1) → [green, blue]
        assert_eq!(cycled[0], red());
        assert_eq!(cycled[1], green());
        assert_eq!(cycled[2], blue());
        assert_eq!(cycled[3], Rgba::opaque(255, 255, 0));
    }

    #[test]
    fn cycle_out_of_bounds_is_noop() {
        let entries = vec![red(), blue()];
        // last=5 is out of bounds (len=2)
        let cycled = palette_cycle(&entries, 0, 5, 1);
        assert_eq!(cycled, entries);
    }
}
