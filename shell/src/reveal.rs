//! The generation reveal effect: a particle field that scatters while a
//! generation is in flight, coheres on the first streamed partial, and snaps to
//! the crisp final image, then hands back to the static viewport.
//!
//! Two halves live here. The CPU half is [`RevealState`] — a tiny per-stage
//! state machine, advanced from the generation signals already on the shell
//! ([`RevealState::begin`] / [`on_partial`](RevealState::on_partial) /
//! [`on_final`](RevealState::on_final)) and ticked each frame. The GPU half is
//! [`RevealRenderer`] plus `reveal.wgsl`: one instanced draw of a fixed particle
//! grid, with its own [`egui_wgpu::CallbackTrait`]. It is kept in the shell (not
//! the UI-agnostic `render` crate) because it is decorative — revisit if it grows.
//!
//! The grid is fixed (capped on the long edge) and independent of the output
//! resolution, so an 8K request animates the same instance count in one draw —
//! the 8K performance constraint.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bytemuck::{Pod, Zeroable};
use eframe::egui;

// ── Tuning ──────────────────────────────────────────────────────────────────

/// Particle convergence held during the scatter phase: a loose cloud, mostly
/// driven by the shader's per-instance drift.
const SCATTER_ASSEMBLY: f32 = 0.1;
/// Convergence the cohere phase eases toward — settled enough to read as
/// forming, loose enough to still shimmer.
const COHERE_ASSEMBLY: f32 = 0.6;
/// Source opacity the cohere phase eases toward, so the streamed partial shows
/// through without fully replacing the cloud.
const COHERE_REVEAL: f32 = 0.6;
/// Seconds the cohere ease runs over.
const COHERE_SECS: f64 = 0.5;
/// Seconds the final snap ease runs over before the effect deactivates.
const SNAP_SECS: f64 = 0.35;
/// Grid cells on the long edge (the short edge scales to the target aspect).
/// Caps the instance count near `GRID_LONG_MAX^2` regardless of output size.
const GRID_LONG_MAX: u32 = 128;

/// Process-wide monotonic source-image id. Each new partial/final gets a unique
/// id so the GPU side re-uploads exactly when the image changes, even across the
/// two stages that share one [`RevealRenderer`]. `0` means "no source yet".
static SOURCE_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_source_id() -> u64 {
    SOURCE_SEQ.fetch_add(1, Ordering::Relaxed)
}

// ── CPU state machine ─────────────────────────────────────────────────────────

/// The three phases of the reveal, a small state machine over the generation
/// signals. First-frame generations skip [`Cohere`](RevealPhase::Cohere) (they
/// stream no partial) and go straight `Scatter -> Snap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RevealPhase {
    /// Generation in flight, no image yet: a scattered, drifting cloud.
    Scatter,
    /// A streamed partial arrived: the cloud settles partway and the partial
    /// shows through.
    Cohere,
    /// The final landed: particles converge to their cells and the image snaps
    /// in, then the effect deactivates.
    Snap,
}

/// A decoded RGBA source image fed to the effect (a streamed partial or the
/// final result). `rgba` is tightly packed `width * height * 4` bytes; `id` is a
/// monotonic token so the renderer re-uploads only when the image changes.
#[derive(Clone)]
pub(crate) struct RevealSource {
    id: u64,
    rgba: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

/// Per-stage reveal animation state. Advanced from generation signals and ticked
/// each frame; `assembly`/`reveal` are pure functions of the phase and elapsed
/// time, so the transitions are unit-testable without a frame loop.
pub(crate) struct RevealState {
    active: bool,
    phase: RevealPhase,
    /// Target output size, for the grid aspect.
    target: (u32, u32),
    source: Option<RevealSource>,
    /// Time the current phase was entered (seconds, egui's input clock).
    phase_started: f64,
    /// `assembly`/`reveal` at the moment the current phase began, so an ease
    /// starts from wherever the previous phase left off.
    assembly_at_entry: f32,
    reveal_at_entry: f32,
    assembly: f32,
    reveal: f32,
}

impl Default for RevealState {
    fn default() -> Self {
        Self {
            active: false,
            phase: RevealPhase::Scatter,
            target: (1, 1),
            source: None,
            phase_started: 0.0,
            assembly_at_entry: SCATTER_ASSEMBLY,
            reveal_at_entry: 0.0,
            assembly: SCATTER_ASSEMBLY,
            reveal: 0.0,
        }
    }
}

impl RevealState {
    /// Starts the effect for a fresh generation at the given target size: phase
    /// [`Scatter`](RevealPhase::Scatter), no source yet.
    pub(crate) fn begin(&mut self, target: (u32, u32), now: f64) {
        self.active = true;
        self.phase = RevealPhase::Scatter;
        self.target = (target.0.max(1), target.1.max(1));
        self.source = None;
        self.phase_started = now;
        self.assembly_at_entry = SCATTER_ASSEMBLY;
        self.reveal_at_entry = 0.0;
        self.assembly = SCATTER_ASSEMBLY;
        self.reveal = 0.0;
    }

    /// Feeds a streamed partial preview. Moves to [`Cohere`](RevealPhase::Cohere)
    /// the first time; later partials just refresh the source. Ignored when the
    /// effect is inactive or already snapping.
    pub(crate) fn on_partial(&mut self, rgba: Vec<u8>, width: u32, height: u32, now: f64) {
        if !self.active || self.phase == RevealPhase::Snap {
            return;
        }
        self.set_source(rgba, width, height);
        if self.phase == RevealPhase::Scatter {
            self.enter(RevealPhase::Cohere, now);
        }
    }

    /// Feeds the final image and starts the snap. Ignored when inactive.
    pub(crate) fn on_final(&mut self, rgba: Vec<u8>, width: u32, height: u32, now: f64) {
        if !self.active {
            return;
        }
        self.set_source(rgba, width, height);
        self.enter(RevealPhase::Snap, now);
    }

    /// Starts the snap without a new source — used when the final landed but its
    /// image could not be decoded, so the effect still completes and hands back
    /// to the static viewport (which shows the real result) instead of stranding
    /// on the scatter cloud. Ignored when inactive.
    pub(crate) fn finish(&mut self, now: f64) {
        if !self.active {
            return;
        }
        self.enter(RevealPhase::Snap, now);
    }

    /// Drops the effect (a generation failed or was canceled), so the surface
    /// falls back to its static state.
    pub(crate) fn fail(&mut self) {
        self.active = false;
    }

    /// Advances `assembly`/`reveal` for `now`, deactivating once the snap
    /// completes. Idempotent within a frame.
    pub(crate) fn tick(&mut self, now: f64) {
        if !self.active {
            return;
        }
        let elapsed = (now - self.phase_started).max(0.0);
        match self.phase {
            RevealPhase::Scatter => {
                self.assembly = SCATTER_ASSEMBLY;
                self.reveal = 0.0;
            }
            RevealPhase::Cohere => {
                let f = ease((elapsed / COHERE_SECS) as f32);
                self.assembly = lerp(self.assembly_at_entry, COHERE_ASSEMBLY, f);
                self.reveal = lerp(self.reveal_at_entry, COHERE_REVEAL, f);
            }
            RevealPhase::Snap => {
                let f = ease((elapsed / SNAP_SECS) as f32);
                self.assembly = lerp(self.assembly_at_entry, 1.0, f);
                self.reveal = lerp(self.reveal_at_entry, 1.0, f);
                if elapsed >= SNAP_SECS {
                    self.active = false;
                }
            }
        }
    }

    /// Whether the effect should render this frame.
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    /// The current phase (for tests and inspection).
    #[cfg(test)]
    pub(crate) fn phase(&self) -> RevealPhase {
        self.phase
    }

    fn set_source(&mut self, rgba: Vec<u8>, width: u32, height: u32) {
        self.source = Some(RevealSource {
            id: next_source_id(),
            rgba: Arc::new(rgba),
            width: width.max(1),
            height: height.max(1),
        });
    }

    fn enter(&mut self, phase: RevealPhase, now: f64) {
        self.assembly_at_entry = self.assembly;
        self.reveal_at_entry = self.reveal;
        self.phase = phase;
        self.phase_started = now;
    }

    /// Builds the per-frame paint command from the current state, the
    /// aspect-fit transform, and the animation clock.
    fn callback(&self, fit: [f32; 2], time: f32) -> RevealCallback {
        let (gw, gh) = reveal_grid(self.target.0, self.target.1);
        RevealCallback {
            fit,
            grid: [gw, gh],
            assembly: self.assembly,
            reveal: self.reveal,
            time,
            source: self.source.clone(),
        }
    }
}

/// Renders the reveal effect into the current UI region when `reveal` is active,
/// returning `true` when it painted (the caller then skips the static viewport).
/// Ticks the state first, so a just-finished snap returns `false` and the static
/// result shows the same frame. With no wgpu (`gpu` false) it ticks but does not
/// paint, so the static surface still appears.
pub(crate) fn render_reveal(reveal: &mut RevealState, ui: &mut egui::Ui, gpu: bool) -> bool {
    if !reveal.is_active() {
        return false;
    }
    let now = ui.input(|i| i.time);
    reveal.tick(now);
    if !reveal.is_active() || !gpu {
        return false;
    }

    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
    let rect = response.rect;
    let ppp = ui.ctx().pixels_per_point();
    let viewport_px = [rect.width() * ppp, rect.height() * ppp];
    if viewport_px[0] < 1.0 || viewport_px[1] < 1.0 {
        return true;
    }
    let fit = reveal_fit(viewport_px, reveal.target);
    #[allow(clippy::cast_possible_truncation)]
    let time = now as f32;
    painter.add(egui_wgpu::Callback::new_paint_callback(rect, reveal.callback(fit, time)));
    // The effect animates between frames; keep repainting while it runs.
    ui.ctx().request_repaint();
    true
}

/// The particle-grid dimensions for a target size: capped at [`GRID_LONG_MAX`]
/// on the long edge, the short edge scaled to the aspect (at least one cell).
/// Falls below the cap when the target is smaller than the cap (a tiny sprite
/// uses its own count).
#[must_use]
pub(crate) fn reveal_grid(width: u32, height: u32) -> (u32, u32) {
    let (w, h) = (width.max(1), height.max(1));
    let long = w.max(h);
    let g = long.min(GRID_LONG_MAX);
    if w >= h { (g, scale_short(g, h, w)) } else { (scale_short(g, w, h), g) }
}

/// Scales the short-edge cell count to the aspect `short / long`, clamped to
/// at least one cell.
fn scale_short(long_cells: u32, short_px: u32, long_px: u32) -> u32 {
    let cells = (u64::from(long_cells) * u64::from(short_px) + u64::from(long_px) / 2) / u64::from(long_px.max(1));
    #[allow(clippy::cast_possible_truncation)]
    let cells = cells as u32;
    cells.max(1)
}

/// The half-extents, in clip space, of the largest box of the target's aspect
/// that fits inside the viewport (a contain fit, centred). One axis is `1.0`
/// (touches the viewport edge), the other is `<= 1.0` (letterboxed).
#[must_use]
pub(crate) fn reveal_fit(viewport_px: [f32; 2], target: (u32, u32)) -> [f32; 2] {
    let (vw, vh) = (viewport_px[0].max(1.0), viewport_px[1].max(1.0));
    #[allow(clippy::cast_precision_loss)]
    let target_aspect = target.0.max(1) as f32 / target.1.max(1) as f32;
    let viewport_aspect = vw / vh;
    if target_aspect > viewport_aspect {
        // Wider than the viewport: width-limited, letterbox top/bottom.
        [1.0, viewport_aspect / target_aspect]
    } else {
        // Taller than (or equal to) the viewport: height-limited, pillarbox.
        [target_aspect / viewport_aspect, 1.0]
    }
}

/// Smoothstep ease over `[0, 1]`, clamped.
fn ease(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Decodes PNG bytes into tightly packed RGBA8 plus dimensions, or `None` if the
/// bytes don't decode. Used to feed a final image into the effect.
#[must_use]
pub(crate) fn decode_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    Some((img.into_raw(), w, h))
}

// ── GPU renderer ──────────────────────────────────────────────────────────────

/// Uniform block shared by the reveal vertex and fragment stages. Field order
/// and padding match `reveal.wgsl`'s `Uniforms`; total size 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Uniforms {
    fit: [f32; 2],
    grid: [f32; 2],
    assembly: f32,
    reveal: f32,
    time: f32,
    has_source: f32,
}

/// The source texture plus the bind group referencing it. Recreated only when
/// the source image's dimensions change; otherwise overwritten in place — the
/// retained `texture` handle is what [`RevealRenderer::ensure_source`] writes to.
struct SourceTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    /// Id of the image currently uploaded (`0` = the 1x1 placeholder).
    id: u64,
}

/// Owns the REVEAL program's GPU resources and the current source texture. Lives
/// in egui-wgpu's `CallbackResources`, built once at startup and shared by both
/// stages' callbacks (only one paints per frame).
pub struct RevealRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buf: wgpu::Buffer,
    source: SourceTexture,
}

impl RevealRenderer {
    /// Builds the REVEAL pipeline and a 1x1 placeholder source. Build the color
    /// target against `target_format` (egui-wgpu's surface format).
    #[must_use]
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("reveal.bgl"),
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
            label: Some("reveal.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("reveal.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("reveal.pipeline"),
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
                    // Particles blend over the panel background as a translucent
                    // cloud; straight-alpha over.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("reveal.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            // Linear so a coarse partial reads as a soft, forming image.
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("reveal.uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let source = Self::make_source(device, &bind_group_layout, &uniform_buf, &sampler, 1, 1);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buf,
            source,
        }
    }

    /// Writes the per-frame uniforms.
    pub fn write_uniforms(&self, queue: &wgpu::Queue, fit: [f32; 2], grid: [f32; 2], assembly: f32, reveal: f32, time: f32, has_source: f32) {
        let uniforms = Uniforms {
            fit,
            grid,
            assembly,
            reveal,
            time,
            has_source,
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Uploads `rgba` (tight RGBA8, `width * height * 4` bytes) as the source
    /// image when `id` differs from what is already loaded. Recreates the texture
    /// only when the dimensions change; otherwise overwrites in place. A no-op
    /// when `id` matches the current source, so it can be called every frame.
    pub fn ensure_source(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, id: u64, rgba: &[u8], width: u32, height: u32) {
        if id == self.source.id || width == 0 || height == 0 {
            return;
        }
        if self.source.width != width || self.source.height != height {
            self.source = Self::make_source(device, &self.bind_group_layout, &self.uniform_buf, &self.sampler, width, height);
        }
        let texture_size = (width as usize) * (height as usize) * 4;
        if rgba.len() >= texture_size {
            upload_rgba(queue, &self.source.texture, &rgba[..texture_size], width, height);
        }
        self.source.id = id;
    }

    /// Issues the reveal draw: `instance_count` cells, six verts each. Call only
    /// after [`Self::write_uniforms`] for this frame.
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>, instance_count: u32) {
        if instance_count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.source.bind_group, &[]);
        render_pass.draw(0..6, 0..instance_count);
    }

    /// Creates a `width x height` source texture (zero-filled) and its bind
    /// group. The texture is overwritten by [`Self::ensure_source`] before it is
    /// ever sampled with `has_source > 0`.
    fn make_source(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> SourceTexture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("reveal.source"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("reveal.bind_group"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        SourceTexture {
            texture,
            bind_group,
            width,
            height,
            id: 0,
        }
    }
}

/// Writes tight RGBA8 into a texture's full extent.
fn upload_rgba(queue: &wgpu::Queue, texture: &wgpu::Texture, rgba: &[u8], width: u32, height: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
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

// ── Paint callback ──────────────────────────────────────────────────────────

/// Per-frame paint command for the REVEAL program. Cheap to build; holds the
/// uniforms and a (refcounted) source handle, never GPU handles.
struct RevealCallback {
    fit: [f32; 2],
    grid: [u32; 2],
    assembly: f32,
    reveal: f32,
    time: f32,
    source: Option<RevealSource>,
}

impl egui_wgpu::CallbackTrait for RevealCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<RevealRenderer>() {
            if let Some(src) = &self.source {
                renderer.ensure_source(device, queue, src.id, &src.rgba, src.width, src.height);
            }
            let has_source = f32::from(u8::from(self.source.is_some()));
            #[allow(clippy::cast_precision_loss)]
            let grid = [self.grid[0] as f32, self.grid[1] as f32];
            renderer.write_uniforms(queue, self.fit, grid, self.assembly, self.reveal, self.time, has_source);
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::epaint::PaintCallbackInfo, render_pass: &mut wgpu::RenderPass<'static>, resources: &egui_wgpu::CallbackResources) {
        if let Some(renderer) = resources.get::<RevealRenderer>() {
            renderer.paint(render_pass, self.grid[0] * self.grid[1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_caps_the_long_edge_and_scales_the_short() {
        // Square caps to 128x128.
        assert_eq!(reveal_grid(1536, 1536), (128, 128));
        // 4:3 landscape: long edge 128, short edge 96.
        assert_eq!(reveal_grid(1536, 1152), (128, 96));
        // 3:4 portrait mirrors it.
        assert_eq!(reveal_grid(1152, 1536), (96, 128));
        // A tiny sprite uses its own (smaller) count, never upscaled past it.
        assert_eq!(reveal_grid(32, 32), (32, 32));
        assert_eq!(reveal_grid(48, 24), (48, 24));
        // Degenerate sizes never produce a zero dimension.
        assert_eq!(reveal_grid(0, 0), (1, 1));
        assert_eq!(reveal_grid(256, 1), (128, 1));
    }

    #[test]
    fn fit_contains_the_target_aspect_centred() {
        // Square target in a wide viewport: pillarboxed (x < 1, y == 1).
        let f = reveal_fit([200.0, 100.0], (512, 512));
        assert!((f[1] - 1.0).abs() < 1e-6);
        assert!((f[0] - 0.5).abs() < 1e-6);
        // Square target in a tall viewport: letterboxed (x == 1, y < 1).
        let f = reveal_fit([100.0, 200.0], (512, 512));
        assert!((f[0] - 1.0).abs() < 1e-6);
        assert!((f[1] - 0.5).abs() < 1e-6);
        // Matching aspect fills both axes.
        let f = reveal_fit([160.0, 90.0], (1920, 1080));
        assert!((f[0] - 1.0).abs() < 1e-4 && (f[1] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn ease_endpoints_and_midpoint() {
        assert!(ease(-1.0).abs() < 1e-6);
        assert!(ease(0.0).abs() < 1e-6);
        assert!((ease(1.0) - 1.0).abs() < 1e-6);
        assert!((ease(2.0) - 1.0).abs() < 1e-6);
        assert!((ease(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn anchor_flow_scatter_cohere_snap() {
        let mut r = RevealState::default();
        assert!(!r.is_active());
        r.begin((1024, 1024), 0.0);
        assert!(r.is_active());
        assert_eq!(r.phase(), RevealPhase::Scatter);
        r.tick(0.1);
        assert!(r.reveal.abs() < 1e-6, "no reveal while scattering");

        // A partial moves to Cohere and eases the source in.
        r.on_partial(vec![0; 4], 1, 1, 1.0);
        assert_eq!(r.phase(), RevealPhase::Cohere);
        r.tick(1.0 + COHERE_SECS);
        assert!((r.reveal - COHERE_REVEAL).abs() < 1e-3, "cohere settles toward its target reveal");
        assert!((r.assembly - COHERE_ASSEMBLY).abs() < 1e-3);

        // The final snaps assembly and reveal to 1, then deactivates.
        r.on_final(vec![0; 4], 1, 1, 5.0);
        assert_eq!(r.phase(), RevealPhase::Snap);
        r.tick(5.0 + SNAP_SECS / 2.0);
        assert!(r.is_active() && r.reveal < 1.0, "mid-snap still animating");
        // A hair past the duration (f64 rounding can leave `now - started` just
        // under SNAP_SECS at the exact boundary).
        r.tick(5.0 + SNAP_SECS + 0.01);
        assert!(!r.is_active(), "snap completes and deactivates");
        assert!((r.reveal - 1.0).abs() < 1e-6 && (r.assembly - 1.0).abs() < 1e-6);
    }

    #[test]
    fn first_frame_flow_scatter_to_snap_without_cohere() {
        let mut r = RevealState::default();
        r.begin((64, 64), 0.0);
        // No partial ever arrives; the final snaps straight from Scatter.
        r.on_final(vec![0; 4], 1, 1, 2.0);
        assert_eq!(r.phase(), RevealPhase::Snap);
        r.tick(2.0 + SNAP_SECS);
        assert!(!r.is_active());
    }

    #[test]
    fn finish_snaps_without_a_source_so_the_effect_never_strands() {
        // The final landed but its image could not be decoded: the effect must
        // still snap and deactivate, handing back to the static viewport.
        let mut r = RevealState::default();
        r.begin((100, 100), 0.0);
        r.finish(1.0);
        assert_eq!(r.phase(), RevealPhase::Snap);
        r.tick(1.0 + SNAP_SECS + 0.01);
        assert!(!r.is_active());
    }

    #[test]
    fn fail_deactivates_and_partial_is_ignored_after() {
        let mut r = RevealState::default();
        r.begin((128, 128), 0.0);
        r.fail();
        assert!(!r.is_active());
        // Signals after a fail are ignored (no resurrection).
        r.on_partial(vec![0; 4], 1, 1, 1.0);
        assert!(!r.is_active());
        r.on_final(vec![0; 4], 1, 1, 2.0);
        assert!(!r.is_active());
    }

    #[test]
    fn source_ids_are_unique_and_monotonic() {
        let mut r = RevealState::default();
        r.begin((64, 64), 0.0);
        r.on_partial(vec![1, 2, 3, 4], 1, 1, 0.1);
        let first = r.source.as_ref().expect("partial set a source").id;
        r.on_partial(vec![5, 6, 7, 8], 1, 1, 0.2);
        let second = r.source.as_ref().expect("partial replaced the source").id;
        assert!(second > first, "each source gets a fresh id");
    }

    #[test]
    fn decode_png_rgba_round_trips_dimensions() {
        let img = image::RgbaImage::from_pixel(5, 3, image::Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode test png");
        let (rgba, w, h) = decode_png_rgba(&bytes).expect("decode");
        assert_eq!((w, h), (5, 3));
        assert_eq!(rgba.len(), 5 * 3 * 4);
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn decode_png_rgba_rejects_garbage() {
        assert!(decode_png_rgba(&[0, 1, 2, 3]).is_none());
    }

    /// Headless device, or `None` when this machine has no usable adapter (CI
    /// without a GPU) — callers skip rather than fail.
    fn headless() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        Some((device, queue))
    }

    /// Builds the pipeline (compiles `reveal.wgsl`), uploads a solid-red source,
    /// and renders the settled effect into a square offscreen target. The centre
    /// must come back red — proves the shader, layout, source upload, and draw
    /// all line up.
    #[test]
    fn settled_effect_paints_the_source() {
        let Some((device, queue)) = headless() else {
            // No usable adapter (CI without a GPU): skip rather than fail.
            return;
        };

        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = RevealRenderer::new(&device, format);

        // A 2x2 solid-red source, uploaded under a fresh id.
        let red: Vec<u8> = (0..2 * 2).flat_map(|_| [255u8, 0, 0, 255]).collect();
        renderer.ensure_source(&device, &queue, 1, &red, 2, 2);
        // Square target -> fit fills the viewport; fully settled and revealed.
        let (gw, gh) = reveal_grid(64, 64);
        renderer.write_uniforms(&queue, [1.0, 1.0], [gw as f32, gh as f32], 1.0, 1.0, 0.0, 1.0);

        const SIZE: u32 = 64;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("reveal.test.target"),
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

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("reveal.test.pass"),
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
            renderer.paint(&mut pass, gw * gh);
        }

        let bytes_per_row = SIZE * 4;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("reveal.test.readback"),
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
            .expect("poll readback");
        let data = slice.get_mapped_range();

        // Centre pixel sits inside a settled, fully revealed red cell.
        let centre = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
        assert!(data[centre] > 200, "centre red channel high (got {})", data[centre]);
        assert!(data[centre + 1] < 64, "centre green channel low (got {})", data[centre + 1]);
    }
}
