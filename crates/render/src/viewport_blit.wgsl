// Samples the canvas texture across the viewport with a nearest sampler.
//
// A fullscreen triangle (positions from the vertex index, no vertex buffer) covers
// the callback rect; egui-wgpu has set the pass viewport to that rect, so clip space
// maps 1:1 to the artboard and the UV runs 0..1 across the sprite. The fragment
// premultiplies the sampled colour to match the egui target's premultiplied-alpha
// blending, so transparent sprite areas let the checkerboard behind show through.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var canvas_tex: texture_2d<f32>;
@group(0) @binding(1) var canvas_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[index];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    // Clip [-1,1] -> UV [0,1], flipping Y for the texture's top-left origin.
    out.uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(canvas_tex, canvas_sampler, in.uv);
    return vec4<f32>(c.rgb * c.a, c.a);
}
