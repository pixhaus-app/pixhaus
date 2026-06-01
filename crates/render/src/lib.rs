//! UI-agnostic `wgpu` viewport renderer for Pixhaus.
//!
//! `render` owns the GPU drawing of the canvas viewport and knows nothing about
//! egui or any UI toolkit. It is embedded by `pixhaus-ui` through an egui paint
//! callback, but exposes only raw `wgpu` types so it survives a UI-toolkit change.
//!
//! Scaffold stage: it draws a single flat-colored fill over the viewport rect —
//! the end-to-end spine that proves pixel data reaches the screen through the
//! egui/wgpu seam. The layer-compositing pipeline lands in Phase 1 (architecture
//! bible section 16).

/// Draws the canvas viewport with raw `wgpu`.
///
/// Holds the render pipeline, built once at startup and reused every frame.
/// [`ViewportRenderer::paint`] records draw calls into an existing render pass —
/// it never begins, ends, or submits the pass, because it runs inside the pass
/// egui-wgpu owns.
pub struct ViewportRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl ViewportRenderer {
    /// Builds the viewport pipeline against `target_format`.
    ///
    /// `target_format` must equal the format egui-wgpu renders into, or pipeline
    /// creation fails. Build this once at startup and reuse it every frame.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("viewport_fill.wgsl"));

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pixhaus.viewport.layout"),
            bind_group_layouts: &[],
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

        Self { pipeline }
    }

    /// Records the viewport draw into `render_pass`.
    ///
    /// egui-wgpu has already set the pass viewport and scissor to the canvas rect,
    /// so this only sets the pipeline and issues the fullscreen-triangle draw. It
    /// must not end the pass or begin another.
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::ViewportRenderer;

    /// Builds the pipeline on a real device when one is available.
    ///
    /// Skips silently where no GPU adapter exists (headless CI), so the workspace
    /// test gate stays green off-GPU while still exercising pipeline creation on a
    /// developer box.
    #[test]
    fn viewport_renderer_builds_when_a_gpu_is_present() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
            return;
        };
        let Ok((device, _queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) else {
            return;
        };
        let _renderer = ViewportRenderer::new(&device, wgpu::TextureFormat::Bgra8UnormSrgb);
    }
}
