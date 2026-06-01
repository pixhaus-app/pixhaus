//! Global shortcut collection.
//!
//! `collect` reads input once per frame with `consume_key` so a focused `TextEdit`
//! and the global handler never both fire. The decision logic is the pure
//! `map_key`, unit-tested without a frame (spec test 6): workspace Cmd+1..5 and
//! Cmd+K always map; bare tool keys are suppressed when a text field is focused, so
//! typing "b" in the prompt does not switch to Pencil.

use crate::contrib_api::ids::{ToolId, WorkspaceId};
use crate::registry::Registries;
use crate::state::intent::{Intent, IntentSink};

/// The canonical bare single-key tool shortcuts (spec "Left-rail tools").
///
/// This constant table is this round's stand-in; the eventual single source is each
/// tool's `meta().shortcut`, which `collect` will drive once the modules register.
const TOOL_KEYS: &[(egui::Key, ToolId)] = &[
    (egui::Key::B, ToolId("pencil")),
    (egui::Key::E, ToolId("eraser")),
    (egui::Key::G, ToolId("fill")),
    (egui::Key::L, ToolId("line")),
    (egui::Key::U, ToolId("rectangle")),
    (egui::Key::O, ToolId("ellipse")),
    (egui::Key::I, ToolId("eyedropper")),
    (egui::Key::M, ToolId("selection")),
    (egui::Key::Q, ToolId("lasso")),
    (egui::Key::V, ToolId("move")),
    (egui::Key::X, ToolId("text")),
    (egui::Key::H, ToolId("hand")),
    (egui::Key::Z, ToolId("zoom")),
    (egui::Key::J, ToolId("ai_brush")),
];

/// The workspace switch shortcuts (spec "Per-workspace placement"): Cmd+1..5.
const WORKSPACE_KEYS: &[(egui::Key, WorkspaceId)] = &[
    (egui::Key::Num1, WorkspaceId("draw")),
    (egui::Key::Num2, WorkspaceId("animate")),
    (egui::Key::Num3, WorkspaceId("tiles")),
    (egui::Key::Num4, WorkspaceId("generate")),
    (egui::Key::Num5, WorkspaceId("export")),
];

/// Pure decision: a key + modifiers + whether a text field is focused -> the intent.
///
/// Command-modifier shortcuts (workspace switch, palette) are not typed characters,
/// so they fire regardless of focus. Bare tool keys are typed characters, so they
/// are suppressed whenever a text field has focus.
pub fn map_key(key: egui::Key, mods: egui::Modifiers, text_field_focused: bool) -> Option<Intent> {
    if mods.command {
        if key == egui::Key::K {
            return Some(Intent::OpenCommandPalette);
        }
        if let Some((_, ws)) = WORKSPACE_KEYS.iter().find(|(k, _)| *k == key) {
            return Some(Intent::SelectWorkspace(*ws));
        }
        return None;
    }
    if text_field_focused {
        return None; // typing in a field: do not steal tool keys
    }
    TOOL_KEYS.iter().find(|(k, _)| *k == key).map(|(_, tool)| Intent::SelectTool(*tool))
}

/// Read input once this frame and push the resulting intents.
///
/// Consumes each matched key so a focused `TextEdit` and this handler do not both
/// fire. `_registries` is taken now so a later round can drive the mapping from the
/// workspaces' authored `meta().shortcut` instead of the constant table.
pub fn collect(ctx: &egui::Context, _registries: &Registries, intents: &mut IntentSink) {
    let text_field_focused = ctx.text_edit_focused();
    let cmd = egui::Modifiers::COMMAND;

    // Command-modifier shortcuts: the palette and the workspace switches.
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::K)) {
        if let Some(intent) = map_key(egui::Key::K, cmd, text_field_focused) {
            intents.push(intent);
        }
    }
    for (key, _) in WORKSPACE_KEYS {
        if ctx.input_mut(|i| i.consume_key(cmd, *key)) {
            if let Some(intent) = map_key(*key, cmd, text_field_focused) {
                intents.push(intent);
            }
        }
    }

    // Bare tool keys, gated on focus.
    if !text_field_focused {
        for (key, _) in TOOL_KEYS {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, *key)) {
                if let Some(intent) = map_key(*key, egui::Modifiers::NONE, text_field_focused) {
                    intents.push(intent);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::map_key;
    use crate::contrib_api::ids::{ToolId, WorkspaceId};
    use crate::state::intent::Intent;

    fn cmd() -> egui::Modifiers {
        egui::Modifiers::COMMAND
    }

    #[test]
    fn cmd_1_selects_first_workspace() {
        let out = map_key(egui::Key::Num1, cmd(), false);
        assert!(matches!(out, Some(Intent::SelectWorkspace(_))));
    }

    #[test]
    fn cmd_k_opens_command_palette() {
        let out = map_key(egui::Key::K, cmd(), false);
        assert!(matches!(out, Some(Intent::OpenCommandPalette)));
    }

    #[test]
    fn bare_b_selects_pencil_tool() {
        let out = map_key(egui::Key::B, egui::Modifiers::NONE, false);
        assert!(matches!(out, Some(Intent::SelectTool(ToolId("pencil")))));
    }

    #[test]
    fn tool_key_suppressed_when_text_field_focused() {
        // Spec test 6: typing "b" in the prompt must NOT switch to Pencil.
        let out = map_key(egui::Key::B, egui::Modifiers::NONE, true);
        assert!(out.is_none());
    }

    #[test]
    fn workspace_shortcut_fires_even_when_text_focused() {
        // A command modifier shortcut is not a typed character, so it is not gated.
        let out = map_key(egui::Key::Num1, cmd(), true);
        assert!(matches!(out, Some(Intent::SelectWorkspace(WorkspaceId("draw")))));
    }

    #[test]
    fn bare_b_with_command_is_not_a_tool_key() {
        // Cmd+B is not the bare tool key; tool keys require no command modifier.
        let out = map_key(egui::Key::B, cmd(), false);
        assert!(out.is_none());
    }
}
