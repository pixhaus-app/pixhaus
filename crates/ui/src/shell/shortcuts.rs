//! Global shortcut collection.
//!
//! `collect` reads input once per frame with `consume_shortcut` so a focused
//! `TextEdit` and the global handler never both fire. The mapping is
//! registry-driven: workspace and tool shortcuts come from each capability's
//! `meta().shortcut`, the single source of truth the modules register. The shell
//! owns one shortcut directly - Cmd+K for the command palette.
//!
//! The pure decision is [`shortcut_fires`], unit-tested without a frame (spec test
//! 6): a command-modifier shortcut (workspace Cmd+1..5, the palette) always fires;
//! a bare single-key shortcut (a tool key) is suppressed when a text field is
//! focused, so typing "b" in the prompt does not switch to Pencil.

use crate::registry::Registries;
use crate::state::intent::{Intent, IntentSink};

/// The command palette shortcut. Shell-owned (not a workspace or tool), so it lives
/// here rather than in a registered capability's `meta()`.
const PALETTE: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::K);

/// Pure focus gate: should a shortcut with these modifiers fire given focus state?
///
/// A command-modifier shortcut is not a typed character, so it fires regardless of
/// focus. A bare shortcut (no command modifier) is a typed character, so it is
/// suppressed whenever a text field has focus - typing in a field must not steal a
/// tool key.
pub fn shortcut_fires(modifiers: egui::Modifiers, text_field_focused: bool) -> bool {
    modifiers.command || !text_field_focused
}

/// Read input once this frame and push the resulting intents.
///
/// Drives the mapping from the registries: every workspace's and tool's
/// `meta().shortcut` is the source of truth. Consumes each fired shortcut so a
/// focused `TextEdit` and this handler do not both act on the same key. Bare tool
/// keys are gated through [`shortcut_fires`]; command-modifier shortcuts are not.
pub fn collect(ctx: &egui::Context, registries: &Registries, intents: &mut IntentSink) {
    let text_field_focused = ctx.text_edit_focused();

    // Shell-owned: the command palette. A command-modifier shortcut, always eligible.
    if ctx.input_mut(|i| i.consume_shortcut(&PALETTE)) {
        intents.push(Intent::OpenCommandPalette);
    }

    // Workspace switches (Cmd+1..5): read each workspace's authored shortcut.
    for ws in registries.workspaces.iter() {
        let sc = ws.meta().shortcut;
        if shortcut_fires(sc.modifiers, text_field_focused) && ctx.input_mut(|i| i.consume_shortcut(&sc)) {
            intents.push(Intent::SelectWorkspace(ws.id()));
        }
    }

    // Tool shortcuts (bare single keys): read each tool's optional authored shortcut,
    // gated on focus so a keystroke in a text field is left for the field.
    for tool in registries.tools.iter() {
        let Some(sc) = tool.meta().shortcut else {
            continue;
        };
        if shortcut_fires(sc.modifiers, text_field_focused) && ctx.input_mut(|i| i.consume_shortcut(&sc)) {
            intents.push(Intent::SelectTool(tool.id()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PALETTE, shortcut_fires};

    /// A bare tool key (no command modifier) is suppressed when a text field is
    /// focused - spec test 6: typing "b" in the prompt must NOT switch to Pencil.
    #[test]
    fn tool_key_suppressed_when_text_field_focused() {
        assert!(!shortcut_fires(egui::Modifiers::NONE, true));
    }

    /// The same bare tool key fires when no text field has focus.
    #[test]
    fn tool_key_fires_when_not_focused() {
        assert!(shortcut_fires(egui::Modifiers::NONE, false));
    }

    /// A command-modifier shortcut (workspace Cmd+1..5) fires even with a text field
    /// focused - it is not a typed character, so it is not gated.
    #[test]
    fn command_shortcut_fires_even_when_text_focused() {
        assert!(shortcut_fires(egui::Modifiers::COMMAND, true));
        assert!(shortcut_fires(egui::Modifiers::COMMAND, false));
    }

    /// The shell-owned palette shortcut is Cmd+K and, being command-modified, is
    /// never gated by focus.
    #[test]
    fn palette_is_cmd_k_and_ungated() {
        assert_eq!(PALETTE, egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::K));
        assert!(shortcut_fires(PALETTE.modifiers, true));
    }
}
