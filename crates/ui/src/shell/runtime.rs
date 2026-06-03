//! The per-frame shell composition and the post-loop intent drain.
//!
//! Three things make this loop borrow-check (spec "The borrow-safe per-frame
//! loop"): panels get a read-only state view plus write channels; `Panel::ui` and
//! `Tool::options_ui` are `&self`, so iterating the registries is a shared borrow
//! that coexists with the sibling `&mut` fields; and mutation is deferred past the
//! loop, where intents are drained and applied after every region borrow drops.
//! The one-frame latency is invisible in immediate mode.

use crate::shell::{about, command_palette, regions, shortcuts, splash};
use crate::state::Host;
use crate::state::intent::apply_intent;
use crate::state::ui_state::SplashPhase;

/// The shell runtime. Owns the per-frame region composition and intent drain.
pub struct Shell;

impl Shell {
    /// Compose every region for this frame, then apply the intents collected.
    ///
    /// Region order is the egui panel-ordering contract: outer panels first, the
    /// `CentralPanel` (canvas stage) last, the palette `Area` after that. The
    /// status bar is declared before the tray so it pins to the lower edge.
    pub fn run(host: &mut Host, ui: &mut egui::Ui) {
        // Clear last frame's intents before anything pushes this frame's.
        host.intents.0.clear();

        // While the splash is active it OWNS the frame: draw only the splash (a full
        // window-covering CentralPanel) and skip every region, so the editor never
        // flashes underneath before the splash. Timing and the click/Escape skip route
        // through the intent queue, drained here before returning.
        if matches!(host.state.ui.splash, SplashPhase::Active { .. }) {
            splash::overlay(host, ui);
            drain_intents(host, ui.ctx());
            return;
        }

        // Collect this frame's shortcut intents before any region runs, so a keystroke
        // and a click land in push order.
        shortcuts::collect(ui.ctx(), &host.registries, &mut host.intents);

        // Advance the animation playhead once per frame, before any region reads it, so
        // the canvas and the timeline see the same clock; then refresh the read-only
        // playback mirror the Animate panels render from. Requesting a repaint while
        // playing keeps the loop awake so the next frame advances too.
        if host.state.ui.playback.playing {
            let dt = ui.ctx().input(|input| input.stable_dt);
            host.state.ui.playback.playhead_seconds += dt;
            ui.ctx().request_repaint();
        }
        super::sync_playback_mirror(host);

        // egui panel order: outer panels first, CentralPanel LAST.
        regions::top_bar::show(host, ui);
        regions::tool_options::show(host, ui);
        regions::left_rail::show(host, ui);
        regions::status_bar::show(host, ui); // outermost bottom - pins below the tray
        regions::bottom_tray::show(host, ui);
        regions::right_dock::show(host, ui);
        regions::canvas_stage::show(host, ui); // CentralPanel - fills the rest
        command_palette::overlay(host, ui); // Area on top if modal == CommandPalette
        about::overlay(host, ui); // Area on top if modal == About

        drain_intents(host, ui.ctx());
    }
}

/// Apply this frame's queued intents in push order. Taking the vec out first ends the
/// `&mut host.intents` borrow before `apply_intent` reborrows the whole host.
fn drain_intents(host: &mut Host, ctx: &egui::Context) {
    let intents = std::mem::take(&mut host.intents.0);
    for intent in intents {
        apply_intent(host, intent, ctx);
    }
}
