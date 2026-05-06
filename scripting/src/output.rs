//! Script output: mutations and registrations produced by a script run.
//!
//! When a script calls write-side methods (renaming a layer, setting a
//! palette color) it queues a `ScriptMutation`. When it registers a
//! verb, command, or panel it records a `Registration`. The runtime
//! collects both and returns them as `ScriptOutput` after execution.
//!
//! The caller (the app layer, arriving via S37) applies mutations
//! through the undo system and forwards registrations to the command
//! palette / verb registry.

use pixhaus_core::project::{BlendMode, FrameIndex, LayerId, PaletteId, Rgba};

/// A mutation queued by a script during execution.
///
/// Each variant maps 1:1 to one undo-able command in the app layer.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptMutation {
    /// Rename a layer.
    SetLayerName {
        /// Target layer.
        layer_id: LayerId,
        /// New display name.
        name: String,
    },
    /// Toggle layer visibility.
    SetLayerVisibility {
        /// Target layer.
        layer_id: LayerId,
        /// New visibility state.
        visible: bool,
    },
    /// Set layer opacity (0–255).
    SetLayerOpacity {
        /// Target layer.
        layer_id: LayerId,
        /// New opacity value.
        opacity: u8,
    },
    /// Set a layer's blend mode.
    SetLayerBlendMode {
        /// Target layer.
        layer_id: LayerId,
        /// New blend mode.
        blend_mode: BlendMode,
    },
    /// Change the RGBA value of a palette entry.
    SetPaletteColor {
        /// Target palette.
        palette_id: PaletteId,
        /// 0-based index into the palette's color list.
        index: usize,
        /// New color value.
        color: Rgba,
    },
    /// Change a frame's display duration.
    SetFrameDuration {
        /// Target frame.
        frame: FrameIndex,
        /// New duration in milliseconds.
        duration_ms: u32,
    },
}

/// Definition of a custom AI verb registered by a script.
///
/// The `invoke` field holds the Lua function name (not the function
/// itself — Lua functions cannot be serialised). The runtime stores
/// both this definition and the live Lua VM so it can call the
/// function by name on demand.
#[derive(Clone, Debug)]
pub struct VerbDef {
    /// Machine-readable identifier, e.g. `"my-plugin:sharp-pencil"`.
    pub id: String,
    /// Human-readable title shown in the AI menu.
    pub title: String,
    /// One-sentence description shown in the command palette.
    pub description: String,
    /// Preferred inference backend ("anthropic", "openai", "ollama", …).
    pub backend: Option<String>,
    /// Name of the Lua global function to call on invocation.
    pub invoke_fn: String,
}

/// Definition of a custom command registered by a script.
#[derive(Clone, Debug)]
pub struct CommandDef {
    /// Unique id, e.g. `"myPlugin:doSomething"`.
    pub id: String,
    /// Title shown in the command palette.
    pub title: String,
    /// Name of the Lua global function to call when the command fires.
    pub callback_fn: String,
}

/// Definition of a custom UI panel registered by a script.
#[derive(Clone, Debug)]
pub struct PanelDef {
    /// Panel title shown in the tab bar.
    pub title: String,
    /// Name of the Lua global function called to build the panel contents.
    pub build_fn: String,
}

/// All output produced by a single script execution.
#[derive(Clone, Debug, Default)]
pub struct ScriptOutput {
    /// Mutations to apply, in order, via the undo system.
    pub mutations: Vec<ScriptMutation>,
    /// Verb registrations to forward to the verb registry.
    pub verbs: Vec<VerbDef>,
    /// Command registrations to forward to the command palette.
    pub commands: Vec<CommandDef>,
    /// Panel registrations to forward to the UI layer.
    pub panels: Vec<PanelDef>,
}

impl ScriptOutput {
    /// Returns `true` when the script produced no mutations or registrations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
            && self.verbs.is_empty()
            && self.commands.is_empty()
            && self.panels.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_output_is_empty() {
        let out = ScriptOutput::default();
        assert!(out.is_empty());
    }

    #[test]
    fn output_not_empty_with_mutation() {
        let mut out = ScriptOutput::default();
        out.mutations.push(ScriptMutation::SetLayerName {
            layer_id: LayerId::new(1),
            name: "bg".into(),
        });
        assert!(!out.is_empty());
    }
}
