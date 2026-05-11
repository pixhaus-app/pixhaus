//! Builds the `app` global table exposed to every script.
//!
//! The table structure mirrors Aseprite's `app` global where possible:
//!
//! - `app.activeSprite` — current sprite, or `nil`
//! - `app.activeLayer`  — current layer, or `nil`
//! - `app.activeFrame`  — current frame, or `nil`
//! - `app.fgColor`      — foreground color
//! - `app.bgColor`      — background color
//! - `app.command`      — command namespace (Aseprite-compat)
//! - `app.commands`     — `{ register = fn }` (Pixhaus extension)
//! - `app.ai`           — `{ registerVerb = fn }` (Pixhaus extension)
//! - `app.ui`           — `{ panel = fn }` (Pixhaus extension, stub)

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::Mutex;
use pixhaus_core::project::FrameIndex;

use crate::bindings::color::LuaColor;
use crate::bindings::frame::LuaFrame;
use crate::bindings::layer::LuaLayer;
use crate::bindings::palette::LuaPalette;
use crate::bindings::sprite::LuaSprite;
use crate::context::{ScriptContext, resolve_active_sprite};
use crate::output::{CommandDef, PanelDef, ScriptMutation, VerbDef};

/// Shared collectors for the output of a single script run.
#[derive(Default)]
pub struct OutputCollectors {
    /// Mutations queued during script execution.
    pub mutations: Arc<Mutex<Vec<ScriptMutation>>>,
    /// Verb registrations accumulated across script runs.
    pub verbs: Arc<Mutex<Vec<VerbDef>>>,
    /// Command palette registrations accumulated across script runs.
    pub commands: Arc<Mutex<Vec<CommandDef>>>,
    /// Panel registrations accumulated across script runs.
    pub panels: Arc<Mutex<Vec<PanelDef>>>,
}

impl OutputCollectors {
    /// Creates fresh empty collectors.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Populates `app` in the Lua globals from `ctx` and wires the
/// mutation/registration sinks to `collectors`.
#[allow(clippy::too_many_lines, clippy::similar_names)]
pub fn register(lua: &Lua, ctx: &ScriptContext, collectors: &OutputCollectors) -> LuaResult<()> {
    let app = lua.create_table()?;

    // activeSprite / activeLayer / activeFrame / activePalette — all
    // derived from the active sprite resolved via the library accessor.
    if let Some(sprite) = resolve_active_sprite(ctx) {
        let lua_sprite = LuaSprite::from_sprite(sprite, &collectors.mutations);
        app.set("activeSprite", lua.create_userdata(lua_sprite)?)?;

        let layer_idx = ctx.active_layer_index.unwrap_or(0);
        if let Some(layer) = sprite.layers.get(layer_idx) {
            let lua_layer = LuaLayer::from_layer(layer, collectors.mutations.clone());
            app.set("activeLayer", lua.create_userdata(lua_layer)?)?;
        } else {
            app.set("activeLayer", LuaValue::Nil)?;
        }

        // activeFrame
        if let Some(frame_idx) = ctx.active_frame_index {
            if let Some(frame) = sprite.frames.get(frame_idx.get() as usize) {
                let lua_frame =
                    LuaFrame::from_frame(frame, frame_idx, collectors.mutations.clone());
                app.set("activeFrame", lua.create_userdata(lua_frame)?)?;
            } else {
                app.set("activeFrame", LuaValue::Nil)?;
            }
        } else if let Some(frame) = sprite.frames.first() {
            // Default to frame 0 when no frame is explicitly focused.
            let lua_frame =
                LuaFrame::from_frame(frame, FrameIndex::new(0), collectors.mutations.clone());
            app.set("activeFrame", lua.create_userdata(lua_frame)?)?;
        } else {
            app.set("activeFrame", LuaValue::Nil)?;
        }

        // activePalette (convenience alias, not in Aseprite API but useful)
        if let Some(palette) = sprite.palettes.first() {
            let lua_pal = LuaPalette::from_palette(palette, collectors.mutations.clone());
            app.set("activePalette", lua.create_userdata(lua_pal)?)?;
        } else {
            app.set("activePalette", LuaValue::Nil)?;
        }
    } else {
        app.set("activeSprite", LuaValue::Nil)?;
        app.set("activeLayer", LuaValue::Nil)?;
        app.set("activeFrame", LuaValue::Nil)?;
        app.set("activePalette", LuaValue::Nil)?;
    }

    // fgColor / bgColor
    app.set(
        "fgColor",
        lua.create_userdata(LuaColor::from(ctx.fg_color))?,
    )?;
    app.set(
        "bgColor",
        lua.create_userdata(LuaColor::from(ctx.bg_color))?,
    )?;

    // app.command — Aseprite-compat command namespace (no-ops for now; wired
    // up by the host after S37 lands)
    let cmd_ns = lua.create_table()?;
    app.set("command", cmd_ns)?;

    // app.commands — Pixhaus extension: register custom command palette entries
    let commands_verbs = collectors.commands.clone();
    let cmds_ns = lua.create_table()?;
    let register_command = lua.create_function(move |_, def: LuaTable| {
        let id: String = def
            .get("id")
            .map_err(|_| LuaError::runtime("app.commands.register: 'id' field required"))?;
        let title: String = def
            .get("title")
            .map_err(|_| LuaError::runtime("app.commands.register: 'title' field required"))?;
        let callback_fn: String = def
            .get("callbackFn")
            .map_err(|_| LuaError::runtime("app.commands.register: 'callbackFn' field required"))?;
        commands_verbs.lock().push(CommandDef {
            id,
            title,
            callback_fn,
        });
        Ok(())
    })?;
    cmds_ns.set("register", register_command)?;
    app.set("commands", cmds_ns)?;

    // app.ai — Pixhaus extension: register custom AI verbs
    let ai_verbs = collectors.verbs.clone();
    let ai_table = lua.create_table()?;
    let register_verb = lua.create_function(move |_, def: LuaTable| {
        let id: String = def
            .get("id")
            .map_err(|_| LuaError::runtime("app.ai.registerVerb: 'id' field required"))?;
        let title: String = def
            .get("title")
            .map_err(|_| LuaError::runtime("app.ai.registerVerb: 'title' field required"))?;
        let description: String = def.get("description").unwrap_or_default();
        let backend: Option<String> = def.get("backend").ok();
        let invoke_fn: String = def
            .get("invokeFn")
            .map_err(|_| LuaError::runtime("app.ai.registerVerb: 'invokeFn' field required"))?;
        ai_verbs.lock().push(VerbDef {
            id,
            title,
            description,
            backend,
            invoke_fn,
        });
        Ok(())
    })?;
    ai_table.set("registerVerb", register_verb)?;
    app.set("ai", ai_table)?;

    // app.ui — Pixhaus extension: register custom panels
    let ui_panels = collectors.panels.clone();
    let ui_table = lua.create_table()?;
    let register_panel = lua.create_function(move |_, def: LuaTable| {
        let title: String = def
            .get("title")
            .map_err(|_| LuaError::runtime("app.ui.panel: 'title' field required"))?;
        let build_fn: String = def
            .get("buildFn")
            .map_err(|_| LuaError::runtime("app.ui.panel: 'buildFn' field required"))?;
        ui_panels.lock().push(PanelDef { title, build_fn });
        Ok(())
    })?;
    ui_table.set("panel", register_panel)?;
    app.set("ui", ui_table)?;

    lua.globals().set("app", app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ScriptContext;
    use pixhaus_core::project::Rgba;

    #[test]
    fn empty_context_populates_nil_sprite() {
        let lua = Lua::new();
        crate::bindings::color::register(&lua).unwrap();
        let collectors = OutputCollectors::new();
        let ctx = ScriptContext::empty();
        register(&lua, &ctx, &collectors).unwrap();
        let app: LuaTable = lua.globals().get("app").unwrap();
        let sprite: LuaValue = app.get("activeSprite").unwrap();
        assert!(matches!(sprite, LuaValue::Nil));
    }

    #[test]
    fn fg_color_reflects_context() {
        let lua = Lua::new();
        crate::bindings::color::register(&lua).unwrap();
        let collectors = OutputCollectors::new();
        let mut ctx = ScriptContext::empty();
        ctx.fg_color = Rgba::opaque(123, 45, 67);
        register(&lua, &ctx, &collectors).unwrap();

        let app: LuaTable = lua.globals().get("app").unwrap();
        let fg: LuaAnyUserData = app.get("fgColor").unwrap();
        let color = fg.borrow::<LuaColor>().unwrap();
        assert_eq!(color.rgba.r, 123);
        assert_eq!(color.rgba.g, 45);
    }
}
