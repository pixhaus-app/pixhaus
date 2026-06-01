# wgpu 29.0.1 — bind groups, layouts, shaders, render & compute pipelines

Verified against docs.rs/wgpu/29.0.1 and wgpu-types/29.0.1. The v29 shape differs from older
majors in ways your memory will get wrong: `entry_point` is `Option<&str>`, every pipeline
stage carries `compilation_options: PipelineCompilationOptions`, pipeline descriptors end
with `cache: Option<&PipelineCache>`, and `PipelineLayoutDescriptor` uses
`bind_group_layouts: &[Option<&BindGroupLayout>]` plus `immediate_size: u32` (no
`push_constant_ranges`).

## Bind group layout

```rust
pub struct BindGroupLayoutDescriptor<'a> { pub label: Label<'a>, pub entries: &'a [BindGroupLayoutEntry] }

pub struct BindGroupLayoutEntry {
    pub binding: u32,                   // matches @binding(N) in WGSL
    pub visibility: ShaderStages,       // ShaderStages::VERTEX | FRAGMENT | COMPUTE
    pub ty: BindingType,
    pub count: Option<NonZero<u32>>,    // Some(_) only for binding arrays
}

pub enum BindingType {
    Buffer { ty: BufferBindingType, has_dynamic_offset: bool, min_binding_size: Option<NonZero<u64>> },
    Sampler(SamplerBindingType),                                  // Filtering | NonFiltering | Comparison
    Texture { sample_type: TextureSampleType, view_dimension: TextureViewDimension, multisampled: bool },
    StorageTexture { access: StorageTextureAccess, format: TextureFormat, view_dimension: TextureViewDimension },
    // AccelerationStructure { .. }, ExternalTexture — not used by Pixhaus
}
```

- `BufferBindingType`: `Uniform`, `Storage { read_only: bool }`.
- `TextureSampleType`: `Float { filterable: bool }` (default `filterable: true`), `Depth`,
  `Sint`, `Uint`. A `Float { filterable: true }` texture must pair with a
  `SamplerBindingType::Filtering` sampler.
- `StorageTextureAccess`: `WriteOnly` (WGSL `write`), `ReadOnly`, `ReadWrite`, `Atomic`.
  `WriteOnly` is core/portable; `ReadOnly`/`ReadWrite` are native-only extensions needing
  `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`. Prefer `WriteOnly` (separate sampled input +
  storage output) unless you have a reason.

`device.create_bind_group_layout(&desc) -> BindGroupLayout`.

## Bind group

```rust
pub struct BindGroupDescriptor<'a> { pub label: Label<'a>, pub layout: &'a BindGroupLayout, pub entries: &'a [BindGroupEntry<'a>] }
pub struct BindGroupEntry<'a> { pub binding: u32, pub resource: BindingResource<'a> }

pub enum BindingResource<'a> {
    Buffer(BufferBinding<'a>),          // { buffer, offset, size }
    Sampler(&'a Sampler),
    TextureView(&'a TextureView),
    // + array variants
}
```

`buffer.as_entire_binding()` returns `BindingResource::Buffer(..)` over the whole buffer.
`device.create_bind_group(&desc) -> BindGroup`.

## Pipeline layout

```rust
pub struct PipelineLayoutDescriptor<'a> {
    pub label: Label<'a>,
    pub bind_group_layouts: &'a [Option<&'a BindGroupLayout>], // index i -> @group(i); write &[Some(&bgl)]
    pub immediate_size: u32,            // v29: replaces push_constant_ranges; 0 if unused
}
```

## Shader modules

```rust
pub struct ShaderModuleDescriptor<'a> { pub label: Label<'a>, pub source: ShaderSource<'a> }
pub enum ShaderSource<'a> { Wgsl(Cow<'a, str>), SpirV(..), Glsl { .. }, Naga(..) } // WGSL is the default feature

// device.create_shader_module(desc: ShaderModuleDescriptor) -> ShaderModule  (by value)
```

`include_wgsl!("canvas.wgsl")` expands to a `ShaderModuleDescriptor` (label = path, source =
`Wgsl(include_str!(..).into())`), path relative to the calling file. For runtime strings:
`ShaderSource::Wgsl(my_string.into())`. WGSL compiles through naga; a shader error surfaces
at `create_shader_module` (or under a validation error scope).

## Render pipeline

```rust
pub struct RenderPipelineDescriptor<'a> {
    pub label: Label<'a>,
    pub layout: Option<&'a PipelineLayout>,    // None = auto layout from reflection
    pub vertex: VertexState<'a>,
    pub primitive: PrimitiveState,
    pub depth_stencil: Option<DepthStencilState>,   // 2D canvas: None
    pub multisample: MultisampleState,
    pub fragment: Option<FragmentState<'a>>,
    pub multiview_mask: Option<NonZeroU32>,    // None for single-view (VERIFY exact name)
    pub cache: Option<&'a PipelineCache>,      // None
}

pub struct VertexState<'a> {
    pub module: &'a ShaderModule,
    pub entry_point: Option<&'a str>,          // v29: Option; None => the sole vertex entry
    pub compilation_options: PipelineCompilationOptions<'a>,  // ::default()
    pub buffers: &'a [VertexBufferLayout<'a>],
}
pub struct FragmentState<'a> {
    pub module: &'a ShaderModule,
    pub entry_point: Option<&'a str>,
    pub compilation_options: PipelineCompilationOptions<'a>,
    pub targets: &'a [Option<ColorTargetState>],  // None entry => target write-disabled
}
```

### Vertex layout

```rust
pub struct VertexBufferLayout<'a> { pub array_stride: u64, pub step_mode: VertexStepMode, pub attributes: &'a [VertexAttribute] }
pub struct VertexAttribute { pub format: VertexFormat, pub offset: u64, pub shader_location: u32 }
```

Use the macro and bind it to a named `const`/`let` so the borrow outlives the descriptor:

```rust
const ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];
```

### Color target & blending — verified constants

```rust
pub struct ColorTargetState { pub format: TextureFormat, pub blend: Option<BlendState>, pub write_mask: ColorWrites }
```

From wgpu-types 29.0.1:
- `BlendComponent::REPLACE` = `{ One, Zero, Add }`
- `BlendComponent::OVER` = `{ One, OneMinusSrcAlpha, Add }`
- `BlendState::REPLACE` (color+alpha REPLACE)
- `BlendState::ALPHA_BLENDING` (color `{ SrcAlpha, OneMinusSrcAlpha, Add }`, alpha OVER)
- `BlendState::PREMULTIPLIED_ALPHA_BLENDING` (color OVER, alpha OVER)

Use **`wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING`** when the canvas texture stores
premultiplied RGBA, which is the convention egui expects — it equals both components as
`{ src_factor: One, dst_factor: OneMinusSrcAlpha, operation: Add }`.

## Worked example — textured quad (uniform MVP + texture + sampler)

WGSL (`canvas.wgsl`):

```wgsl
struct Mvp { matrix: mat4x4<f32> };
@group(0) @binding(0) var<uniform> mvp: Mvp;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_main(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>) -> VsOut {
    var o: VsOut;
    o.clip = mvp.matrix * vec4<f32>(pos, 0.0, 1.0);
    o.uv = uv;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv); // premultiplied RGBA expected
}
```

Rust (against `target_format` supplied by egui-wgpu — see the pixhaus-egui-wgpu skill):

```rust
let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("canvas.bgl"),
    entries: &[
        wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false, min_binding_size: None }, count: None },
        wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
        wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
    ],
});

let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
    label: Some("canvas.layout"),
    bind_group_layouts: &[Some(&bgl)],   // note Option<&_>
    immediate_size: 0,
});

let shader = device.create_shader_module(wgpu::include_wgsl!("canvas.wgsl"));
const ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: Some("canvas.pipeline"),
    layout: Some(&layout),
    vertex: wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        buffers: &[wgpu::VertexBufferLayout {
            array_stride: 16, step_mode: wgpu::VertexStepMode::Vertex, attributes: &ATTRS,
        }],
    },
    primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList,
        cull_mode: None, ..Default::default() },
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        targets: &[Some(wgpu::ColorTargetState {
            format: target_format,                                   // MUST equal egui-wgpu's
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
    }),
    multiview_mask: None,
    cache: None,
});

let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("canvas.bind_group"),
    layout: &bgl,
    entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: mvp_buffer.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tex_view) },
        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
    ],
});
```

## Compute pipeline

```rust
pub struct ComputePipelineDescriptor<'a> {
    pub label: Label<'a>,
    pub layout: Option<&'a PipelineLayout>,
    pub module: &'a ShaderModule,
    pub entry_point: Option<&'a str>,          // None => the sole compute entry
    pub compilation_options: PipelineCompilationOptions<'a>,
    pub cache: Option<&'a PipelineCache>,
}
// device.create_compute_pipeline(&desc) -> ComputePipeline   (no stage struct)
```

WGSL (`invert.wgsl`) using a write-only storage texture (portable path):

```wgsl
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(dst);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; } // bounds guard for non-multiple sizes
    let c = textureLoad(src, vec2<i32>(gid.xy), 0);
    textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(1.0 - c.rgb, c.a));
}
```

```rust
let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("invert.bgl"),
    entries: &[
        wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
        wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly,
                format: wgpu::TextureFormat::Rgba8Unorm, view_dimension: wgpu::TextureViewDimension::D2 },
            count: None },
    ],
});
// ... create_pipeline_layout(&[Some(&bgl)], immediate_size: 0), create_shader_module, then:
let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
    label: Some("invert.pipeline"), layout: Some(&layout), module: &shader,
    entry_point: Some("main"), compilation_options: wgpu::PipelineCompilationOptions::default(),
    cache: None,
});
// Dispatch math lives in commands-and-passes.md.
```

## v29 quick checks

- `entry_point: Some("name")`, not a bare `&str`; `None` requires exactly one matching entry.
- `PipelineLayoutDescriptor` has `immediate_size: u32`, no `push_constant_ranges`;
  `bind_group_layouts` elements are `Option<&BindGroupLayout>` — write `&[Some(&bgl)]`.
- Every `VertexState`/`FragmentState`/`ComputePipelineDescriptor` needs
  `compilation_options: PipelineCompilationOptions::default()`; both pipeline descriptors end
  with `cache: None`.
- `targets` is `&[Option<ColorTargetState>]`; `format` must equal egui-wgpu's target format or
  pipeline creation fails.
- `vertex_attr_array!` returns an array value — bind it to a named `const`/`let`.
- `multiview_mask: Option<NonZeroU32>` — VERIFY exact field name against the 29.0.1 struct page.
