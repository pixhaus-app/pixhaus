//! The wgpu `ViewportRenderer`: owns the SPRITE pipeline and the frame texture,
//! displays a composited sprite frame, and swaps the texture when playback
//! advances.
//!
//! Stays egui-independent — it speaks raw wgpu (`Device`, `Queue`,
//! `RenderPass`, `TextureFormat`). The egui embedding (the `CallbackTrait`
//! impl, the `CallbackResources` slot) lives in the `shell` crate.

use bytemuck::{Pod, Zeroable};
use pixhaus_core::canvas::PixelBuffer;

/// Uniform block shared by the SPRITE vertex and fragment stages. Field order
/// and padding match `sprite.wgsl`'s `Uniforms`; total size 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    scroll: [f32; 2],
    sprite_size: [f32; 2],
    zoom: f32,
    has_tile: f32,
}

/// The frame texture plus the bind group that references it. Recreated when the
/// sprite's canvas size changes.
///
/// The `texture` handle is retained (not just the bind group) so
/// [`ViewportRenderer::upload_dirty_rect`] can write a single dirty sub-rect in
/// place rather than re-uploading the whole frame — the drawing hot path.
struct FrameTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// Owns the SPRITE program's GPU resources and the current frame texture.
pub struct ViewportRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buf: wgpu::Buffer,
    /// `None` until the first frame is uploaded; a placeholder keeps the bind
    /// group valid so `paint` can always run.
    frame: FrameTexture,
    /// Current sprite size in canvas pixels, mirrored into the uniform.
    sprite_size: [f32; 2],
    /// 1.0 when `frame` holds real pixels, 0.0 when it is the placeholder.
    has_tile: f32,
}

impl ViewportRenderer {
    /// Builds the SPRITE pipeline and a 1x1 placeholder frame texture. Build
    /// the color target against `target_format` (egui-wgpu's surface format).
    #[must_use]
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite.bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("sprite.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite.pipeline"),
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
                    // The SPRITE program always outputs opaque pixels; REPLACE
                    // overwrites the sprite rect and leaves the rest untouched.
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite.uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame = Self::make_placeholder(device, &bind_group_layout, &uniform_buf, &sampler);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buf,
            frame,
            sprite_size: [0.0, 0.0],
            has_tile: 0.0,
        }
    }

    /// Creates a 1x1 placeholder texture and its bind group. The placeholder is
    /// never sampled (`has_tile` stays 0 until a real frame lands); it exists
    /// only so the bind group is always valid and `paint` can run.
    fn make_placeholder(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, uniform_buf: &wgpu::Buffer, sampler: &wgpu::Sampler) -> FrameTexture {
        let texture = device.create_texture(&placeholder_descriptor());
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = make_bind_group(device, bgl, uniform_buf, &view, sampler);
        FrameTexture {
            texture,
            bind_group,
            width: 1,
            height: 1,
        }
    }

    /// Uploads a composited frame, recreating the texture if the canvas size
    /// changed. Sets `has_tile` so the shader composites it over the checker.
    #[allow(clippy::cast_precision_loss)] // sprite dims <= 8192 are exact in f32
    pub fn set_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &PixelBuffer) {
        let width = frame.width();
        let height = frame.height();
        if width == 0 || height == 0 {
            self.has_tile = 0.0;
            return;
        }

        // Allocate a fresh texture only when the canvas size changes; otherwise
        // keep the retained texture and overwrite its pixels in place so the
        // handle that `upload_dirty_rect` writes to stays valid across frames.
        if self.frame.width != width || self.frame.height != height {
            let texture = device.create_texture(&frame_descriptor(width, height));
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = make_bind_group(device, &self.bind_group_layout, &self.uniform_buf, &view, &self.sampler);
            Self::upload_pixels(queue, &texture, frame);
            self.frame = FrameTexture {
                texture,
                bind_group,
                width,
                height,
            };
        } else {
            Self::upload_pixels(queue, &self.frame.texture, frame);
        }

        self.sprite_size = [width as f32, height as f32];
        self.has_tile = 1.0;
    }

    /// Writes a single dirty sub-rect from `frame` into the retained frame
    /// texture, touching only `(x, y)`–`(x+w, y+h)`. This is the drawing hot
    /// path: a brush dab uploads its few dirty rows, never the whole canvas, so
    /// per-stroke GPU work is bounded by the dirty region rather than the canvas
    /// size — the 8K performance constraint.
    ///
    /// `frame` is the full-canvas buffer; the rect is read straight out of it at
    /// the source stride (no intermediate copy). The rect is clamped to the
    /// texture bounds; a rect fully outside the texture is a no-op. Call
    /// [`Self::set_frame`] at least once first so the texture exists at the
    /// right size.
    pub fn upload_dirty_rect(&self, queue: &wgpu::Queue, frame: &PixelBuffer, x: u32, y: u32, w: u32, h: u32) {
        if self.has_tile <= 0.0 || x >= self.frame.width || y >= self.frame.height {
            return;
        }
        let w = w.min(self.frame.width - x).min(frame.width().saturating_sub(x));
        let h = h.min(self.frame.height - y).min(frame.height().saturating_sub(y));
        if w == 0 || h == 0 {
            return;
        }
        let offset = u64::from(y) * u64::from(frame.stride()) + u64::from(x) * 4;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.frame.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            frame.as_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset,
                bytes_per_row: Some(frame.stride()),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    fn upload_pixels(queue: &wgpu::Queue, texture: &wgpu::Texture, frame: &PixelBuffer) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            frame.as_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.stride()),
                rows_per_image: Some(frame.height()),
            },
            wgpu::Extent3d {
                width: frame.width(),
                height: frame.height(),
                depth_or_array_layers: 1,
            },
        );
    }

    /// Writes the per-frame camera uniforms. `resolution` is the viewport size
    /// in physical pixels, `scroll` the canvas coord at the viewport centre,
    /// `zoom` physical pixels per canvas pixel.
    pub fn write_uniforms(&self, queue: &wgpu::Queue, resolution: [f32; 2], scroll: [f32; 2], zoom: f32) {
        let uniforms = Uniforms {
            resolution,
            scroll,
            sprite_size: self.sprite_size,
            zoom,
            has_tile: self.has_tile,
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Issues the SPRITE draw into an already-bound render pass. Call only after
    /// [`Self::write_uniforms`] for this frame.
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        if self.sprite_size[0] <= 0.0 || self.sprite_size[1] <= 0.0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.frame.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

fn frame_descriptor(width: u32, height: u32) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("sprite.frame"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        // COPY_SRC lets the frame be read back (screenshots, visual-regression
        // tests). The dirty-rect upload uses COPY_DST.
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    }
}

fn placeholder_descriptor() -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("sprite.placeholder"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    }
}

fn make_bind_group(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    uniform_buf: &wgpu::Buffer,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite.bind_group"),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headless device, or `None` when this machine has no usable adapter
    /// (CI without a GPU) — callers should skip rather than fail.
    fn headless() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        Some((device, queue))
    }

    #[test]
    fn renders_a_frame_to_an_offscreen_target() {
        let Some((device, queue)) = headless() else {
            eprintln!("no wgpu adapter; skipping headless smoke test");
            return;
        };

        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = ViewportRenderer::new(&device, format);

        // A 4x4 solid opaque red sprite.
        let pixels: Vec<u8> = (0..4 * 4).flat_map(|_| [255u8, 0, 0, 255]).collect();
        let frame = PixelBuffer::from_raw(4, 4, 4 * 4, pixels).unwrap();
        renderer.set_frame(&device, &queue, &frame);

        // 64x64 target so the sprite (4px * 16 zoom) fills it and the copy
        // row stride (64*4 = 256) is already aligned.
        const SIZE: u32 = 64;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test.target"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        renderer.write_uniforms(&queue, [SIZE as f32, SIZE as f32], [2.0, 2.0], 16.0);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("test.pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_view,
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
                })
                .forget_lifetime();
            renderer.paint(&mut pass);
        }

        // Copy the target into a readback buffer (256-byte aligned rows).
        let bytes_per_row = SIZE * 4;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test.readback"),
            size: u64::from(bytes_per_row * SIZE),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(SIZE),
                },
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        let data = slice.get_mapped_range();

        // Centre pixel sits inside the sprite -> red.
        let centre = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
        assert_eq!(data[centre], 255, "centre red channel");
        assert_eq!(data[centre + 1], 0, "centre green channel");
        assert_eq!(data[centre + 2], 0, "centre blue channel");
    }

    #[test]
    fn dirty_rect_upload_touches_only_the_rect() {
        let Some((device, queue)) = headless() else {
            eprintln!("no wgpu adapter; skipping dirty-rect smoke test");
            return;
        };

        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = ViewportRenderer::new(&device, format);

        // Start with a 16x16 opaque-red frame.
        let mut pixels: Vec<u8> = (0..16 * 16).flat_map(|_| [255u8, 0, 0, 255]).collect();
        let mut frame = PixelBuffer::from_raw(16, 16, 16 * 4, pixels.clone()).unwrap();
        renderer.set_frame(&device, &queue, &frame);

        // Paint a 2x2 green block at (3,4) into the CPU buffer, then upload only
        // that sub-rect to the GPU texture.
        for yy in 4..6u32 {
            for xx in 3..5u32 {
                let off = (yy as usize * 16 + xx as usize) * 4;
                pixels[off..off + 4].copy_from_slice(&[0, 255, 0, 255]);
            }
        }
        frame = PixelBuffer::from_raw(16, 16, 16 * 4, pixels).unwrap();
        renderer.upload_dirty_rect(&queue, &frame, 3, 4, 2, 2);

        // Read the texture back by copying it to a buffer (rows already aligned:
        // 16*4 = 64, padded to 256 below).
        let bytes_per_row = 256u32; // 16*4=64 rounded up to COPY alignment
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dirty.readback"),
            size: u64::from(bytes_per_row * 16),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &renderer.frame.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(16),
                },
            },
            wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        let data = slice.get_mapped_range();

        let at = |x: usize, y: usize| {
            let o = y * bytes_per_row as usize + x * 4;
            [data[o], data[o + 1], data[o + 2], data[o + 3]]
        };
        // Inside the dirty rect: green.
        assert_eq!(at(3, 4), [0, 255, 0, 255], "rect pixel should be green");
        assert_eq!(at(4, 5), [0, 255, 0, 255], "rect pixel should be green");
        // Outside the dirty rect: untouched red.
        assert_eq!(at(0, 0), [255, 0, 0, 255], "pixel outside rect stays red");
        assert_eq!(at(5, 5), [255, 0, 0, 255], "pixel just outside rect stays red");
    }
}
