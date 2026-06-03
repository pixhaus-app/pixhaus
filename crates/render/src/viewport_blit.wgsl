// Samples the canvas texture across the artboard with a nearest sampler.
//
// A fullscreen triangle (positions from the vertex index, no vertex buffer) covers the
// pass viewport. The fragment derives its texture coordinate from its own framebuffer
// position relative to the board's origin/size (the `view` uniform, in physical pixels),
// NOT from the viewport — egui-wgpu clamps the pass viewport to the window, so a board
// panned or zoomed past the edge would otherwise squash the whole texture into the
// clamped viewport. Mapping from the framebuffer position instead crops correctly. The
// fragment premultiplies the sampled colour to match the egui target's premultiplied-
// alpha blending, so transparent sprite areas let the checkerboard behind show through.

struct View {
    // Board top-left in physical pixels.
    origin: vec2<f32>,
    // Board size in physical pixels.
    size: vec2<f32>,
};

@group(0) @binding(0) var canvas_tex: texture_2d<f32>;
@group(0) @binding(1) var canvas_sampler: sampler;
@group(0) @binding(2) var<uniform> view: View;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // Framebuffer pixel (top-left origin, y-down) -> sprite UV. The texture's row 0 is
    // the sprite top and screen y is also down, so no flip is needed.
    let uv = (frag.xy - view.origin) / view.size;
    let c = textureSample(canvas_tex, canvas_sampler, uv);
    return vec4<f32>(c.rgb * c.a, c.a);
}
