# wgpu 29.0.1 — command encoding, passes, draw/dispatch, submission

Signatures copied from docs.rs/wgpu/29.0.1.

## Lifetime model (v29) — the important part

`RenderPass` and `ComputePass` carry a single `'encoder` lifetime tied to the parent
`CommandEncoder`, not `'static`:

```rust
pub struct RenderPass<'encoder> { /* private */ }
pub struct ComputePass<'encoder> { /* private */ }
```

The resources you bind (pipeline, bind group, buffers) are decoupled from that lifetime —
passed by `&`/slice handle and ref-counted internally, so they do not have to outlive the
pass borrow the way the pre-0.19 borrow-based API required. `set_bind_group` even takes the
group via a generic `BG where Option<&BindGroup>: From<BG>`, so both `&bind_group` and
`Some(&bind_group)` work. This is what lets egui-wgpu hand the render crate a longer-lived
`&mut RenderPass` and have it issue draws against short-lived borrows.

egui-wgpu's `CallbackTrait::paint` gives you `&mut wgpu::RenderPass<'static>` — that is just
egui's chosen concrete lifetime; your draw calls are agnostic to it. See the
pixhaus-egui-wgpu skill for that integration layer.

## CommandEncoder

```rust
// device.create_command_encoder(&CommandEncoderDescriptor { label }) -> CommandEncoder
pub fn begin_render_pass<'e>(&'e mut self, desc: &RenderPassDescriptor<'_>) -> RenderPass<'e>
pub fn begin_compute_pass<'e>(&'e mut self, desc: &ComputePassDescriptor<'_>) -> ComputePass<'e>
pub fn finish(self) -> CommandBuffer    // consumes the encoder
```

The open pass holds the encoder borrow, so the pass must be dropped before `finish()`.

### Copy commands — v29 `TexelCopy*` parameter names

```rust
pub fn copy_buffer_to_buffer(&mut self, src: &Buffer, src_off: BufferAddress,
    dst: &Buffer, dst_off: BufferAddress, copy_size: impl Into<Option<BufferAddress>>)
pub fn copy_buffer_to_texture(&mut self, source: TexelCopyBufferInfo<'_>,
    destination: TexelCopyTextureInfo<'_>, copy_size: Extent3d)
pub fn copy_texture_to_buffer(&mut self, source: TexelCopyTextureInfo<'_>,
    destination: TexelCopyBufferInfo<'_>, copy_size: Extent3d)
pub fn copy_texture_to_texture(&mut self, source: TexelCopyTextureInfo<'_>,
    destination: TexelCopyTextureInfo<'_>, copy_size: Extent3d)
```

`copy_texture_to_buffer`'s `bytes_per_row` must be a multiple of
`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT` (256). An 8192-wide RGBA8 row is `32768` bytes —
already a multiple of 256; narrower readbacks need rounding up. See buffers-and-textures.md.

## Render pass descriptor and attachments

```rust
pub struct RenderPassDescriptor<'a> {
    pub label: Label<'a>,
    pub color_attachments: &'a [Option<RenderPassColorAttachment<'a>>],
    pub depth_stencil_attachment: Option<RenderPassDepthStencilAttachment<'a>>, // 2D: None
    pub timestamp_writes: Option<RenderPassTimestampWrites<'a>>,
    pub occlusion_query_set: Option<&'a QuerySet>,
    pub multiview_mask: Option<NonZeroU32>,
}
pub struct RenderPassColorAttachment<'t> {
    pub view: &'t TextureView,
    pub depth_slice: Option<u32>,        // None for non-3D views
    pub resolve_target: Option<&'t TextureView>,
    pub ops: Operations<Color>,          // { load: LoadOp<Color>, store: StoreOp }
}
// LoadOp<V>: Clear(V) | Load | DontCare(..)    StoreOp: Store | Discard
// Color { r, g, b, a: f64 } with consts BLACK, TRANSPARENT, ...
```

## RenderPass methods (the calls issued inside egui's callback)

```rust
pub fn set_pipeline(&mut self, pipeline: &RenderPipeline)
pub fn set_bind_group<'a, BG>(&mut self, index: u32, bind_group: BG, offsets: &[DynamicOffset])
    where Option<&'a BindGroup>: From<BG>   // DynamicOffset = u32; &[] when none
pub fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: BufferSlice<'_>)
pub fn set_index_buffer(&mut self, buffer_slice: BufferSlice<'_>, index_format: IndexFormat) // Uint16|Uint32
pub fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>)
pub fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>)
pub fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32, min_depth: f32, max_depth: f32)
pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32)
pub fn set_blend_constant(&mut self, color: Color)
```

`BufferSlice` comes from `buffer.slice(range)`. egui-wgpu sets the viewport/scissor to the
callback rect before calling `paint`, so you normally do not call `set_viewport` yourself
inside the callback.

## ComputePass methods

```rust
pub fn set_pipeline(&mut self, pipeline: &ComputePipeline)
pub fn set_bind_group<'a, BG>(&mut self, index: u32, bind_group: BG, offsets: &[DynamicOffset])
    where Option<&'a BindGroup>: From<BG>
pub fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32)
pub fn dispatch_workgroups_indirect(&mut self, indirect_buffer: &Buffer, indirect_offset: BufferAddress)
```

`ComputePassDescriptor { label, timestamp_writes: Option<..> }`.

## Queue submission

```rust
pub fn submit<I: IntoIterator<Item = CommandBuffer>>(&self, command_buffers: I) -> SubmissionIndex
```

Pass `Some(cmd)`, `[cmd]`, or a `Vec`. The returned `SubmissionIndex` feeds
`device.poll(PollType::Wait { submission_index: Some(idx), .. })` to block until that
submission completes (headless readback). See device-and-lifecycle.md.

## Worked example — compute pass over a region (dispatch math)

```rust
const WG: u32 = 16;                       // must match @workgroup_size in WGSL
let groups_x = width.div_ceil(WG);        // ceil — covers non-multiple sizes
let groups_y = height.div_ceil(WG);       // 8192 -> 512 exactly

let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
    label: Some("pixhaus.image_op.encoder"),
});
{
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("pixhaus.image_op"), timestamp_writes: None,
    });
    pass.set_pipeline(&compute_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(groups_x, groups_y, 1);
} // pass dropped -> encoder borrow released
let _idx = queue.submit(Some(encoder.finish()));
```

For the 8K constraint, dispatch only over the dirty region: compute `groups_*` from the
dirty extent and pass a dirty-rect origin/extent uniform, never the full canvas.

## Worked example — offscreen render-to-texture (previews/tests)

The render crate begins its own pass only when rendering into a texture it owns (not egui's
surface):

```rust
let view = offscreen.create_view(&wgpu::TextureViewDescriptor::default());
let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
    label: Some("pixhaus.offscreen.encoder"),
});
{
    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("pixhaus.offscreen"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view, depth_slice: None, resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,    // 2D
        timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
    });
    rpass.set_pipeline(&quad_pipeline);
    rpass.set_bind_group(0, &quad_bind_group, &[]);
    rpass.set_vertex_buffer(0, quad_vbuf.slice(..));
    rpass.draw(0..4, 0..1);                // triangle-strip quad
} // drop before finish()
queue.submit(Some(encoder.finish()));
// then copy_texture_to_buffer + map for readback in tests
```

## Inside egui-wgpu's callback — do NOT begin/end a pass

When drawing the on-screen canvas through egui-wgpu, egui owns the surface pass and gives
your `paint` a `&mut RenderPass<'static>`. Issue only:

```rust
rpass.set_pipeline(&self.canvas_pipeline);
rpass.set_bind_group(0, &self.canvas_bind_group, &[]);
rpass.set_vertex_buffer(0, self.vbuf.slice(..));
rpass.draw(0..6, 0..1);
```

Never `begin_render_pass`, never `finish` an encoder, never `queue.submit` for on-screen
draws — egui does all of that. GPU work needing its own encoder/compute pass (image ops,
building a texture) goes in `CallbackTrait::prepare` (which gives `&mut CommandEncoder` and
lets you return extra `CommandBuffer`s egui submits) or your own pre-frame step. The exact
egui-wgpu contract is the pixhaus-egui-wgpu skill's domain.

## VERIFY

- Exact `PollType::Wait` field/ctor names for blocking on a `SubmissionIndex` (see
  device-and-lifecycle.md). `multi_draw_indirect` exists but is gated behind
  `Features::MULTI_DRAW_INDIRECT` — confirm adapter support before using.
