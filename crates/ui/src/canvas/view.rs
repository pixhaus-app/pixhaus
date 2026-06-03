//! Pure canvas-view math: the camera that places the sprite on the stage.
//!
//! Egui-free of any `Context` so it is unit-testable without a UI (it names egui's
//! `Copy` value types — `Vec2`/`Rect`/`Pos2` — which construct without a frame). The
//! canvas stage and every future drawing tool derive their geometry from these
//! functions, so screen-to-pixel mapping cannot drift between display and input.
//!
//! The model is a single scale + pan, not a matrix: `zoom` is the scale in screen
//! points per sprite pixel (so `zoom == 1.0` is a literal 1:1, an honest 100%), and
//! the artboard is the sprite centered on the stage plus the pan offset:
//! `artboard = Rect::from_center_size(stage_center + pan, sprite_px * scale)`. Pan/zoom
//! are pure artboard geometry at the egui level; the wgpu renderer never sees them.

use egui::{Pos2, Rect, Vec2};

/// Minimum scale: screen points per sprite pixel. At `0.05` a 4096px sprite is ~205pt
/// wide — small but still grabbable, so zoom-out can never lose the board entirely.
pub const MIN_SCALE: f32 = 0.05;

/// Maximum scale: 64 screen points per sprite pixel. Generous for pixel inspection and
/// kept `>= 16` so the existing `SetZoom(16.0)` path stays in range.
pub const MAX_SCALE: f32 = 64.0;

/// Fraction of the stage a fit fills, leaving a margin so the board never kisses the
/// panel edge.
pub const FIT_MARGIN: f32 = 0.9;

/// Points of the board kept on-stage on each axis, so a pan can never push it fully
/// off-screen and out of reach.
const MIN_VISIBLE_PX: f32 = 24.0;

/// Clamp a proposed scale to the reachable `[MIN_SCALE, MAX_SCALE]` range.
#[must_use]
pub fn clamp_scale(scale: f32) -> f32 {
    scale.clamp(MIN_SCALE, MAX_SCALE)
}

/// The scale that fits `sprite_px` into `margin` of `stage_size`.
///
/// Picks the limiting axis (the smaller of the width/height ratios) so the whole sprite
/// stays visible, then clamps to `[MIN_SCALE, MAX_SCALE]`. Returns the raw (continuous)
/// scale; pixel-perfect snapping is applied separately by [`snap_scale`] so continuous
/// mode keeps the exact fit. A degenerate (zero-area) sprite falls back to `1.0`.
#[must_use]
pub fn fit_scale(stage_size: Vec2, sprite_px: Vec2, margin: f32) -> f32 {
    if sprite_px.x <= 0.0 || sprite_px.y <= 0.0 {
        return 1.0;
    }
    let by_w = stage_size.x * margin / sprite_px.x;
    let by_h = stage_size.y * margin / sprite_px.y;
    clamp_scale(by_w.min(by_h))
}

/// Snap a scale to the nearest pixel-perfect step (pixel-perfect zoom mode only).
///
/// At or above `1.0`, the nearest whole number of points per sprite pixel (1x, 2x, 3x…)
/// so cells stay square. Below `1.0`, the nearest unit fraction `1/n` (1/2, 1/3…) so a
/// whole number of sprite pixels maps to one point and the downscale reads evenly. The
/// result is clamped to the reachable range.
#[must_use]
pub fn snap_scale(scale: f32) -> f32 {
    let snapped = if scale >= 1.0 {
        scale.round()
    } else {
        let n = (1.0 / scale).round().max(1.0);
        1.0 / n
    };
    clamp_scale(snapped)
}

/// The next zoom scale one discrete step in (`zoom_in`) or out from `scale`.
///
/// In pixel-perfect mode the step lands on the adjacent whole points-per-pixel (or unit
/// fraction below 1x), so it always moves by a visible amount — a fixed multiplier can
/// round back to the same integer and stick. In continuous mode it scales by a fixed
/// factor. Always clamped to the reachable range.
#[must_use]
pub fn zoom_step(scale: f32, zoom_in: bool, pixel_perfect: bool) -> f32 {
    if !pixel_perfect {
        let factor = if zoom_in { 1.25 } else { 0.8 };
        return clamp_scale(scale * factor);
    }
    let snapped = snap_scale(scale);
    let next = if snapped >= 1.0 {
        if zoom_in { snapped + 1.0 } else { snapped - 1.0 }
    } else {
        let n = (1.0 / snapped).round().max(1.0);
        if zoom_in {
            if n <= 1.0 { 1.0 } else { 1.0 / (n - 1.0) }
        } else {
            1.0 / (n + 1.0)
        }
    };
    // Crossing 1x downward lands on 0; the step below 1x is a half.
    let next = if next <= 0.0 { 0.5 } else { next };
    clamp_scale(next)
}

/// The on-screen artboard rect: the sprite centered on the stage, displaced by `pan`.
#[must_use]
pub fn artboard_rect(stage_rect: Rect, sprite_px: Vec2, scale: f32, pan: Vec2) -> Rect {
    Rect::from_center_size(stage_rect.center() + pan, sprite_px * scale)
}

/// Map a screen position to sprite-pixel coordinates (origin at the artboard top-left).
///
/// The inverse of [`sprite_to_screen`]; the foundation the hover readout and future
/// drawing tools sample. Returns `(0, 0)` for a non-positive scale rather than dividing.
#[must_use]
pub fn screen_to_sprite(screen_pos: Pos2, artboard_min: Pos2, scale: f32) -> Vec2 {
    if scale <= 0.0 {
        return Vec2::ZERO;
    }
    (screen_pos - artboard_min) / scale
}

/// Map a sprite-pixel coordinate back to a screen position. Inverse of
/// [`screen_to_sprite`].
#[must_use]
pub fn sprite_to_screen(sprite_pos: Vec2, artboard_min: Pos2, scale: f32) -> Pos2 {
    artboard_min + sprite_pos * scale
}

/// The new pan that keeps the sprite point currently under `cursor` fixed as the scale
/// changes from `old_scale` to `new_scale` (cursor-anchored zoom).
///
/// Derivation: the artboard top-left is `min = center + pan - sprite_px*scale/2`, and a
/// screen point `p` maps to sprite coords `s = (p - min) / scale`. Holding `s` under
/// `cursor` across the scale change gives `new_min = cursor - (cursor - old_min) *
/// (new_scale/old_scale)`, and inverting the `min` relation yields the pan below. When
/// the scale is unchanged the ratio is 1 and the pan is returned untouched.
#[must_use]
pub fn zoom_anchored(stage_center: Pos2, sprite_px: Vec2, old_scale: f32, pan: Vec2, cursor: Pos2, new_scale: f32) -> Vec2 {
    if old_scale <= 0.0 {
        return pan;
    }
    let old_min = stage_center + pan - sprite_px * old_scale / 2.0;
    let new_min = cursor - (cursor - old_min) * (new_scale / old_scale);
    (new_min - stage_center) + sprite_px * new_scale / 2.0
}

/// Clamp `pan` so at least `MIN_VISIBLE_PX` of the board stays inside `stage_rect` on
/// each axis. Re-applied every frame, so the geometry-free `SetPan` intent and any
/// runaway drag self-correct; idempotent once the board is in bounds.
#[must_use]
pub fn clamp_pan(stage_rect: Rect, sprite_px: Vec2, scale: f32, pan: Vec2) -> Vec2 {
    let half = sprite_px * scale / 2.0;
    let center = stage_rect.center() + pan;
    let cx = clamp_axis(center.x, half.x, stage_rect.left(), stage_rect.right());
    let cy = clamp_axis(center.y, half.y, stage_rect.top(), stage_rect.bottom());
    egui::vec2(cx - stage_rect.center().x, cy - stage_rect.center().y)
}

/// Clamp one axis of the board center so at least `min(MIN_VISIBLE_PX, board width)`
/// stays between `lo_edge` and `hi_edge`. Falls back to the midpoint when the stage is
/// narrower than the kept margin.
fn clamp_axis(center: f32, half: f32, lo_edge: f32, hi_edge: f32) -> f32 {
    let visible = MIN_VISIBLE_PX.min(2.0 * half);
    let min_center = lo_edge + visible - half;
    let max_center = hi_edge - visible + half;
    if min_center <= max_center {
        center.clamp(min_center, max_center)
    } else {
        f32::midpoint(min_center, max_center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn clamp_scale_bounds_the_range() {
        assert!(approx(clamp_scale(0.0), MIN_SCALE), "below the floor clamps up");
        assert!(approx(clamp_scale(1000.0), MAX_SCALE), "above the ceiling clamps down");
        assert!(approx(clamp_scale(4.0), 4.0), "an in-range scale is untouched");
    }

    #[test]
    fn fit_scale_floors_to_fill_a_small_sprite() {
        // 64px sprite in an 800x600 stage at 90%: 800*0.9/64 = 11.25, 600*0.9/64 = 8.43.
        // The limiting axis is height -> 8.4375 (continuous; snapping is separate).
        let scale = fit_scale(egui::vec2(800.0, 600.0), egui::vec2(64.0, 64.0), FIT_MARGIN);
        assert!(approx(scale, 600.0 * 0.9 / 64.0), "fit uses the limiting (height) axis, raw");
    }

    #[test]
    fn fit_scale_picks_the_limiting_axis() {
        // A wide 256x16 sprite in a square stage is limited by width.
        let scale = fit_scale(egui::vec2(400.0, 400.0), egui::vec2(256.0, 16.0), FIT_MARGIN);
        assert!(approx(scale, 400.0 * 0.9 / 256.0), "the wide axis limits the fit");
    }

    #[test]
    fn fit_scale_is_subunity_and_clamped_for_a_huge_sprite() {
        // 4096px sprite in an 800-wide stage -> well below 1.0, but never below MIN.
        let scale = fit_scale(egui::vec2(800.0, 800.0), egui::vec2(4096.0, 4096.0), FIT_MARGIN);
        assert!(scale < 1.0, "a sprite larger than the stage fits at a fractional scale");
        assert!(scale >= MIN_SCALE, "the fit never drops below the floor");
    }

    #[test]
    fn fit_scale_falls_back_for_a_degenerate_sprite() {
        assert!(approx(fit_scale(egui::vec2(800.0, 600.0), egui::vec2(0.0, 0.0), FIT_MARGIN), 1.0));
    }

    #[test]
    fn snap_scale_rounds_to_integers_when_zoomed_in() {
        assert!(approx(snap_scale(3.4), 3.0), "rounds down to the nearest integer");
        assert!(approx(snap_scale(3.6), 4.0), "rounds up to the nearest integer");
        assert!(approx(snap_scale(1.0), 1.0), "1x is already snapped");
    }

    #[test]
    fn snap_scale_uses_unit_fractions_when_zoomed_out() {
        // 0.45 -> 1/round(1/0.45)=1/round(2.22)=1/2=0.5.
        assert!(approx(snap_scale(0.45), 0.5), "snaps to the nearest 1/n below 1x");
        assert!(approx(snap_scale(0.3), 1.0 / 3.0), "0.3 -> 1/3");
    }

    #[test]
    fn zoom_step_always_moves_in_pixel_perfect_mode() {
        // The trap a fixed multiplier hits: 1.0 * 1.25 rounds back to 1. Stepping must move.
        assert!(approx(zoom_step(1.0, true, true), 2.0), "1x steps up to 2x");
        assert!(approx(zoom_step(1.0, false, true), 0.5), "1x steps down to 1/2");
        assert!(approx(zoom_step(0.5, true, true), 1.0), "1/2 steps up to 1x");
        assert!(approx(zoom_step(0.5, false, true), 1.0 / 3.0), "1/2 steps down to 1/3");
        assert!(approx(zoom_step(4.0, true, true), 5.0), "4x steps up to 5x");
    }

    #[test]
    fn zoom_step_scales_by_a_factor_in_continuous_mode() {
        assert!(approx(zoom_step(4.0, true, false), 5.0), "continuous zoom-in multiplies by 1.25");
        assert!(approx(zoom_step(5.0, false, false), 4.0), "continuous zoom-out multiplies by 0.8");
    }

    #[test]
    fn screen_to_sprite_round_trips() {
        let artboard_min = egui::pos2(100.0, 50.0);
        let scale = 7.0;
        let sprite = egui::vec2(12.5, 3.25);
        let screen = sprite_to_screen(sprite, artboard_min, scale);
        let back = screen_to_sprite(screen, artboard_min, scale);
        assert!(approx(back.x, sprite.x) && approx(back.y, sprite.y), "the transform inverts itself");
    }

    #[test]
    fn screen_to_sprite_guards_zero_scale() {
        assert_eq!(screen_to_sprite(egui::pos2(5.0, 5.0), egui::Pos2::ZERO, 0.0), Vec2::ZERO);
    }

    #[test]
    fn zoom_anchored_keeps_the_point_under_the_cursor_fixed() {
        let stage_center = egui::pos2(400.0, 300.0);
        let sprite_px = egui::vec2(64.0, 48.0);
        // A few (old_scale, pan, cursor, new_scale) rows: the sprite point under the
        // cursor must read the same before and after the zoom.
        let rows = [
            (4.0_f32, egui::vec2(0.0, 0.0), egui::pos2(420.0, 290.0), 8.0_f32),
            (4.0, egui::vec2(30.0, -20.0), egui::pos2(360.0, 330.0), 2.0),
            (1.0, egui::vec2(-50.0, 10.0), egui::pos2(500.0, 260.0), 6.5),
        ];
        for (old_scale, pan, cursor, new_scale) in rows {
            let old_min = artboard_rect(Rect::from_center_size(stage_center, Vec2::ZERO), sprite_px, old_scale, pan).min;
            let before = screen_to_sprite(cursor, old_min, old_scale);
            let new_pan = zoom_anchored(stage_center, sprite_px, old_scale, pan, cursor, new_scale);
            let new_min = artboard_rect(Rect::from_center_size(stage_center, Vec2::ZERO), sprite_px, new_scale, new_pan).min;
            let after = screen_to_sprite(cursor, new_min, new_scale);
            assert!(
                approx(before.x, after.x) && approx(before.y, after.y),
                "anchor drifted: before {before:?} after {after:?} (old {old_scale} new {new_scale})",
            );
        }
    }

    #[test]
    fn zoom_anchored_is_a_noop_when_scale_is_unchanged() {
        let pan = egui::vec2(17.0, -9.0);
        let out = zoom_anchored(egui::pos2(400.0, 300.0), egui::vec2(64.0, 64.0), 5.0, pan, egui::pos2(450.0, 320.0), 5.0);
        assert!(approx(out.x, pan.x) && approx(out.y, pan.y), "an unchanged scale leaves pan untouched");
    }

    #[test]
    fn clamp_pan_keeps_the_board_reachable() {
        let stage = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let sprite_px = egui::vec2(64.0, 64.0);
        let scale = 4.0; // board is 256x256, smaller than the stage
        // A pan that would fling the board far off the right/bottom is pulled back.
        let runaway = egui::vec2(5000.0, 5000.0);
        let clamped = clamp_pan(stage, sprite_px, scale, runaway);
        let board = artboard_rect(stage, sprite_px, scale, clamped);
        assert!(board.intersects(stage), "a runaway pan is clamped so the board stays on-stage");
        assert!(board.right() >= stage.left() + MIN_VISIBLE_PX - 0.01, "at least the kept margin stays visible");
    }

    #[test]
    fn clamp_pan_is_idempotent_in_bounds() {
        let stage = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let sprite_px = egui::vec2(64.0, 64.0);
        let pan = egui::vec2(20.0, -15.0);
        let once = clamp_pan(stage, sprite_px, 4.0, pan);
        assert!(approx(once.x, pan.x) && approx(once.y, pan.y), "a centered, in-bounds pan is left alone");
    }
}
