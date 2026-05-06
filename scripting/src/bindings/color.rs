//! Lua `Color` type — mirrors Aseprite's Color class.
//!
//! Constructed via `Color(r, g, b[, a])` or `Color{r=, g=, b=, a=}` for
//! RGBA, or `Color{index=N}` for palette-index mode. Read/write `.r`,
//! `.g`, `.b`, `.a`. Palette-index colors expose `.index`.
//!
//! HSV helpers (`.hsvHue`, `.hsvSaturation`, `.hsvValue`) follow
//! Aseprite convention: hue in [0, 360), saturation/value in [0, 1].

use mlua::prelude::*;
use pixhaus_core::project::Rgba;

/// Lua `Color` userdata.
///
/// Wraps either an RGBA color or a palette-index color. Index colors
/// carry the index and an RGBA of `(0, 0, 0, 0)`; they resolve to
/// actual RGBA at the rendering boundary via the sprite's active palette.
#[derive(Clone, Debug, PartialEq)]
pub struct LuaColor {
    /// RGBA value. For index-mode colors this is the zero color.
    pub rgba: Rgba,
    /// Palette index, or `None` for direct RGBA colors.
    pub index: Option<u8>,
}

impl LuaColor {
    /// Constructs an RGBA color.
    #[must_use]
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            rgba: Rgba::new(r, g, b, a),
            index: None,
        }
    }

    /// Constructs a palette-index color.
    #[must_use]
    pub fn from_index(index: u8) -> Self {
        Self {
            rgba: Rgba::transparent(),
            index: Some(index),
        }
    }

    /// Returns `"rgb"` or `"index"`, matching Aseprite's `color.type`.
    #[must_use]
    pub fn color_type(&self) -> &'static str {
        if self.index.is_some() { "index" } else { "rgb" }
    }

    /// Converts this color's RGB values to HSV.
    ///
    /// Returns `(hue_degrees, saturation, value)` where hue is in [0, 360),
    /// saturation and value are in [0, 1]. Aseprite convention.
    #[must_use]
    pub fn to_hsv(&self) -> (f64, f64, f64) {
        rgb_to_hsv(self.rgba)
    }
}

impl From<Rgba> for LuaColor {
    fn from(rgba: Rgba) -> Self {
        Self { rgba, index: None }
    }
}

impl From<LuaColor> for Rgba {
    fn from(c: LuaColor) -> Self {
        c.rgba
    }
}

impl FromLua for LuaColor {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected Color userdata")),
        }
    }
}

impl LuaUserData for LuaColor {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("r", |_, this| Ok(this.rgba.r));
        fields.add_field_method_set("r", |_, this, v: u8| {
            this.rgba.r = v;
            Ok(())
        });

        fields.add_field_method_get("g", |_, this| Ok(this.rgba.g));
        fields.add_field_method_set("g", |_, this, v: u8| {
            this.rgba.g = v;
            Ok(())
        });

        fields.add_field_method_get("b", |_, this| Ok(this.rgba.b));
        fields.add_field_method_set("b", |_, this, v: u8| {
            this.rgba.b = v;
            Ok(())
        });

        fields.add_field_method_get("a", |_, this| Ok(this.rgba.a));
        fields.add_field_method_set("a", |_, this, v: u8| {
            this.rgba.a = v;
            Ok(())
        });

        fields.add_field_method_get("index", |_, this| Ok(this.index.map(u32::from)));
        fields.add_field_method_get("type", |_, this| Ok(this.color_type()));

        fields.add_field_method_get("hsvHue", |_, this| Ok(rgb_to_hsv(this.rgba).0));
        fields.add_field_method_get("hsvSaturation", |_, this| Ok(rgb_to_hsv(this.rgba).1));
        fields.add_field_method_get("hsvValue", |_, this| Ok(rgb_to_hsv(this.rgba).2));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(format!(
                "Color(r={}, g={}, b={}, a={})",
                this.rgba.r, this.rgba.g, this.rgba.b, this.rgba.a
            ))
        });
        methods.add_meta_method(LuaMetaMethod::Eq, |_, this, other: LuaColor| {
            Ok(*this == other)
        });
    }
}

/// Registers the `Color` constructor in the Lua globals.
///
/// After this call, scripts can write:
/// ```lua
/// local c = Color(255, 0, 0)          -- opaque red
/// local c = Color(255, 0, 0, 128)     -- semi-transparent red
/// local c = Color{r=255, g=0, b=0}    -- table form
/// local c = Color{index=3}            -- palette index
/// ```
pub fn register(lua: &Lua) -> LuaResult<()> {
    let ctor = lua.create_function(|lua, args: LuaMultiValue| {
        if args.len() == 1 {
            if let Some(LuaValue::Table(t)) = args.iter().next() {
                if let Ok(LuaValue::Integer(idx)) = t.get("index") {
                    let byte = u8::try_from(idx)
                        .map_err(|_| LuaError::runtime("Color{index}: index must be 0-255"))?;
                    return lua.create_userdata(LuaColor::from_index(byte));
                }
                let r: u8 = t.get::<u8>("r").unwrap_or(0);
                let g: u8 = t.get::<u8>("g").unwrap_or(0);
                let b: u8 = t.get::<u8>("b").unwrap_or(0);
                let a: u8 = t.get::<u8>("a").unwrap_or(255);
                return lua.create_userdata(LuaColor::from_rgba(r, g, b, a));
            }
        }
        let mut iter = args.iter();
        let r = next_u8(&mut iter, "r")?;
        let g = next_u8(&mut iter, "g")?;
        let b = next_u8(&mut iter, "b")?;
        let a = iter.next().and_then(to_u8).unwrap_or(255);
        lua.create_userdata(LuaColor::from_rgba(r, g, b, a))
    })?;

    lua.globals().set("Color", ctor)?;
    Ok(())
}

fn next_u8<'a>(iter: &mut impl Iterator<Item = &'a LuaValue>, name: &str) -> LuaResult<u8> {
    let v = iter
        .next()
        .ok_or_else(|| LuaError::runtime(format!("Color: missing argument '{name}'")))?;
    to_u8(v).ok_or_else(|| LuaError::runtime(format!("Color: '{name}' must be an integer 0-255")))
}

fn to_u8(v: &LuaValue) -> Option<u8> {
    match v {
        LuaValue::Integer(i) => u8::try_from(*i).ok(),
        LuaValue::Number(f) => {
            #[allow(clippy::cast_possible_truncation)]
            let i = *f as i64;
            u8::try_from(i).ok()
        }
        _ => None,
    }
}

fn rgb_to_hsv(rgba: Rgba) -> (f64, f64, f64) {
    let r = f64::from(rgba.r) / 255.0;
    let g = f64::from(rgba.g) / 255.0;
    let b = f64::from(rgba.b) / 255.0;

    let cmax = r.max(g).max(b);
    let cmin = r.min(g).min(b);
    let delta = cmax - cmin;

    let hue = if delta < f64::EPSILON {
        0.0
    } else if (cmax - r).abs() < f64::EPSILON {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (cmax - g).abs() < f64::EPSILON {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    let saturation = if cmax < f64::EPSILON {
        0.0
    } else {
        delta / cmax
    };

    (hue, saturation, cmax)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_rgba_round_trip() {
        let c = LuaColor::from_rgba(10, 20, 30, 40);
        assert_eq!(c.rgba, Rgba::new(10, 20, 30, 40));
        assert!(c.index.is_none());
        assert_eq!(c.color_type(), "rgb");
    }

    #[test]
    fn from_index_sets_type() {
        let c = LuaColor::from_index(7);
        assert_eq!(c.index, Some(7));
        assert_eq!(c.color_type(), "index");
    }

    #[test]
    fn rgba_to_lua_color_conversion() {
        let rgba = Rgba::opaque(50, 100, 150);
        let c: LuaColor = rgba.into();
        assert_eq!(c.rgba, rgba);
        assert!(c.index.is_none());
    }

    #[test]
    fn lua_color_to_rgba_conversion() {
        let c = LuaColor::from_rgba(50, 100, 150, 200);
        let rgba: Rgba = c.into();
        assert_eq!(rgba, Rgba::new(50, 100, 150, 200));
    }

    #[test]
    fn hsv_pure_red() {
        let (h, s, v) = rgb_to_hsv(Rgba::opaque(255, 0, 0));
        assert!((h - 0.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);
    }

    #[test]
    fn hsv_pure_green() {
        let (h, s, v) = rgb_to_hsv(Rgba::opaque(0, 255, 0));
        assert!((h - 120.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);
    }

    #[test]
    fn hsv_black() {
        let (_, s, v) = rgb_to_hsv(Rgba::opaque(0, 0, 0));
        assert!((s - 0.0).abs() < 0.01);
        assert!((v - 0.0).abs() < 0.01);
    }

    #[test]
    fn hsv_white() {
        let (_, s, v) = rgb_to_hsv(Rgba::opaque(255, 255, 255));
        assert!((s - 0.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);
    }

    #[test]
    fn to_u8_integer() {
        assert_eq!(to_u8(&LuaValue::Integer(128)), Some(128));
        assert_eq!(to_u8(&LuaValue::Integer(256)), None);
        assert_eq!(to_u8(&LuaValue::Integer(-1)), None);
    }
}
