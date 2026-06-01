// Fills the viewport with a flat canvas backdrop color.
//
// Scaffold-stage placeholder for the real compositing pipeline: it draws a single
// fullscreen triangle (positions derived from the vertex index, so no vertex
// buffer) and writes a constant color. egui-wgpu has already clipped the pass to
// the canvas rect, so the triangle only fills that region.

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    return vec4<f32>(positions[idx], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // Dark neutral backdrop, opaque, premultiplied alpha.
    return vec4<f32>(0.11, 0.11, 0.13, 1.0);
}
