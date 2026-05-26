//! Viewport coordinate math for the canvas surface.
//!
//! Ported from `ui/src/canvas/viewport.ts`. All transforms are pure functions
//! so they are testable without a GPU or window. "Canvas coordinates" are
//! sprite pixel coordinates (0,0 = top-left of the sprite). "Screen
//! coordinates" are physical/CSS pixels relative to the viewport (0,0 =
//! top-left). The viewport is centred: [`Viewport::scroll`] is the canvas
//! coordinate that maps to the viewport's centre point.

use glam::Vec2;

/// Zoom levels that the +/- keys and scroll wheel snap to.
pub const SNAP_ZOOMS: [f32; 9] = [
    1.0 / 16.0, // 6.25 %
    1.0 / 8.0,  // 12.5 %
    1.0 / 4.0,  // 25 %
    1.0 / 2.0,  // 50 %
    1.0,        // 100 %
    2.0,        // 200 %
    4.0,        // 400 %
    8.0,        // 800 %
    16.0,       // 1600 %
];

/// Minimum zoom (one canvas pixel maps to 1/16 screen pixel).
pub const MIN_ZOOM: f32 = 1.0 / 16.0;
/// Maximum zoom (one canvas pixel maps to 16 screen pixels).
pub const MAX_ZOOM: f32 = 16.0;

/// Centred viewport state: which canvas coordinate sits at the viewport centre
/// ([`Viewport::scroll`]) and how many screen pixels one canvas pixel spans
/// ([`Viewport::zoom`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Canvas coordinate mapped to the viewport's centre.
    pub scroll: Vec2,
    /// Screen pixels per canvas pixel.
    pub zoom: f32,
}

impl Viewport {
    /// A viewport scrolled to the origin at 100 % zoom.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll: Vec2::ZERO,
            zoom: 1.0,
        }
    }

    /// Canvas pixel `canvas` -> screen pixel, given the viewport size `vp`.
    #[must_use]
    pub fn canvas_to_screen(&self, canvas: Vec2, vp: Vec2) -> Vec2 {
        (canvas - self.scroll) * self.zoom + vp * 0.5
    }

    /// Screen pixel `screen` -> canvas pixel. Inverse of [`Self::canvas_to_screen`].
    #[must_use]
    pub fn screen_to_canvas(&self, screen: Vec2, vp: Vec2) -> Vec2 {
        (screen - vp * 0.5) / self.zoom + self.scroll
    }

    /// Recomputes [`Self::scroll`] so the canvas coordinate currently under
    /// `screen` stays fixed while the zoom changes to `new_zoom`.
    pub fn zoom_at(&mut self, screen: Vec2, vp: Vec2, new_zoom: f32) {
        let canvas = self.screen_to_canvas(screen, vp);
        let new_zoom = clamp_zoom(new_zoom);
        self.scroll = canvas - (screen - vp * 0.5) / new_zoom;
        self.zoom = new_zoom;
    }

    /// Pans by a screen-pixel delta (e.g. a drag), keeping zoom fixed.
    pub fn pan_by_screen(&mut self, delta_screen: Vec2) {
        self.scroll -= delta_screen / self.zoom;
    }

    /// Centres the viewport on the sprite's geometric centre.
    pub fn scroll_to_centre(&mut self, sprite: Vec2) {
        self.scroll = sprite * 0.5;
    }

    /// Sets zoom to fit `sprite` into viewport `vp` and centres on it.
    pub fn fit(&mut self, sprite: Vec2, vp: Vec2) {
        self.zoom = fit_zoom(sprite, vp, 16.0);
        self.scroll_to_centre(sprite);
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the next snap zoom level in `dir` (+1 = in, -1 = out), the nearest
/// snap strictly beyond `current` in that direction.
#[must_use]
pub fn snap_zoom(current: f32, dir: i32) -> f32 {
    if dir > 0 {
        SNAP_ZOOMS
            .iter()
            .copied()
            .find(|&z| z > current)
            .unwrap_or(MAX_ZOOM)
    } else {
        SNAP_ZOOMS
            .iter()
            .rev()
            .copied()
            .find(|&z| z < current)
            .unwrap_or(MIN_ZOOM)
    }
}

/// Clamps zoom to `[MIN_ZOOM, MAX_ZOOM]`.
#[must_use]
pub fn clamp_zoom(z: f32) -> f32 {
    z.clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Fits a sprite into the viewport at the largest snap zoom leaving at least
/// `padding` screen pixels on each side. Falls back to [`MIN_ZOOM`] when even
/// the smallest snap does not fit.
#[must_use]
pub fn fit_zoom(sprite: Vec2, vp: Vec2, padding: f32) -> f32 {
    let max_w = vp.x - padding * 2.0;
    let max_h = vp.y - padding * 2.0;
    if max_w <= 0.0 || max_h <= 0.0 || sprite.x == 0.0 || sprite.y == 0.0 {
        return 1.0;
    }
    let raw = (max_w / sprite.x).min(max_h / sprite.y);
    for &candidate in SNAP_ZOOMS.iter().rev() {
        if candidate <= raw {
            return clamp_zoom(candidate);
        }
    }
    clamp_zoom(SNAP_ZOOMS[0])
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_canvas_screen() {
        let vp = Viewport {
            scroll: Vec2::new(32.0, 32.0),
            zoom: 4.0,
        };
        let size = Vec2::new(800.0, 600.0);
        let canvas = Vec2::new(10.0, 20.0);
        let screen = vp.canvas_to_screen(canvas, size);
        let back = vp.screen_to_canvas(screen, size);
        assert!((back - canvas).length() < 1e-3);
    }

    #[test]
    fn zoom_at_keeps_point_under_cursor_fixed() {
        let mut vp = Viewport {
            scroll: Vec2::new(32.0, 32.0),
            zoom: 1.0,
        };
        let size = Vec2::new(800.0, 600.0);
        let cursor = Vec2::new(500.0, 300.0);
        let before = vp.screen_to_canvas(cursor, size);
        vp.zoom_at(cursor, size, 4.0);
        let after = vp.screen_to_canvas(cursor, size);
        assert!((before - after).length() < 1e-3);
    }

    #[test]
    fn snap_zoom_steps() {
        assert_eq!(snap_zoom(1.0, 1), 2.0);
        assert_eq!(snap_zoom(1.0, -1), 0.5);
        assert_eq!(snap_zoom(16.0, 1), 16.0);
        assert_eq!(snap_zoom(MIN_ZOOM, -1), MIN_ZOOM);
    }

    #[test]
    fn fit_zoom_leaves_padding() {
        // 64px sprite in a 800x600 viewport fits at the 8x snap (64*8=512<768).
        let z = fit_zoom(Vec2::splat(64.0), Vec2::new(800.0, 600.0), 16.0);
        assert_eq!(z, 8.0);
    }
}
