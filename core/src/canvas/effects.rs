//! Non-destructive layer-effect application.
//!
//! Each function takes a source [`PixelBuffer`] and returns a new buffer with
//! the effect applied; the source is never mutated. [`apply_effects`] runs an
//! effect stack in order. Effects are clipped to the source buffer's bounds —
//! an outline or shadow that would fall outside the buffer is simply cropped,
//! which matches operating on a canvas-sized layer buffer.
//!
//! Effect semantics adopted from Pixelorama's per-layer effect shaders
//! (`src/Shaders/Effects/`). The implementations here are CPU ports kept
//! deterministic for snapshot testing. See `THIRD_PARTY_NOTICES.md`.

// Pixel-art coordinate math: x/y offsets cross the u32<->i32 boundary
// constantly and are bounded by canvas sizes well under 2^31. Channel
// arithmetic stays within 0..=255 after the clamps below.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use crate::project::Rgba;
use crate::project::effect::LayerEffect;

use super::buffer::PixelBuffer;
use super::error::Result;

/// Applies an effect stack to `src` in order, returning the result.
///
/// Returns a clone of `src` when `effects` is empty.
///
/// # Errors
///
/// Propagates [`super::error::Error`] from intermediate buffer allocation.
pub fn apply_effects(src: &PixelBuffer, effects: &[LayerEffect]) -> Result<PixelBuffer> {
    let mut current = src.clone();
    for effect in effects {
        current = apply_effect(&current, effect)?;
    }
    Ok(current)
}

/// Applies a single effect to `src`, returning a new buffer.
///
/// # Errors
///
/// Propagates [`super::error::Error`] from buffer allocation.
pub fn apply_effect(src: &PixelBuffer, effect: &LayerEffect) -> Result<PixelBuffer> {
    match *effect {
        LayerEffect::Outline {
            color,
            thickness,
            diagonal,
        } => Ok(outline(src, color, thickness.max(1), diagonal)),
        LayerEffect::DropShadow {
            color,
            offset,
            opacity,
        } => drop_shadow(src, color, offset.x, offset.y, opacity),
        LayerEffect::Brightness { delta } => Ok(brightness(src, delta)),
        LayerEffect::Invert => Ok(invert(src)),
    }
}

/// Per-channel `brightness` adjustment, alpha preserved.
#[must_use]
fn brightness(src: &PixelBuffer, delta: i16) -> PixelBuffer {
    map_rgb(src, |c| {
        let adj = |v: u8| -> u8 {
            let n = i32::from(v) + i32::from(delta);
            n.clamp(0, 255) as u8
        };
        Rgba::new(adj(c.r), adj(c.g), adj(c.b), c.a)
    })
}

/// RGB inversion, alpha preserved.
#[must_use]
fn invert(src: &PixelBuffer) -> PixelBuffer {
    map_rgb(src, |c| Rgba::new(255 - c.r, 255 - c.g, 255 - c.b, c.a))
}

/// Applies `f` to every pixel of `src`, returning a new buffer of the same
/// dimensions and stride.
fn map_rgb(src: &PixelBuffer, f: impl Fn(Rgba) -> Rgba) -> PixelBuffer {
    let mut out = src.clone();
    for y in 0..out.height() {
        for x in 0..out.width() {
            if let Some(c) = src.pixel(x, y) {
                out.set_pixel(x, y, f(c));
            }
        }
    }
    out
}

/// Draws an `color` outline `thickness` pixels wide around opaque pixels,
/// filling only previously-transparent pixels so source art stays on top.
fn outline(src: &PixelBuffer, color: Rgba, thickness: u8, diagonal: bool) -> PixelBuffer {
    let w = src.width();
    let h = src.height();
    if w == 0 || h == 0 {
        return src.clone();
    }

    // Distance (in expansion steps) from the nearest opaque pixel, capped at
    // thickness + 1. 0 marks an originally-opaque pixel.
    let cap = u32::from(thickness) + 1;
    let idx = |x: u32, y: u32| (y * w + x) as usize;
    let mut dist = vec![u32::MAX; (w * h) as usize];
    let mut frontier: Vec<(u32, u32)> = Vec::new();

    for y in 0..h {
        for x in 0..w {
            if src.pixel(x, y).is_some_and(|c| c.a > 0) {
                dist[idx(x, y)] = 0;
                frontier.push((x, y));
            }
        }
    }

    // Multi-source BFS dilation outward up to `thickness` steps.
    let mut step = 0u32;
    while step < u32::from(thickness) && !frontier.is_empty() {
        step += 1;
        let mut next = Vec::new();
        for (x, y) in frontier.drain(..) {
            for (nx, ny) in neighbours(x, y, w, h, diagonal) {
                if dist[idx(nx, ny)] == u32::MAX {
                    dist[idx(nx, ny)] = step;
                    next.push((nx, ny));
                }
            }
        }
        frontier = next;
    }

    let mut out = src.clone();
    for y in 0..h {
        for x in 0..w {
            let d = dist[idx(x, y)];
            if d != u32::MAX && d != 0 && d <= u32::from(thickness) && d < cap {
                out.set_pixel(x, y, color);
            }
        }
    }
    out
}

/// Returns the in-bounds 4- or 8-connected neighbours of `(x, y)`.
fn neighbours(x: u32, y: u32, w: u32, h: u32, diagonal: bool) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(8);
    let mut push = |dx: i32, dy: i32| {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && ny >= 0 && (nx as u32) < w && (ny as u32) < h {
            out.push((nx as u32, ny as u32));
        }
    };
    push(-1, 0);
    push(1, 0);
    push(0, -1);
    push(0, 1);
    if diagonal {
        push(-1, -1);
        push(1, -1);
        push(-1, 1);
        push(1, 1);
    }
    out
}

/// Composites a `color` shadow offset by `(dx, dy)` beneath the source.
fn drop_shadow(
    src: &PixelBuffer,
    color: Rgba,
    dx: i32,
    dy: i32,
    opacity: u8,
) -> Result<PixelBuffer> {
    let w = src.width();
    let h = src.height();
    let mut out = PixelBuffer::new(w, h)?;

    // Lay down the shadow first.
    for y in 0..h {
        for x in 0..w {
            let Some(s) = src.pixel(x, y) else { continue };
            if s.a == 0 {
                continue;
            }
            let tx = x as i32 + dx;
            let ty = y as i32 + dy;
            if tx < 0 || ty < 0 || tx as u32 >= w || ty as u32 >= h {
                continue;
            }
            let a = (u32::from(s.a) * u32::from(opacity) / 255) as u8;
            out.set_pixel(
                tx as u32,
                ty as u32,
                Rgba::new(color.r, color.g, color.b, a),
            );
        }
    }

    // Source art over the shadow.
    for y in 0..h {
        for x in 0..w {
            let Some(s) = src.pixel(x, y) else { continue };
            if s.a == 0 {
                continue;
            }
            out.set_pixel(x, y, s);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::geometry::IVec2;

    const CLEAR: Rgba = Rgba::new(0, 0, 0, 0);
    const WHITE: Rgba = Rgba::new(255, 255, 255, 255);
    const BLACK: Rgba = Rgba::new(0, 0, 0, 255);

    fn dot_center(w: u32, h: u32) -> PixelBuffer {
        let mut buf = PixelBuffer::new(w, h).unwrap();
        buf.set_pixel(w / 2, h / 2, WHITE);
        buf
    }

    #[test]
    fn invert_flips_rgb_keeps_alpha() {
        let mut buf = PixelBuffer::new(1, 1).unwrap();
        buf.set_pixel(0, 0, Rgba::new(10, 20, 30, 200));
        let out = apply_effect(&buf, &LayerEffect::Invert).unwrap();
        assert_eq!(out.pixel(0, 0), Some(Rgba::new(245, 235, 225, 200)));
    }

    #[test]
    fn brightness_clamps_and_preserves_alpha() {
        let mut buf = PixelBuffer::new(1, 1).unwrap();
        buf.set_pixel(0, 0, Rgba::new(250, 5, 100, 128));
        let out = apply_effect(&buf, &LayerEffect::Brightness { delta: 20 }).unwrap();
        assert_eq!(out.pixel(0, 0), Some(Rgba::new(255, 25, 120, 128)));
        let dark = apply_effect(&buf, &LayerEffect::Brightness { delta: -20 }).unwrap();
        assert_eq!(dark.pixel(0, 0), Some(Rgba::new(230, 0, 80, 128)));
    }

    #[test]
    fn outline_rings_a_single_dot() {
        let buf = dot_center(5, 5);
        let out = apply_effect(
            &buf,
            &LayerEffect::Outline {
                color: BLACK,
                thickness: 1,
                diagonal: false,
            },
        )
        .unwrap();
        // Center stays white, 4-neighbours become outline, corners untouched.
        assert_eq!(out.pixel(2, 2), Some(WHITE));
        assert_eq!(out.pixel(1, 2), Some(BLACK));
        assert_eq!(out.pixel(3, 2), Some(BLACK));
        assert_eq!(out.pixel(2, 1), Some(BLACK));
        assert_eq!(out.pixel(2, 3), Some(BLACK));
        assert_eq!(out.pixel(1, 1), Some(CLEAR));
    }

    #[test]
    fn outline_diagonal_fills_corners() {
        let buf = dot_center(5, 5);
        let out = apply_effect(
            &buf,
            &LayerEffect::Outline {
                color: BLACK,
                thickness: 1,
                diagonal: true,
            },
        )
        .unwrap();
        assert_eq!(out.pixel(1, 1), Some(BLACK));
        assert_eq!(out.pixel(3, 3), Some(BLACK));
    }

    #[test]
    fn drop_shadow_offsets_alpha_under_source() {
        let buf = dot_center(5, 5);
        let out = apply_effect(
            &buf,
            &LayerEffect::DropShadow {
                color: BLACK,
                offset: IVec2::new(1, 1),
                opacity: 128,
            },
        )
        .unwrap();
        // Source unchanged.
        assert_eq!(out.pixel(2, 2), Some(WHITE));
        // Shadow one pixel down-right at half alpha.
        assert_eq!(out.pixel(3, 3), Some(Rgba::new(0, 0, 0, 128)));
    }

    #[test]
    fn empty_stack_is_identity() {
        let buf = dot_center(4, 4);
        let out = apply_effects(&buf, &[]).unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn stack_applies_in_order() {
        let buf = dot_center(3, 3);
        // Outline then invert: outline black -> inverted to white.
        let effects = vec![
            LayerEffect::Outline {
                color: BLACK,
                thickness: 1,
                diagonal: false,
            },
            LayerEffect::Invert,
        ];
        let out = apply_effects(&buf, &effects).unwrap();
        // Original outline pixel (1,1)->(0,1) neighbour was black, now white.
        assert_eq!(out.pixel(0, 1), Some(WHITE));
    }
}
