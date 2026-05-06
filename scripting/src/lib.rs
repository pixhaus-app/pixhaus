//! Pixhaus scripting: Lua 5.4 bindings via `mlua`, with hot-reload support.
//!
//! # API surface
//!
//! Scripts have access to the `app` global, mirroring Aseprite's Lua API
//! where possible so existing scripts have a migration path. Pixhaus
//! adds extensions for AI verbs, custom commands, and UI panels.
//!
//! ```lua
//! -- Read sprite info
//! local sprite = app.activeSprite
//! if sprite then
//!     print(sprite.width, sprite.height)
//!     for i, layer in ipairs(sprite.layers) do
//!         print(i, layer.name)
//!     end
//! end
//!
//! -- Work with colors
//! local c = Color(255, 0, 0)
//! local c2 = Color{r=0, g=255, b=0, a=128}
//!
//! -- Register a custom AI verb (Pixhaus extension)
//! function invokeSharpener(context) end
//! app.ai.registerVerb{
//!     id = "my-plugin:sharpen",
//!     title = "Sharpen",
//!     invokeFn = "invokeSharpener"
//! }
//! ```
//!
//! # Integration points
//!
//! `LuaRuntime` is the main entry point. The app layer (S37) holds a
//! `LuaRuntime` per plugin, calls `execute` when the plugin script
//! loads, and calls `execute_file` for hot-reload.
//!
//! `ScriptContext` carries a snapshot of the current editor state.
//! `ScriptOutput` returns mutations (apply via the undo system) and
//! registrations (forward to the verb registry and command palette).

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        clippy::disallowed_methods,
    )
)]

pub mod bindings;
pub mod context;
pub mod error;
pub mod output;
pub mod runtime;

pub use context::ScriptContext;
pub use error::{Error, Result};
pub use output::{CommandDef, PanelDef, ScriptMutation, ScriptOutput, VerbDef};
pub use runtime::LuaRuntime;
