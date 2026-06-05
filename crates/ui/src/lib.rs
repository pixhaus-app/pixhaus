//! Pixhaus UI layer: the egui contribution surface and the canvas embedding.
//!
//! `ui` is the only crate that knows both egui and `render`. It owns the
//! Panel/Tool/Workspace/`Module` contribution traits, the registries, the theme
//! tokens, the shared widgets, and the shell runtime (architecture bible sections
//! 7 and 8). The `Provider` trait lives in `services`, not here, and the
//! import/export surface (Importer/Exporter/Validator) is not yet landed.
//!
//! It also carries the egui-to-`render` seam — installing the [`ViewportRenderer`]
//! into egui-wgpu's resource store and dispatching the canvas draw through
//! [`CanvasCallback`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::float_cmp))]

pub mod brand;
pub mod canvas;
pub mod contrib_api;
pub mod icons;
pub mod playback;
pub mod region;
pub mod registry;
pub mod shell;
pub mod state;
pub mod theme;
pub mod widgets;

use std::sync::Arc;

use egui::epaint::PaintCallbackInfo;
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState, ScreenDescriptor};
use wgpu::RenderPass;

use pixhaus_render::{ViewParams, ViewportRenderer};

use crate::state::ui_state::GridMode;
use crate::theme::Theme;

/// Install `egui_extras`' image loaders on the context so the [`brand`] PNGs render.
///
/// Call once at startup, before the first frame draws a [`brand`] image. Re-exported
/// here so the app installs loaders without its own `egui_extras` manifest entry -
/// the PNG decoder behind it is activated by `ui`'s `image/png` feature.
pub use egui_extras::install_image_loaders;

/// Installs the [`ViewportRenderer`] into egui-wgpu's callback resource store.
///
/// Call once at startup with the render state eframe hands you. The renderer is
/// built against the live target format and retrieved later by [`CanvasCallback`]
/// during painting.
pub fn install_canvas_renderer(render_state: &RenderState) {
    let renderer = ViewportRenderer::new(&render_state.device, render_state.target_format);
    render_state.renderer.write().callback_resources.insert(renderer);
}

/// A composited RGBA8 frame handed to the GPU for one draw.
///
/// The bytes live behind an [`Arc`] so the per-frame [`CanvasCallback`] is cheap to
/// move (a refcount bump, not a pixel copy).
pub struct CanvasFrame {
    /// Tightly-packed (`stride == width * 4`) RGBA8 bytes.
    pub rgba: Arc<Vec<u8>>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

/// View scale (points per sprite pixel) at and above which the per-sprite-pixel grid is
/// drawn — below it the 1px lines would be too dense to read.
const MIN_PIXEL_GRID_SCALE: f32 = 6.0;

/// The per-frame canvas chrome the GPU draws: the transparency checkerboard and the pixel
/// grid. Built from theme tokens + view state by [`chrome_params`], carried by
/// [`CanvasCallback`], and packed into [`ViewParams`] in `prepare`. Plain floats (colors
/// sRGB-normalized via [`srgb_rgba`]) so `render` never sees an egui type.
#[derive(Copy, Clone)]
pub struct CanvasChrome {
    /// Light checker cell, sRGB-normalized rgba.
    pub checker_lo: [f32; 4],
    /// Dark checker cell, sRGB-normalized rgba.
    pub checker_hi: [f32; 4],
    /// Minor (per-sprite-pixel) grid line: sRGB-normalized rgb, strength in alpha.
    pub grid_minor: [f32; 4],
    /// Major grid line: sRGB-normalized rgb, strength in alpha.
    pub grid_major: [f32; 4],
    /// View scale in points per sprite pixel (× pixels-per-point = the minor-grid period).
    pub scale: f32,
    /// Major-grid period in sprite pixels (0 = no major grid).
    pub major_period: f32,
    /// Whether the per-sprite-pixel grid is drawn (gated on zoom).
    pub minor_on: bool,
    /// Whether a sprite is present to composite over the checker.
    pub has_tile: bool,
}

/// Build the canvas chrome from the active grid mode, view scale, theme, and whether a
/// sprite is present. Pure (no egui `Context`, no GPU), so it is table-testable.
#[must_use]
pub fn chrome_params(grid: GridMode, scale: f32, theme: &Theme, has_tile: bool) -> CanvasChrome {
    let major_period = match grid {
        GridMode::Off => 0.0,
        GridMode::Px8 => 8.0,
        GridMode::Px16 => 16.0,
    };
    CanvasChrome {
        checker_lo: srgb_rgba(theme.mock.checker[0], 1.0),
        checker_hi: srgb_rgba(theme.mock.checker[1], 1.0),
        grid_minor: srgb_rgba(theme.roles.border, 0.45),
        grid_major: srgb_rgba(theme.roles.border, 1.0),
        scale,
        major_period,
        minor_on: scale >= MIN_PIXEL_GRID_SCALE,
        has_tile,
    }
}

/// A theme `Color32` as sRGB-normalized rgb in 0..1, with alpha set to the grid line
/// `strength` (1.0 for the opaque checker). The renderer linearizes these for an sRGB
/// color target (and passes them through otherwise), so the GPU chrome lands on the same
/// value the theme token defines — the renderer is the only layer that knows the target
/// color space.
#[must_use]
pub fn srgb_rgba(color: egui::Color32, strength: f32) -> [f32; 4] {
    let [r, g, b, _] = color.to_array();
    [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, strength]
}

/// egui paint callback that displays the sprite canvas via [`ViewportRenderer`].
///
/// Per-frame and cheap: it carries the composited frame to upload — `Some` only when
/// the document changed since the last upload, `None` to reuse the retained GPU
/// texture untouched. The GPU resources (pipeline, texture, bind group) live in the
/// [`ViewportRenderer`] inside egui-wgpu's callback resource store
/// (see [`install_canvas_renderer`]), not here.
pub struct CanvasCallback {
    /// The frame to upload this draw, or `None` to reuse the retained texture.
    pub frame: Option<CanvasFrame>,
    /// The on-screen artboard rect in points. The renderer maps texture coordinates
    /// from this (not the egui-wgpu pass viewport, which is window-clamped), so a board
    /// panned or zoomed past the window edge crops instead of squashing.
    pub artboard: egui::Rect,
    /// The checkerboard + grid the GPU draws this frame.
    pub chrome: CanvasChrome,
}

impl CallbackTrait for CanvasCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        // `prepare` runs before the egui pass with `&mut` resources and the queue;
        // `paint` only reads. Upload the frame only when it changed, but refresh the
        // view every frame since pan/zoom move the board without a re-upload.
        if let Some(renderer) = callback_resources.get_mut::<ViewportRenderer>() {
            if let Some(frame) = self.frame.as_ref() {
                renderer.upload_frame(device, queue, &frame.rgba, frame.width, frame.height);
            }
            let ppp = screen_descriptor.pixels_per_point;
            let c = &self.chrome;
            let mut flags = 0u32;
            if c.has_tile {
                flags |= 1;
            }
            if c.minor_on {
                flags |= 2;
            }
            if c.major_period > 0.0 {
                flags |= 4;
            }
            renderer.set_params(
                queue,
                &ViewParams {
                    origin_px: [self.artboard.min.x * ppp, self.artboard.min.y * ppp],
                    size_px: [self.artboard.width() * ppp, self.artboard.height() * ppp],
                    checker_lo: c.checker_lo,
                    checker_hi: c.checker_hi,
                    minor_color: c.grid_minor,
                    major_color: c.grid_major,
                    checker_cell_px: 8.0 * ppp,
                    ppsp: c.scale * ppp,
                    major_period: c.major_period,
                    flags,
                },
            );
        }
        // The upload and params write ride the queue; no extra command buffers to submit.
        Vec::new()
    }

    fn paint(&self, _info: PaintCallbackInfo, render_pass: &mut RenderPass<'static>, callback_resources: &CallbackResources) {
        if let Some(renderer) = callback_resources.get::<ViewportRenderer>() {
            renderer.paint(render_pass);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CanvasCallback, CanvasChrome, CanvasFrame, chrome_params, srgb_rgba};
    use crate::state::ui_state::GridMode;
    use crate::theme::Theme;
    use std::sync::Arc;

    fn sample_chrome() -> CanvasChrome {
        chrome_params(GridMode::Px8, 8.0, &Theme::dark(), true)
    }

    /// The callback carries only the changing frame data (an `Arc`-backed buffer) and the
    /// chrome params, never GPU handles — those live in the callback resource store.
    #[test]
    fn canvas_callback_holds_an_optional_frame() {
        let board = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0));
        let empty = CanvasCallback {
            frame: None,
            artboard: board,
            chrome: sample_chrome(),
        };
        assert!(empty.frame.is_none());
        let with_frame = CanvasCallback {
            frame: Some(CanvasFrame {
                rgba: Arc::new(vec![0u8; 4]),
                width: 1,
                height: 1,
            }),
            artboard: board,
            chrome: sample_chrome(),
        };
        assert!(with_frame.frame.is_some());
    }

    #[test]
    fn chrome_params_maps_grid_mode_to_major_period() {
        let t = Theme::dark();
        assert_eq!(chrome_params(GridMode::Off, 8.0, &t, true).major_period, 0.0);
        assert_eq!(chrome_params(GridMode::Px8, 8.0, &t, true).major_period, 8.0);
        assert_eq!(chrome_params(GridMode::Px16, 8.0, &t, true).major_period, 16.0);
    }

    #[test]
    fn chrome_params_gates_the_minor_grid_on_zoom() {
        let t = Theme::dark();
        assert!(!chrome_params(GridMode::Px8, 5.99, &t, true).minor_on, "minor grid is off below 6x");
        assert!(chrome_params(GridMode::Px8, 6.0, &t, true).minor_on, "minor grid is on at 6x");
        assert!(chrome_params(GridMode::Px8, 64.0, &t, true).minor_on);
    }

    #[test]
    fn chrome_params_passes_has_tile_and_token_colors() {
        let t = Theme::dark();
        let on = chrome_params(GridMode::Px8, 8.0, &t, true);
        assert!(on.has_tile);
        assert!(!chrome_params(GridMode::Px8, 8.0, &t, false).has_tile, "has_tile passes through");
        assert_eq!(on.checker_lo, srgb_rgba(t.mock.checker[0], 1.0), "checker comes from the theme token");
        assert_eq!(on.grid_minor, srgb_rgba(t.roles.border, 0.45), "minor grid is the faint border");
    }

    #[test]
    fn srgb_rgba_overrides_alpha_with_the_strength() {
        let c = srgb_rgba(egui::Color32::WHITE, 0.45);
        assert!(
            (c[0] - 1.0).abs() < 1e-6 && (c[1] - 1.0).abs() < 1e-6 && (c[2] - 1.0).abs() < 1e-6,
            "white is 1.0"
        );
        assert!((c[3] - 0.45).abs() < 1e-6, "alpha carries the line strength");
    }
}
