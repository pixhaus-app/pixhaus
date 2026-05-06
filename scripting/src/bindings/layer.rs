//! Lua `Layer` userdata — mirrors Aseprite's Layer class.
//!
//! Read-write properties: `name`, `opacity`, `blendMode`, `isVisible`,
//! `isLocked`. Read-only: `isTilemap`, `isGroup`, `id`.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::Mutex;
use pixhaus_core::project::{BlendMode, Layer, LayerId, LayerKind};

use crate::output::ScriptMutation;

/// Lua `Layer` userdata.
///
/// Holds a snapshot of the layer for reads; mutations are queued into
/// the shared sink rather than applied to the snapshot.
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct LuaLayer {
    pub(crate) id: LayerId,
    pub(crate) name: String,
    pub(crate) opacity: u8,
    pub(crate) blend_mode: BlendMode,
    pub(crate) visible: bool,
    pub(crate) locked: bool,
    pub(crate) is_tilemap: bool,
    pub(crate) is_group: bool,
    pub(crate) mutations: Arc<Mutex<Vec<ScriptMutation>>>,
}

impl LuaLayer {
    /// Wraps a core `Layer` with a shared mutation sink.
    #[must_use]
    pub fn from_layer(layer: &Layer, mutations: Arc<Mutex<Vec<ScriptMutation>>>) -> Self {
        let is_tilemap = matches!(layer.kind, LayerKind::Tilemap { .. });
        let is_group = matches!(layer.kind, LayerKind::Group { .. });
        Self {
            id: layer.id,
            name: layer.name.clone(),
            opacity: layer.opacity,
            blend_mode: layer.blend_mode,
            visible: layer.visible,
            locked: layer.locked,
            is_tilemap,
            is_group,
            mutations,
        }
    }
}

impl LuaUserData for LuaLayer {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        // id — read-only
        fields.add_field_method_get("id", |_, this| Ok(this.id.get()));

        // name — read/write
        fields.add_field_method_get("name", |_, this| Ok(this.name.clone()));
        fields.add_field_method_set("name", |_, this, v: String| {
            this.mutations.lock().push(ScriptMutation::SetLayerName {
                layer_id: this.id,
                name: v.clone(),
            });
            this.name = v;
            Ok(())
        });

        // opacity — read/write (0-255, matching Aseprite)
        fields.add_field_method_get("opacity", |_, this| Ok(this.opacity));
        fields.add_field_method_set("opacity", |_, this, v: u8| {
            this.mutations.lock().push(ScriptMutation::SetLayerOpacity {
                layer_id: this.id,
                opacity: v,
            });
            this.opacity = v;
            Ok(())
        });

        // blendMode — read/write, string name
        fields.add_field_method_get("blendMode", |_, this| {
            Ok(blend_mode_to_str(this.blend_mode))
        });
        fields.add_field_method_set("blendMode", |_, this, v: String| {
            let bm = blend_mode_from_str(&v)
                .ok_or_else(|| LuaError::runtime(format!("unknown blend mode: '{v}'")))?;
            this.mutations
                .lock()
                .push(ScriptMutation::SetLayerBlendMode {
                    layer_id: this.id,
                    blend_mode: bm,
                });
            this.blend_mode = bm;
            Ok(())
        });

        // isVisible — read/write
        fields.add_field_method_get("isVisible", |_, this| Ok(this.visible));
        fields.add_field_method_set("isVisible", |_, this, v: bool| {
            this.mutations
                .lock()
                .push(ScriptMutation::SetLayerVisibility {
                    layer_id: this.id,
                    visible: v,
                });
            this.visible = v;
            Ok(())
        });

        // isLocked — read-only (lock toggling needs confirmation UI, not script)
        fields.add_field_method_get("isLocked", |_, this| Ok(this.locked));

        // Kind flags — read-only
        fields.add_field_method_get("isTilemap", |_, this| Ok(this.is_tilemap));
        fields.add_field_method_get("isGroup", |_, this| Ok(this.is_group));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(format!("Layer(\"{}\")", this.name))
        });
    }
}

/// Maps a `BlendMode` to its Aseprite-compatible string name.
#[must_use]
pub fn blend_mode_to_str(bm: BlendMode) -> &'static str {
    match bm {
        BlendMode::Normal => "normal",
        BlendMode::Multiply => "multiply",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::Darken => "darken",
        BlendMode::Lighten => "lighten",
        BlendMode::ColorDodge => "color_dodge",
        BlendMode::ColorBurn => "color_burn",
        BlendMode::HardLight => "hard_light",
        BlendMode::SoftLight => "soft_light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hsl_hue",
        BlendMode::Saturation => "hsl_saturation",
        BlendMode::Color => "hsl_color",
        BlendMode::Luminosity => "hsl_luminosity",
        BlendMode::Addition => "addition",
        BlendMode::Subtract => "subtract",
        BlendMode::Divide => "divide",
    }
}

/// Parses an Aseprite-compatible blend mode string.
#[must_use]
pub fn blend_mode_from_str(s: &str) -> Option<BlendMode> {
    match s {
        "normal" => Some(BlendMode::Normal),
        "multiply" => Some(BlendMode::Multiply),
        "screen" => Some(BlendMode::Screen),
        "overlay" => Some(BlendMode::Overlay),
        "darken" => Some(BlendMode::Darken),
        "lighten" => Some(BlendMode::Lighten),
        "color_dodge" => Some(BlendMode::ColorDodge),
        "color_burn" => Some(BlendMode::ColorBurn),
        "hard_light" => Some(BlendMode::HardLight),
        "soft_light" => Some(BlendMode::SoftLight),
        "difference" => Some(BlendMode::Difference),
        "exclusion" => Some(BlendMode::Exclusion),
        "hsl_hue" => Some(BlendMode::Hue),
        "hsl_saturation" => Some(BlendMode::Saturation),
        "hsl_color" => Some(BlendMode::Color),
        "hsl_luminosity" => Some(BlendMode::Luminosity),
        "addition" => Some(BlendMode::Addition),
        "subtract" => Some(BlendMode::Subtract),
        "divide" => Some(BlendMode::Divide),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::LayerId;

    fn make_layer() -> Layer {
        Layer::raster(LayerId::new(1), "bg")
    }

    #[test]
    fn from_layer_copies_fields() {
        let l = make_layer();
        let sink = Arc::new(Mutex::new(Vec::new()));
        let ll = LuaLayer::from_layer(&l, sink);
        assert_eq!(ll.name, "bg");
        assert_eq!(ll.opacity, 255);
        assert!(ll.visible);
        assert!(!ll.locked);
        assert!(!ll.is_tilemap);
        assert!(!ll.is_group);
    }

    #[test]
    fn blend_mode_round_trip() {
        let modes = [
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
        ];
        for bm in modes {
            let s = blend_mode_to_str(bm);
            assert_eq!(
                blend_mode_from_str(s),
                Some(bm),
                "round-trip failed for {bm:?}"
            );
        }
    }

    #[test]
    fn unknown_blend_mode_returns_none() {
        assert!(blend_mode_from_str("not_a_mode").is_none());
    }

    #[test]
    fn tilemap_layer_flags() {
        use pixhaus_core::project::{LayerKind, TilesetId, UserData};
        let l = Layer {
            id: LayerId::new(2),
            name: "tiles".into(),
            kind: LayerKind::Tilemap {
                tileset: TilesetId::new(1),
            },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            user_data: UserData::default(),
        };
        let sink = Arc::new(Mutex::new(Vec::new()));
        let ll = LuaLayer::from_layer(&l, sink);
        assert!(ll.is_tilemap);
        assert!(!ll.is_group);
    }
}
