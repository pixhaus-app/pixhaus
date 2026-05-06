//! Lua binding registration.
//!
//! Call [`register_all`] once per `Lua` VM to install all globals
//! (`Color`, `app`, etc.) before running any scripts.

pub mod app;
pub mod color;
pub mod frame;
pub mod layer;
pub mod palette;
pub mod sprite;

pub use app::OutputCollectors;

use mlua::prelude::*;

use crate::context::ScriptContext;

/// Installs all Pixhaus host API globals into `lua`.
///
/// Order matters: `color` must register before `app` so the `Color`
/// constructor is available for `app.fgColor`.
pub fn register_all(
    lua: &Lua,
    ctx: &ScriptContext,
    collectors: &OutputCollectors,
) -> LuaResult<()> {
    color::register(lua)?;
    app::register(lua, ctx, collectors)?;
    Ok(())
}
