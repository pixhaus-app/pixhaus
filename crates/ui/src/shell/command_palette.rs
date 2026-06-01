//! Command palette overlay (Ctrl/Cmd+K). An `egui::Area`, modal-gated. Body lands in SHELL.12.

use crate::state::Host;

/// Draw the palette overlay when the modal is open. Body lands in SHELL.12.
pub fn overlay(_host: &mut Host, _ui: &mut egui::Ui) {}
