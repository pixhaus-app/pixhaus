// SPRITE program — the only viewport program in the vertical slice.
//
// Draws a quad covering the sprite's canvas bounds (0,0)..(sprite_size).
// Ported from the WebGL2 SPRITE program in ui/src/canvas/renderer/shaders.ts:
// a procedural checkerboard represents the transparent canvas, and when a
// frame texture is present its RGBA is alpha-composited over the checker.
//
// The quad is generated from @builtin(vertex_index) so no vertex buffer is
// needed; positions are the unit-square corners scaled by sprite_size.

struct Uniforms {
    resolution: vec2<f32>,  // viewport size in physical pixels
    scroll: vec2<f32>,      // canvas coord mapped to the viewport centre
    sprite_size: vec2<f32>, // sprite dimensions in canvas pixels
    zoom: f32,              // physical pixels per canvas pixel
    has_tile: f32,          // 1.0 when the frame texture holds data, else 0.0
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tile_tex: texture_2d<f32>;
@group(0) @binding(2) var tile_samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) canvas: vec2<f32>, // canvas-space coordinate, interpolated
};

// Unit-square corners as two triangles.
const CORNERS = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let canvas = CORNERS[vi] * u.sprite_size;
    let screen = (canvas - u.scroll) * u.zoom + u.resolution * 0.5;
    let clip = (screen / u.resolution) * 2.0 - 1.0;
    var o: VsOut;
    o.clip = vec4<f32>(clip.x, -clip.y, 0.0, 1.0);
    o.canvas = canvas;
    return o;
}

// 8x8 canvas-pixel checker squares in two shades of grey.
fn checkerboard(canvas_pos: vec2<f32>) -> vec3<f32> {
    let cell = floor(canvas_pos / 8.0);
    let even = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) / 2.0);
    let v = select(0.58, 0.47, even < 0.5);
    return vec3<f32>(v, v, v);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let bg = checkerboard(in.canvas);
    if (u.has_tile > 0.5) {
        let uv = in.canvas / u.sprite_size;
        let tex = textureSample(tile_tex, tile_samp, uv);
        // Straight-alpha composite over the opaque checker background.
        return vec4<f32>(mix(bg, tex.rgb, tex.a), 1.0);
    }
    return vec4<f32>(bg, 1.0);
}
