//! The Lua scripting runtime.
//!
//! `LuaRuntime` owns a persistent Lua 5.4 VM. Each call to `execute`
//! configures the `app` global from a `ScriptContext`, runs the chunk,
//! and drains all queued mutations and registrations into a
//! `ScriptOutput`.
//!
//! The VM persists across calls so that registered verbs, commands, and
//! panels survive for the editor session. The mutation sink is drained
//! (not replaced) after each execution, giving a clean output per call.

use mlua::prelude::*;
use mlua::{LuaOptions, StdLib};

use crate::bindings::{self, OutputCollectors};
use crate::context::ScriptContext;
use crate::error::{Error, Result};
use crate::output::ScriptOutput;

/// Standard libraries loaded for sandboxed (untrusted) scripts.
///
/// Deliberately excludes `os`, `io`, `debug`, `package`, and `ffi`. A
/// sandboxed script therefore cannot touch the filesystem (`io`), spawn
/// processes or read wall-clock time (`os`), load native code
/// (`package`/`ffi`), or use the `debug` library to reach outside the VM.
/// All pixel and project access is mediated by the host `app`/`Color`
/// globals. See `docs/planning/architecture/plugin-trust-model.md`.
fn sandboxed_stdlib() -> StdLib {
    StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8
}

/// A Lua 5.4 scripting VM with the Pixhaus host API pre-installed.
pub struct LuaRuntime {
    lua: Lua,
    collectors: OutputCollectors,
}

impl LuaRuntime {
    /// Creates a sandboxed runtime for untrusted scripts.
    ///
    /// Only the safe standard libraries are loaded (see
    /// [`sandboxed_stdlib`]); `os`, `io`, and `debug` are unavailable.
    /// The `Color` global is registered immediately. The `app` global is
    /// populated (or re-populated) on each `execute` call. Use this for
    /// any script whose provenance is not fully trusted.
    pub fn new() -> Result<Self> {
        Self::with_stdlib(sandboxed_stdlib())
    }

    /// Creates a runtime for trusted (developer-authored, in-repo)
    /// scripts that additionally grants the `os` and `io` libraries.
    ///
    /// This widens the sandbox to permit filesystem and clock access —
    /// e.g. the `palette-export` sample writes a file via `io`. Only use
    /// it for scripts the host has explicitly vetted; `debug`, `ffi`,
    /// and `package` stay unavailable even here. See
    /// `docs/planning/architecture/plugin-trust-model.md`.
    pub fn new_trusted() -> Result<Self> {
        Self::with_stdlib(sandboxed_stdlib() | StdLib::OS | StdLib::IO)
    }

    fn with_stdlib(libs: StdLib) -> Result<Self> {
        let lua = Lua::new_with(libs, LuaOptions::default())?;
        // Register the Color constructor once; it doesn't depend on context.
        crate::bindings::color::register(&lua)?;

        Ok(Self {
            lua,
            collectors: OutputCollectors::new(),
        })
    }

    /// Executes `script` with the given `ctx` and returns the output.
    ///
    /// Steps:
    /// 1. Re-populate `app` from `ctx` (overwrites any previous `app`).
    /// 2. Run the script chunk.
    /// 3. Drain mutations and registrations into a `ScriptOutput`.
    ///
    /// Registrations (verbs, commands, panels) are **accumulated**
    /// across calls — a plugin that registers a verb on load keeps
    /// that registration alive for the session. Mutations are per-call.
    ///
    /// On a Lua runtime error the partial output is rolled back: any
    /// mutations the script queued before the error are discarded, and
    /// any verb/command/panel registrations the failing call added are
    /// truncated back to their pre-call state. Registrations from
    /// **previous successful** calls remain in place; the host gets a
    /// clean error rather than a half-registered plugin.
    pub fn execute(&self, script: &str, ctx: &ScriptContext) -> Result<ScriptOutput> {
        // Re-populate app global from the current context.
        bindings::app::register(&self.lua, ctx, &self.collectors)?;

        // Snapshot the pre-call lengths of every accumulator. Used
        // below to roll back partial registrations if the script
        // errors mid-run.
        let pre_verbs = self.collectors.verbs.lock().len();
        let pre_commands = self.collectors.commands.lock().len();
        let pre_panels = self.collectors.panels.lock().len();

        // Run the script. The load+call pattern avoids needing exec/eval.
        let chunk = self.lua.load(script).set_name("pixhaus-script");
        if let Err(err) = chunk.call::<LuaMultiValue>(()) {
            // Roll back this call's partial state so the next execute()
            // doesn't surface mutations or registrations from a failed
            // run. Mutations get fully cleared (only this call could
            // have queued any after the previous successful drain).
            self.collectors.mutations.lock().clear();
            self.collectors.verbs.lock().truncate(pre_verbs);
            self.collectors.commands.lock().truncate(pre_commands);
            self.collectors.panels.lock().truncate(pre_panels);
            return Err(err.into());
        }

        // Drain mutations (per-call) and snapshot registrations (accumulated).
        let mutations = {
            let mut guard = self.collectors.mutations.lock();
            std::mem::take(&mut *guard)
        };
        let verbs = self.collectors.verbs.lock().clone();
        let commands = self.collectors.commands.lock().clone();
        let panels = self.collectors.panels.lock().clone();

        Ok(ScriptOutput {
            mutations,
            verbs,
            commands,
            panels,
        })
    }

    /// Executes a script file given its path on disk.
    pub fn execute_file(
        &self,
        path: &std::path::Path,
        ctx: &ScriptContext,
    ) -> Result<ScriptOutput> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| Error::Execution(format!("could not read {}: {e}", path.display())))?;
        self.execute(&source, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ScriptContext;

    #[test]
    fn runtime_constructs_without_error() {
        LuaRuntime::new().unwrap();
    }

    #[test]
    fn execute_empty_script_returns_empty_output() {
        let rt = LuaRuntime::new().unwrap();
        let ctx = ScriptContext::empty();
        let out = rt.execute("", &ctx).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn execute_script_can_use_color_global() {
        let rt = LuaRuntime::new().unwrap();
        let ctx = ScriptContext::empty();
        // Just verify Color() is available and the script runs without error.
        let out = rt
            .execute("local c = Color(255, 0, 0); assert(c.r == 255)", &ctx)
            .unwrap();
        assert!(out.mutations.is_empty());
    }

    #[test]
    fn execute_script_registers_command() {
        let rt = LuaRuntime::new().unwrap();
        let ctx = ScriptContext::empty();
        let script = r#"
            function myCallback() end
            app.commands.register{
                id = "myPlugin:hello",
                title = "Hello",
                callbackFn = "myCallback"
            }
        "#;
        let out = rt.execute(script, &ctx).unwrap();
        assert_eq!(out.commands.len(), 1);
        assert_eq!(out.commands[0].id, "myPlugin:hello");
    }

    #[test]
    fn execute_script_registers_verb() {
        let rt = LuaRuntime::new().unwrap();
        let ctx = ScriptContext::empty();
        let script = r#"
            function invokeMyVerb(ctx) end
            app.ai.registerVerb{
                id = "my-plugin:sharp-pencil",
                title = "Sharp Pencil",
                description = "Sharpens the pencil strokes",
                invokeFn = "invokeMyVerb"
            }
        "#;
        let out = rt.execute(script, &ctx).unwrap();
        assert_eq!(out.verbs.len(), 1);
        assert_eq!(out.verbs[0].id, "my-plugin:sharp-pencil");
    }

    #[test]
    fn execute_script_with_project_sees_sprite() {
        use pixhaus_core::project::{Layer, LayerId, Project, Size, Sprite, SpriteId};

        let rt = LuaRuntime::new().unwrap();
        let mut project = Project::new("test");
        let mut sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(16, 16));
        sprite.layers.push(Layer::raster(LayerId::new(1), "base"));
        install_test_sprite(&mut project, sprite);

        let mut ctx = ScriptContext::with_project(project);
        ctx.active_sprite_id = Some(SpriteId::new(1));

        let script = r"
            assert(app.activeSprite ~= nil)
            assert(app.activeSprite.width == 16)
            assert(app.activeSprite.height == 16)
        ";
        rt.execute(script, &ctx).unwrap();
    }

    #[test]
    fn execute_lua_error_surfaces_as_error() {
        let rt = LuaRuntime::new().unwrap();
        let ctx = ScriptContext::empty();
        let result = rt.execute("error('intentional test error')", &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn execute_rolls_back_partial_registrations_on_error() {
        // A script that registers a verb and then errors should not
        // leave the verb registration in the accumulator. The next
        // successful execute() should see a fresh state.
        let rt = LuaRuntime::new().unwrap();
        let ctx = ScriptContext::empty();
        let bad = r#"
            function invokeMyVerb(ctx) end
            app.ai.registerVerb{
                id = "broken-plugin:verb",
                title = "Broken Verb",
                description = "Registered then error",
                invokeFn = "invokeMyVerb"
            }
            error("intentional failure after register")
        "#;
        assert!(rt.execute(bad, &ctx).is_err());

        // Subsequent successful run must not see the broken-plugin verb.
        let out = rt.execute("", &ctx).unwrap();
        assert_eq!(
            out.verbs.len(),
            0,
            "rolled-back verbs leaked into next call"
        );
        assert!(out.mutations.is_empty(), "rolled-back mutations leaked");
    }

    #[test]
    fn execute_rolls_back_mutations_on_error() {
        // Mutations queued before the error must not surface in the
        // next successful call's output.
        use pixhaus_core::project::{Layer, LayerId, Project, Size, Sprite, SpriteId};

        let rt = LuaRuntime::new().unwrap();
        let mut project = Project::new("test");
        let mut sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(16, 16));
        sprite.layers.push(Layer::raster(LayerId::new(1), "base"));
        install_test_sprite(&mut project, sprite);
        let mut ctx = ScriptContext::with_project(project);
        ctx.active_sprite_id = Some(SpriteId::new(1));

        let bad = r#"
            app.activeSprite.name = "renamed-but-rolled-back"
            error("die after mutation")
        "#;
        assert!(rt.execute(bad, &ctx).is_err());

        let out = rt.execute("", &ctx).unwrap();
        assert!(out.mutations.is_empty(), "rolled-back mutations leaked");
    }

    #[test]
    fn sandboxed_runtime_denies_dangerous_libraries() {
        let rt = LuaRuntime::new().unwrap();
        let ctx = ScriptContext::empty();
        // os / io / debug are not loaded: indexing the nil global errors.
        assert!(
            rt.execute("return os.time()", &ctx).is_err(),
            "os must be unavailable in the sandbox"
        );
        assert!(
            rt.execute("return io.open('x', 'w')", &ctx).is_err(),
            "io must be unavailable in the sandbox"
        );
        assert!(
            rt.execute("return debug.getinfo(1)", &ctx).is_err(),
            "debug must be unavailable in the sandbox"
        );
        // Safe libraries remain available.
        rt.execute("assert(string.upper('a') == 'A')", &ctx)
            .unwrap();
        rt.execute("assert(math.max(1, 2) == 2)", &ctx).unwrap();
    }

    #[test]
    fn trusted_runtime_grants_os_and_io() {
        let rt = LuaRuntime::new_trusted().unwrap();
        let ctx = ScriptContext::empty();
        rt.execute("assert(type(os.time) == 'function')", &ctx)
            .unwrap();
        rt.execute("assert(type(io.open) == 'function')", &ctx)
            .unwrap();
        // debug stays unavailable even in trusted mode.
        assert!(
            rt.execute("return debug.getinfo(1)", &ctx).is_err(),
            "debug must stay unavailable even for trusted scripts"
        );
    }

    /// Wraps a sprite into the project library as a `Custom`-kind
    /// entity so the rest of the test body keeps the same shape after
    /// the B9.1 cleanup removed `Project.sprites`.
    fn install_test_sprite(
        project: &mut pixhaus_core::project::Project,
        sprite: pixhaus_core::project::Sprite,
    ) {
        use pixhaus_core::project::{
            ActiveTarget, AiMetadata, Entity, EntityContent, EntityDefaults, EntityId, EntityKind,
            NamedSprite, StateId, UserData,
        };

        let id = sprite.id;
        project.library.entities.push(Entity {
            id: EntityId::new(1),
            kind: EntityKind::Custom("Test".into()),
            name: "test".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: vec![NamedSprite {
                    id: StateId::new(1),
                    state_name: "primary".into(),
                    sprite,
                    engine_tags: Vec::new(),
                }],
                reference_sheet: None,
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });
        project.active = ActiveTarget::State {
            entity_id: EntityId::new(1),
            state_id: StateId::new(1),
        };
        let _ = id;
    }
}
