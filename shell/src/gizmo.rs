//! The shared transform gizmo: a movable, scalable, rotatable rectangle in
//! image-pixel space.
//!
//! One widget, two consumers. The AI studio's inpaint refine surface
//! ([`crate::studio`]) rasterizes the gizmo's rotated interior into a boolean
//! mask; the free-transform tool ([`crate::canvas`]) resamples the lifted
//! selection pixels through the gizmo's affine. Both render and hit-test the
//! identical box — only the commit differs (mask rasterize vs pixel resample).
//!
//! Coordinates are image pixels (canvas pixels for the editor, candidate pixels
//! for the studio). The box is stored as a centre, half-extents, and rotation,
//! so a uniform scale, a non-uniform scale, a rotation, and a translation are
//! all one struct update. Hit-testing the corners and the rotate stem is done in
//! screen space by the caller (fixed pixel tolerance regardless of zoom); the
//! body falls back to [`BoxGizmo::contains`] in image space.

use eframe::egui;

/// A transformable rectangle in image-pixel coordinates. Its centre, half-width,
/// half-height, and rotation define the box; [`BoxGizmo::corners`] and
/// [`BoxGizmo::aabb`] derive the screen geometry, and [`BoxGizmo::contains`]
/// answers the point-in-box test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BoxGizmo {
    /// Centre x in image pixels.
    pub cx: f32,
    /// Centre y in image pixels.
    pub cy: f32,
    /// Half-width in image pixels (always positive).
    pub hw: f32,
    /// Half-height in image pixels (always positive).
    pub hh: f32,
    /// Rotation in radians, clockwise (image y grows downward).
    pub angle: f32,
}

/// Which gizmo handle a drag is moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GizmoHandle {
    /// Drag the body to translate.
    Move,
    /// Drag a corner (0=TL, 1=TR, 2=BR, 3=BL) to scale about the centre.
    Corner(usize),
    /// Drag the stem above the top edge to rotate.
    Rotate,
}

impl BoxGizmo {
    /// A box a quarter of the `w` x `h` image, centred. The studio's default
    /// inpaint box.
    #[allow(clippy::cast_precision_loss)] // image dims <= 8192 are exact in f32
    pub(crate) fn centered(w: u32, h: u32) -> Self {
        Self {
            cx: w as f32 / 2.0,
            cy: h as f32 / 2.0,
            hw: (w as f32 / 4.0).max(1.0),
            hh: (h as f32 / 4.0).max(1.0),
            angle: 0.0,
        }
    }

    /// A box covering the inclusive bounds `(x0, y0, x1, y1)` exactly, unrotated.
    /// The free-transform tool seeds the gizmo to the selection's bounding box.
    #[allow(clippy::cast_precision_loss)] // image dims <= 8192 are exact in f32
    pub(crate) fn from_bounds(x0: u32, y0: u32, x1: u32, y1: u32) -> Self {
        // The box spans pixel cells x0..=x1, so its extent in continuous space is
        // [x0, x1 + 1]; the centre lands on the cell-grid midpoint.
        let left = x0 as f32;
        let top = y0 as f32;
        let right = x1 as f32 + 1.0;
        let bottom = y1 as f32 + 1.0;
        Self {
            cx: f32::midpoint(left, right),
            cy: f32::midpoint(top, bottom),
            hw: ((right - left) / 2.0).max(0.5),
            hh: ((bottom - top) / 2.0).max(0.5),
            angle: 0.0,
        }
    }

    /// A local offset `(lx, ly)` rotated into image space and added to the
    /// centre.
    pub(crate) fn point(&self, lx: f32, ly: f32) -> (f32, f32) {
        let (s, c) = self.angle.sin_cos();
        (self.cx + lx * c - ly * s, self.cy + lx * s + ly * c)
    }

    /// The four corner points in image space, in order TL, TR, BR, BL.
    pub(crate) fn corners(&self) -> [(f32, f32); 4] {
        [
            self.point(-self.hw, -self.hh),
            self.point(self.hw, -self.hh),
            self.point(self.hw, self.hh),
            self.point(-self.hw, self.hh),
        ]
    }

    /// The rotate-handle point, set off above the top edge by a margin that grows
    /// with the box so the stem stays grabbable at any size.
    pub(crate) fn rotate_handle(&self) -> (f32, f32) {
        self.point(0.0, -self.hh - self.hh.max(self.hw).mul_add(0.4, 8.0))
    }

    /// Whether image-space `(px, py)` is inside the rotated rectangle.
    pub(crate) fn contains(&self, px: f32, py: f32) -> bool {
        let (s, c) = self.angle.sin_cos();
        let dx = px - self.cx;
        let dy = py - self.cy;
        let lx = dx * c + dy * s;
        let ly = -dx * s + dy * c;
        lx.abs() <= self.hw && ly.abs() <= self.hh
    }

    /// The rectangle's axis-aligned bounding box, clamped to `w` x `h`, as
    /// `[x, y, w, h]`. Used to bound both the mask rasterize and the resample, so
    /// per-frame work is O(box area), not O(canvas) — the 8K constraint.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
    pub(crate) fn aabb(&self, w: u32, h: u32) -> [u32; 4] {
        let xs = self.corners().map(|p| p.0);
        let ys = self.corners().map(|p| p.1);
        let min_x = xs.iter().copied().fold(f32::INFINITY, f32::min).floor().clamp(0.0, w as f32);
        let max_x = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max).ceil().clamp(0.0, w as f32);
        let min_y = ys.iter().copied().fold(f32::INFINITY, f32::min).floor().clamp(0.0, h as f32);
        let max_y = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max).ceil().clamp(0.0, h as f32);
        [min_x as u32, min_y as u32, (max_x - min_x) as u32, (max_y - min_y) as u32]
    }

    /// Maps an image-space point back into the box's local, axis-aligned frame:
    /// undo the translate, then the rotation. The inverse of [`BoxGizmo::point`]
    /// without the half-extent offset. Used by the free-transform resample to
    /// find each destination pixel's source position.
    pub(crate) fn to_local(self, px: f32, py: f32) -> (f32, f32) {
        let (s, c) = self.angle.sin_cos();
        let dx = px - self.cx;
        let dy = py - self.cy;
        (dx * c + dy * s, -dx * s + dy * c)
    }

    /// Picks the handle a press at screen position `p` grabs, given the
    /// image-to-screen map `to_screen` and the screen position `pointer_image` of
    /// the same press in image space. Corners and the rotate stem hit-test in
    /// screen space within `tol` pixels; the body falls back to a point-in-box
    /// test in image space. `None` means the press missed the gizmo.
    pub(crate) fn pick_handle(
        &self,
        to_screen: impl Fn(f32, f32) -> egui::Pos2,
        p: egui::Pos2,
        pointer_image: (f32, f32),
        tol: f32,
    ) -> Option<GizmoHandle> {
        for (k, &(cx, cy)) in self.corners().iter().enumerate() {
            if to_screen(cx, cy).distance(p) <= tol {
                return Some(GizmoHandle::Corner(k));
            }
        }
        let (rx, ry) = self.rotate_handle();
        if to_screen(rx, ry).distance(p) <= tol {
            return Some(GizmoHandle::Rotate);
        }
        self.contains(pointer_image.0, pointer_image.1).then_some(GizmoHandle::Move)
    }

    /// Applies a pointer drag on `handle` to the box, in image space.
    ///
    /// - [`GizmoHandle::Move`] translates the centre by `(dx, dy)`.
    /// - [`GizmoHandle::Corner`] sets the half-extents from the pointer's
    ///   distance to the centre in the box's local frame; with `uniform` the
    ///   larger axis drives both so the aspect ratio is held (Shift-scale).
    /// - [`GizmoHandle::Rotate`] points the stem at the pointer.
    ///
    /// `(px, py)` is the pointer in image space, `(dx, dy)` the frame's drag
    /// delta in image space.
    pub(crate) fn drag_handle(&mut self, handle: GizmoHandle, px: f32, py: f32, dx: f32, dy: f32, uniform: bool) {
        match handle {
            GizmoHandle::Move => {
                self.cx += dx;
                self.cy += dy;
            }
            GizmoHandle::Corner(_) => {
                let (lx, ly) = self.to_local(px, py);
                let hw = lx.abs().max(0.5);
                let hh = ly.abs().max(0.5);
                if uniform {
                    let m = hw.max(hh);
                    self.hw = m;
                    self.hh = m;
                } else {
                    self.hw = hw;
                    self.hh = hh;
                }
            }
            GizmoHandle::Rotate => {
                self.angle = (px - self.cx).atan2(-(py - self.cy));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bounds_centres_and_sizes_the_box() {
        // A 4x4 selection rooted at (2, 2) spans cells 2..=5, continuous [2, 6].
        let g = BoxGizmo::from_bounds(2, 2, 5, 5);
        assert!((g.cx - 4.0).abs() < 1e-6);
        assert!((g.cy - 4.0).abs() < 1e-6);
        assert!((g.hw - 2.0).abs() < 1e-6);
        assert!((g.hh - 2.0).abs() < 1e-6);
        assert!((g.angle).abs() < 1e-6);
    }

    #[test]
    fn contains_respects_rotation() {
        let g = BoxGizmo {
            cx: 4.0,
            cy: 4.0,
            hw: 2.0,
            hh: 2.0,
            angle: 0.0,
        };
        assert!(g.contains(4.0, 4.0), "centre is inside");
        assert!(g.contains(5.9, 4.0), "just inside the right edge");
        assert!(!g.contains(6.1, 4.0), "just outside the right edge");
    }

    #[test]
    fn corners_are_tl_tr_br_bl_unrotated() {
        let g = BoxGizmo {
            cx: 4.0,
            cy: 4.0,
            hw: 2.0,
            hh: 1.0,
            angle: 0.0,
        };
        let c = g.corners();
        assert_eq!(c[0], (2.0, 3.0), "TL");
        assert_eq!(c[1], (6.0, 3.0), "TR");
        assert_eq!(c[2], (6.0, 5.0), "BR");
        assert_eq!(c[3], (2.0, 5.0), "BL");
    }

    #[test]
    fn aabb_clamps_to_the_image() {
        let g = BoxGizmo {
            cx: 1.0,
            cy: 1.0,
            hw: 4.0,
            hh: 4.0,
            angle: 0.0,
        };
        // The box runs from -3 to 5 on each axis; the aabb clamps to [0, 8).
        let [x, y, w, h] = g.aabb(8, 8);
        assert_eq!([x, y], [0, 0]);
        assert_eq!([w, h], [5, 5]);
    }

    #[test]
    fn to_local_inverts_point_without_extents() {
        let g = BoxGizmo {
            cx: 10.0,
            cy: 6.0,
            hw: 3.0,
            hh: 2.0,
            angle: 0.7,
        };
        let (wx, wy) = g.point(1.5, -0.5);
        let (lx, ly) = g.to_local(wx, wy);
        assert!((lx - 1.5).abs() < 1e-4, "local x round-trips");
        assert!((ly + 0.5).abs() < 1e-4, "local y round-trips");
    }

    #[test]
    fn drag_corner_uniform_locks_aspect() {
        let mut g = BoxGizmo {
            cx: 0.0,
            cy: 0.0,
            hw: 1.0,
            hh: 1.0,
            angle: 0.0,
        };
        // Pointer at (4, 2): non-uniform sets hw=4, hh=2; uniform sets both to 4.
        g.drag_handle(GizmoHandle::Corner(2), 4.0, 2.0, 0.0, 0.0, true);
        assert!((g.hw - 4.0).abs() < 1e-6);
        assert!((g.hh - 4.0).abs() < 1e-6);
    }

    #[test]
    fn drag_corner_free_sets_each_axis() {
        let mut g = BoxGizmo {
            cx: 0.0,
            cy: 0.0,
            hw: 1.0,
            hh: 1.0,
            angle: 0.0,
        };
        g.drag_handle(GizmoHandle::Corner(2), 4.0, 2.0, 0.0, 0.0, false);
        assert!((g.hw - 4.0).abs() < 1e-6);
        assert!((g.hh - 2.0).abs() < 1e-6);
    }

    #[test]
    fn drag_move_translates_the_centre() {
        let mut g = BoxGizmo {
            cx: 5.0,
            cy: 5.0,
            hw: 1.0,
            hh: 1.0,
            angle: 0.0,
        };
        g.drag_handle(GizmoHandle::Move, 0.0, 0.0, 3.0, -2.0, false);
        assert!((g.cx - 8.0).abs() < 1e-6);
        assert!((g.cy - 3.0).abs() < 1e-6);
    }
}
