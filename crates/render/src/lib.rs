//! UI-agnostic `wgpu` viewport renderer for Pixhaus.
//!
//! `render` owns the GPU drawing of the canvas viewport and knows nothing about
//! egui or any UI toolkit. It is embedded by `pixhaus-ui` through an egui paint
//! callback, but exposes only raw `wgpu` types so it survives a UI-toolkit change.
//!
//! Foundation stage: [`ViewportRenderer`] is a textured-quad blitter. The UI
//! composites a sprite on the CPU (`core::composite`), hands the RGBA bytes to
//! [`ViewportRenderer::upload_frame`], and [`ViewportRenderer::paint`] samples them
//! with a nearest filter inside egui's render pass. Layer compositing on the GPU and
//! dirty-rect uploads are the documented follow-ups (architecture bible section 16).

/// Draws the canvas viewport with raw `wgpu`.
///
/// Holds the render pipeline and a retained canvas texture, both reused across
/// frames. [`ViewportRenderer::paint`] records draw calls into an existing render
/// pass — it never begins, ends, or submits the pass, because it runs inside the
/// pass egui-wgpu owns.
pub struct ViewportRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The format for the canvas texture; sRGB iff the target is, so authored bytes
    /// round-trip through the GPU's encode/decode.
    texture_format: wgpu::TextureFormat,
    /// Whether the color target is sRGB. When it is, the hardware encodes linear→sRGB on
    /// write, so the chrome colors (passed in as sRGB) are linearized first; otherwise
    /// they are written through unchanged. This mirrors how the sprite texture's format
    /// absorbs the same difference.
    srgb: bool,
    /// The view uniform (96 bytes): the board origin/size in physical pixels, the checker
    /// and grid colors, and the cell/period/flags the fragment shader draws the canvas
    /// chrome from. Written each frame by [`ViewportRenderer::set_params`].
    view_buffer: wgpu::Buffer,
    /// The retained sprite texture; recreated only when the sprite size changes. `None`
    /// for an empty document — the shader then draws checker + grid only.
    texture: Option<CanvasTexture>,
    /// A 1x1 texture bound when there is no sprite, so the pass always has a valid texture
    /// to draw the checkerboard/grid over. Its contents are never sampled (`has_tile` is 0).
    placeholder: CanvasTexture,
}

/// Per-frame canvas-chrome parameters the fragment shader reads. Plain data — `render`
/// stays UI-agnostic. Colors arrive sRGB-normalized in 0..1; [`ViewportRenderer::set_params`]
/// linearizes them for an sRGB target (the only layer that knows the target color space).
pub struct ViewParams {
    /// Board top-left in physical pixels.
    pub origin_px: [f32; 2],
    /// Board size in physical pixels.
    pub size_px: [f32; 2],
    /// Light checker cell, sRGB-normalized rgba in 0..1.
    pub checker_lo: [f32; 4],
    /// Dark checker cell, sRGB-normalized rgba in 0..1.
    pub checker_hi: [f32; 4],
    /// Minor (per-sprite-pixel) grid line: sRGB-normalized rgb, line strength in alpha.
    pub minor_color: [f32; 4],
    /// Major grid line: sRGB-normalized rgb, line strength in alpha.
    pub major_color: [f32; 4],
    /// Checker cell size in physical pixels.
    pub checker_cell_px: f32,
    /// Physical pixels per sprite pixel (the minor-grid period).
    pub ppsp: f32,
    /// Major-grid period in sprite pixels (0 = no major grid).
    pub major_period: f32,
    /// Bit flags: 1 = `has_tile` (sample the sprite), 2 = minor grid on, 4 = major grid on.
    pub flags: u32,
}

/// A retained canvas texture and its bind group, sized to the current sprite.
struct CanvasTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl ViewportRenderer {
    /// Builds the blit pipeline against `target_format`.
    ///
    /// `target_format` must equal the format egui-wgpu renders into, or pipeline
    /// creation fails. Build this once at startup and reuse it every frame.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // GPU object construction below (shader, pipeline, sampler, textures) is build-once at
        // startup on controlled, in-crate inputs (a vendored .wgsl, fixed descriptors), so a
        // validation error here is a programming bug, not a runtime condition: we let wgpu's
        // default uncaptured-error panic surface it. If render later grows a fallible runtime
        // path (user shaders, dynamic formats), add a crate-local thiserror Error/Result and an
        // error scope (or device.on_uncaptured_error) around these calls. See pixhaus-wgpu.
        let shader = device.create_shader_module(wgpu::include_wgsl!("viewport_blit.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pixhaus.viewport.bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Nearest everywhere: pixel art must not be filtered (bible 16.4).
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixhaus.viewport.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pixhaus.viewport.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pixhaus.viewport.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // The shader composites the sprite over an opaque checkerboard and
                    // outputs opaque pixels, so the board fully replaces its rect.
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let srgb = target_format.is_srgb();
        let texture_format = if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        // origin/size (2x vec2) + 4x vec4 colors + 1x vec4 params; std140 at 96 bytes.
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixhaus.viewport.view"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // The placeholder keeps a valid texture bound for the empty-document checker.
        let placeholder = Self::make_texture(device, &bind_group_layout, &sampler, &view_buffer, texture_format, 1, 1);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            texture_format,
            srgb,
            view_buffer,
            texture: None,
            placeholder,
        }
    }

    /// Writes the per-frame canvas-chrome parameters for the next paint. The fragment
    /// shader derives each pixel's board coordinate from its framebuffer position and
    /// `origin/size`, so the canvas is correct no matter how egui-wgpu clamps the pass
    /// viewport — a board panned or zoomed past the window edge crops rather than
    /// squashing — and draws the checkerboard and grid procedurally (O(visible pixels)).
    ///
    /// The chrome colors arrive as sRGB-normalized 0..1; they are linearized here when the
    /// target is sRGB (the hardware then re-encodes on write) and passed through otherwise,
    /// so the checker and grid land on the same value the UI's theme token defines. Call
    /// every frame.
    pub fn set_params(&self, queue: &wgpu::Queue, p: &ViewParams) {
        // The packing is a pure free fn so the std140 offset layout has off-GPU coverage:
        // a wrong offset silently corrupts the uniform, and the device tests only run on a
        // box with a GPU adapter. Keeping set_params thin leaves only the queue write here.
        let b = pack_view(self.srgb, p);
        queue.write_buffer(&self.view_buffer, 0, &b);
    }

    /// Uploads a tightly-packed (`stride == width * 4`) RGBA8 frame for display.
    ///
    /// Recreates the GPU texture only when the size changes; otherwise it overwrites
    /// the retained texture in place. Full-frame upload is fine for the small
    /// Generate sprites of the foundation; dirty-rect upload is the 8K follow-up.
    ///
    /// # Panics
    ///
    /// In debug builds, if `rgba` is not exactly `width * height * 4` bytes. The upload
    /// hardcodes `bytes_per_row = width * 4`, so a loosely-packed slice would otherwise
    /// trip a wgpu validation error at `write_texture`; the assert turns that into a
    /// clear failure at the call boundary.
    pub fn upload_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rgba: &[u8], width: u32, height: u32) {
        if frame_upload_is_skippable(width, height) {
            return;
        }
        debug_assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "upload_frame requires a tightly-packed RGBA8 frame (stride == width * 4)"
        );
        let needs_alloc = self.texture.as_ref().is_none_or(|t| t.width != width || t.height != height);
        if needs_alloc {
            self.texture = Some(Self::make_texture(
                device,
                &self.bind_group_layout,
                &self.sampler,
                &self.view_buffer,
                self.texture_format,
                width,
                height,
            ));
        }
        if let Some(canvas) = self.texture.as_ref() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &canvas.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Records the canvas draw into `render_pass`. Always draws: the fragment shader
    /// renders the checkerboard and grid procedurally and composites the sprite over them
    /// when one is uploaded. With no sprite it binds the placeholder texture (never
    /// sampled, since `has_tile` is 0). It must not end the pass or begin another.
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        let bind_group = self.texture.as_ref().map_or(&self.placeholder.bind_group, |canvas| &canvas.bind_group);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Creates a texture of `width`x`height` and its bind group (texture + sampler + the
    /// shared view uniform). An associated function so the placeholder can be built in
    /// [`Self::new`] before `self` exists.
    fn make_texture(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        view_buffer: &wgpu::Buffer,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> CanvasTexture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pixhaus.viewport.canvas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pixhaus.viewport.bind_group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: view_buffer.as_entire_binding(),
                },
            ],
        });
        CanvasTexture {
            texture,
            bind_group,
            width,
            height,
        }
    }
}

/// Builds the 96-byte view uniform from `p`, encoding the chrome colors for `srgb`.
///
/// 96-byte std140 layout: `origin@0` `size@8` `checker_lo@16` `checker_hi@32`
/// `minor@48` `major@64` `params@80`. `flags` rides as a float (values <= 7, exactly
/// representable; the shader reads it back with `u32(params.w)`).
///
/// The view uniform is hand-packed byte by byte rather than derived from a bytemuck
/// Pod, so render keeps wgpu as its only dependency and stays bytemuck-free. The fields
/// are written at fixed std140 offsets (documented above); a wrong offset would silently
/// corrupt the uniform, so this pure fn is unit-tested off-GPU where the device tests skip.
#[allow(clippy::cast_precision_loss)]
fn pack_view(srgb: bool, p: &ViewParams) -> [u8; 96] {
    let mut b = [0u8; 96];
    put(&mut b, 0, p.origin_px[0]);
    put(&mut b, 4, p.origin_px[1]);
    put(&mut b, 8, p.size_px[0]);
    put(&mut b, 12, p.size_px[1]);
    for (i, v) in encode_color(srgb, p.checker_lo).iter().enumerate() {
        put(&mut b, 16 + i * 4, *v);
    }
    for (i, v) in encode_color(srgb, p.checker_hi).iter().enumerate() {
        put(&mut b, 32 + i * 4, *v);
    }
    for (i, v) in encode_color(srgb, p.minor_color).iter().enumerate() {
        put(&mut b, 48 + i * 4, *v);
    }
    for (i, v) in encode_color(srgb, p.major_color).iter().enumerate() {
        put(&mut b, 64 + i * 4, *v);
    }
    put(&mut b, 80, p.checker_cell_px);
    put(&mut b, 84, p.ppsp);
    put(&mut b, 88, p.major_period);
    put(&mut b, 92, p.flags as f32);
    b
}

/// Encodes an sRGB-normalized chrome color into the space the color target expects:
/// linearized rgb for an sRGB target (the hardware re-encodes on write), unchanged
/// otherwise. Alpha is a grid-line strength, not a color, so it is never converted.
fn encode_color(srgb: bool, c: [f32; 4]) -> [f32; 4] {
    if srgb {
        [srgb_to_linear(c[0]), srgb_to_linear(c[1]), srgb_to_linear(c[2]), c[3]]
    } else {
        c
    }
}

/// Writes a little-endian f32 into `bytes` at `off` (the std140 uniform packer).
fn put(bytes: &mut [u8], off: usize, v: f32) {
    bytes[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// True when a frame upload can be skipped entirely: a zero-area frame has no texels
/// to write and would make `make_texture` create a zero-sized texture. Pure decision,
/// factored out so it has coverage on headless CI where the GPU upload test skips.
fn frame_upload_is_skippable(width: u32, height: u32) -> bool {
    width == 0 || height == 0
}

/// The standard sRGB → linear transfer for one 0..1 channel.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[cfg(test)]
mod tests {
    use super::ViewportRenderer;

    /// `srgb_to_linear` is the color-correctness transfer the chrome colors pass
    /// through for an sRGB target; pin its fixed points and both segments. No GPU, so
    /// this runs on headless CI where the device tests below skip.
    #[test]
    fn srgb_to_linear_maps_known_anchors() {
        // 0 and 1 are fixed points of the transfer.
        assert!(super::srgb_to_linear(0.0).abs() < 1e-6);
        assert!((super::srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        // Below the 0.04045 breakpoint the transfer is the linear c / 12.92 segment.
        let toe = 0.04_f32;
        assert!((super::srgb_to_linear(toe) - toe / 12.92).abs() < 1e-6);
        // Mid-gray sits on the power segment at ~0.214.
        let mid = super::srgb_to_linear(0.5);
        assert!((mid - 0.214_f32).abs() < 1e-3, "0.5 sRGB should linearize to ~0.214, got {mid}");
    }

    /// `put` is the std140 byte packer behind `set_params`; a wrong offset silently
    /// corrupts the uniform, so pin the little-endian write and that it stays in lane.
    #[test]
    fn put_writes_a_little_endian_f32_at_the_offset() {
        let mut bytes = [0u8; 12];
        super::put(&mut bytes, 4, 1.0_f32);
        assert_eq!(&bytes[0..4], &[0, 0, 0, 0], "bytes before the offset are untouched");
        assert_eq!(&bytes[4..8], &1.0_f32.to_le_bytes(), "the f32 lands little-endian at the offset");
        assert_eq!(&bytes[8..12], &[0, 0, 0, 0], "bytes after the write are untouched");
    }

    /// `frame_upload_is_skippable` gates `upload_frame`; pin it for both zero-area
    /// cases and the normal path. No GPU, so this runs on headless CI.
    #[test]
    fn frame_upload_is_skippable_only_on_zero_area() {
        assert!(super::frame_upload_is_skippable(0, 4));
        assert!(super::frame_upload_is_skippable(4, 0));
        assert!(super::frame_upload_is_skippable(0, 0));
        assert!(!super::frame_upload_is_skippable(1, 1));
        assert!(!super::frame_upload_is_skippable(4, 8));
    }

    /// A representative set of view parameters for the off-GPU packing tests. Distinct,
    /// non-trivial values per field so a swapped offset shows up as a wrong read-back.
    fn sample_params(flags: u32) -> super::ViewParams {
        super::ViewParams {
            origin_px: [1.0, 2.0],
            size_px: [3.0, 4.0],
            checker_lo: [0.10, 0.20, 0.30, 1.0],
            checker_hi: [0.40, 0.50, 0.60, 1.0],
            minor_color: [0.70, 0.80, 0.90, 0.4],
            major_color: [0.15, 0.25, 0.35, 1.0],
            checker_cell_px: 8.0,
            ppsp: 4.0,
            major_period: 16.0,
            flags,
        }
    }

    /// Reads the little-endian f32 at `off` in a packed view uniform.
    fn read(b: &[u8; 96], off: usize) -> f32 {
        let mut chunk = [0u8; 4];
        chunk.copy_from_slice(&b[off..off + 4]);
        f32::from_le_bytes(chunk)
    }

    /// Asserts the f32 at `off` packed exactly (compared on bytes to dodge `float_cmp`,
    /// which is correct here: the value was written via `to_le_bytes` and is unmodified).
    fn assert_at(b: &[u8; 96], off: usize, want: f32, field: &str) {
        assert_eq!(&b[off..off + 4], &want.to_le_bytes(), "{field}");
    }

    /// `encode_color` is the sRGB-vs-passthrough branch behind every chrome color in the
    /// uniform. Pin both arms off-GPU, where the device tests skip.
    #[test]
    fn encode_color_passes_through_unless_srgb() {
        let c = [0.10_f32, 0.20, 0.30, 0.4];
        // Non-sRGB target: the color is written through unchanged, alpha included.
        // Compared as bits so the exact passthrough does not trip the float_cmp lint.
        assert_eq!(super::encode_color(false, c).map(f32::to_bits), c.map(f32::to_bits));
        // sRGB target: each rgb channel is linearized; alpha (grid strength) is untouched.
        let got = super::encode_color(true, c);
        assert!((got[0] - super::srgb_to_linear(c[0])).abs() < 1e-6);
        assert!((got[1] - super::srgb_to_linear(c[1])).abs() < 1e-6);
        assert!((got[2] - super::srgb_to_linear(c[2])).abs() < 1e-6);
        assert_eq!(got[3].to_bits(), 0.4_f32.to_bits(), "alpha is a grid-line strength, never color-converted");
    }

    /// `pack_view` lays the uniform out at fixed std140 offsets; a wrong offset silently
    /// corrupts it on a GPU box only. Pin every field's offset for both color spaces.
    #[test]
    fn pack_view_lands_each_field_at_its_std140_offset() {
        let p = sample_params(0b111);

        // Non-sRGB: colors pass through, so every field reads back its source value.
        let b = super::pack_view(false, &p);
        assert_at(&b, 0, 1.0, "origin.x @0");
        assert_at(&b, 4, 2.0, "origin.y @4");
        assert_at(&b, 8, 3.0, "size.x @8");
        assert_at(&b, 12, 4.0, "size.y @12");
        assert_at(&b, 16, 0.10, "checker_lo.r @16");
        assert_at(&b, 32, 0.40, "checker_hi.r @32");
        assert_at(&b, 48, 0.70, "minor.r @48");
        assert_at(&b, 60, 0.4, "minor.a @60 (grid strength, not linearized)");
        assert_at(&b, 64, 0.15, "major.r @64");
        assert_at(&b, 80, 8.0, "checker_cell_px @80");
        assert_at(&b, 84, 4.0, "ppsp @84");
        assert_at(&b, 88, 16.0, "major_period @88");
        assert_at(&b, 92, 7.0, "flags @92 ride as f32");

        // sRGB: rgb channels are linearized in place; the alpha and scalar slots stay raw.
        let bs = super::pack_view(true, &p);
        assert!((read(&bs, 16) - super::srgb_to_linear(0.10)).abs() < 1e-6, "checker_lo.r linearized @16");
        assert!((read(&bs, 48) - super::srgb_to_linear(0.70)).abs() < 1e-6, "minor.r linearized @48");
        assert_at(&bs, 60, 0.4, "minor.a stays raw under sRGB too");
        assert_at(&bs, 80, 8.0, "scalar params are never color-encoded");

        // flags round-trip as a float for a second bit pattern.
        let b6 = super::pack_view(false, &sample_params(0b110));
        assert_at(&b6, 92, 6.0, "flags 0b110 reads back 6.0");
    }

    /// Builds the renderer and uploads two differently-sized frames (forcing a
    /// texture reallocation) on a real device when one is available.
    ///
    /// Skips silently where no GPU adapter exists (headless CI), so the workspace
    /// test gate stays green off-GPU while still exercising the GPU path on a
    /// developer box.
    #[test]
    fn upload_frame_handles_a_size_change() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) else {
            return;
        };

        let mut renderer = ViewportRenderer::new(&device, wgpu::TextureFormat::Bgra8UnormSrgb);
        renderer.upload_frame(&device, &queue, &[255u8; 4 * 4 * 4], 4, 4);
        // Different size: forces the texture to be reallocated.
        renderer.upload_frame(&device, &queue, &[0u8; 8 * 8 * 4], 8, 8);
    }

    /// Records a real paint into an offscreen target with and without a tile, on a GPU
    /// when present. This exercises `set_params` (the 96-byte uniform) and the draw, so a
    /// shader / bind-group / uniform-size mismatch surfaces as a validation panic.
    #[test]
    fn paint_renders_with_and_without_a_tile() {
        fn paint_once(device: &wgpu::Device, queue: &wgpu::Queue, view: &wgpu::TextureView, renderer: &ViewportRenderer) {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("test.pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                renderer.paint(&mut pass);
            }
            queue.submit(Some(encoder.finish()));
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
        }

        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) else {
            return;
        };

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let mut renderer = ViewportRenderer::new(&device, format);

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test.target"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let params = |flags: u32| super::ViewParams {
            origin_px: [0.0, 0.0],
            size_px: [16.0, 16.0],
            checker_lo: [0.3, 0.3, 0.3, 1.0],
            checker_hi: [0.2, 0.2, 0.2, 1.0],
            minor_color: [0.5, 0.5, 0.5, 0.4],
            major_color: [0.6, 0.6, 0.6, 1.0],
            checker_cell_px: 8.0,
            ppsp: 4.0,
            major_period: 8.0,
            flags,
        };

        // Empty board: placeholder bound, checker + grid only (has_tile clear).
        renderer.set_params(&queue, &params(0b110));
        paint_once(&device, &queue, &view, &renderer);

        // With a real sprite uploaded: has_tile set.
        renderer.upload_frame(&device, &queue, &[255u8; 4 * 4 * 4], 4, 4);
        renderer.set_params(&queue, &params(0b111));
        paint_once(&device, &queue, &view, &renderer);
    }
}
