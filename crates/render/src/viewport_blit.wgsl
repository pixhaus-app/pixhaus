// The canvas: a procedural transparency checkerboard and pixel grid with the sprite
// composited over them, all in one fragment pass.
//
// A fullscreen triangle covers the pass viewport; each fragment maps its framebuffer
// position back to the board through origin/size (the same mapping the plain blit used,
// so it crops correctly under egui-wgpu's window-clamped viewport). It then tints a
// screen-aligned checker, composites the sprite over it with straight alpha when
// `has_tile`, and overlays the minor (per-sprite-pixel) and major grid lines. The cost is
// O(visible pixels) and zoom-independent — this replaces the per-cell CPU rect loop that
// stalled at high zoom (one rect per 8px cell across the whole artboard).
//
// Colors arrive LINEAR; the target is an sRGB framebuffer that re-encodes on write, so the
// GPU checker matches the value egui's CPU shapes produced. Output is OPAQUE (alpha = 1):
// the board fully owns its rect, drawn with a REPLACE blend.

struct View {
    origin: vec2<f32>,      // board top-left, physical px
    size: vec2<f32>,        // board size, physical px
    checker_lo: vec4<f32>,  // light checker cell, linear rgba
    checker_hi: vec4<f32>,  // dark checker cell, linear rgba
    minor_color: vec4<f32>, // per-sprite-pixel grid line, linear rgb + strength in a
    major_color: vec4<f32>, // major grid line, linear rgb + strength in a
    params: vec4<f32>,      // x = checker cell px, y = px-per-sprite-pixel, z = major period (sprite px), w = flags
};

@group(0) @binding(0) var canvas_tex: texture_2d<f32>;
@group(0) @binding(1) var canvas_sampler: sampler;
@group(0) @binding(2) var<uniform> view: View;

const FLAG_HAS_TILE: u32 = 1u;
const FLAG_MINOR: u32 = 2u;
const FLAG_MAJOR: u32 = 4u;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

// Line coverage 0..1 for `coord` (physical px from the board origin) against lines spaced
// every `period_px`: 1 on a line, fading to 0 about a pixel away. The falloff has a >=1px
// footprint on purpose — a hard half-pixel threshold drops a line out entirely when it
// lands exactly between two pixel centers (both at distance 0.5), which made the grid
// flicker away at certain zoom levels. Returns 0 when the lines are too dense to read.
fn grid_coverage(coord: f32, period_px: f32) -> f32 {
    if (period_px < 2.0) {
        return 0.0;
    }
    let q = coord / period_px;
    let dist = abs(q - round(q)) * period_px;
    return clamp(1.0 - dist, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let flags = u32(view.params.w);
    let cell_px = view.params.x;
    let ppsp = view.params.y;
    let major_px = view.params.z * ppsp;

    // Board-local position in physical px (frag is the pixel center).
    let local = frag.xy - view.origin;

    // 1. Screen-aligned checkerboard (fixed 8-screen-point cells, like the old painter).
    let cell = floor(local / cell_px);
    let parity = cell.x + cell.y - 2.0 * floor((cell.x + cell.y) * 0.5);
    var rgb = select(view.checker_hi.rgb, view.checker_lo.rgb, parity < 0.5);

    // 2. Sprite over the checker, straight alpha.
    if ((flags & FLAG_HAS_TILE) != 0u) {
        let uv = local / view.size;
        let tex = textureSample(canvas_tex, canvas_sampler, uv);
        rgb = mix(rgb, tex.rgb, tex.a);
    }

    // 3. Per-sprite-pixel grid (gated by the UI on zoom).
    if ((flags & FLAG_MINOR) != 0u) {
        let cov = max(grid_coverage(local.x, ppsp), grid_coverage(local.y, ppsp));
        rgb = mix(rgb, view.minor_color.rgb, view.minor_color.a * cov);
    }

    // 4. Major grid every N sprite pixels.
    if ((flags & FLAG_MAJOR) != 0u) {
        let cov = max(grid_coverage(local.x, major_px), grid_coverage(local.y, major_px));
        rgb = mix(rgb, view.major_color.rgb, view.major_color.a * cov);
    }

    return vec4<f32>(rgb, 1.0);
}
