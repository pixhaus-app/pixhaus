//! Verb: Auto-mesh deformation (S33).
//!
//! Overlays a parameterised grid mesh on a sprite layer, creates a
//! control point at the centre of each active cell, and returns two
//! effects: a mesh-visualisation layer and a rig-data payload.
//!
//! # Scope
//!
//! This iteration uses classical grid-based segmentation — no inference
//! backend is required. A follow-up stream will add a backend-driven
//! path (requiring [`BackendCapabilities::SEGMENTATION`]) that produces
//! semantically-aware regions (head, torso, limbs). The protocol surface
//! and output format are stable; the segmentation algorithm is the only
//! thing that changes.
//!
//! See `docs/verbs/auto-mesh-deformation.md` for user-facing reference.

use std::time::Duration;

use async_trait::async_trait;
use pixhaus_core::project::{Cel, FrameIndex, Layer, LayerId, PixelBufferId, Size};
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

// ── Public constants ──────────────────────────────────────────────────────────

/// Stable identifier for this verb.
pub const AUTO_MESH_DEFORMATION_VERB_ID: &str = "pixhaus.builtin.auto-mesh-deformation";

/// Minimum accepted mesh resolution (grid cells per dimension).
pub const MESH_RESOLUTION_MIN: u8 = 2;

/// Maximum accepted mesh resolution (grid cells per dimension).
pub const MESH_RESOLUTION_MAX: u8 = 16;

/// Default mesh resolution when the caller does not specify one.
pub const MESH_RESOLUTION_DEFAULT: u8 = 8;

// ── Private constants ─────────────────────────────────────────────────────────

/// Effect-local placeholder for the visualisation layer and its buffer.
/// The host rewrites both at commit time.
const PLACEHOLDER: u32 = 0;

/// Grid-line RGBA colour: semi-transparent medium blue.
const GRID_COLOR: [u8; 4] = [0, 100, 200, 150];

/// Control-point marker RGBA colour: opaque red.
const CP_COLOR: [u8; 4] = [220, 50, 50, 255];

/// Half-size of a control-point marker, in pixels.
const CP_RADIUS: u32 = 2;

// ── Input / output types ──────────────────────────────────────────────────────

/// Inputs for [`AutoMeshDeformationVerb`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AutoMeshInputs {
    /// Pixels of the sprite layer to rig.
    pub pixels: PixelData,
    /// Grid cells per dimension. Must be in
    /// [`MESH_RESOLUTION_MIN`]`..=`[`MESH_RESOLUTION_MAX`]. Defaults to
    /// [`MESH_RESOLUTION_DEFAULT`] when absent.
    #[serde(default)]
    pub mesh_resolution: Option<u8>,
    /// Display name for the mesh-visualisation layer. Defaults to
    /// `"Mesh rig"`.
    #[serde(default)]
    pub layer_name: Option<String>,
}

/// One control point in the deformation rig.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigControlPoint {
    /// Sequential ID within this rig.
    pub id: u32,
    /// Human-readable label (`"cp_<row>_<col>"`).
    pub label: String,
    /// X position in canvas pixels, at the centre of the grid cell.
    pub x: f64,
    /// Y position in canvas pixels.
    pub y: f64,
}

/// One active cell in the deformation mesh grid.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RigRegion {
    /// Sequential ID within this rig.
    pub id: u32,
    /// Row index (0-based, top-to-bottom).
    pub row: u32,
    /// Column index (0-based, left-to-right).
    pub col: u32,
    /// Left edge, inclusive, in canvas pixels.
    pub x0: u32,
    /// Top edge, inclusive, in canvas pixels.
    pub y0: u32,
    /// Right edge, exclusive, in canvas pixels.
    pub x1: u32,
    /// Bottom edge, exclusive, in canvas pixels.
    pub y1: u32,
    /// ID of the [`RigControlPoint`] that drives this cell.
    pub control_point_id: u32,
}

/// Complete deformation rig produced by [`AutoMeshDeformationVerb`].
///
/// Carried as the `payload` of a [`VerbEffect::Custom`] named
/// `"pixhaus.builtin.auto-mesh-deformation.rig"`. The host reads it to
/// build a parameter-slider panel for posing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigData {
    /// Width of the source sprite, in pixels.
    pub sprite_width: u32,
    /// Height of the source sprite, in pixels.
    pub sprite_height: u32,
    /// Grid cells per dimension used when building the rig.
    pub mesh_resolution: u8,
    /// Control points, one per active grid cell.
    pub control_points: Vec<RigControlPoint>,
    /// Active grid regions.
    pub regions: Vec<RigRegion>,
}

// ── Verb ──────────────────────────────────────────────────────────────────────

/// Auto-mesh deformation verb (S33).
///
/// Divides a sprite into a configurable grid, marks cells that contain
/// non-transparent pixels as active, places a control point at each
/// active cell's centre, and returns:
///
/// 1. A [`VerbEffect::AddLayer`] with a mesh-visualisation overlay
///    (grid lines + control-point markers).
/// 2. A [`VerbEffect::Custom`] with the [`RigData`] encoded as JSON.
///
/// The rig enables no-bones deformation: the host exposes the control
/// points as parameter sliders and warps the sprite pixels at render
/// time without the artist having to redraw frames.
#[derive(Debug)]
pub struct AutoMeshDeformationVerb {
    descriptor: VerbDescriptor,
}

impl AutoMeshDeformationVerb {
    /// Constructs a fresh verb instance.
    #[must_use]
    // `serde_json::json!` calls `Result::unwrap` on infallible internal
    // builders. The workspace disallows `unwrap` everywhere, so we exempt
    // this constructor rather than open-coding the schema as `Value::Object`.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pixels": {
                    "type": "object",
                    "properties": {
                        "width":           {"type": "integer", "minimum": 1},
                        "height":          {"type": "integer", "minimum": 1},
                        "bytes_per_pixel": {"type": "integer"},
                        "stride":          {"type": "integer"},
                        "bytes":           {"type": "array", "items": {"type": "integer"}}
                    },
                    "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                },
                "mesh_resolution": {
                    "type": ["integer", "null"],
                    "minimum": MESH_RESOLUTION_MIN,
                    "maximum": MESH_RESOLUTION_MAX
                },
                "layer_name": {"type": ["string", "null"]}
            },
            "required": ["pixels"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(AUTO_MESH_DEFORMATION_VERB_ID),
                display_name: "Auto-mesh deformation".into(),
                description: "Overlay a deformation mesh on a sprite, creating control points \
                               for no-bones posing without redrawing frames"
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                // Classical segmentation only in this iteration; SEGMENTATION
                // capability will be declared required when the backend path
                // lands in the follow-up stream.
                required_capabilities: BackendCapabilities::empty(),
                input_schema,
                output_schema: None,
                output_kinds: vec![
                    EffectKind::AddLayer,
                    EffectKind::Custom("pixhaus.builtin.auto-mesh-deformation.rig".into()),
                ],
                cost_estimate: CostEstimate::free(),
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/auto-mesh-deformation.md".into()),
            },
        }
    }
}

impl Default for AutoMeshDeformationVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for AutoMeshDeformationVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: AutoMeshInputs = inputs.deserialize()?;
        if !parsed.pixels.is_well_formed() {
            return Err(VerbError::Schema(
                "auto-mesh-deformation: pixel buffer is malformed".into(),
            ));
        }
        if let Some(res) = parsed.mesh_resolution {
            if !(MESH_RESOLUTION_MIN..=MESH_RESOLUTION_MAX).contains(&res) {
                return Err(VerbError::Schema(format!(
                    "auto-mesh-deformation: mesh_resolution must be {MESH_RESOLUTION_MIN}..={MESH_RESOLUTION_MAX}, got {res}"
                )));
            }
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
        // B10.3 anchor: this verb intentionally ignores `ctx.anchor`.
        // Pure CPU compute (grid segmentation + mesh build); no backend
        // invocation.
        let started = std::time::Instant::now();

        let inputs: AutoMeshInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;
        let active_frame = ctx.active_frame.unwrap_or(FrameIndex::new(0));
        let resolution = inputs.mesh_resolution.unwrap_or(MESH_RESOLUTION_DEFAULT);
        let layer_name = inputs.layer_name.unwrap_or_else(|| "Mesh rig".into());
        let source_pixels = inputs.pixels;

        progress
            .send(VerbProgressEvent::Started { backend: None })
            .await;
        progress.step(Some(0.2), "analyzing sprite").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Grid segmentation runs on a blocking thread — pixel iteration
        // is CPU-bound and must not block the tokio reactor.
        let rig = tokio::task::spawn_blocking(move || build_grid_rig(&source_pixels, resolution))
            .await
            .map_err(|e| VerbError::Aborted(e.to_string()))?;

        progress.step(Some(0.6), "building mesh").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let rig_for_viz = rig.clone();
        let viz_pixels = tokio::task::spawn_blocking(move || render_mesh_overlay(&rig_for_viz))
            .await
            .map_err(|e| VerbError::Aborted(e.to_string()))?;

        progress.step(Some(0.9), "preparing output").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let viz_layer = Layer::raster(LayerId::new(PLACEHOLDER), &layer_name);
        let viz_buffer_id = PixelBufferId::new(PLACEHOLDER);
        let viz_cel = Cel::raster(
            viz_layer.id,
            active_frame,
            viz_buffer_id,
            Size::new(viz_pixels.width, viz_pixels.height),
        );
        let viz_buffer = NewPixelBuffer {
            placeholder: viz_buffer_id,
            pixels: viz_pixels.clone(),
        };

        let rig_payload = serde_json::to_value(&rig)?;
        let active_cps = rig.control_points.len();

        progress.step(Some(1.0), "rig ready").await;

        let elapsed = started.elapsed();
        let summary = format!(
            "Add mesh rig \"{layer_name}\" ({active_cps} control point{}, {resolution}×{resolution} grid)",
            if active_cps == 1 { "" } else { "s" }
        );

        let notes = if active_cps == 0 {
            vec![
                "No active pixels found — the rig has no control points. \
                 Apply to a non-empty layer."
                    .into(),
            ]
        } else {
            vec![]
        };

        Ok(VerbOutput {
            summary,
            effects: vec![
                VerbEffect::AddLayer {
                    sprite: sprite_id,
                    layer: viz_layer,
                    cels: vec![viz_cel],
                    pixel_buffers: vec![viz_buffer],
                },
                VerbEffect::Custom {
                    name: "pixhaus.builtin.auto-mesh-deformation.rig".into(),
                    payload: rig_payload,
                },
            ],
            thumbnail: Some(viz_pixels),
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes,
        })
    }
}

// ── Grid segmentation ─────────────────────────────────────────────────────────

/// Builds a grid rig by dividing the sprite canvas into `resolution × resolution`
/// cells and creating a control point for each cell that contains at least one
/// non-transparent pixel.
fn build_grid_rig(pixels: &PixelData, resolution: u8) -> RigData {
    let w = pixels.width;
    let h = pixels.height;
    let res = u32::from(resolution);
    let bpp = u32::from(pixels.bytes_per_pixel);
    let stride = pixels.stride;

    // Ceiling division so every pixel falls in exactly one cell.
    let cell_w = w.div_ceil(res);
    let cell_h = h.div_ceil(res);

    let mut control_points: Vec<RigControlPoint> = Vec::new();
    let mut regions: Vec<RigRegion> = Vec::new();
    let mut cp_id: u32 = 0;
    let mut region_id: u32 = 0;

    for row in 0..res {
        for col in 0..res {
            let x0 = col * cell_w;
            let y0 = row * cell_h;

            // Skip cells that fall entirely outside the canvas.
            if x0 >= w || y0 >= h {
                continue;
            }

            let x1 = (x0 + cell_w).min(w);
            let y1 = (y0 + cell_h).min(h);

            if !cell_has_opaque_pixel(pixels, x0, y0, x1, y1, bpp, stride) {
                continue;
            }

            control_points.push(RigControlPoint {
                id: cp_id,
                label: format!("cp_{row}_{col}"),
                x: f64::from(x0 + x1) / 2.0,
                y: f64::from(y0 + y1) / 2.0,
            });
            regions.push(RigRegion {
                id: region_id,
                row,
                col,
                x0,
                y0,
                x1,
                y1,
                control_point_id: cp_id,
            });
            cp_id += 1;
            region_id += 1;
        }
    }

    RigData {
        sprite_width: w,
        sprite_height: h,
        mesh_resolution: resolution,
        control_points,
        regions,
    }
}

/// Returns `true` if any pixel in `[x0, x1) × [y0, y1)` is opaque.
///
/// For RGBA8 (`bpp == 4`) opacity is the alpha byte (offset + 3).
/// For other formats any non-zero byte is considered opaque, which
/// errs toward including cells rather than leaving gaps.
fn cell_has_opaque_pixel(
    pixels: &PixelData,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    bpp: u32,
    stride: u32,
) -> bool {
    for cy in y0..y1 {
        for cx in x0..x1 {
            let off = (cy * stride + cx * bpp) as usize;
            if bpp == 4 {
                // RGBA8: check alpha channel at off + 3.
                if off + 3 < pixels.bytes.len() && pixels.bytes[off + 3] > 0 {
                    return true;
                }
            } else {
                // Indexed / grayscale: any non-zero byte means content.
                let end = (off + bpp as usize).min(pixels.bytes.len());
                if off < end && pixels.bytes[off..end].iter().any(|&b| b != 0) {
                    return true;
                }
            }
        }
    }
    false
}

// ── Mesh visualisation ────────────────────────────────────────────────────────

/// Renders a semi-transparent mesh overlay: grid lines in
/// [`GRID_COLOR`] and control-point markers in [`CP_COLOR`].
/// Returns a tightly-packed RGBA8 [`PixelData`].
fn render_mesh_overlay(rig: &RigData) -> PixelData {
    let w = rig.sprite_width;
    let h = rig.sprite_height;
    let stride = w * 4;
    let mut bytes = vec![0u8; (stride * h) as usize];

    let res = u32::from(rig.mesh_resolution);
    let cell_w = w.div_ceil(res);
    let cell_h = h.div_ceil(res);

    // Vertical grid lines.
    for col in 0..=res {
        let x = (col * cell_w).min(w.saturating_sub(1));
        for y in 0..h {
            set_pixel(&mut bytes, stride, x, y, GRID_COLOR);
        }
    }

    // Horizontal grid lines.
    for row in 0..=res {
        let y = (row * cell_h).min(h.saturating_sub(1));
        for x in 0..w {
            set_pixel(&mut bytes, stride, x, y, GRID_COLOR);
        }
    }

    // Control-point markers — compute centre from region bounds to keep integer arithmetic.
    for region in &rig.regions {
        let cx = u32::midpoint(region.x0, region.x1);
        let cy = u32::midpoint(region.y0, region.y1);
        for dy in 0..=(2 * CP_RADIUS) {
            for dx in 0..=(2 * CP_RADIUS) {
                let px = cx.saturating_sub(CP_RADIUS) + dx;
                let py = cy.saturating_sub(CP_RADIUS) + dy;
                if px < w && py < h {
                    set_pixel(&mut bytes, stride, px, py, CP_COLOR);
                }
            }
        }
    }

    PixelData::rgba8(w, h, bytes)
}

/// Writes a single RGBA8 pixel at `(x, y)`.
#[inline]
fn set_pixel(bytes: &mut [u8], stride: u32, x: u32, y: u32, color: [u8; 4]) {
    let off = (y * stride + x * 4) as usize;
    // Four consecutive bytes; guard against malformed callers.
    if off + 3 < bytes.len() {
        bytes[off] = color[0];
        bytes[off + 1] = color[1];
        bytes[off + 2] = color[2];
        bytes[off + 3] = color[3];
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "mesh-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn ctx_with_sprite() -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_frame = Some(FrameIndex::new(0));
        ctx
    }

    /// 8×8 fully-opaque RGBA8 sprite.
    fn solid_8x8() -> PixelData {
        PixelData::rgba8(8, 8, vec![255u8; 8 * 8 * 4])
    }

    /// 8×8 fully-transparent RGBA8 sprite.
    fn empty_8x8() -> PixelData {
        PixelData::rgba8(8, 8, vec![0u8; 8 * 8 * 4])
    }

    // ── Descriptor ───────────────────────────────────────────────────────────

    #[test]
    fn descriptor_is_free_and_cancellable() {
        let verb = AutoMeshDeformationVerb::new();
        let d = verb.descriptor();
        assert!(d.required_capabilities.is_empty());
        assert!(d.cancellable);
        assert!(d.streaming);
        assert_eq!(d.cost_estimate.typical_usd_cents, 0.0);
        assert_eq!(d.cost_estimate.typical_latency, std::time::Duration::ZERO);
    }

    #[test]
    fn descriptor_declares_two_effect_kinds() {
        let verb = AutoMeshDeformationVerb::new();
        assert_eq!(verb.descriptor().output_kinds.len(), 2);
        assert!(matches!(
            verb.descriptor().output_kinds[0],
            EffectKind::AddLayer
        ));
        assert!(matches!(
            verb.descriptor().output_kinds[1],
            EffectKind::Custom(_)
        ));
    }

    // ── Validate ─────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_malformed_pixels() {
        let verb = AutoMeshDeformationVerb::new();
        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: PixelData {
                width: 8,
                height: 8,
                bytes_per_pixel: 4,
                stride: 32,
                bytes: vec![0; 8], // should be 32 * 8 = 256
            },
            mesh_resolution: None,
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_out_of_range_resolution() {
        let verb = AutoMeshDeformationVerb::new();
        for bad in [0u8, 1, 17, 255] {
            let inputs = VerbInputs::from_struct(&AutoMeshInputs {
                pixels: solid_8x8(),
                mesh_resolution: Some(bad),
                layer_name: None,
            })
            .unwrap();
            assert!(
                verb.validate(&inputs).is_err(),
                "expected error for resolution {bad}"
            );
        }
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let verb = AutoMeshDeformationVerb::new();
        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: solid_8x8(),
            mesh_resolution: Some(4),
            layer_name: Some("rig".into()),
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    #[test]
    fn validate_accepts_absent_resolution() {
        let verb = AutoMeshDeformationVerb::new();
        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: solid_8x8(),
            mesh_resolution: None,
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    // ── Grid segmentation ─────────────────────────────────────────────────────

    #[test]
    fn solid_sprite_produces_expected_control_points() {
        // 8×8 sprite with 4×4 mesh → all 16 cells active.
        let rig = build_grid_rig(&solid_8x8(), 4);
        assert_eq!(rig.sprite_width, 8);
        assert_eq!(rig.sprite_height, 8);
        assert_eq!(rig.mesh_resolution, 4);
        assert_eq!(rig.control_points.len(), 16);
        assert_eq!(rig.regions.len(), 16);
    }

    #[test]
    fn empty_sprite_produces_no_control_points() {
        let rig = build_grid_rig(&empty_8x8(), 4);
        assert!(rig.control_points.is_empty());
        assert!(rig.regions.is_empty());
    }

    #[test]
    fn control_points_are_within_sprite_bounds() {
        let rig = build_grid_rig(&solid_8x8(), 4);
        for cp in &rig.control_points {
            assert!(cp.x >= 0.0 && cp.x <= 8.0, "cp.x={} out of bounds", cp.x);
            assert!(cp.y >= 0.0 && cp.y <= 8.0, "cp.y={} out of bounds", cp.y);
        }
    }

    #[test]
    fn partial_sprite_skips_empty_cells() {
        // Top-left 4×4 quadrant opaque, rest transparent.
        let mut bytes = vec![0u8; 8 * 8 * 4];
        for y in 0..4usize {
            for x in 0..4usize {
                let off = (y * 8 + x) * 4;
                bytes[off + 3] = 255; // alpha = opaque
            }
        }
        let pixels = PixelData::rgba8(8, 8, bytes);
        // 2×2 grid → 4 cells, only 1 (top-left) is active.
        let rig = build_grid_rig(&pixels, 2);
        assert_eq!(rig.control_points.len(), 1);
        assert_eq!(rig.regions[0].row, 0);
        assert_eq!(rig.regions[0].col, 0);
    }

    #[test]
    fn region_ids_are_sequential() {
        let rig = build_grid_rig(&solid_8x8(), 4);
        for (i, r) in rig.regions.iter().enumerate() {
            assert_eq!(r.id, u32::try_from(i).unwrap());
        }
    }

    #[test]
    fn control_point_ids_are_sequential() {
        let rig = build_grid_rig(&solid_8x8(), 4);
        for (i, cp) in rig.control_points.iter().enumerate() {
            assert_eq!(cp.id, u32::try_from(i).unwrap());
        }
    }

    #[test]
    fn rig_data_round_trips_as_json() {
        let rig = build_grid_rig(&solid_8x8(), 2);
        let json = serde_json::to_string(&rig).unwrap();
        let back: RigData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rig);
    }

    // ── Visualisation ─────────────────────────────────────────────────────────

    #[test]
    fn overlay_matches_sprite_dimensions() {
        let rig = build_grid_rig(&solid_8x8(), 4);
        let overlay = render_mesh_overlay(&rig);
        assert_eq!(overlay.width, 8);
        assert_eq!(overlay.height, 8);
        assert!(overlay.is_well_formed());
    }

    #[test]
    fn overlay_is_well_formed_for_empty_rig() {
        let rig = build_grid_rig(&empty_8x8(), 4);
        let overlay = render_mesh_overlay(&rig);
        assert!(overlay.is_well_formed());
    }

    // ── Runtime integration ───────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_produces_two_effects() {
        let runtime = VerbRuntime::new();
        runtime.register(AutoMeshDeformationVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: solid_8x8(),
            mesh_resolution: Some(4),
            layer_name: Some("Rig".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUTO_MESH_DEFORMATION_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.output.effects.len(), 2);
    }

    #[tokio::test]
    async fn invoke_requires_active_sprite() {
        let runtime = VerbRuntime::new();
        runtime.register(AutoMeshDeformationVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: solid_8x8(),
            mesh_resolution: None,
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUTO_MESH_DEFORMATION_VERB_ID),
                VerbContext::empty(metadata()),
                inputs,
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active sprite"))
        ));
    }

    #[tokio::test]
    async fn invoke_with_empty_sprite_emits_note() {
        let runtime = VerbRuntime::new();
        runtime.register(AutoMeshDeformationVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: empty_8x8(),
            mesh_resolution: Some(4),
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUTO_MESH_DEFORMATION_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();
        assert!(!preview.output.notes.is_empty());
    }

    #[tokio::test]
    async fn invoke_uses_custom_layer_name() {
        let runtime = VerbRuntime::new();
        runtime.register(AutoMeshDeformationVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: solid_8x8(),
            mesh_resolution: Some(4),
            layer_name: Some("My rig".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUTO_MESH_DEFORMATION_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();
        match &preview.output.effects[0] {
            VerbEffect::AddLayer { layer, .. } => {
                assert_eq!(layer.name, "My rig");
            }
            other => panic!("expected AddLayer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_uses_default_layer_name_when_absent() {
        let runtime = VerbRuntime::new();
        runtime.register(AutoMeshDeformationVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: solid_8x8(),
            mesh_resolution: None,
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUTO_MESH_DEFORMATION_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();
        match &preview.output.effects[0] {
            VerbEffect::AddLayer { layer, .. } => {
                assert_eq!(layer.name, "Mesh rig");
            }
            other => panic!("expected AddLayer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_second_effect_is_custom_rig() {
        let runtime = VerbRuntime::new();
        runtime.register(AutoMeshDeformationVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&AutoMeshInputs {
            pixels: solid_8x8(),
            mesh_resolution: Some(2),
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUTO_MESH_DEFORMATION_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();
        match &preview.output.effects[1] {
            VerbEffect::Custom { name, payload } => {
                assert_eq!(name, "pixhaus.builtin.auto-mesh-deformation.rig");
                let rig: RigData = serde_json::from_value(payload.clone()).unwrap();
                assert_eq!(rig.mesh_resolution, 2);
                assert_eq!(rig.control_points.len(), 4); // 2×2 all active
            }
            other => panic!("expected Custom, got {other:?}"),
        }
    }
}
