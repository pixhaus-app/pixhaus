// REVEAL program — the generation loading/reveal effect.
//
// An instanced particle field: one instance per grid cell, six verts each. A
// cell starts scattered across the aspect-fitted box and drifts to its cell
// centre as `assembly` rises (0 = scattered cloud, 1 = settled mosaic). Its
// colour crossfades from a placeholder tint to the sampled source image as
// `reveal` rises (0 = placeholder, 1 = source). The grid is fixed and
// independent of the output resolution, so an 8K request still animates the
// same instance count in one draw — the 8K performance constraint.

struct Uniforms {
    fit: vec2<f32>,        // half-extents (clip space) of the aspect-fitted box
    grid: vec2<f32>,       // grid columns, rows
    assembly: f32,         // 0 scattered .. 1 settled into cells
    reveal: f32,           // 0 placeholder .. 1 source image
    time: f32,             // seconds, for the scatter shimmer
    has_source: f32,       // 1.0 when the source texture holds a real image
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var src_tex: texture_2d<f32>;
@group(0) @binding(2) var src_smp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) cell_uv: vec2<f32>,
    @location(1) shade: f32,
};

// Dave Hoskins integer hashes (https://www.shadertoy.com/view/4djSRW): a stable
// pseudo-random scatter start and per-cell brightness from the instance index,
// so no per-instance buffer is needed.
fn hash11(p: f32) -> f32 {
    var x = fract(p * 0.1031);
    x = x * (x + 33.33);
    x = x * (x + x);
    return fract(x);
}

fn hash21(p: f32) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p, p, p) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

fn smooth01(x: f32) -> f32 {
    let t = clamp(x, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Unit-square corners as two triangles, scaled per instance into a cell quad.
const QUAD = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let gw = max(u.grid.x, 1.0);
    let gh = max(u.grid.y, 1.0);
    let col = f32(ii % u32(gw));
    let row = f32(ii / u32(gw));
    let cell_uv = vec2<f32>((col + 0.5) / gw, (row + 0.5) / gh);

    // The cell's home: its centre in the aspect-fitted box (clip y is up, the
    // image's v axis is down, so v is flipped here). `target` is a WGSL reserved
    // word, hence `home`.
    let home = vec2<f32>((cell_uv.x * 2.0 - 1.0) * u.fit.x, (1.0 - cell_uv.y * 2.0) * u.fit.y);

    // A stable scattered start spread across the whole fitted box.
    let r = hash21(f32(ii) * 1.7 + 3.1);
    let scattered = vec2<f32>((r.x * 2.0 - 1.0) * u.fit.x, (r.y * 2.0 - 1.0) * u.fit.y);

    let a = smooth01(u.assembly);
    var center = mix(scattered, home, a);

    // Per-instance drift and a slow vertical shimmer, both fading as cells settle.
    let j = hash21(f32(ii) * 0.37 + 1.0) - vec2<f32>(0.5, 0.5);
    let shimmer = sin(u.time * 1.7 + f32(ii)) * 0.012;
    center = center + j * (1.0 - a) * 0.5 + vec2<f32>(0.0, shimmer * (1.0 - a));

    // Half-size of one cell in clip space; particles grow to exactly tile their
    // cell at a = 1, so the settled field reads as a solid mosaic.
    let cell_half = vec2<f32>(u.fit.x / gw, u.fit.y / gh);
    let size = cell_half * mix(0.4, 1.0, a);

    var out: VsOut;
    out.clip = vec4<f32>(center + QUAD[vi] * size, 0.0, 1.0);
    out.cell_uv = cell_uv;
    out.shade = 0.78 + 0.22 * hash11(f32(ii) * 2.13);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let placeholder = vec3<f32>(0.26, 0.29, 0.55) * in.shade;
    var rgb = placeholder;
    var alpha = 1.0;
    if (u.has_source > 0.5) {
        let s = textureSampleLevel(src_tex, src_smp, in.cell_uv, 0.0);
        let r = clamp(u.reveal, 0.0, 1.0);
        rgb = mix(placeholder, s.rgb, r);
        // Honour the source alpha as it reveals, so a keyed/transparent source
        // doesn't paint an opaque slab.
        alpha = mix(1.0, s.a, r);
    }
    return vec4<f32>(rgb, alpha);
}
