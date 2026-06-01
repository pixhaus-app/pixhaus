//! Global shortcut collection: workspace Cmd+1..5, Cmd+K, focus-gated tool keys.
//! Real body and the pure key->intent mapping land in SHELL.11.

use crate::registry::Registries;
use crate::state::intent::IntentSink;

/// Read input once per frame and push the resulting intents. Body lands in SHELL.11.
pub fn collect(_ctx: &egui::Context, _registries: &Registries, _intents: &mut IntentSink) {}
