//! Lua `Sprite` userdata — mirrors Aseprite's Sprite class.
//!
//! Exposes width, height, colorMode, and the layer, frame, and palette
//! collections. All collections are returned as Lua tables (1-indexed).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::Mutex;
use pixhaus_core::project::{ColorMode, FrameIndex, Sprite};

use crate::bindings::frame::LuaFrame;
use crate::bindings::layer::LuaLayer;
use crate::bindings::palette::LuaPalette;
use crate::output::ScriptMutation;

/// Lua `Sprite` userdata.
#[derive(Clone)]
pub struct LuaSprite {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) color_mode: ColorMode,
    pub(crate) layers: Vec<LuaLayer>,
    pub(crate) frames: Vec<LuaFrame>,
    pub(crate) palettes: Vec<LuaPalette>,
}

impl LuaSprite {
    /// Wraps a core `Sprite` snapshot with a shared mutation sink.
    #[must_use]
    pub fn from_sprite(sprite: &Sprite, mutations: &Arc<Mutex<Vec<ScriptMutation>>>) -> Self {
        let layers = sprite
            .layers
            .iter()
            .map(|l| LuaLayer::from_layer(l, mutations.clone()))
            .collect();

        let frames = sprite
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| {
                #[allow(clippy::cast_possible_truncation)]
                let idx = FrameIndex::new(i as u32);
                LuaFrame::from_frame(f, idx, mutations.clone())
            })
            .collect();

        let palettes = sprite
            .palettes
            .iter()
            .map(|p| LuaPalette::from_palette(p, mutations.clone()))
            .collect();

        Self {
            width: sprite.canvas.width,
            height: sprite.canvas.height,
            color_mode: sprite.color_mode,
            layers,
            frames,
            palettes,
        }
    }

    /// Maps `ColorMode` to its Aseprite-compatible string.
    #[must_use]
    pub fn color_mode_str(&self) -> &'static str {
        match self.color_mode {
            ColorMode::Rgba => "rgb",
            ColorMode::Grayscale => "grayscale",
            ColorMode::Indexed => "indexed",
        }
    }
}

impl LuaUserData for LuaSprite {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.width));
        fields.add_field_method_get("height", |_, this| Ok(this.height));
        fields.add_field_method_get("colorMode", |_, this| Ok(this.color_mode_str()));

        // layers — Lua table, 1-indexed
        fields.add_field_method_get("layers", |lua, this| {
            let t = lua.create_table()?;
            for (i, layer) in this.layers.iter().enumerate() {
                t.set(i + 1, lua.create_userdata(layer.clone())?)?;
            }
            Ok(t)
        });

        // frames — Lua table, 1-indexed
        fields.add_field_method_get("frames", |lua, this| {
            let t = lua.create_table()?;
            for (i, frame) in this.frames.iter().enumerate() {
                t.set(i + 1, lua.create_userdata(frame.clone())?)?;
            }
            Ok(t)
        });

        // palettes — Lua table, 1-indexed
        fields.add_field_method_get("palettes", |lua, this| {
            let t = lua.create_table()?;
            for (i, palette) in this.palettes.iter().enumerate() {
                t.set(i + 1, lua.create_userdata(palette.clone())?)?;
            }
            Ok(t)
        });

        // palette — the active (first) palette, or nil
        fields.add_field_method_get("palette", |lua, this| {
            if let Some(pal) = this.palettes.first() {
                Ok(LuaValue::UserData(lua.create_userdata(pal.clone())?))
            } else {
                Ok(LuaValue::Nil)
            }
        });
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(format!("Sprite({}x{})", this.width, this.height))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{Layer, LayerId, Palette, PaletteId, Rgba, Size, Sprite, SpriteId};

    fn make_sprite() -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "test", Size::new(32, 32));
        s.layers.push(Layer::raster(LayerId::new(1), "bg"));
        s.palettes.push(Palette::from_colors(
            PaletteId::new(1),
            "main",
            vec![Rgba::opaque(255, 0, 0)],
        ));
        s
    }

    #[test]
    fn from_sprite_copies_dimensions() {
        let s = make_sprite();
        let sink = Arc::new(Mutex::new(Vec::new()));
        let ls = LuaSprite::from_sprite(&s, &sink);
        assert_eq!(ls.width, 32);
        assert_eq!(ls.height, 32);
    }

    #[test]
    fn layers_and_palettes_copied() {
        let s = make_sprite();
        let sink = Arc::new(Mutex::new(Vec::new()));
        let ls = LuaSprite::from_sprite(&s, &sink);
        assert_eq!(ls.layers.len(), 1);
        assert_eq!(ls.palettes.len(), 1);
    }

    #[test]
    fn color_mode_str_rgba() {
        let s = make_sprite();
        let sink = Arc::new(Mutex::new(Vec::new()));
        let ls = LuaSprite::from_sprite(&s, &sink);
        assert_eq!(ls.color_mode_str(), "rgb");
    }
}
