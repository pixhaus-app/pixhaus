//! The application shell: per-frame region composition, the command palette,
//! shortcut routing, and the menu structure (architecture bible section 8).
//!
//! `Shell::run` is called from `App::ui`; `drain_background` from `App::logic`.

pub mod about;
pub mod command_palette;
pub mod menus;
pub mod regions;
pub mod runtime;
pub mod shortcuts;
pub mod splash;

pub use runtime::Shell;

use crate::state::{BackgroundMsg, Host};

/// Drain background-channel results into session state, called from `App::logic`.
///
/// This is the single mpsc-drain front door (spec "Region composition and the
/// shell runtime"). This round it is a structured no-op: an empty `try_recv` loop
/// with no live sender beyond the bootstrap one, plus the one path that applies a
/// `BackgroundMsg` to `ai_status` to prove the channel path lives (bible rule 5).
/// It runs in `logic`, not `ui`, because `logic` runs even when the window is
/// occluded but a repaint was requested; if anything landed it requests a repaint
/// so the new state shows immediately.
pub fn drain_background(host: &mut Host, ctx: &egui::Context) {
    let mut landed = false;
    // `try_recv` returns `Err` on both an empty and a disconnected channel, so a
    // `while let Ok` cleanly stops draining in either case.
    while let Ok(msg) = host.bg.rx.try_recv() {
        match msg {
            BackgroundMsg::AiStatusChanged(status) => {
                host.state.session.ai_status = status;
                landed = true;
            }
        }
    }
    if landed {
        ctx.request_repaint();
    }
}
