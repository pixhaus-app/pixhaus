//! Verb: Cleanup (S27).
//!
//! Snaps a generated or imported sprite cel to the project palette,
//! removes sub-pixel anti-aliasing, and detects pivot drift across
//! animation frames. Runs entirely on the local CPU — no inference
//! backend required.
//!
//! # Operations
//!
//! **Palette snap.** Every opaque pixel is replaced by the nearest
//! palette entry (squared Euclidean sRGB distance, alpha ignored in
//! the distance calculation). Transparent pixels pass through as
//! `(0, 0, 0, 0)`.
//!
//! **AA removal** (opt-in). Semi-transparent pixels adjacent to opaque
//! content are merged into the nearest palette colour at full opacity.
//! Isolated semi-transparent pixels become fully transparent. This
//! removes the blurred fringe that image generators and PSD importers
//! routinely leave behind.
//!
//! **Pivot drift detection** (opt-in, structural only). Inspects each
//! slice's per-frame pivot keys. When any pivot deviates from the base
//! key by more than two pixels (see `PIVOT_DRIFT_THRESHOLD`) the slice name is
//! surfaced in `VerbOutput::notes`. No structural edits are committed —
//! the current effect protocol does not include a slice-update variant.

use std::time::Duration;

use async_trait::async_trait;
use pixhaus_core::project::{Cel, FrameIndex, Palette, PixelBufferId, Rgba, Size, Slice, Sprite};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable identifier for the built-in cleanup verb.
pub const CLEANUP_VERB_ID: &str = "pixhaus.builtin.cleanup";

/// Effect-local placeholder ID for the replacement pixel buffer. The
/// host rewrites this to a real [`PixelBufferId`] on commit.
const CLEANUP_PLACEHOLDER: u32 = 0;

/// Allowed per-axis pivot deviation (pixels) before drift is reported.
const PIVOT_DRIFT_THRESHOLD: u32 = 2;

// ── Inputs ────────────────────────────────────────────────────────────────────

/// Inputs for [`CleanupVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CleanupInputs {
    /// Pixels of the active cel to clean up. Must be RGBA8
    /// (`bytes_per_pixel == 4`).
    pub pixels: PixelData,

    /// Alpha threshold: pixels at or below this value are treated as
    /// fully transparent and zeroed out. Range `0`–`254`. `0` means
    /// only alpha == 0 pixels are transparent; `128` would also remove
    /// anything half-transparent or below.
    #[serde(default)]
    pub alpha_threshold: u8,

    /// Run the anti-aliasing removal pass in addition to palette snap.
    /// Semi-transparent pixels touching opaque content are snapped to
    /// the nearest palette colour at full opacity; isolated
    /// semi-transparent pixels are made fully transparent. Defaults to
    /// `true`.
    #[serde(default = "defaults::fix_antialiasing")]
    pub fix_antialiasing: bool,

    /// Detect pivot drift across animation frames. Findings are
    /// surfaced as notes — no edits are committed. Defaults to `true`.
    #[serde(default = "defaults::fix_pivot_drift")]
    pub fix_pivot_drift: bool,
}

mod defaults {
    pub fn fix_antialiasing() -> bool {
        true
    }
    pub fn fix_pivot_drift() -> bool {
        true
    }
}

// ── Verb ─────────────────────────────────────────────────────────────────────

/// Verb: snap a cel to the project palette, remove anti-aliasing, and
/// detect pivot drift.
///
/// Runs entirely on the local CPU (`required_capabilities` is empty).
/// The VLM ambiguity path mentioned in the brief is deferred to a
/// follow-up stream once the backend adapters (S22) are online.
#[derive(Debug)]
pub struct CleanupVerb {
    descriptor: VerbDescriptor,
}

impl CleanupVerb {
    /// Constructs the cleanup verb.
    #[must_use]
    // `serde_json::json!` calls `unwrap` internally on infallible
    // builders. The workspace forbids `unwrap` everywhere, so we allow
    // it here rather than open-coding the schema as a `Value::Object`.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pixels": {
                    "type": "object",
                    "properties": {
                        "width":  {"type": "integer", "minimum": 1},
                        "height": {"type": "integer", "minimum": 1},
                        "bytes_per_pixel": {"type": "integer", "enum": [4]},
                        "stride": {"type": "integer"},
                        "bytes":  {"type": "array", "items": {"type": "integer"}}
                    },
                    "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                },
                "alpha_threshold":  {"type": "integer", "minimum": 0, "maximum": 254, "default": 0},
                "fix_antialiasing": {"type": "boolean", "default": true},
                "fix_pivot_drift":  {"type": "boolean", "default": true}
            },
            "required": ["pixels"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(CLEANUP_VERB_ID),
                display_name: "Cleanup".into(),
                description: "Snap a cel to the project palette, remove anti-aliasing, and detect pivot drift".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::empty(),
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::ReplaceCels],
                cost_estimate: CostEstimate::free(),
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/cleanup.md".into()),
            },
        }
    }
}

impl Default for CleanupVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for CleanupVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: CleanupInputs = inputs.deserialize()?;
        if !parsed.pixels.is_well_formed() {
            return Err(VerbError::Schema(
                "cleanup: pixel buffer dimensions and byte count are inconsistent".into(),
            ));
        }
        if parsed.pixels.bytes_per_pixel != 4 {
            return Err(VerbError::Schema(format!(
                "cleanup: only RGBA8 (4 bytes per pixel) is supported; got {}",
                parsed.pixels.bytes_per_pixel,
            )));
        }
        // alpha_threshold == 255 would zero every pixel — that's a
        // no-op-shaped bug, not a useful cleanup setting. The JSON
        // schema already advertises `maximum: 254` to clients; mirror
        // that in the runtime check so direct callers (Lua, Rust) get
        // the same error rather than a silently-blank buffer.
        if parsed.alpha_threshold == u8::MAX {
            return Err(VerbError::Schema(
                "cleanup: alpha_threshold must be in the range 0..=254 \
                 (255 would zero every pixel)"
                    .into(),
            ));
        }
        Ok(())
    }

    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = std::time::Instant::now();
        let inputs: CleanupInputs = inputs.deserialize_owned()?;

        let sprite_id = ctx.require_sprite_id()?;
        let layer_id = ctx.require_active_layer()?;
        let frame_idx = ctx.active_frame.unwrap_or(FrameIndex::new(0));
        let palette = ctx
            .active_palette
            .ok_or(VerbError::MissingContext("active palette"))?;

        progress
            .send(VerbProgressEvent::Started { backend: None })
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress.step(Some(0.1), "snapping to palette").await;

        // CPU-bound: palette snap + optional AA removal.
        let src = inputs.pixels;
        let alpha_threshold = inputs.alpha_threshold;
        let fix_aa = inputs.fix_antialiasing;
        let palette_snap = palette.clone();
        let cleaned = tokio::task::spawn_blocking(move || {
            snap_pixels(&src, &palette_snap, alpha_threshold, fix_aa)
        })
        .await
        .map_err(|e| VerbError::Backend(format!("cleanup worker panicked: {e}")))?;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress.step(Some(0.85), "pixels cleaned").await;

        // Structural pass: detect pivot drift. No pixel ops; no commit effect.
        let mut notes: Vec<String> = Vec::new();
        if inputs.fix_pivot_drift {
            if let Some(sprite) = &ctx.sprite {
                if let Some(note) = detect_pivot_drift(sprite) {
                    notes.push(note);
                }
            }
        }

        progress.step(Some(1.0), "cleanup complete").await;

        let buffer_id = PixelBufferId::new(CLEANUP_PLACEHOLDER);
        let cel = Cel::raster(
            layer_id,
            frame_idx,
            buffer_id,
            Size::new(cleaned.width, cleaned.height),
        );
        let pixel_buffer = NewPixelBuffer {
            placeholder: buffer_id,
            pixels: cleaned.clone(),
        };

        let elapsed = started.elapsed();
        let summary = format!(
            "Cleanup {}×{} — palette snap{}{}",
            cleaned.width,
            cleaned.height,
            if fix_aa { ", AA removal" } else { "" },
            if notes.is_empty() {
                ""
            } else {
                ", pivot drift detected"
            },
        );

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::ReplaceCels {
                sprite: sprite_id,
                cels: vec![cel],
                pixel_buffers: vec![pixel_buffer],
            }],
            thumbnail: Some(cleaned),
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes,
        })
    }
}

// ── Pixel operations ──────────────────────────────────────────────────────────

/// Snaps every non-transparent pixel in `src` to the nearest palette
/// color, optionally removing anti-aliased fringe pixels.
///
/// Input: RGBA8 with arbitrary stride. Output: tight-packed RGBA8
/// (stride == width × 4). When the palette is empty every pixel is
/// passed through unchanged.
fn snap_pixels(src: &PixelData, palette: &Palette, alpha_threshold: u8, fix_aa: bool) -> PixelData {
    let width = src.width as usize;
    let height = src.height as usize;
    let stride = src.stride as usize;
    // Zeroed buffer: fully transparent by default.
    let mut out = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let in_off = y * stride + x * 4;
            let out_off = (y * width + x) * 4;

            let r = src.bytes[in_off];
            let g = src.bytes[in_off + 1];
            let b = src.bytes[in_off + 2];
            let a = src.bytes[in_off + 3];

            if a <= alpha_threshold {
                // Transparent — already zeroed.
                continue;
            }

            if fix_aa && a < 255 {
                // Semi-transparent pixel: potential AA artifact. Use a
                // fully-opaque neighbour test (alpha > 254 i.e. == 255)
                // rather than the user's `alpha_threshold` — the
                // threshold controls the transparency-zeroing pass; for
                // AA fringe detection we only want to snap pixels that
                // touch an actually-solid neighbour. Without this, a
                // chain of semi-transparent pixels at alpha 128/129/...
                // would all be marked "adjacent to opaque" and the
                // gradient would collapse to flat colour.
                if has_opaque_neighbor(&src.bytes, x, y, width, height, stride, 254) {
                    // Adjacent to opaque content — snap to nearest palette
                    // colour at full opacity to absorb the fringe.
                    let snapped = nearest_palette_color(r, g, b, palette);
                    out[out_off] = snapped.r;
                    out[out_off + 1] = snapped.g;
                    out[out_off + 2] = snapped.b;
                    out[out_off + 3] = 255;
                }
                // Isolated semi-transparent pixel — stays transparent.
                continue;
            }

            // Fully opaque, or semi-transparent with AA removal off.
            let snapped = nearest_palette_color(r, g, b, palette);
            out[out_off] = snapped.r;
            out[out_off + 1] = snapped.g;
            out[out_off + 2] = snapped.b;
            out[out_off + 3] = a;
        }
    }

    PixelData::rgba8(src.width, src.height, out)
}

/// Returns the nearest palette color for the given RGB triplet.
/// Falls back to the original colour when the palette is empty.
fn nearest_palette_color(r: u8, g: u8, b: u8, palette: &Palette) -> Rgba {
    let target = Rgba::new(r, g, b, 255);
    palette
        .nearest_index(target)
        .and_then(|idx| palette.color_at(idx))
        .unwrap_or(Rgba::opaque(r, g, b))
}

/// Returns `true` if any 4-connected neighbor has alpha above
/// `threshold` (i.e. is considered opaque).
fn has_opaque_neighbor(
    bytes: &[u8],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    stride: usize,
    threshold: u8,
) -> bool {
    // Use checked arithmetic to avoid signed/unsigned casts on pixel indices.
    let alpha = |nx: usize, ny: usize| bytes[ny * stride + nx * 4 + 3];

    (x > 0 && alpha(x - 1, y) > threshold)
        || (x + 1 < width && alpha(x + 1, y) > threshold)
        || (y > 0 && alpha(x, y - 1) > threshold)
        || (y + 1 < height && alpha(x, y + 1) > threshold)
}

// ── Pivot drift detection ─────────────────────────────────────────────────────

/// Inspects each slice's per-frame pivot keys and returns a note when
/// any slice has a pivot that deviates from the base key by more than
/// [`PIVOT_DRIFT_THRESHOLD`] pixels on either axis.
fn detect_pivot_drift(sprite: &Sprite) -> Option<String> {
    let drifting: Vec<&str> = sprite
        .slices
        .iter()
        .filter(|s| slice_has_pivot_drift(s))
        .map(|s| s.name.as_str())
        .collect();

    if drifting.is_empty() {
        return None;
    }

    Some(format!(
        "Pivot drift detected in {count} slice{plural}: {names}. \
         Review pivot keys in the slice panel — the effect protocol \
         does not currently support slice key updates.",
        count = drifting.len(),
        plural = if drifting.len() == 1 { "" } else { "s" },
        names = drifting.join(", "),
    ))
}

/// Returns `true` if the slice's pivot keys deviate from the first
/// key's pivot by more than [`PIVOT_DRIFT_THRESHOLD`] pixels.
fn slice_has_pivot_drift(slice: &Slice) -> bool {
    let mut pivots = slice
        .keys
        .iter()
        .filter_map(|k| k.pivot.as_ref().map(|p| p.offset));

    let Some(base) = pivots.next() else {
        return false;
    };

    // Compute the deltas in i64 before taking the absolute value.
    // `p.x` and `base.x` are i32, so a difference between i32::MIN and
    // any positive value would overflow. Widening to i64 makes the
    // subtraction safe; the threshold fits in u32 so the comparison
    // stays meaningful.
    let threshold = i64::from(PIVOT_DRIFT_THRESHOLD);
    pivots.any(|p| {
        (i64::from(p.x) - i64::from(base.x)).abs() > threshold
            || (i64::from(p.y) - i64::from(base.y)).abs() > threshold
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{
        IVec2, LayerId, PaletteId, Pivot, ProjectMetadata, Rect, SliceId, SliceKey, SpriteId,
        UserData,
    };

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "cleanup-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn two_color_palette() -> Palette {
        Palette::from_colors(
            PaletteId::new(1),
            "two-color",
            vec![Rgba::opaque(0, 0, 0), Rgba::opaque(255, 255, 255)],
        )
    }

    fn ctx_with_palette(palette: Palette) -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_frame = Some(FrameIndex::new(0));
        ctx.active_layer = Some(LayerId::new(1));
        ctx.active_palette = Some(palette);
        ctx
    }

    // ── descriptor ──────────────────────────────────────────────────────────

    #[test]
    fn descriptor_advertises_no_capabilities() {
        let verb = CleanupVerb::new();
        assert!(verb.descriptor().required_capabilities.is_empty());
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
        assert_eq!(
            verb.descriptor().output_kinds,
            vec![EffectKind::ReplaceCels]
        );
    }

    // ── validate ────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_malformed_pixels() {
        let verb = CleanupVerb::new();
        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels: PixelData {
                width: 2,
                height: 2,
                bytes_per_pixel: 4,
                stride: 8,
                bytes: vec![0; 4], // wrong: too few bytes
            },
            alpha_threshold: 0,
            fix_antialiasing: true,
            fix_pivot_drift: true,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_non_rgba() {
        let verb = CleanupVerb::new();
        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels: PixelData {
                width: 1,
                height: 1,
                bytes_per_pixel: 1, // indexed, not supported
                stride: 1,
                bytes: vec![0],
            },
            alpha_threshold: 0,
            fix_antialiasing: false,
            fix_pivot_drift: false,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let verb = CleanupVerb::new();
        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels: PixelData::rgba8(2, 2, vec![0; 16]),
            alpha_threshold: 0,
            fix_antialiasing: true,
            fix_pivot_drift: true,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    #[test]
    fn validate_rejects_alpha_threshold_255() {
        // 255 would zero every pixel — reject as a no-op-shaped bug.
        let verb = CleanupVerb::new();
        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels: PixelData::rgba8(2, 2, vec![0; 16]),
            alpha_threshold: 255,
            fix_antialiasing: false,
            fix_pivot_drift: false,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_alpha_threshold_254() {
        // 254 is the documented upper bound — must still pass.
        let verb = CleanupVerb::new();
        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels: PixelData::rgba8(2, 2, vec![0; 16]),
            alpha_threshold: 254,
            fix_antialiasing: false,
            fix_pivot_drift: false,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    // ── palette snap ────────────────────────────────────────────────────────

    #[test]
    fn opaque_pixel_snaps_to_nearest_palette_color() {
        let palette = two_color_palette(); // black + white
        // Dark gray is closer to black (dist² to black=7500, to white=120000)
        let src = PixelData::rgba8(1, 1, vec![50, 50, 50, 255]);
        let result = snap_pixels(&src, &palette, 0, false);
        assert_eq!(result.bytes[0], 0, "r should be black");
        assert_eq!(result.bytes[3], 255, "alpha should be preserved");
    }

    #[test]
    fn transparent_pixel_becomes_zero() {
        let palette = two_color_palette();
        let src = PixelData::rgba8(1, 1, vec![255, 255, 255, 0]);
        let result = snap_pixels(&src, &palette, 0, false);
        assert_eq!(&result.bytes[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn alpha_threshold_treats_low_alpha_as_transparent() {
        let palette = two_color_palette();
        // alpha=10, threshold=15 → treated as transparent
        let src = PixelData::rgba8(1, 1, vec![255, 255, 255, 10]);
        let result = snap_pixels(&src, &palette, 15, false);
        assert_eq!(result.bytes[3], 0);
    }

    #[test]
    fn output_is_tight_packed_regardless_of_input_stride() {
        let palette = two_color_palette();
        // 2-wide image with stride=12 (one pixel of padding per row)
        let src = PixelData {
            width: 2,
            height: 1,
            bytes_per_pixel: 4,
            stride: 12,
            bytes: vec![
                255, 255, 255, 255, // pixel (0,0): white
                0, 0, 0, 255, // pixel (1,0): black
                0, 0, 0, 0, // padding
            ],
        };
        let result = snap_pixels(&src, &palette, 0, false);
        assert_eq!(result.stride, 8, "output stride should be tight-packed");
        assert_eq!(result.bytes.len(), 8);
        assert_eq!(result.bytes[0], 255, "pixel 0: white r");
        assert_eq!(result.bytes[4], 0, "pixel 1: black r");
    }

    // ── AA removal ──────────────────────────────────────────────────────────

    #[test]
    fn aa_fringe_adjacent_to_opaque_snaps_to_full_opacity() {
        let palette = two_color_palette();
        // 3×1: [opaque white] [semi-transparent near-white] [transparent]
        // The middle pixel has an opaque left neighbor → snap to white at a=255.
        let src = PixelData::rgba8(
            3,
            1,
            vec![
                255, 255, 255, 255, // (0,0): opaque white
                200, 200, 200, 128, // (1,0): AA fringe
                0, 0, 0, 0, // (2,0): transparent
            ],
        );
        let result = snap_pixels(&src, &palette, 0, true);
        // Middle pixel: near-white snapped to white at a=255.
        assert_eq!(result.bytes[4], 255, "r should be 255 (white)");
        assert_eq!(result.bytes[7], 255, "alpha should be 255");
    }

    #[test]
    fn isolated_semi_transparent_pixel_becomes_transparent() {
        let palette = two_color_palette();
        // Isolated semi-transparent pixel with no opaque neighbors.
        let src = PixelData::rgba8(
            3,
            1,
            vec![
                0, 0, 0, 0, // (0,0): transparent
                128, 128, 128, 128, // (1,0): isolated semi-transparent
                0, 0, 0, 0, // (2,0): transparent
            ],
        );
        let result = snap_pixels(&src, &palette, 0, true);
        assert_eq!(
            result.bytes[7], 0,
            "isolated semi-transparent should be transparent"
        );
    }

    #[test]
    fn aa_removal_off_preserves_semi_transparent_alpha() {
        let palette = two_color_palette();
        // Without AA removal, semi-transparent pixels keep their alpha.
        let src = PixelData::rgba8(1, 1, vec![200, 200, 200, 128]);
        let result = snap_pixels(&src, &palette, 0, false);
        assert_eq!(
            result.bytes[3], 128,
            "alpha should be preserved when fix_aa=false"
        );
        assert_eq!(result.bytes[0], 255, "rgb snapped to white");
    }

    // ── pivot drift ─────────────────────────────────────────────────────────

    fn make_slice_with_pivots(pivots: &[(i32, i32)]) -> Slice {
        let keys: Vec<SliceKey> = pivots
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| SliceKey {
                frame: FrameIndex::new(u32::try_from(i).unwrap()),
                bounds: Rect::from_xywh(0, 0, 16, 16),
                nine_slice: None,
                pivot: Some(Pivot {
                    offset: IVec2::new(x, y),
                }),
            })
            .collect();
        Slice {
            id: SliceId::new(1),
            name: "test_slice".into(),
            keys,
            user_data: UserData::default(),
        }
    }

    #[test]
    fn stable_pivots_are_not_drifting() {
        let slice = make_slice_with_pivots(&[(8, 8), (8, 8), (8, 8)]);
        assert!(!slice_has_pivot_drift(&slice));
    }

    #[test]
    fn small_pivot_deviation_within_threshold_is_not_drifting() {
        // drift of 1 pixel on y — within the 2-pixel threshold
        let slice = make_slice_with_pivots(&[(8, 8), (8, 9)]);
        assert!(!slice_has_pivot_drift(&slice));
    }

    #[test]
    fn pivot_deviation_exceeding_threshold_is_drifting() {
        // drift of 7 pixels on y — exceeds the 2-pixel threshold
        let slice = make_slice_with_pivots(&[(8, 8), (8, 15)]);
        assert!(slice_has_pivot_drift(&slice));
    }

    #[test]
    fn slice_with_no_pivots_is_not_drifting() {
        let slice = Slice {
            id: SliceId::new(1),
            name: "no_pivot".into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::from_xywh(0, 0, 16, 16),
                nine_slice: None,
                pivot: None,
            }],
            user_data: UserData::default(),
        };
        assert!(!slice_has_pivot_drift(&slice));
    }

    // ── runtime round-trip ───────────────────────────────────────────────────

    #[tokio::test]
    async fn cleanup_round_trips_through_runtime() {
        let runtime = VerbRuntime::new();
        runtime.register(CleanupVerb::new()).unwrap();

        let pixels = PixelData::rgba8(
            2,
            2,
            vec![
                100, 0, 0, 255, 200, 0, 0, 255, // row 0
                0, 0, 0, 0, 50, 50, 50, 255, // row 1
            ],
        );
        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels,
            alpha_threshold: 0,
            fix_antialiasing: false,
            fix_pivot_drift: false,
        })
        .unwrap();

        let ctx = ctx_with_palette(two_color_palette());
        let inv = runtime
            .invoke(&VerbId::new(CLEANUP_VERB_ID), ctx, inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.output.effects.len(), 1);
        match &preview.output.effects[0] {
            VerbEffect::ReplaceCels {
                cels,
                pixel_buffers,
                ..
            } => {
                assert_eq!(cels.len(), 1);
                assert_eq!(pixel_buffers.len(), 1);
                // 2×2 tight-packed = 16 bytes
                assert_eq!(pixel_buffers[0].pixels.bytes.len(), 16);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cleanup_requires_active_layer() {
        let runtime = VerbRuntime::new();
        runtime.register(CleanupVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels: PixelData::rgba8(1, 1, vec![255, 0, 0, 255]),
            alpha_threshold: 0,
            fix_antialiasing: false,
            fix_pivot_drift: false,
        })
        .unwrap();

        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_palette = Some(two_color_palette());
        // active_layer intentionally absent

        let inv = runtime
            .invoke(&VerbId::new(CLEANUP_VERB_ID), ctx, inputs)
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active layer"))
        ));
    }

    #[tokio::test]
    async fn cleanup_requires_active_palette() {
        let runtime = VerbRuntime::new();
        runtime.register(CleanupVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&CleanupInputs {
            pixels: PixelData::rgba8(1, 1, vec![255, 0, 0, 255]),
            alpha_threshold: 0,
            fix_antialiasing: false,
            fix_pivot_drift: false,
        })
        .unwrap();

        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_layer = Some(LayerId::new(1));
        // active_palette intentionally absent

        let inv = runtime
            .invoke(&VerbId::new(CLEANUP_VERB_ID), ctx, inputs)
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active palette"))
        ));
    }
}
