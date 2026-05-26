//! Animation sheet normalization.
//!
//! Generated sprite sheets come back with three defects that make them
//! unusable in-game: a flat chroma background, subjects that drift off the
//! foot baseline, and per-frame scale jumps (attack frames often render
//! smaller than idle and walk). This module is the reusable post-pass that
//! locks all three, applied to any sequence of frames — it is not tied to
//! one verb.
//!
//! The pass sequences the seven steps from
//! `docs/planning/work/animation-generation-pipeline.md`:
//!
//! 1. Split frames by the alpha bounding box (the caller does this — slice
//!    the sheet, hand the frames here).
//! 2. Remove the chroma background ([`chroma_key`]).
//! 3. Measure each frame ([`measure`]): visible extent, centre-x, foot
//!    baseline.
//! 4. Correct scale across frames — scale every subject to the reference
//!    visible height.
//! 5. Re-pad each frame to a fixed canvas with a consistent centre-x and
//!    foot baseline ([`repad`]).
//! 6. Rebuild and 7. verify — the caller rebuilds the atlas and previews;
//!    [`NormalizeReport`] carries the drift / scale / seam measurements the
//!    UI surfaces.
//!
//! Two rules from the methodology drive the defaults: the loop seam (last
//! frame must match the first) and the foot baseline (must hold across every
//! frame) are non-negotiable, so both are measured and reported.

use serde::{Deserialize, Serialize};

use crate::canvas::buffer::PixelBuffer;
use crate::project::Rgba;

use super::error::{Error, Result};
use super::scale::scale_nearest;

/// A flat background colour to key out to transparency.
///
/// Generated sheets use a solid magenta (`#FF00FF`) or green (`#00FF00`)
/// backdrop so the subject silhouette is unambiguous; chroma-keying turns
/// that backdrop into alpha without touching the subject.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChromaKey {
    /// The background colour to remove (alpha is ignored on the key).
    pub color: Rgba,
    /// Per-channel tolerance. A pixel keys out when every channel is
    /// within `tolerance` of `color`. `0` keys only exact matches.
    pub tolerance: u8,
}

impl ChromaKey {
    /// Magenta key (`#FF00FF`) with a small tolerance for JPEG-style ringing.
    #[must_use]
    pub const fn magenta() -> Self {
        Self {
            color: Rgba::opaque(255, 0, 255),
            tolerance: 16,
        }
    }

    /// Green key (`#00FF00`) with a small tolerance.
    #[must_use]
    pub const fn green() -> Self {
        Self {
            color: Rgba::opaque(0, 255, 0),
            tolerance: 16,
        }
    }

    /// Returns `true` when `px` is within tolerance of the key colour.
    #[must_use]
    fn matches(self, px: Rgba) -> bool {
        let within = |a: u8, b: u8| a.abs_diff(b) <= self.tolerance;
        within(px.r, self.color.r) && within(px.g, self.color.g) && within(px.b, self.color.b)
    }
}

/// Per-frame alpha bounding box and the derived placement landmarks.
///
/// All coordinates are in the frame's own pixel space. A fully transparent
/// frame has no bbox; its landmarks default to the frame centre / bottom so
/// downstream maths never divides by zero.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameMetrics {
    /// Left edge of the opaque bounding box.
    pub bbox_x: u32,
    /// Top edge of the opaque bounding box.
    pub bbox_y: u32,
    /// Width of the opaque content (`0` for a fully transparent frame).
    pub visible_width: u32,
    /// Height of the opaque content (`0` for a fully transparent frame).
    pub visible_height: u32,
    /// Horizontal centre of the opaque content.
    pub center_x: u32,
    /// Bottom row of opaque pixels — the foot baseline. For a transparent
    /// frame this is the frame's bottom edge.
    pub foot_baseline_y: u32,
    /// `true` when the frame had no opaque pixels.
    pub empty: bool,
}

/// Loop-seam verdict: how close the last frame is to the first.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeamMatch {
    /// First and last frames are within tolerance — the loop is clean.
    Ok,
    /// Close but not exact; usable, surfaced as a soft warning.
    Close,
    /// The loop seam visibly jumps.
    Drift,
}

/// The measurements the normalization review surfaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalizeReport {
    /// Worst foot-baseline deviation, in pixels, across the normalized
    /// frames. `0` means every subject lands on the same baseline.
    pub baseline_drift_px: u32,
    /// How well subject heights agree, as a percentage. `100` means every
    /// subject was already at the reference height before correction.
    pub scale_match_pct: u32,
    /// Loop-seam verdict between the first and last normalized frame.
    pub seam: SeamMatch,
    /// The reference visible height every subject was scaled toward.
    pub reference_height: u32,
    /// Human-readable warnings (empty subject, large drift, scale jump).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Options for [`normalize_frames`].
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizeOptions {
    /// Output canvas width every frame is re-padded to.
    pub canvas_width: u32,
    /// Output canvas height every frame is re-padded to.
    pub canvas_height: u32,
    /// Alpha at or below this value counts as background when measuring
    /// the bounding box.
    pub alpha_threshold: u8,
    /// Optional chroma key applied before measuring. `None` skips
    /// background removal (the frames already have alpha).
    pub chroma: Option<ChromaKey>,
    /// Reference visible height every subject is scaled to. `None` uses the
    /// tallest subject in the sequence, which is what "scale the smaller
    /// attack frames up to the idle/walk height" wants.
    pub reference_height: Option<u32>,
    /// Bottom margin in pixels: the foot baseline is placed this many pixels
    /// above the canvas bottom. Keeps feet off the very edge.
    pub bottom_margin: u32,
}

impl NormalizeOptions {
    /// Square-canvas defaults: magenta key, alpha threshold 8, no bottom
    /// margin, reference height inferred from the tallest frame.
    #[must_use]
    pub fn square(side: u32) -> Self {
        Self {
            canvas_width: side,
            canvas_height: side,
            alpha_threshold: 8,
            chroma: Some(ChromaKey::magenta()),
            reference_height: None,
            bottom_margin: 0,
        }
    }
}

/// Result of [`normalize_frames`]: the locked frames plus their measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizeResult {
    /// Normalized frames, in input order, all at the requested canvas size.
    pub frames: Vec<PixelBuffer>,
    /// Per-frame metrics measured on the normalized output.
    pub metrics: Vec<FrameMetrics>,
    /// Aggregate drift / scale / seam report.
    pub report: NormalizeReport,
}

/// Keys `key.color` out of `buf` to transparency, leaving the subject
/// untouched. Pixels within tolerance get alpha `0`; their RGB is preserved
/// so a later opacity tweak doesn't reveal a colour shift.
#[must_use]
pub fn chroma_key(buf: &PixelBuffer, key: ChromaKey) -> PixelBuffer {
    let mut out = buf.clone();
    for y in 0..buf.height() {
        for x in 0..buf.width() {
            if let Some(px) = buf.pixel(x, y)
                && px.a != 0
                && key.matches(px)
            {
                out.set_pixel(x, y, Rgba::new(px.r, px.g, px.b, 0));
            }
        }
    }
    out
}

/// Measures the opaque bounding box of `buf` and the placement landmarks.
///
/// `alpha_threshold` sets the cutoff: alpha strictly greater than the
/// threshold counts as opaque content.
#[must_use]
pub fn measure(buf: &PixelBuffer, alpha_threshold: u8) -> FrameMetrics {
    let (w, h) = (buf.width(), buf.height());
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if let Some(px) = buf.pixel(x, y)
                && px.a > alpha_threshold
            {
                any = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if !any {
        return FrameMetrics {
            bbox_x: 0,
            bbox_y: 0,
            visible_width: 0,
            visible_height: 0,
            center_x: w / 2,
            foot_baseline_y: h.saturating_sub(1),
            empty: true,
        };
    }
    let visible_width = max_x - min_x + 1;
    let visible_height = max_y - min_y + 1;
    FrameMetrics {
        bbox_x: min_x,
        bbox_y: min_y,
        visible_width,
        visible_height,
        center_x: min_x + visible_width / 2,
        foot_baseline_y: max_y,
        empty: false,
    }
}

/// Auto-detects the likely background colour by sampling the 1px border of
/// `buf` and returning the most common fully-opaque colour there.
///
/// A flat-background sheet has the same colour all around its edge, so the
/// border's dominant opaque colour is a good first guess for the key. Returns
/// `None` when the border has no opaque pixels (already-transparent edges).
#[must_use]
pub fn detect_key_color(buf: &PixelBuffer) -> Option<Rgba> {
    let (w, h) = (buf.width(), buf.height());
    if w == 0 || h == 0 {
        return None;
    }
    // Tally opaque colours around the 1px border. Corners counted once.
    let mut counts: std::collections::HashMap<(u8, u8, u8), u32> = std::collections::HashMap::new();
    let mut tally = |px: Rgba| {
        if px.a != 0 {
            *counts.entry((px.r, px.g, px.b)).or_insert(0) += 1;
        }
    };
    for x in 0..w {
        if let Some(px) = buf.pixel(x, 0) {
            tally(px);
        }
        if h > 1 {
            if let Some(px) = buf.pixel(x, h - 1) {
                tally(px);
            }
        }
    }
    for y in 1..h.saturating_sub(1) {
        if let Some(px) = buf.pixel(0, y) {
            tally(px);
        }
        if w > 1 {
            if let Some(px) = buf.pixel(w - 1, y) {
                tally(px);
            }
        }
    }
    counts.into_iter().max_by_key(|&(_, count)| count).map(|((r, g, b), _)| Rgba::opaque(r, g, b))
}

/// How a chroma-key attempt turned out, judged by how much opaque content it
/// removed. Drives the "this frame still needs AI" flag in the UI.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Keying removed a plausible slice of the frame; the result looks usable.
    Ok,
    /// Keying removed almost nothing — the key colour probably missed the
    /// background. A likely candidate for the AI fallback.
    Missed,
    /// Keying removed almost everything — the key was too broad and ate the
    /// subject. Also a candidate for the AI fallback (or a tighter tolerance).
    TooBroad,
}

/// Judges a chroma-key result by comparing opaque coverage before and after.
///
/// `alpha_threshold` sets the opaque cutoff (alpha strictly greater counts).
/// A key that clears under [`KEY_MISSED_PERCENT`] of the opaque pixels missed;
/// one that leaves under [`KEY_TOO_BROAD_PERCENT`] of the canvas opaque was too
/// broad. Anything between reads as a usable removal. Comparisons are integer
/// (cross-multiplied) so there are no float casts on the pixel counts.
#[must_use]
pub fn judge_key(before: &PixelBuffer, after: &PixelBuffer, alpha_threshold: u8) -> KeyOutcome {
    let total = u64::from(after.width()) * u64::from(after.height());
    let before_opaque = opaque_count(before, alpha_threshold);
    let after_opaque = opaque_count(after, alpha_threshold);
    if before_opaque == 0 || total == 0 {
        return KeyOutcome::Ok;
    }
    let removed = before_opaque.saturating_sub(after_opaque);
    if removed * 100 < before_opaque * KEY_MISSED_PERCENT {
        return KeyOutcome::Missed;
    }
    if after_opaque * 100 < total * KEY_TOO_BROAD_PERCENT {
        return KeyOutcome::TooBroad;
    }
    KeyOutcome::Ok
}

/// A key that removes less than this percent of the opaque pixels is judged to
/// have missed the background.
pub const KEY_MISSED_PERCENT: u64 = 2;

/// A key that leaves less than this percent of the canvas opaque is judged to
/// have eaten the subject.
pub const KEY_TOO_BROAD_PERCENT: u64 = 2;

/// Counts pixels whose alpha is strictly greater than `alpha_threshold`.
fn opaque_count(buf: &PixelBuffer, alpha_threshold: u8) -> u64 {
    let mut count = 0u64;
    for y in 0..buf.height() {
        for x in 0..buf.width() {
            if let Some(px) = buf.pixel(x, y)
                && px.a > alpha_threshold
            {
                count += 1;
            }
        }
    }
    count
}

/// Re-pads `subject` onto a fresh transparent `canvas_width × canvas_height`
/// buffer so the subject's horizontal centre lands on the canvas centre and
/// its bottom row lands `bottom_margin` pixels above the canvas bottom.
///
/// `subject` is expected to be a tightly-cropped opaque region (the alpha
/// bbox crop). Pixels that would fall outside the canvas are clipped.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if the canvas would be `0 × 0`.
pub fn repad(subject: &PixelBuffer, canvas_width: u32, canvas_height: u32, bottom_margin: u32) -> Result<PixelBuffer> {
    if canvas_width == 0 || canvas_height == 0 {
        return Err(Error::EmptyBuffer);
    }
    let mut canvas = PixelBuffer::new(canvas_width, canvas_height)?;
    let sw = subject.width();
    let sh = subject.height();
    if sw == 0 || sh == 0 {
        return Ok(canvas);
    }
    // Centre horizontally; bottom-align with the requested margin. Signed
    // maths so an oversized subject overhangs symmetrically rather than
    // pinning to the left/top.
    let dest_x = (i64::from(canvas_width) - i64::from(sw)) / 2;
    let dest_y = i64::from(canvas_height) - i64::from(bottom_margin) - i64::from(sh);
    for sy in 0..sh {
        let ty = dest_y + i64::from(sy);
        if ty < 0 || ty >= i64::from(canvas_height) {
            continue;
        }
        for sx in 0..sw {
            let tx = dest_x + i64::from(sx);
            if tx < 0 || tx >= i64::from(canvas_width) {
                continue;
            }
            if let Some(px) = subject.pixel(sx, sy)
                && px.a != 0
            {
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                canvas.set_pixel(tx as u32, ty as u32, px);
            }
        }
    }
    Ok(canvas)
}

/// The shared height every subject is scaled toward.
///
/// Defaults to the tallest subject, but caps it so the tallest-and-widest
/// subject still fits the output cell — i2v source frames are hundreds of
/// pixels tall, and without the cap a subject keeps its native height,
/// overhangs a small cell, and re-pad clips it to a blank strip.
fn reference_height(source_metrics: &[FrameMetrics], max_visible: u32, opts: &NormalizeOptions) -> u32 {
    let max_visible_width = source_metrics.iter().map(|m| m.visible_width).max().unwrap_or(0);
    let fit_height = fit_reference_height(
        max_visible_width,
        max_visible,
        opts.canvas_width,
        opts.canvas_height.saturating_sub(opts.bottom_margin),
    );
    opts.reference_height.unwrap_or(max_visible).min(fit_height).max(1)
}

/// Largest reference height that keeps a `w × h` subject inside a
/// `canvas_w × canvas_h` cell, preserving aspect.
///
/// Capped by the canvas height directly, and by the height at which the
/// subject's width just fills the canvas. Returns `canvas_h` when the subject
/// has no extent.
fn fit_reference_height(w: u32, h: u32, canvas_w: u32, canvas_h: u32) -> u32 {
    if w == 0 || h == 0 {
        return canvas_h.max(1);
    }
    let by_width = u32::try_from(u64::from(h) * u64::from(canvas_w) / u64::from(w)).unwrap_or(canvas_h);
    canvas_h.min(by_width).max(1)
}

/// Crops `buf` to the rectangle `(x, y, w, h)`, clamped to the buffer.
fn crop(buf: &PixelBuffer, x: u32, y: u32, w: u32, h: u32) -> Result<PixelBuffer> {
    let w = w.min(buf.width().saturating_sub(x)).max(1);
    let h = h.min(buf.height().saturating_sub(y)).max(1);
    let mut out = PixelBuffer::new(w, h)?;
    for oy in 0..h {
        for ox in 0..w {
            if let Some(px) = buf.pixel(x + ox, y + oy) {
                out.set_pixel(ox, oy, px);
            }
        }
    }
    Ok(out)
}

/// Crops one keyed frame to its subject, scales it to the reference height,
/// and re-pads it onto the fixed canvas. Empty frames return a blank canvas.
fn normalize_one(frame: &PixelBuffer, m: &FrameMetrics, reference_height: u32, opts: &NormalizeOptions) -> Result<PixelBuffer> {
    if m.empty {
        return Ok(PixelBuffer::new(opts.canvas_width, opts.canvas_height)?);
    }
    let cropped = crop(frame, m.bbox_x, m.bbox_y, m.visible_width, m.visible_height)?;
    let subject = if m.visible_height == reference_height {
        cropped
    } else {
        // Preserve aspect: width scales by the same factor as height.
        let new_h = reference_height;
        let new_w = u32::try_from((u64::from(m.visible_width) * u64::from(new_h) + u64::from(m.visible_height) / 2) / u64::from(m.visible_height))
            .unwrap_or(m.visible_width)
            .max(1);
        scale_nearest(&cropped, new_w, new_h)?
    };
    repad(&subject, opts.canvas_width, opts.canvas_height, opts.bottom_margin)
}

/// Runs the full normalization pass over `frames`.
///
/// Every frame is chroma-keyed (if requested), measured, scaled to the
/// reference visible height, and re-padded onto the fixed canvas with a
/// locked centre-x and foot baseline. The returned [`NormalizeReport`]
/// carries the drift, scale-match, and loop-seam measurements.
///
/// # Errors
///
/// Returns an error if the canvas is degenerate or an internal scale /
/// crop step fails.
pub fn normalize_frames(frames: &[PixelBuffer], opts: &NormalizeOptions) -> Result<NormalizeResult> {
    if frames.is_empty() {
        return Ok(NormalizeResult {
            frames: Vec::new(),
            metrics: Vec::new(),
            report: NormalizeReport {
                baseline_drift_px: 0,
                scale_match_pct: 100,
                seam: SeamMatch::Ok,
                reference_height: 0,
                warnings: vec!["no frames to normalize".into()],
            },
        });
    }

    // Pass 1: chroma-key and measure to discover the reference height.
    let keyed: Vec<PixelBuffer> = frames
        .iter()
        .map(|f| match opts.chroma {
            Some(k) => chroma_key(f, k),
            None => f.clone(),
        })
        .collect();
    let source_metrics: Vec<FrameMetrics> = keyed.iter().map(|f| measure(f, opts.alpha_threshold)).collect();

    let max_visible = source_metrics.iter().map(|m| m.visible_height).max().unwrap_or(0);
    let reference_height = reference_height(&source_metrics, max_visible, opts);

    let mut warnings = Vec::new();
    let mut min_visible = u32::MAX;
    for (i, m) in source_metrics.iter().enumerate() {
        if m.empty {
            warnings.push(format!("frame {i} has no opaque pixels"));
        } else {
            min_visible = min_visible.min(m.visible_height);
        }
    }

    // Pass 2: crop to bbox, scale to the reference height, re-pad.
    let mut out_frames = Vec::with_capacity(keyed.len());
    for (frame, m) in keyed.iter().zip(&source_metrics) {
        out_frames.push(normalize_one(frame, m, reference_height, opts)?);
    }

    // Measure the normalized output to verify baseline lock.
    let out_metrics: Vec<FrameMetrics> = out_frames.iter().map(|f| measure(f, opts.alpha_threshold)).collect();
    let target_baseline = opts.canvas_height.saturating_sub(opts.bottom_margin).saturating_sub(1);
    let baseline_drift_px = out_metrics
        .iter()
        .filter(|m| !m.empty)
        .map(|m| m.foot_baseline_y.abs_diff(target_baseline))
        .max()
        .unwrap_or(0);
    if baseline_drift_px > 0 {
        warnings.push(format!("foot baseline drifts up to {baseline_drift_px}px after normalization"));
    }

    let scale_match_pct = if max_visible == 0 || min_visible == u32::MAX {
        100
    } else {
        u32::try_from(u64::from(min_visible) * 100 / u64::from(max_visible)).unwrap_or(100)
    };
    if scale_match_pct < 60 {
        warnings.push(format!("subject heights vary widely ({scale_match_pct}% match) before scale correction"));
    }

    let seam = seam_match(out_frames.first(), out_frames.last());
    if seam == SeamMatch::Drift {
        warnings.push("loop seam: last frame differs noticeably from the first".into());
    }

    Ok(NormalizeResult {
        frames: out_frames,
        metrics: out_metrics,
        report: NormalizeReport {
            baseline_drift_px,
            scale_match_pct,
            seam,
            reference_height,
            warnings,
        },
    })
}

/// Classifies the loop seam by mean per-channel difference between the first
/// and last frame.
fn seam_match(first: Option<&PixelBuffer>, last: Option<&PixelBuffer>) -> SeamMatch {
    let (Some(a), Some(b)) = (first, last) else {
        return SeamMatch::Ok;
    };
    if a.width() != b.width() || a.height() != b.height() || a.is_empty() {
        return SeamMatch::Ok;
    }
    let mut total: u64 = 0;
    let mut count: u64 = 0;
    for y in 0..a.height() {
        for x in 0..a.width() {
            let (Some(pa), Some(pb)) = (a.pixel(x, y), b.pixel(x, y)) else {
                continue;
            };
            total += u64::from(pa.r.abs_diff(pb.r));
            total += u64::from(pa.g.abs_diff(pb.g));
            total += u64::from(pa.b.abs_diff(pb.b));
            total += u64::from(pa.a.abs_diff(pb.a));
            count += 4;
        }
    }
    if count == 0 {
        return SeamMatch::Ok;
    }
    // Mean difference per channel, 0..=255.
    let mean = total / count;
    if mean <= 4 {
        SeamMatch::Ok
    } else if mean <= 20 {
        SeamMatch::Close
    } else {
        SeamMatch::Drift
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(w, h, color).unwrap()
    }

    #[test]
    fn chroma_key_removes_background_keeps_subject() {
        let mut buf = solid(4, 4, Rgba::opaque(255, 0, 255));
        buf.set_pixel(1, 1, Rgba::opaque(10, 20, 30));
        let out = chroma_key(&buf, ChromaKey::magenta());
        assert_eq!(out.pixel(0, 0).unwrap().a, 0, "background keyed out");
        assert_eq!(out.pixel(1, 1), Some(Rgba::opaque(10, 20, 30)), "subject kept");
    }

    #[test]
    fn measure_finds_bbox_and_baseline() {
        let mut buf = PixelBuffer::new(8, 8).unwrap();
        buf.set_pixel(2, 1, Rgba::opaque(255, 255, 255));
        buf.set_pixel(4, 5, Rgba::opaque(255, 255, 255));
        let m = measure(&buf, 0);
        assert!(!m.empty);
        assert_eq!(m.bbox_x, 2);
        assert_eq!(m.bbox_y, 1);
        assert_eq!(m.visible_width, 3); // x 2..=4
        assert_eq!(m.visible_height, 5); // y 1..=5
        assert_eq!(m.foot_baseline_y, 5);
        assert_eq!(m.center_x, 2 + 3 / 2);
    }

    #[test]
    fn measure_empty_frame() {
        let buf = PixelBuffer::new(4, 4).unwrap();
        let m = measure(&buf, 0);
        assert!(m.empty);
        assert_eq!(m.visible_width, 0);
    }

    #[test]
    fn repad_centers_and_bottom_aligns() {
        let subject = solid(2, 2, Rgba::opaque(255, 0, 0));
        let out = repad(&subject, 6, 6, 0).unwrap();
        // Centred horizontally: subject occupies x 2..=3.
        assert_eq!(out.pixel(2, 5), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(out.pixel(3, 5), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(out.pixel(2, 5).unwrap().a, 255);
        assert_eq!(out.pixel(0, 0).unwrap().a, 0);
    }

    #[test]
    fn normalize_locks_baseline_across_frames() {
        let mut f0 = solid(16, 16, Rgba::opaque(255, 0, 255));
        for y in 2..6 {
            for x in 6..10 {
                f0.set_pixel(x, y, Rgba::opaque(0, 0, 0));
            }
        }
        let mut f1 = solid(16, 16, Rgba::opaque(255, 0, 255));
        for y in 8..14 {
            for x in 5..11 {
                f1.set_pixel(x, y, Rgba::opaque(0, 0, 0));
            }
        }
        let opts = NormalizeOptions::square(16);
        let res = normalize_frames(&[f0, f1], &opts).unwrap();
        assert_eq!(res.frames.len(), 2);
        let b0 = res.metrics[0].foot_baseline_y;
        let b1 = res.metrics[1].foot_baseline_y;
        assert_eq!(b0, b1, "baselines locked");
        assert_eq!(res.report.baseline_drift_px, 0);
    }

    #[test]
    fn normalize_empty_input() {
        let opts = NormalizeOptions::square(16);
        let res = normalize_frames(&[], &opts).unwrap();
        assert!(res.frames.is_empty());
        assert!(!res.report.warnings.is_empty());
    }

    #[test]
    fn detect_key_color_picks_the_dominant_border_color() {
        // Magenta border, a different-coloured subject in the middle.
        let mut buf = solid(8, 8, Rgba::opaque(255, 0, 255));
        for y in 2..6 {
            for x in 2..6 {
                buf.set_pixel(x, y, Rgba::opaque(10, 20, 30));
            }
        }
        assert_eq!(detect_key_color(&buf), Some(Rgba::opaque(255, 0, 255)));
    }

    #[test]
    fn detect_key_color_none_when_border_transparent() {
        let buf = PixelBuffer::new(8, 8).unwrap();
        assert_eq!(detect_key_color(&buf), None);
    }

    #[test]
    fn judge_key_ok_when_flat_background_removed() {
        let mut buf = solid(16, 16, Rgba::opaque(255, 0, 255));
        for y in 4..12 {
            for x in 4..12 {
                buf.set_pixel(x, y, Rgba::opaque(10, 20, 30));
            }
        }
        let keyed = chroma_key(&buf, ChromaKey::magenta());
        assert_eq!(judge_key(&buf, &keyed, 8), KeyOutcome::Ok);
    }

    #[test]
    fn judge_key_missed_when_key_clears_nothing() {
        let mut buf = solid(16, 16, Rgba::opaque(255, 0, 255));
        buf.set_pixel(8, 8, Rgba::opaque(10, 20, 30));
        // Green key against a magenta sheet removes nothing.
        let keyed = chroma_key(&buf, ChromaKey::green());
        assert_eq!(judge_key(&buf, &keyed, 8), KeyOutcome::Missed);
    }

    #[test]
    fn judge_key_too_broad_when_key_eats_the_subject() {
        // The subject *is* magenta too, so a magenta key clears the whole frame.
        let buf = solid(16, 16, Rgba::opaque(255, 0, 255));
        let keyed = chroma_key(&buf, ChromaKey::magenta());
        assert_eq!(judge_key(&buf, &keyed, 8), KeyOutcome::TooBroad);
    }

    #[test]
    fn fit_reference_height_caps_by_width_and_height() {
        // Tall, narrow: height-bound.
        assert_eq!(fit_reference_height(40, 200, 64, 64), 64);
        // Wide, short: width-bound — fitting width gives a smaller height.
        assert_eq!(fit_reference_height(200, 100, 64, 64), 32);
        // No extent: falls back to the canvas height.
        assert_eq!(fit_reference_height(0, 0, 64, 64), 64);
    }
}
