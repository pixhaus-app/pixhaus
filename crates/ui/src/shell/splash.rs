//! Startup splash: a full-window `CentralPanel` showing the brand logo at launch,
//! gated on `UiState.splash == Active`. While active the runtime draws the splash
//! ALONE (no regions), so it is the first and only thing on screen until it dismisses
//! - after a fixed beat (see `SPLASH_SECONDS`) or on a click or Escape.
//!
//! Dismissal routes through the intent queue ([`Intent::DismissSplash`]), never a
//! mid-frame mutation. The overlay requests a repaint every active frame so the timer
//! advances on an idle launch where no input wakes the loop.

use crate::state::Host;
use crate::state::intent::Intent;
use crate::state::ui_state::SplashPhase;
use crate::theme::tokens::SurfaceTier;

/// How long the splash shows before it auto-dismisses, in seconds.
const SPLASH_SECONDS: f64 = 5.0;

/// The widest the logo may render, in points, capped to a share of the viewport so it
/// never overflows on a small window. NEAREST keeps the pixel art crisp.
const LOGO_MAX_WIDTH: f32 = 520.0;

/// Draw the splash overlay while the splash phase is `Active`.
pub fn overlay(host: &mut Host, ui: &mut egui::Ui) {
    let SplashPhase::Active { since } = host.state.ui.splash else {
        return;
    };

    let now = ui.ctx().input(|i| i.time);
    match since {
        // First active frame: stamp the start time, then keep painting until it elapses.
        None => host.intents.push(Intent::SetSplashStart(now)),
        Some(start) if now - start >= SPLASH_SECONDS => {
            host.intents.push(Intent::DismissSplash);
        }
        Some(_) => {}
    }

    // Skip on click or Escape.
    let skipped = ui.ctx().input(|i| i.pointer.any_click() || i.key_pressed(egui::Key::Escape));
    if skipped {
        host.intents.push(Intent::DismissSplash);
    }

    // Keep the loop awake so the timer advances without input.
    ui.ctx().request_repaint();

    // The runtime draws the splash alone (no regions) while active, so a CentralPanel
    // filling the window is the reliable full cover from the first frame - unlike an
    // Area sized from `content_rect`, which is not yet the full surface on frame one.
    let fill = host.theme.surface(SurfaceTier::AppFrame);
    let logo_width = LOGO_MAX_WIDTH.min(ui.available_width() * 0.6);

    egui::CentralPanel::default().frame(egui::Frame::new().fill(fill)).show_inside(ui, |ui| {
        ui.centered_and_justified(|ui| {
            ui.add(
                egui::Image::new(crate::brand::LOGO)
                    .texture_options(egui::TextureOptions::NEAREST)
                    .max_width(logo_width)
                    .maintain_aspect_ratio(true),
            );
        });
    });
}
