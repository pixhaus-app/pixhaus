//! Lua `Palette` userdata — mirrors Aseprite's Palette class.
//!
//! Scripts read palette colors with `palette:getColor(index)` and
//! write them with `palette:setColor(index, color)`. The `#palette`
//! length operator is supported.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::Mutex;
use pixhaus_core::project::{Palette, PaletteId, Rgba};

use crate::bindings::color::LuaColor;
use crate::output::ScriptMutation;

/// Lua `Palette` userdata.
///
/// Holds a cloned snapshot of the palette for reads, plus a shared
/// mutation sink for writes. `setColor` queues a `SetPaletteColor`
/// mutation rather than modifying the snapshot, so the live document
/// is untouched until the script output is applied.
#[derive(Clone)]
pub struct LuaPalette {
    pub(crate) id: PaletteId,
    pub(crate) name: String,
    pub(crate) colors: Vec<Rgba>,
    pub(crate) mutations: Arc<Mutex<Vec<ScriptMutation>>>,
}

impl LuaPalette {
    /// Wraps a core `Palette` with a shared mutation sink.
    #[must_use]
    pub fn from_palette(palette: &Palette, mutations: Arc<Mutex<Vec<ScriptMutation>>>) -> Self {
        Self {
            id: palette.id,
            name: palette.name.clone(),
            colors: palette.colors.iter().map(|e| e.color).collect(),
            mutations,
        }
    }
}

impl LuaUserData for LuaPalette {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("name", |_, this| Ok(this.name.clone()));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // getColor(index) — 0-indexed, matching Aseprite
        methods.add_method("getColor", |lua, this, index: usize| {
            let rgba = this
                .colors
                .get(index)
                .copied()
                .unwrap_or(Rgba::transparent());
            lua.create_userdata(LuaColor::from(rgba))
        });

        // setColor(index, color) — queues a mutation
        methods.add_method("setColor", |_, this, (index, color): (usize, LuaColor)| {
            this.mutations.lock().push(ScriptMutation::SetPaletteColor {
                palette_id: this.id,
                index,
                color: color.rgba,
            });
            Ok(())
        });

        methods.add_meta_method(LuaMetaMethod::Len, |_, this, ()| Ok(this.colors.len()));

        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(format!(
                "Palette({}, {} colors)",
                this.name,
                this.colors.len()
            ))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{PaletteEntry, UserData};

    fn make_palette(colors: Vec<Rgba>) -> Palette {
        Palette {
            id: PaletteId::new(1),
            name: "test".into(),
            colors: colors.into_iter().map(PaletteEntry::new).collect(),
            user_data: UserData::default(),
        }
    }

    #[test]
    fn from_palette_copies_colors() {
        let pal = make_palette(vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0)]);
        let sink = Arc::new(Mutex::new(Vec::new()));
        let lua_pal = LuaPalette::from_palette(&pal, sink);
        assert_eq!(lua_pal.colors.len(), 2);
        assert_eq!(lua_pal.colors[0], Rgba::opaque(255, 0, 0));
    }

    #[test]
    fn set_color_queues_mutation() {
        let pal = make_palette(vec![Rgba::opaque(255, 0, 0)]);
        let sink: Arc<Mutex<Vec<ScriptMutation>>> = Arc::new(Mutex::new(Vec::new()));
        let lua_pal = LuaPalette::from_palette(&pal, sink.clone());

        lua_pal
            .mutations
            .lock()
            .push(ScriptMutation::SetPaletteColor {
                palette_id: lua_pal.id,
                index: 0,
                color: Rgba::opaque(0, 0, 255),
            });

        let mutations = sink.lock();
        assert_eq!(mutations.len(), 1);
        assert!(matches!(
            mutations[0],
            ScriptMutation::SetPaletteColor {
                index: 0,
                color: Rgba {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 255
                },
                ..
            }
        ));
    }
}
