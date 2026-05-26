//! The egui-wgpu canvas: the paint callback that drives [`ViewportRenderer`],
//! plus the canvas widget that routes pan/zoom input and submits the callback.
//!
//! The renderer itself lives in egui-wgpu's `CallbackResources` (inserted in
//! [`super::app::ShellApp::install_renderer`]); the per-frame [`SpriteCallback`]
//! carries only the camera uniforms.

use eframe::egui;
use glam::Vec2;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_render::{viewport::clamp_zoom, ViewportRenderer};

use crate::app::ShellApp;

/// Per-frame paint command for the SPRITE program. Cheap to build; holds only
/// the camera state, never GPU handles.
struct SpriteCallback {
    resolution: [f32; 2],
    scroll: [f32; 2],
    zoom: f32,
}

impl egui_wgpu::CallbackTrait for SpriteCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<ViewportRenderer>() {
            renderer.write_uniforms(queue, self.resolution, self.scroll, self.zoom);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::epaint::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<ViewportRenderer>() {
            renderer.paint(render_pass);
        }
    }
}

impl ShellApp {
    /// Uploads a composited frame to the GPU via the stored render state.
    /// `refit` marks the viewport to re-fit on the next canvas paint (used on
    /// sprite selection, not on playback ticks where the camera should hold).
    #[allow(clippy::cast_precision_loss)] // canvas dims <= 8192 are exact in f32
    pub(crate) fn upload_frame(&mut self, frame: &PixelBuffer, refit: bool) {
        let Some(rs) = self.render_state.as_ref() else {
            return;
        };
        let mut guard = rs.renderer.write();
        if let Some(renderer) = guard.callback_resources.get_mut::<ViewportRenderer>() {
            renderer.set_frame(&rs.device, &rs.queue, frame);
            self.frame_size = Some([frame.width() as f32, frame.height() as f32]);
            if refit {
                self.needs_fit = true;
            }
        }
    }

    /// Draws the wgpu canvas and routes pan (drag) and zoom (wheel) input.
    pub(crate) fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        if self.render_state.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("wgpu unavailable — canvas disabled");
            });
            return;
        }

        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        let ppp = ui.ctx().pixels_per_point();
        let vp_px = Vec2::new(rect.width() * ppp, rect.height() * ppp);
        if vp_px.x < 1.0 || vp_px.y < 1.0 {
            return;
        }

        // Fit the freshly-uploaded frame once the viewport size is known.
        if self.needs_fit {
            if let Some(size) = self.frame_size {
                self.viewport.fit(Vec2::from(size), vp_px);
            }
            self.needs_fit = false;
        }

        // Pan on drag.
        if response.dragged() {
            let d = response.drag_delta();
            self.viewport
                .pan_by_screen(Vec2::new(d.x * ppp, d.y * ppp));
        }

        // Zoom on wheel, keeping the canvas point under the cursor fixed.
        if response.hovered() {
            let scroll_y = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > f32::EPSILON {
                if let Some(pos) = response.hover_pos() {
                    let local = pos - rect.min;
                    let cursor_px = Vec2::new(local.x * ppp, local.y * ppp);
                    let factor = (scroll_y * 0.005).exp();
                    let new_zoom = clamp_zoom(self.viewport.zoom * factor);
                    self.viewport.zoom_at(cursor_px, vp_px, new_zoom);
                    ui.ctx().request_repaint();
                }
            }
        }

        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            SpriteCallback {
                resolution: vp_px.into(),
                scroll: self.viewport.scroll.into(),
                zoom: self.viewport.zoom,
            },
        ));
    }
}
