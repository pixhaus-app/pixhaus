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

/// The shell runtime. Owns the per-frame region composition and intent drain.
pub struct Shell;

impl Shell {
    /// Compose every region for this frame, then apply the intents collected.
    ///
    /// Region order is the egui panel-ordering contract: outer panels first, the
    /// `CentralPanel` (canvas stage) last, the palette `Area` after that. The
    /// status bar is declared before the tray so it pins to the lower edge.
    pub fn run(host: &mut Host, ui: &mut egui::Ui) {
        // Clear last frame's intents, then collect this frame's shortcut intents
        // before any region runs, so a keystroke and a click land in push order.
        host.intents.0.clear();
        shortcuts::collect(ui.ctx(), &host.registries, &mut host.intents);

        // Splash first: its full-screen Foreground Area covers the regions while
        // active, but the regions still run beneath it on the uniform path (no early
        // return) so layout settles and the first post-splash frame is ready.
        splash::overlay(host, ui);

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

        // All region borrows have dropped. Take the queued intents out so the
        // `&mut host.intents` borrow ends before `apply_intent(host, ...)` reborrows
        // the host, then apply them in push order.
        let intents = std::mem::take(&mut host.intents.0);
        for intent in intents {
            apply_intent(host, intent, ui.ctx());
        }
    }
}
