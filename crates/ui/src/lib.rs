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
pub mod region;
pub mod registry;
pub mod shell;
pub mod state;
pub mod theme;
pub mod widgets;

use egui::epaint::PaintCallbackInfo;
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState};
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

/// egui paint callback that draws the canvas viewport via [`ViewportRenderer`].
///
/// A unit struct carrying no GPU state: the renderer lives in the callback
/// resource store (see [`install_canvas_renderer`]), and this only dispatches the
/// draw inside egui's render pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct CanvasCallback;

impl CallbackTrait for CanvasCallback {
    fn paint(&self, _info: PaintCallbackInfo, render_pass: &mut RenderPass<'static>, callback_resources: &CallbackResources) {
        if let Some(renderer) = callback_resources.get::<ViewportRenderer>() {
            renderer.paint(render_pass);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CanvasCallback;

    /// The callback is a zero-sized handle — it must hold no GPU state, since that
    /// lives in the callback resource store instead.
    #[test]
    fn canvas_callback_is_zero_sized() {
        assert_eq!(std::mem::size_of::<CanvasCallback>(), 0);
    }
}
