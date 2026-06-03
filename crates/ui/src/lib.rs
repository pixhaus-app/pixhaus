//! Pixhaus UI layer: the egui contribution surface and the canvas embedding.
//!
//! `ui` is the only crate that knows both egui and `render`. It will host the
//! Panel/Tool/Workspace/Provider/Importer/Exporter/Validator traits, the
//! registries, the `Module` trait, and the theme tokens (architecture bible
//! sections 7 and 8).
//!
//! Scaffold stage: it carries the egui-to-`render` seam — installing the
//! [`ViewportRenderer`] into egui-wgpu's resource store and dispatching the canvas
//! draw through [`CanvasCallback`]. The registries and trait surface land as the
//! workspaces are built.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::float_cmp))]

pub mod brand;
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

use pixhaus_render::ViewportRenderer;

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
}

impl CallbackTrait for CanvasCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        // Upload happens in `prepare` (before the egui pass): it has `&mut` resources
        // and the queue. `paint` only reads. Upload nothing when the frame is unchanged.
        if let (Some(frame), Some(renderer)) = (self.frame.as_ref(), callback_resources.get_mut::<ViewportRenderer>()) {
            renderer.upload_frame(device, queue, &frame.rgba, frame.width, frame.height);
        }
        // The upload rides the queue; no extra command buffers to submit.
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
    use super::{CanvasCallback, CanvasFrame};
    use std::sync::Arc;

    /// The callback carries only the changing frame data (an `Arc`-backed buffer),
    /// never GPU handles — those live in the callback resource store.
    #[test]
    fn canvas_callback_holds_an_optional_frame() {
        let empty = CanvasCallback { frame: None };
        assert!(empty.frame.is_none());
        let with_frame = CanvasCallback {
            frame: Some(CanvasFrame {
                rgba: Arc::new(vec![0u8; 4]),
                width: 1,
                height: 1,
            }),
        };
        assert!(with_frame.frame.is_some());
    }
}
