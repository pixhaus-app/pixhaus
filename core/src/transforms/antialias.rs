//! Morphological anti-aliasing (MLAA).
//!
//! Adapted from `OpenToonz` `toonz/sources/common/trop/tantialias.cpp`
//! under BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
//!
//! Algorithm: Alexander Reshetov, "Morphological Antialiasing"
//! (Intel Labs, 2009).
//!
//! # Approach
//!
//! Two-pass driver. The row pass walks adjacent row pairs looking for
//! runs of columns where the two rows hold different but internally
//! uniform colors. Each run is the visible footprint of a sloped edge
//! that the rasteriser quantised to horizontal steps. The pass classifies
//! each run by its corner geometry, then blends triangular-area coverage
//! into the pixels along the run. The column pass repeats over adjacent
//! column pairs.
//!
//! # Naming
//!
//! The `OpenToonz` source names its two scan rows `L` (lower) and `U`
//! (upper). The convention is that `L = src->pixels(y)` and
//! `U = src->pixels(y + 1)`, so in screen coordinates `L` is the *top*
//! row of the pair and `U` is the *bottom* row. We keep the same `L`/`U`
//! names in this module to make the side-by-side translation easier to
//! audit; the buffer access layer (`Axis::read_l`, `Axis::read_u`) maps
//! `L` to `y` and `U` to `y + 1` for the row pass, and `L` to `x`,
//! `U` to `x + 1` for the column pass.

// The algorithm shuttles between integer scan positions and float
// triangular-area math; the casts are part of its shape. Wrap the
// whole module in the standard set of allows that other transforms
// (rotate.rs, scale.rs) use for the same reason. We also relax
// `similar_names` because the OpenToonz-side names (uniteUL/uniteLL
// /uniteUR/uniteLR) are load-bearing for translation correctness and
// renaming them obscures the audit trail.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::similar_names
)]

use super::error::Result;
use crate::canvas::buffer::PixelBuffer;
use crate::project::Rgba;

/// Configuration for [`morphological_antialias`].
///
/// Defaults to `threshold = 16, softness = 128`, matching `OpenToonz`'s
/// recommended starting values for 8-bit-per-channel RGBA content.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MlaaConfig {
    /// Per-channel maximum difference at or below which two pixels are
    /// treated as equal. A higher value tolerates more colour drift
    /// before a separation line is reported. Range 0..=255.
    pub threshold: u8,
    /// Softening factor applied to each separation-line slope. `0` skips
    /// all filtering and returns the source unchanged. Higher values
    /// extend the triangular coverage farther into each run. Range
    /// 0..=255.
    pub softness: u8,
}

impl Default for MlaaConfig {
    fn default() -> Self {
        Self {
            threshold: 16,
            softness: 128,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Public entry point
// ──────────────────────────────────────────────────────────────────────────────

/// Applies morphological anti-aliasing to `src` and returns a new buffer.
///
/// Two passes: rows first (smooths horizontal-step edges), then columns
/// (smooths vertical-step edges). The two passes compound: a 45° diagonal
/// staircase ends up softened on both axes.
///
/// # Errors
///
/// - [`super::error::Error::Buffer`] on allocation failure of the output
///   or scratch buffers.
///
/// The current implementation never fails internally; the `Result`
/// stays in the signature to keep parity with the rest of the
/// `transforms::` API and to leave room for future variants that may
/// reject inputs (e.g., extremely large allocations under SIMD
/// alignment).
#[allow(clippy::unnecessary_wraps, clippy::trivially_copy_pass_by_ref)]
pub fn morphological_antialias(src: &PixelBuffer, config: &MlaaConfig) -> Result<PixelBuffer> {
    if config.softness == 0 || src.is_empty() {
        return Ok(src.clone());
    }

    let slope = 50.0_f64 / f64::from(config.softness);
    let h_start = 0.5_f64;
    let sel = PixelSelector::new(config.threshold);

    // Row pass: src -> tmp. For each row pair (y, y+1), L = row y,
    // U = row y+1, scan_len = width. The 1-pixel-run secondary-edge
    // probe is enabled (`do_one_line = true`).
    let mut tmp = src.clone();
    let width = src.width();
    let height = src.height();
    if height >= 2 {
        for y in 0..height - 1 {
            let axis = RowAxis {
                y_l: y,
                y_u: y + 1,
                height,
            };
            process_line(src, &mut tmp, axis, width, h_start, slope, sel, true);
        }
    }

    // Column pass: tmp -> dst. For each column pair (x, x+1), L = col x,
    // U = col x+1, scan_len = height. Single-pixel runs disabled
    // (matches OpenToonz `do1Line = false`).
    let mut dst = tmp.clone();
    if width >= 2 {
        for x in 0..width - 1 {
            let axis = ColAxis {
                x_l: x,
                x_u: x + 1,
                width,
            };
            process_line(&tmp, &mut dst, axis, height, h_start, slope, sel, false);
        }
    }

    Ok(dst)
}

// ──────────────────────────────────────────────────────────────────────────────
// Pixel selector
// ──────────────────────────────────────────────────────────────────────────────

/// Per-channel max-diff classifier. Mirrors `OpenToonz`'s `PixelSelector`
/// template specialization for `TPixel32`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct PixelSelector {
    threshold: i32,
}

impl PixelSelector {
    pub(crate) const fn new(threshold: u8) -> Self {
        Self {
            threshold: threshold as i32,
        }
    }

    /// Returns `true` when the per-channel maximum absolute difference
    /// between `a` and `b` is strictly less than the threshold.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(crate) fn are_equal(&self, a: Rgba, b: Rgba) -> bool {
        let dr = (i32::from(a.r) - i32::from(b.r)).abs();
        let dg = (i32::from(a.g) - i32::from(b.g)).abs();
        let db = (i32::from(a.b) - i32::from(b.b)).abs();
        let da = (i32::from(a.a) - i32::from(b.a)).abs();
        let max = dr.max(dg).max(db).max(da);
        max < self.threshold
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Axis abstraction
// ──────────────────────────────────────────────────────────────────────────────
//
// The row pass and column pass differ only in how scan-position and
// off-axis position map to (x, y) pixel coordinates. A small trait
// keeps the `process_line` body axis-agnostic.

trait Axis: Copy {
    /// Reads the "lower" (L) pixel of the row pair at scan position `pos`.
    fn read_l(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba>;
    /// Reads the "upper" (U) pixel of the row pair at scan position `pos`.
    fn read_u(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba>;
    /// Writes the L pixel at scan position `pos`.
    fn write_l(self, buf: &mut PixelBuffer, pos: u32, color: Rgba);
    /// Writes the U pixel at scan position `pos`.
    fn write_u(self, buf: &mut PixelBuffer, pos: u32, color: Rgba);
    /// Reads the L pixel from the row immediately *before* the L row
    /// (visually above L). Returns `None` at the buffer boundary.
    fn read_l_prev_off_axis(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba>;
    /// Reads the U pixel from the row immediately *after* the U row
    /// (visually below U). Returns `None` at the buffer boundary.
    fn read_u_next_off_axis(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba>;
}

#[derive(Copy, Clone)]
struct RowAxis {
    y_l: u32,
    y_u: u32,
    height: u32,
}

impl Axis for RowAxis {
    fn read_l(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        buf.pixel(pos, self.y_l)
    }
    fn read_u(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        buf.pixel(pos, self.y_u)
    }
    fn write_l(self, buf: &mut PixelBuffer, pos: u32, color: Rgba) {
        buf.set_pixel(pos, self.y_l, color);
    }
    fn write_u(self, buf: &mut PixelBuffer, pos: u32, color: Rgba) {
        buf.set_pixel(pos, self.y_u, color);
    }
    fn read_l_prev_off_axis(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        // The "row above L". L is at y = y_l (top of the pair); the row
        // above it is y_l - 1.
        if self.y_l == 0 {
            None
        } else {
            buf.pixel(pos, self.y_l - 1)
        }
    }
    fn read_u_next_off_axis(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        // The "row below U". U is at y_u; the row below is y_u + 1.
        if self.y_u + 1 >= self.height {
            None
        } else {
            buf.pixel(pos, self.y_u + 1)
        }
    }
}

#[derive(Copy, Clone)]
struct ColAxis {
    x_l: u32,
    x_u: u32,
    width: u32,
}

impl Axis for ColAxis {
    fn read_l(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        buf.pixel(self.x_l, pos)
    }
    fn read_u(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        buf.pixel(self.x_u, pos)
    }
    fn write_l(self, buf: &mut PixelBuffer, pos: u32, color: Rgba) {
        buf.set_pixel(self.x_l, pos, color);
    }
    fn write_u(self, buf: &mut PixelBuffer, pos: u32, color: Rgba) {
        buf.set_pixel(self.x_u, pos, color);
    }
    fn read_l_prev_off_axis(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        if self.x_l == 0 {
            None
        } else {
            buf.pixel(self.x_l - 1, pos)
        }
    }
    fn read_u_next_off_axis(self, buf: &PixelBuffer, pos: u32) -> Option<Rgba> {
        if self.x_u + 1 >= self.width {
            None
        } else {
            buf.pixel(self.x_u + 1, pos)
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Test-only edge classifier surface
// ──────────────────────────────────────────────────────────────────────────────

/// A classified separation marker between row `y` and row `y + 1` at
/// column `x`. `present` is `true` when the per-channel max diff exceeds
/// the configured threshold (i.e., the two pixels are *not* equal).
///
/// Only constructed by [`classify_row_edges`]; the production driver
/// classifies edges inline as it scans each row pair.
#[cfg(test)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RowEdge {
    pub present: bool,
}

/// Returns a flat `width * (height - 1)` vector of row-edge markers.
///
/// Index `y * width + x` corresponds to the edge between `src.pixel(x, y)`
/// and `src.pixel(x, y + 1)`. For `height < 2` the vector is empty.
#[cfg(test)]
#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn classify_row_edges(src: &PixelBuffer, config: &MlaaConfig) -> Vec<RowEdge> {
    let width = src.width() as usize;
    let height = src.height() as usize;
    if height < 2 || width == 0 {
        return Vec::new();
    }
    let mut out = vec![RowEdge::default(); width * (height - 1)];
    let sel = PixelSelector::new(config.threshold);
    for y in 0..height - 1 {
        for x in 0..width {
            let (Some(a), Some(b)) = (
                src.pixel(x as u32, y as u32),
                src.pixel(x as u32, (y + 1) as u32),
            ) else {
                continue;
            };
            out[y * width + x] = RowEdge {
                present: !sel.are_equal(a, b),
            };
        }
    }
    out
}

// ──────────────────────────────────────────────────────────────────────────────
// Per-line driver
// ──────────────────────────────────────────────────────────────────────────────
//
// Translation of OpenToonz `processLine`. Walks `scan_len` positions
// along the line, finds runs where (L_color, U_color) differ but stay
// internally uniform, and applies triangular-area coverage at each end.

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn process_line<A: Axis>(
    src: &PixelBuffer,
    dst: &mut PixelBuffer,
    axis: A,
    scan_len: u32,
    h_start: f64,
    slope: f64,
    sel: PixelSelector,
    do_one_line: bool,
) {
    let scan_len = scan_len as usize;
    if scan_len < 2 {
        return;
    }

    let read = |buf: &PixelBuffer, pos: usize| -> Option<(Rgba, Rgba)> {
        let p = pos as u32;
        let l = axis.read_l(buf, p)?;
        let u = axis.read_u(buf, p)?;
        Some((l, u))
    };

    let mut pos: usize = 0;

    // Special case: a run at scan position 0 has no left corner; only
    // the right corner exists. The slope is halved to compensate
    // (matches OpenToonz: `slope / (lLine << 1)`).
    let Some((l0, u0)) = read(src, 0) else {
        return;
    };
    if !sel.are_equal(l0, u0) {
        let mut end = 1;
        while end < scan_len {
            let Some((l, u)) = read(src, end) else {
                break;
            };
            if !(sel.are_equal(l, l0) && sel.are_equal(u, u0)) {
                break;
            }
            end += 1;
        }
        if end < scan_len {
            let l_line = end; // run length in pixels (matches OpenToonz `lLine`)
            let Some((post_l, post_u)) = read(src, end) else {
                return;
            };
            // Last in-run pixels (uniform, so equal to l0/u0):
            let last_l = l0;
            let last_u = u0;
            let unite_ur = sel.are_equal(last_u, post_l);
            let unite_lr = sel.are_equal(last_l, post_u);
            if unite_ur || unite_lr {
                let unite_ur = if unite_ur && unite_lr {
                    // Ambiguous corner. OpenToonz consults a 2-neighbour
                    // probe; we collapse deterministically. The miss
                    // case is a 1-pixel-off blend on rare X-junctions.
                    true
                } else {
                    unite_ur
                };
                let denom = (l_line as f64) * 2.0;
                let per_pixel = slope / denom;
                if check_length_right(src, axis, end, sel, l_line, unite_ur, do_one_line) {
                    apply_coverage(
                        src,
                        dst,
                        axis,
                        end - 1, // start at last in-run pixel
                        -1,
                        l_line,
                        h_start,
                        per_pixel,
                        unite_ur,
                    );
                }
            }
        }
        pos = end;
    }

    // Main scan: find a run start, process both ends, repeat.
    while pos < scan_len {
        // Advance until L != U at `pos`.
        let l0;
        let u0;
        loop {
            if pos >= scan_len {
                return;
            }
            match read(src, pos) {
                Some((l, u)) if !sel.are_equal(l, u) => {
                    l0 = l;
                    u0 = u;
                    break;
                }
                Some(_) => pos += 1,
                None => return,
            }
        }

        // Find run end.
        let mut end = pos + 1;
        while end < scan_len {
            let Some((l, u)) = read(src, end) else {
                break;
            };
            if !(sel.are_equal(l, l0) && sel.are_equal(u, u0)) {
                break;
            }
            end += 1;
        }

        if end >= scan_len {
            // Terminal run: only the left corner is available, slope
            // halved (matches OpenToonz post-loop branch).
            if pos == 0 {
                // pos == 0 was already handled above.
                return;
            }
            let l_line = end - pos;
            let Some((pre_l, pre_u)) = read(src, pos - 1) else {
                return;
            };
            let unite_ul = sel.are_equal(u0, pre_l);
            let unite_ll = sel.are_equal(l0, pre_u);
            if unite_ul || unite_ll {
                let unite_ul = if unite_ul && unite_ll { true } else { unite_ul };
                let denom = (l_line as f64) * 2.0;
                let per_pixel = slope / denom;
                if check_length_left(src, axis, pos, sel, l_line, unite_ul, do_one_line) {
                    apply_coverage(src, dst, axis, pos, 1, l_line, h_start, per_pixel, unite_ul);
                }
            }
            return;
        }

        // Two-sided run.
        let l_line = end - pos;
        let Some((post_l, post_u)) = read(src, end) else {
            return;
        };
        let pre = if pos == 0 { None } else { read(src, pos - 1) };

        // Filter left corner (forward into the run).
        if let Some((pre_l, pre_u)) = pre {
            let unite_ul = sel.are_equal(u0, pre_l);
            let unite_ll = sel.are_equal(l0, pre_u);
            if unite_ul || unite_ll {
                let unite_ul_final = if unite_ul && unite_ll {
                    // Ambiguous; collapse deterministically. Use the
                    // opposite default from the right end so paired
                    // X-junctions stay anti-symmetric and we don't
                    // accidentally bias every X to the same side.
                    false
                } else {
                    unite_ul
                };
                let per_pixel = slope / l_line as f64;
                if check_length_left(src, axis, pos, sel, l_line, unite_ul_final, do_one_line) {
                    apply_coverage(
                        src,
                        dst,
                        axis,
                        pos,
                        1,
                        l_line,
                        h_start,
                        per_pixel,
                        unite_ul_final,
                    );
                }
            }
        }

        // Filter right corner (backward into the run).
        let unite_ur = sel.are_equal(u0, post_l);
        let unite_lr = sel.are_equal(l0, post_u);
        if unite_ur || unite_lr {
            let unite_ur_final = if unite_ur && unite_lr { true } else { unite_ur };
            let per_pixel = slope / l_line as f64;
            if check_length_right(src, axis, end, sel, l_line, unite_ur_final, do_one_line) {
                apply_coverage(
                    src,
                    dst,
                    axis,
                    end - 1,
                    -1,
                    l_line,
                    h_start,
                    per_pixel,
                    unite_ur_final,
                );
            }
        }

        pos = end;
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// `checkLength` — secondary-edge probe for 1-pixel runs
// ──────────────────────────────────────────────────────────────────────────────
//
// For the row pass, OpenToonz allows 1-pixel runs only when the
// secondary edge at the corner actually steps. That probe consults the
// pixels in the row *above* L (or *below* U) at the corner column and
// its neighbour. For multi-pixel runs the probe is irrelevant and
// `checkLength` returns `true` unconditionally.

#[allow(clippy::too_many_arguments)]
fn check_length_left<A: Axis>(
    src: &PixelBuffer,
    axis: A,
    pos: usize,
    sel: PixelSelector,
    l_line: usize,
    unite_upper: bool,
    do_one_line: bool,
) -> bool {
    if l_line > 1 {
        return true;
    }
    if !do_one_line {
        return false;
    }
    // pos is the in-run start; the corner is between (pos-1, pos).
    let p_at_pos = pos as u32;
    if pos == 0 {
        return false; // no left corner exists
    }
    let p_pre = (pos - 1) as u32;
    if unite_upper {
        // Probe L row (the top row) vs the row above it.
        let Some(l_pre_above) = axis.read_l_prev_off_axis(src, p_pre) else {
            return true;
        };
        let Some(l_at_above) = axis.read_l_prev_off_axis(src, p_at_pos) else {
            return true;
        };
        let Some(l_pre) = axis.read_l(src, p_pre) else {
            return false;
        };
        let Some(l_at) = axis.read_l(src, p_at_pos) else {
            return false;
        };
        // Allow the run only when the L row is NOT a continuation of
        // its previous row (i.e., a real corner exists going up).
        !(sel.are_equal(l_pre, l_pre_above) && sel.are_equal(l_at, l_at_above))
    } else {
        let Some(u_pre_below) = axis.read_u_next_off_axis(src, p_pre) else {
            return true;
        };
        let Some(u_at_below) = axis.read_u_next_off_axis(src, p_at_pos) else {
            return true;
        };
        let Some(u_pre) = axis.read_u(src, p_pre) else {
            return false;
        };
        let Some(u_at) = axis.read_u(src, p_at_pos) else {
            return false;
        };
        !(sel.are_equal(u_pre, u_pre_below) && sel.are_equal(u_at, u_at_below))
    }
}

#[allow(clippy::too_many_arguments)]
fn check_length_right<A: Axis>(
    src: &PixelBuffer,
    axis: A,
    end: usize,
    sel: PixelSelector,
    l_line: usize,
    unite_upper: bool,
    do_one_line: bool,
) -> bool {
    if l_line > 1 {
        return true;
    }
    if !do_one_line {
        return false;
    }
    // end is the first post-run position; the corner is between
    // (end-1, end). Probe the off-axis row at end-1 and end.
    if end == 0 {
        return false;
    }
    let p_last = (end - 1) as u32;
    let p_post = end as u32;
    if unite_upper {
        let Some(l_last_above) = axis.read_l_prev_off_axis(src, p_last) else {
            return true;
        };
        let Some(l_post_above) = axis.read_l_prev_off_axis(src, p_post) else {
            return true;
        };
        let Some(l_last) = axis.read_l(src, p_last) else {
            return false;
        };
        let Some(l_post) = axis.read_l(src, p_post) else {
            return false;
        };
        !(sel.are_equal(l_last, l_last_above) && sel.are_equal(l_post, l_post_above))
    } else {
        let Some(u_last_below) = axis.read_u_next_off_axis(src, p_last) else {
            return true;
        };
        let Some(u_post_below) = axis.read_u_next_off_axis(src, p_post) else {
            return true;
        };
        let Some(u_last) = axis.read_u(src, p_last) else {
            return false;
        };
        let Some(u_post) = axis.read_u(src, p_post) else {
            return false;
        };
        !(sel.are_equal(u_last, u_last_below) && sel.are_equal(u_post, u_post_below))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Triangular-area coverage application
// ──────────────────────────────────────────────────────────────────────────────
//
// Translation of OpenToonz `filterLine`. Walks `l_line` pixels along
// `axis` starting from `start_pos` in `step` direction, blending each
// pixel toward the other-side colour with weight equal to the triangle
// area covered.
//
// `filter_l = true` (= OpenToonz `filterLower`) blends the L pixel
// toward the U colour at this column. `filter_l = false` blends the U
// pixel toward the L colour.

#[allow(clippy::too_many_arguments)]
fn apply_coverage<A: Axis>(
    src: &PixelBuffer,
    dst: &mut PixelBuffer,
    axis: A,
    start_pos: usize,
    step: i32,
    l_line: usize,
    h_start: f64,
    slope: f64,
    filter_l: bool,
) {
    if slope <= 0.0 || h_start <= 0.0 {
        return;
    }
    let base = h_start / slope;
    let ll = l_line as f64;
    let full_pixels = base.floor().min(ll) as usize;

    let mut h0 = h_start;
    let mut pos = start_pos as i32;
    for _ in 0..full_pixels {
        let h1 = (h0 - slope).max(0.0);
        let area = 0.5 * (h0 + h1);
        if pos < 0 {
            break;
        }
        let p = pos as u32;
        let Some(in_l) = axis.read_l(src, p) else {
            break;
        };
        let Some(in_u) = axis.read_u(src, p) else {
            break;
        };
        if filter_l {
            // Blend the L (top) pixel toward the U (bottom) colour.
            if let Some(out_l) = axis.read_l(dst, p) {
                let blended = blend_lerp(out_l, in_u, area);
                axis.write_l(dst, p, blended);
            }
        } else if let Some(out_u) = axis.read_u(dst, p) {
            let blended = blend_lerp(out_u, in_l, area);
            axis.write_u(dst, p, blended);
        }
        h0 = h1;
        pos += step;
    }
    if full_pixels < l_line {
        let remnant = base - full_pixels as f64;
        if remnant > 0.0 && pos >= 0 {
            let area = 0.5 * remnant * h0;
            let p = pos as u32;
            let in_l = axis.read_l(src, p);
            let in_u = axis.read_u(src, p);
            if let (Some(in_l), Some(in_u)) = (in_l, in_u) {
                if filter_l {
                    if let Some(out_l) = axis.read_l(dst, p) {
                        let blended = blend_lerp(out_l, in_u, area);
                        axis.write_l(dst, p, blended);
                    }
                } else if let Some(out_u) = axis.read_u(dst, p) {
                    let blended = blend_lerp(out_u, in_l, area);
                    axis.write_u(dst, p, blended);
                }
            }
        }
    }
}

/// Linearly interpolates each channel of `a` toward `b` by `t`. The
/// alpha channel rides along with the same weight, matching `OpenToonz`'s
/// `weightPix` shape.
fn blend_lerp(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    let mix = |x: u8, y: u8| -> u8 {
        let v = f64::from(x) * inv + f64::from(y) * t;
        v.round().clamp(0.0, 255.0) as u8
    };
    Rgba::new(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b), mix(a.a, b.a))
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn black() -> Rgba {
        Rgba::opaque(0, 0, 0)
    }

    fn white() -> Rgba {
        Rgba::opaque(255, 255, 255)
    }

    /// Builds an N×N buffer where the lower-left triangle (`x < y`) is
    /// black and the rest is white — a 45° staircase running top-left
    /// to bottom-right.
    fn staircase_buffer(n: u32) -> PixelBuffer {
        let mut buf = PixelBuffer::filled(n, n, white()).unwrap();
        for y in 0..n {
            for x in 0..n {
                if x < y {
                    buf.set_pixel(x, y, black());
                }
            }
        }
        buf
    }

    #[test]
    fn default_config_has_documented_values() {
        let c = MlaaConfig::default();
        assert_eq!(c.threshold, 16);
        assert_eq!(c.softness, 128);
    }

    #[test]
    fn mlaa_zero_softness_is_identity() {
        let buf = staircase_buffer(16);
        let out = morphological_antialias(
            &buf,
            &MlaaConfig {
                threshold: 16,
                softness: 0,
            },
        )
        .unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn mlaa_flat_buffer_unchanged() {
        let buf = PixelBuffer::filled(8, 8, Rgba::opaque(128, 128, 128)).unwrap();
        let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(out.pixel(x, y), buf.pixel(x, y));
            }
        }
    }

    #[test]
    fn mlaa_empty_buffer_returns_empty() {
        let buf = PixelBuffer::empty();
        let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn selector_treats_equal_pixels_as_equal() {
        let sel = PixelSelector::new(16);
        assert!(sel.are_equal(Rgba::opaque(10, 10, 10), Rgba::opaque(10, 10, 10)));
        // 5-channel diff is below threshold of 16.
        assert!(sel.are_equal(Rgba::opaque(10, 10, 10), Rgba::opaque(15, 10, 10)));
        // 20-channel diff exceeds threshold.
        assert!(!sel.are_equal(Rgba::opaque(10, 10, 10), Rgba::opaque(30, 10, 10)));
    }

    #[test]
    fn mlaa_classifies_horizontal_edge() {
        // 8x4 buffer with top half black, bottom half white -> exactly
        // 8 classified edges between row 1 and row 2 (one per column).
        let mut buf = PixelBuffer::new(8, 4).unwrap();
        for y in 0..2 {
            for x in 0..8 {
                buf.set_pixel(x, y, black());
            }
        }
        for y in 2..4 {
            for x in 0..8 {
                buf.set_pixel(x, y, white());
            }
        }
        let edges = classify_row_edges(&buf, &MlaaConfig::default());
        assert_eq!(edges.len(), 8 * 3);
        let between_1_2 = &edges[8..16];
        assert!(between_1_2.iter().all(|e| e.present));
        assert!(edges[0..8].iter().all(|e| !e.present));
        assert!(edges[16..24].iter().all(|e| !e.present));
    }

    #[test]
    fn mlaa_preserves_horizontal_edge() {
        // A clean horizontal boundary: the run-shape detector finds no
        // corner pixels stepping into the line, so no softening is
        // applied. Output should match input pixel-for-pixel.
        let mut buf = PixelBuffer::new(16, 16).unwrap();
        for y in 0..8 {
            for x in 0..16 {
                buf.set_pixel(x, y, black());
            }
        }
        for y in 8..16 {
            for x in 0..16 {
                buf.set_pixel(x, y, white());
            }
        }
        let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(out.pixel(x, y), buf.pixel(x, y), "drift at ({x}, {y})");
            }
        }
    }

    #[test]
    fn mlaa_preserves_vertical_edge() {
        let mut buf = PixelBuffer::new(16, 16).unwrap();
        for y in 0..16 {
            for x in 0..16 {
                let c = if x < 8 { black() } else { white() };
                buf.set_pixel(x, y, c);
            }
        }
        let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(out.pixel(x, y), buf.pixel(x, y), "drift at ({x}, {y})");
            }
        }
    }

    #[test]
    fn mlaa_softens_45_staircase() {
        let buf = staircase_buffer(16);
        let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
        // The interior of each side should keep its color; the corner
        // pixels of every step should land at an intermediate gray.
        let mut found_intermediate = false;
        let mut found_unchanged_black = false;
        let mut found_unchanged_white = false;
        for y in 0..16 {
            for x in 0..16 {
                let p = out.pixel(x, y).unwrap();
                if p == black() {
                    found_unchanged_black = true;
                } else if p == white() {
                    found_unchanged_white = true;
                } else if p.r == p.g && p.g == p.b && p.r > 0 && p.r < 255 {
                    found_intermediate = true;
                }
            }
        }
        assert!(found_unchanged_black, "no untouched black pixel remained");
        assert!(found_unchanged_white, "no untouched white pixel remained");
        assert!(
            found_intermediate,
            "staircase corners should produce an intermediate gray"
        );
    }

    #[test]
    fn mlaa_respects_threshold() {
        // Buffer of 16×16 with a step from 100 to 110 — well below the
        // default threshold of 16. No edge should be classified.
        let a = Rgba::opaque(100, 100, 100);
        let b = Rgba::opaque(110, 110, 110);
        let mut buf = PixelBuffer::new(16, 16).unwrap();
        for y in 0..16 {
            for x in 0..16 {
                let c = if x < y { a } else { b };
                buf.set_pixel(x, y, c);
            }
        }
        let edges = classify_row_edges(&buf, &MlaaConfig::default());
        let any_present = edges.iter().any(|e| e.present);
        assert!(
            !any_present,
            "10-channel delta must stay under default threshold"
        );
    }

    // Throughput is tracked by the criterion benches in core/benches/,
    // not by a wall-clock unit assertion (which flakes on shared CI).

    #[test]
    fn blend_lerp_is_endpoint_safe() {
        let a = Rgba::new(10, 20, 30, 40);
        let b = Rgba::new(200, 210, 220, 230);
        assert_eq!(blend_lerp(a, b, 0.0), a);
        assert_eq!(blend_lerp(a, b, 1.0), b);
    }

    #[test]
    fn blend_lerp_midpoint_is_average() {
        let a = Rgba::opaque(0, 0, 0);
        let b = Rgba::opaque(200, 200, 200);
        let m = blend_lerp(a, b, 0.5);
        assert_eq!(m, Rgba::opaque(100, 100, 100));
    }

    #[test]
    fn mlaa_preserves_white_corner_of_staircase() {
        // The top-right corner of the staircase is solid white; the
        // top-left and bottom-right corners sit on solid-color regions
        // that the run-shape detector should leave alone.
        let buf = staircase_buffer(8);
        let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
        // Top-right corner: pure white inside the white region.
        assert_eq!(out.pixel(7, 0), Some(white()));
        // Bottom-left corner: pure black inside the black region.
        assert_eq!(out.pixel(0, 7), Some(black()));
    }
}
