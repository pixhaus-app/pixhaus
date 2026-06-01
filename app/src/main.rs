//! Pixhaus application binary: the eframe + egui host shell.
//!
//! The Host App layer (architecture bible section 4.1). It owns the single tokio
//! runtime, boots the window, and runs the egui loop; the canvas is drawn by
//! `render` through the `ui` paint callback. Module registration and the workspace
//! host land as those systems are built.

use anyhow::Context;
use eframe::egui;

/// Top-level application state, owned across frames by the eframe loop.
///
/// A unit struct at scaffold stage: the document, tool selection, and undo stack
/// move in as the shared sprite-editing core lands.
struct PixhausApp;

impl PixhausApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            pixhaus_ui::install_canvas_renderer(render_state);
        }
        Self
    }
}

impl eframe::App for PixhausApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Pixhaus");
                ui.separator();
                ui.label("v3 scaffold");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            painter.add(egui_wgpu::Callback::new_paint_callback(response.rect, pixhaus_ui::CanvasCallback));
        });
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // The binary owns the single tokio runtime; entering it makes tokio::spawn
    // available to the egui loop for the background work the editor will grow.
    let runtime = tokio::runtime::Runtime::new().context("failed to start the tokio runtime")?;
    let _guard = runtime.enter();

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixhaus")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native("pixhaus", options, Box::new(|cc| Ok(Box::new(PixhausApp::new(cc)))))?;

    Ok(())
}
