//! JSON metadata types for sprite sheet exports.
//!
//! The schema matches the Aseprite JSON **array-of-frames** dialect as
//! specified in `docs/unity-handoff.md` (B6). All types derive
//! [`serde::Serialize`]; there is no deserialize path because the exporter
//! only writes, never reads, the JSON.

use serde::Serialize;

use pixhaus_core::project::{BlendMode, LayerKind, LoopDirection, Sprite};

use crate::Result;
use crate::png::pack::FramePlacement;

// ── Root structure ───────────────────────────────────────────────────────────

/// Root JSON structure emitted by the sprite sheet exporter.
#[derive(Debug, Serialize)]
pub struct SheetJson {
    /// Per-frame placement, timing, and source-rect data.
    pub frames: Vec<FrameJson>,
    /// Sheet-level metadata consumed by engine importers.
    pub meta: MetaJson,
}

// ── Frame entries ────────────────────────────────────────────────────────────

/// One frame entry in the sprite sheet JSON.
#[derive(Debug, Serialize)]
pub struct FrameJson {
    /// `"{sprite_name} {frame_index}"`. Zero-indexed.
    pub filename: String,
    /// Rectangle of this frame within the packed PNG.
    pub frame: RectJson,
    /// Always `false`. Frame rotation in the atlas is not supported.
    pub rotated: bool,
    /// `true` when the frame was alpha-trimmed (set from `FrameTrim.trimmed`).
    pub trimmed: bool,
    /// Canvas region covered by this frame.
    ///
    /// Equals `frame` when not trimmed; when trimmed it describes
    /// where the trimmed region sits within the full canvas.
    #[serde(rename = "spriteSourceSize")]
    pub sprite_source_size: RectJson,
    /// Full canvas size.
    #[serde(rename = "sourceSize")]
    pub source_size: SizeJson,
    /// Display duration in milliseconds.
    pub duration: u32,
}

// ── Meta block ───────────────────────────────────────────────────────────────

/// Sprite sheet metadata block (`meta` key in the JSON).
#[derive(Debug, Serialize)]
pub struct MetaJson {
    /// Exporter identifier. Always `"Pixhaus"`.
    pub app: String,
    /// Handoff schema version. Always `"1.0"`.
    pub version: String,
    /// PNG filename referenced by this JSON (basename only, no directory).
    pub image: String,
    /// Pixel format. Always `"RGBA8888"`.
    pub format: String,
    /// Packed sheet dimensions.
    pub size: SizeJson,
    /// Scale factor. Always `"1"` for Aseprite importer compatibility.
    pub scale: String,
    /// Named frame ranges from `Sprite.frame_tags`.
    #[serde(rename = "frameTags")]
    pub frame_tags: Vec<FrameTagJson>,
    /// Layer list: all non-reference layers, bottom-to-top.
    pub layers: Vec<LayerJson>,
    /// Named slice regions from `Sprite.slices`.
    pub slices: Vec<SliceJson>,
}

// ── Frame tags ───────────────────────────────────────────────────────────────

/// Named frame range entry in `meta.frameTags`.
#[derive(Debug, Serialize)]
pub struct FrameTagJson {
    /// Tag display name.
    pub name: String,
    /// First frame index, inclusive.
    pub from: u32,
    /// Last frame index, inclusive.
    pub to: u32,
    /// Playback direction per the B6 spec.
    pub direction: String,
    /// Repeat count. `0` means loop forever.
    pub repeat: u32,
    /// Colour hint. Always `"#000000ff"`.
    pub color: String,
}

// ── Layers ───────────────────────────────────────────────────────────────────

/// Layer entry in `meta.layers`.
#[derive(Debug, Serialize)]
pub struct LayerJson {
    /// Layer display name.
    pub name: String,
    /// Layer opacity, `0`–`255`.
    pub opacity: u32,
    /// Blend mode string per the B6 spec.
    #[serde(rename = "blendMode")]
    pub blend_mode: String,
}

// ── Slices ───────────────────────────────────────────────────────────────────

/// Named slice region in `meta.slices`.
#[derive(Debug, Serialize)]
pub struct SliceJson {
    /// Slice display name.
    pub name: String,
    /// Colour hint. Always `"#0000ffff"`.
    pub color: String,
    /// Per-frame slice geometry keys.
    pub keys: Vec<SliceKeyJson>,
}

/// Per-frame slice geometry entry.
#[derive(Debug, Serialize)]
pub struct SliceKeyJson {
    /// Frame from which this key takes effect.
    pub frame: u32,
    /// Slice rectangle in canvas coordinates.
    pub bounds: RectJson,
    /// Nine-slice center patch, present when the slice has nine-slice insets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<RectJson>,
    /// Pivot offset from `bounds.origin`, present when the slice has a pivot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pivot: Option<IVec2Json>,
}

// ── Primitive JSON shapes ────────────────────────────────────────────────────

/// A rectangle `{ x, y, w, h }`.
///
/// `x` and `y` are `i32` to accommodate canvas coordinates, which may be
/// negative for off-canvas cels and slices.
#[derive(Debug, Serialize, Clone, Copy)]
pub struct RectJson {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

/// A size `{ w, h }`.
#[derive(Debug, Serialize, Clone, Copy)]
pub struct SizeJson {
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

/// A 2D integer vector `{ x, y }`.
#[derive(Debug, Serialize, Clone, Copy)]
pub struct IVec2Json {
    /// Horizontal component.
    pub x: i32,
    /// Vertical component.
    pub y: i32,
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Build and serialize the sprite sheet JSON for `sprite`.
///
/// `placements` and `trims` must each have the same length as `sprite.frames`.
/// `sprite_name` is used as the base name for frame filenames and the PNG
/// filename in `meta.image`.
#[allow(clippy::cast_possible_wrap)]
pub fn build_json(
    sprite: &Sprite,
    placements: &[FramePlacement],
    trims: &[super::FrameTrim],
    sheet_width: u32,
    sheet_height: u32,
    sprite_name: &str,
) -> Result<Vec<u8>> {
    let cw = sprite.canvas.width;
    let ch = sprite.canvas.height;
    let png_name = format!("{sprite_name}.png");

    // Internal invariant: one placement and one trim per frame. The
    // public `export_sprite_sheet` validates frame counts at the API
    // boundary, so a mismatch here would mean the exporter pipeline is
    // out of sync with itself. Indexing (rather than `zip`) makes the
    // failure loud — out-of-bounds panics in release; the debug_assert
    // catches the same misuse in tests with a clearer message.
    debug_assert_eq!(
        placements.len(),
        sprite.frames.len(),
        "build_json: one placement per frame"
    );
    debug_assert_eq!(
        trims.len(),
        sprite.frames.len(),
        "build_json: one trim per frame"
    );

    let frames: Vec<FrameJson> = sprite
        .frames
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            let placement = &placements[index];
            let trim = &trims[index];
            // Rect of this frame's bitmap within the packed sheet.
            let in_sheet = RectJson {
                x: placement.x as i32,
                y: placement.y as i32,
                w: trim.w,
                h: trim.h,
            };
            // Where the trimmed region sits within the full canvas.
            let on_canvas = RectJson {
                x: trim.x as i32,
                y: trim.y as i32,
                w: trim.w,
                h: trim.h,
            };
            FrameJson {
                filename: format!("{sprite_name} {index}"),
                frame: in_sheet,
                rotated: false,
                trimmed: trim.trimmed,
                sprite_source_size: on_canvas,
                source_size: SizeJson { w: cw, h: ch },
                duration: frame.duration_ms,
            }
        })
        .collect();

    let frame_tags: Vec<FrameTagJson> = sprite
        .frame_tags
        .iter()
        .map(|tag| FrameTagJson {
            name: tag.name.clone(),
            from: tag.range.start.get(),
            to: tag.range.end.get(),
            direction: loop_direction_str(tag.loop_direction).to_owned(),
            repeat: u32::from(tag.repeat),
            color: "#000000ff".to_owned(),
        })
        .collect();

    // All non-reference layers, bottom-to-top (the order they appear in
    // Sprite.layers, which is index 0 = bottom).
    let layers: Vec<LayerJson> = sprite
        .layers
        .iter()
        .filter(|l| !matches!(l.kind, LayerKind::Reference { .. }))
        .map(|l| LayerJson {
            name: l.name.clone(),
            opacity: u32::from(l.opacity),
            blend_mode: blend_mode_str(l.blend_mode).to_owned(),
        })
        .collect();

    let slices = build_slices(sprite);

    let root = SheetJson {
        frames,
        meta: MetaJson {
            app: "Pixhaus".to_owned(),
            version: "1.0".to_owned(),
            image: png_name,
            format: "RGBA8888".to_owned(),
            size: SizeJson {
                w: sheet_width,
                h: sheet_height,
            },
            scale: "1".to_owned(),
            frame_tags,
            layers,
            slices,
        },
    };

    serde_json::to_vec_pretty(&root).map_err(crate::Error::JsonSerialize)
}

// ── Slice builder ────────────────────────────────────────────────────────────

fn build_slices(sprite: &Sprite) -> Vec<SliceJson> {
    sprite
        .slices
        .iter()
        .map(|slice| SliceJson {
            name: slice.name.clone(),
            color: "#0000ffff".to_owned(),
            keys: slice
                .keys
                .iter()
                .map(|key| SliceKeyJson {
                    frame: key.frame.get(),
                    bounds: RectJson {
                        x: key.bounds.origin.x,
                        y: key.bounds.origin.y,
                        w: key.bounds.size.width,
                        h: key.bounds.size.height,
                    },
                    center: key.nine_slice.map(|ns| RectJson {
                        x: ns.center.origin.x,
                        y: ns.center.origin.y,
                        w: ns.center.size.width,
                        h: ns.center.size.height,
                    }),
                    pivot: key.pivot.map(|p| IVec2Json {
                        x: p.offset.x,
                        y: p.offset.y,
                    }),
                })
                .collect(),
        })
        .collect()
}

// ── Mapping helpers ──────────────────────────────────────────────────────────

/// Map [`LoopDirection`] to the B6 JSON string.
///
/// `PingPong` maps to `"pingpong"` (no underscore), matching the Aseprite
/// dialect. The data model's `snake_case` serialization (`"ping_pong"`) is
/// different and must not be used here.
fn loop_direction_str(dir: LoopDirection) -> &'static str {
    match dir {
        LoopDirection::Forward => "forward",
        LoopDirection::Reverse => "reverse",
        LoopDirection::PingPong => "pingpong",
        LoopDirection::PingPongReverse => "pingpong_reverse",
    }
}

/// Map [`BlendMode`] to the B6 JSON string.
fn blend_mode_str(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "normal",
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::ColorBurn => "color_burn",
        BlendMode::Lighten => "lighten",
        BlendMode::Screen => "screen",
        BlendMode::ColorDodge => "color_dodge",
        BlendMode::Addition => "addition",
        BlendMode::Overlay => "overlay",
        BlendMode::SoftLight => "soft_light",
        BlendMode::HardLight => "hard_light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Subtract => "subtract",
        BlendMode::Divide => "divide",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
        BlendMode::LinearBurn => "linear_burn",
        BlendMode::DarkerColor => "darker_color",
        BlendMode::LinearDodge => "linear_dodge",
        BlendMode::LighterColor => "lighter_color",
        BlendMode::VividLight => "vivid_light",
        BlendMode::LinearLight => "linear_light",
        BlendMode::PinLight => "pin_light",
        BlendMode::HardMix => "hard_mix",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pingpong_direction_has_no_underscore() {
        assert_eq!(loop_direction_str(LoopDirection::PingPong), "pingpong");
    }

    #[test]
    fn pingpong_reverse_matches_spec() {
        assert_eq!(
            loop_direction_str(LoopDirection::PingPongReverse),
            "pingpong_reverse"
        );
    }

    #[test]
    fn all_loop_directions_are_mapped() {
        // Exhaustive — if a new variant is added to LoopDirection the match
        // in loop_direction_str will fail to compile.
        assert_eq!(loop_direction_str(LoopDirection::Forward), "forward");
        assert_eq!(loop_direction_str(LoopDirection::Reverse), "reverse");
        assert_eq!(loop_direction_str(LoopDirection::PingPong), "pingpong");
        assert_eq!(
            loop_direction_str(LoopDirection::PingPongReverse),
            "pingpong_reverse"
        );
    }

    #[test]
    fn color_burn_blend_mode_has_underscore() {
        assert_eq!(blend_mode_str(BlendMode::ColorBurn), "color_burn");
    }

    #[test]
    fn normal_blend_mode_maps_correctly() {
        assert_eq!(blend_mode_str(BlendMode::Normal), "normal");
    }
}
