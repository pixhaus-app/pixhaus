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
    /// The view uniform: the board's `[origin_x, origin_y, size_x, size_y]` in physical
    /// pixels. The fragment shader maps each pixel's framebuffer position through it, so
    /// the blit stays undistorted even when egui-wgpu clamps the pass viewport.
    view_buffer: wgpu::Buffer,
    /// The retained texture; recreated only when the sprite size changes.
    texture: Option<CanvasTexture>,
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let texture_format = if target_format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        // 4 f32 (origin xy, size xy); std140-compatible at 16 bytes.
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixhaus.viewport.view"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            texture_format,
            view_buffer,
            texture: None,
        }
    }

    /// Sets the board's top-left `origin_px` and `size_px`, in physical pixels, for the
    /// next paint. The fragment shader derives each pixel's texture coordinate from its
    /// framebuffer position and these, so the blit is correct no matter how egui-wgpu
    /// clamps the pass viewport — a board panned or zoomed past the window edge crops
    /// instead of squashing the texture into the clamped viewport. Call every frame.
    pub fn set_view(&self, queue: &wgpu::Queue, origin_px: [f32; 2], size_px: [f32; 2]) {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&origin_px[0].to_le_bytes());
        bytes[4..8].copy_from_slice(&origin_px[1].to_le_bytes());
        bytes[8..12].copy_from_slice(&size_px[0].to_le_bytes());
        bytes[12..16].copy_from_slice(&size_px[1].to_le_bytes());
        queue.write_buffer(&self.view_buffer, 0, &bytes);
    }

    /// Uploads a tightly-packed (`stride == width * 4`) RGBA8 frame for display.
    ///
    /// Recreates the GPU texture only when the size changes; otherwise it overwrites
    /// the retained texture in place. Full-frame upload is fine for the small
    /// Generate sprites of the foundation; dirty-rect upload is the 8K follow-up.
    pub fn upload_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rgba: &[u8], width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let needs_alloc = self.texture.as_ref().is_none_or(|t| t.width != width || t.height != height);
        if needs_alloc {
            self.texture = Some(self.allocate(device, width, height));
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

    /// Records the canvas draw into `render_pass`. A no-op until a frame is uploaded.
    ///
    /// Draws a fullscreen triangle over the pass viewport; the fragment shader maps each
    /// pixel back to texture space via the view uniform ([`Self::set_view`]), so the blit
    /// does not depend on egui-wgpu's (window-clamped) viewport. It must not end the pass
    /// or begin another.
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        let Some(canvas) = self.texture.as_ref() else {
            return;
        };
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &canvas.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Creates a texture of `width`x`height` and its bind group.
    fn allocate(&self, device: &wgpu::Device, width: u32, height: u32) -> CanvasTexture {
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
            format: self.texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pixhaus.viewport.bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.view_buffer.as_entire_binding(),
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

#[cfg(test)]
mod tests {
    use super::ViewportRenderer;

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
}
