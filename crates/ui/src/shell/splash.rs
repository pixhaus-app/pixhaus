//! Startup splash overlay: a full-screen `egui::Area` showing the brand logo for a
//! short beat at launch, gated on `UiState.splash == Active`. It auto-dismisses after
//! a fixed beat (see `SPLASH_SECONDS`) and is skippable by a click or Escape.
//!
//! Dismissal routes through the intent queue ([`Intent::DismissSplash`]), never a
//! mid-frame mutation. The overlay requests a repaint every active frame so the timer
//! advances on an idle launch where no input wakes the loop.

use crate::state::Host;
use crate::state::intent::Intent;
use crate::state::ui_state::SplashPhase;
use crate::theme::tokens::SurfaceTier;

/// How long the splash shows before it auto-dismisses, in seconds.
const SPLASH_SECONDS: f64 = 1.8;

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

    let theme = &host.theme;
    let content = ui.ctx().content_rect();
    let fill = theme.surface(SurfaceTier::AppFrame);
    let logo_width = LOGO_MAX_WIDTH.min(content.width() * 0.6);

    egui::Area::new(egui::Id::new("pixhaus.splash"))
        .order(egui::Order::Foreground)
        .fixed_pos(content.min)
        .show(ui.ctx(), |ui| {
            // A frame the size of the whole content rect, filled with the app-frame
            // surface, covers the regions beneath while the splash is active.
            let frame = egui::Frame::new().fill(fill);
            frame.show(ui, |ui| {
                ui.set_min_size(content.size());
                ui.centered_and_justified(|ui| {
                    ui.add(
                        egui::Image::new(crate::brand::LOGO)
                            .texture_options(egui::TextureOptions::NEAREST)
                            .max_width(logo_width)
                            .maintain_aspect_ratio(true),
                    );
                });
            });
        });
}
