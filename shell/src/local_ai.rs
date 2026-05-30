//! Local-model UI plumbing — the download manager and the settings actions.
//!
//! The off-thread spawns model the shapes the rest of the shell already uses:
//! `spawn_model_download` mirrors `ai::spawn_clip` (a cancel token owned by the
//! [`ShellApp`] plus a progress closure that sends a `ShellMsg` and repaints),
//! and [`spawn_model_probe`] mirrors `ai::spawn_backend_key_op`'s blocking shape
//! (a presence check off the UI thread). The editor-mode Generate actions land
//! in a later phase; this module owns the settings-side plumbing.
//!
//! Everything that names `pixhaus_flux` / the on-device backend is gated behind
//! `local-flux`. With the feature off, the download spawn reports an immediate
//! failure and the probe reports "not downloaded", so the settings card degrades
//! to a disabled state without a separate code path in the UI.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use eframe::egui;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{Cel, Layer, LayerId, PixelBufferId};
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use crate::app::{JobStatus, LocalGenMode, LocalGenPanel, LocalModelStatus, ShellApp, ShellMsg};
use crate::commands::{PixelRegionEdit, push_sprite_edit_with_buffers};

impl ShellApp {
    /// Re-runs local-backend registration after a download completes, the cache
    /// directory changes, or the weights are deleted. Presence-only and instant —
    /// it never loads the multi-GB model — so it is safe on the UI thread.
    pub(crate) fn refresh_local_backend(&mut self) {
        let ready = crate::ai::try_register_local_flux(&self.verb_runtime, self.local_ai.device, self.local_ai.cache_dir.clone());
        // Local registration can flip overall readiness on (offline, no cloud key)
        // or, after a delete, the local backend simply stops being registered;
        // either way fold it into the status-bar chip.
        if ready {
            self.backend_ready = true;
        }
    }

    /// Starts a model download: creates a fresh cancel token (owned here so the
    /// Cancel button can abort), flips the card to `Downloading`, and spawns the
    /// streaming transfer off-thread.
    ///
    /// Driven only by the `local-flux` settings card; gated so a feature-off build
    /// carries no dead download driver.
    #[cfg(feature = "local-flux")]
    pub(crate) fn start_model_download(&mut self) {
        let cancel = CancellationToken::new();
        self.local_ai_cancel = Some(cancel.clone());
        self.local_ai_status = LocalModelStatus::Downloading {
            fraction: 0.0,
            bytes: 0,
            total: 0,
        };
        spawn_model_download(
            self.runtime.handle(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            self.local_ai.default_model.clone(),
            self.local_ai.cache_dir.clone(),
            cancel,
        );
    }

    /// Cancels an in-flight model download. The streaming task sees the token and
    /// reports [`ShellMsg::ModelDownloadFailed`] (cancelled), which clears the
    /// token and flips the card back to a retry state.
    #[cfg(feature = "local-flux")]
    pub(crate) fn cancel_model_download(&mut self) {
        if let Some(cancel) = self.local_ai_cancel.take() {
            cancel.cancel();
        }
    }

    /// Deletes the downloaded weights off the UI thread, then re-probes presence
    /// and re-runs registration so the card and the backend chip update. A
    /// missing cache directory is treated as already deleted.
    #[cfg(feature = "local-flux")]
    pub(crate) fn delete_model(&mut self) {
        let Some(root) = self.local_model_cache_root() else {
            self.set_status("No cache directory to delete.");
            return;
        };
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let model = self.local_ai.default_model.clone();
        let cache_dir = self.local_ai.cache_dir.clone();
        let probe_handle = self.runtime.handle().clone();
        self.runtime.handle().spawn_blocking(move || {
            // Best-effort recursive delete; a not-found dir is success. Any other
            // error surfaces as a failed status so the user knows the weights are
            // still on disk.
            let result = match std::fs::remove_dir_all(&root) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err.to_string()),
            };
            match result {
                Ok(()) => {
                    // Re-probe so the card flips to NotDownloaded.
                    spawn_model_probe(&probe_handle, ctx.clone(), tx.clone(), model.clone(), cache_dir);
                }
                Err(error) => {
                    let _ = tx.send(ShellMsg::ModelDownloadFailed { model, error });
                }
            }
            ctx.request_repaint();
        });
    }

    /// The cache root the local model lives under, honoring a settings override
    /// and falling back to the app-data default. `None` when no app-data
    /// directory can be resolved and no override is set.
    #[cfg(feature = "local-flux")]
    pub(crate) fn local_model_cache_root(&self) -> Option<PathBuf> {
        match &self.local_ai.cache_dir {
            Some(dir) => Some(dir.join(&self.local_ai.default_model)),
            None => default_model_cache_root(&self.local_ai.default_model),
        }
    }

    // ── Editor-mode Generate actions (spec section 7) ──────────────────────

    /// The reason a Generate menu item is disabled, for `on_disabled_hover_text`.
    /// Order matters: the most actionable fix first. The CTA to the download
    /// settings lives on the menu's own "Open download settings" item.
    pub(crate) fn local_gen_disabled_reason(&self, has_sprite: bool) -> &'static str {
        if !cfg!(feature = "local-flux") {
            "This build was compiled without on-device generation"
        } else if !matches!(self.local_ai_status, LocalModelStatus::Ready) {
            "Download the FLUX.2 model in Settings -> AI first"
        } else if !has_sprite {
            "Open or create a sprite first"
        } else {
            // Ready and a sprite is active: the item is enabled, so this is unused.
            "Ready"
        }
    }

    /// Opens the Generate popover for `mode`, clearing any stale status. A no-op
    /// when the backend is not ready — the menu items are disabled in that case,
    /// so this only ever runs when a generation can actually start.
    pub(crate) fn open_local_gen(&mut self, mode: LocalGenMode) {
        if !self.local_ai_ready() {
            return;
        }
        self.local_gen = Some(LocalGenPanel::for_mode(mode));
        self.local_gen_status = JobStatus::Idle;
    }

    /// Renders the Generate popover: the mode title, a multiline prompt, the
    /// strength slider (image-to-image / inpaint), the new-layer toggle
    /// (image-to-image), the Generate button gated on ready and not-running, and
    /// the spinner + label status block. Closing the window drops the panel.
    pub(crate) fn show_local_gen_panel(&mut self, ctx: &egui::Context) {
        // Read the mode out, dropping the immutable borrow before the closure
        // mutably borrows `self.local_gen` again.
        let Some(mode) = self.local_gen.as_ref().map(|p| p.mode) else {
            return;
        };
        let title = mode.title();
        let running = matches!(self.local_gen_status, JobStatus::Running(_));
        let palette = crate::theme::Palette::for_theme(ctx.theme());

        let mut open = true;
        let mut generate = false;
        let mut cancel = false;
        egui::Window::new(format!("{} {title}", crate::icons::SPARKLE))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                // Borrow the panel mutably only inside the closure so the window
                // chrome (open flag) is free.
                if let Some(panel) = self.local_gen.as_mut() {
                    ui.label(egui::RichText::new("Runs on this machine. 4 distilled steps.").small().weak());
                    ui.add_space(4.0);
                    ui.label("Prompt");
                    ui.add(egui::TextEdit::multiline(&mut panel.prompt).desired_rows(3).hint_text("describe the result"));

                    if matches!(mode, LocalGenMode::ImageToImage | LocalGenMode::Inpaint) {
                        ui.add(egui::Slider::new(&mut panel.strength, 0.0..=1.0).text("strength").fixed_decimals(2))
                            .on_hover_text("How far the result departs from the source (0 = keep, 1 = redraw)");
                    }

                    if matches!(mode, LocalGenMode::ImageToImage) {
                        ui.checkbox(&mut panel.as_new_layer, "Add as new layer")
                            .on_hover_text("On: land above the current layer. Off: replace the canvas pixels.");
                    }
                }

                ui.add_space(6.0);
                let can_generate = self.local_ai_ready() && !running;
                let button = egui::Button::new(egui::RichText::new(format!("{}  Generate", crate::icons::SPARKLE)).color(palette.text_on_accent))
                    .fill(palette.accent)
                    .min_size(egui::vec2(ui.available_width(), 30.0));
                if ui
                    .add_enabled(can_generate, button)
                    .on_disabled_hover_text(if running {
                        "A generation is already running"
                    } else {
                        "The on-device model is not ready"
                    })
                    .clicked()
                {
                    generate = true;
                }

                match &self.local_gen_status {
                    JobStatus::Running(msg) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(msg.clone());
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    }
                    JobStatus::Failed(err) => {
                        ui.colored_label(palette.error, format!("failed: {err}"));
                    }
                    JobStatus::Idle => {}
                }
            });

        if cancel {
            self.cancel_local_gen();
        }
        if generate {
            self.start_local_gen();
        }
        if !open {
            self.local_gen = None;
        }
    }

    /// Cancels an in-flight editor-mode generation: bumps the epoch so a late
    /// result is dropped and fires the shared cancel token, then clears the
    /// running status. The clip path's cancel token is reused.
    pub(crate) fn cancel_local_gen(&mut self) {
        if let Some(cancel) = self.local_ai_cancel.take() {
            cancel.cancel();
        }
        // A bumped epoch drops any result that lands after this point.
        self.local_gen_epoch = self.local_gen_epoch.wrapping_add(1);
        self.local_gen_status = JobStatus::Idle;
    }

    /// Builds the [`crate::ai::LocalImageJob`] from the popover and the active
    /// document, bumps the epoch, and spawns the generation. Captures the init
    /// image, the selection mask, and the landing parameters now, so a later
    /// popover edit cannot change where an in-flight result lands.
    pub(crate) fn start_local_gen(&mut self) {
        let Some(panel) = self.local_gen.as_ref() else {
            return;
        };
        let Some(sprite) = self.doc.active_sprite() else {
            self.local_gen_status = JobStatus::Failed("no active sprite".to_owned());
            return;
        };
        let canvas = (sprite.canvas.width, sprite.canvas.height);
        let mode = panel.mode;
        let prompt = panel.prompt.clone();
        let strength = panel.strength;
        let as_new_layer = panel.as_new_layer;

        // Per-mode inputs: the init image (active layer cel buffer, else the
        // composite), the inpaint mask, and the landing region.
        let (init_png, mask_png, region) = match mode {
            LocalGenMode::TextToImage => (None, None, None),
            LocalGenMode::ImageToImage => {
                let Some(png) = self.local_gen_source_png() else {
                    self.local_gen_status = JobStatus::Failed("could not read the source image".to_owned());
                    return;
                };
                (Some(png), None, None)
            }
            LocalGenMode::Inpaint => {
                let Some(mask) = self.editor.selection.as_ref() else {
                    self.local_gen_status = JobStatus::Failed("make a selection first".to_owned());
                    return;
                };
                let bounds = mask.bounds();
                if bounds.is_empty() {
                    self.local_gen_status = JobStatus::Failed("the selection is empty".to_owned());
                    return;
                }
                let Some(mask_png) = selection_mask_to_png(mask) else {
                    self.local_gen_status = JobStatus::Failed("could not encode the selection mask".to_owned());
                    return;
                };
                let Some(png) = self.local_gen_source_png() else {
                    self.local_gen_status = JobStatus::Failed("could not read the source image".to_owned());
                    return;
                };
                #[allow(clippy::cast_sign_loss)]
                let region = (
                    bounds.origin.x.max(0) as u32,
                    bounds.origin.y.max(0) as u32,
                    bounds.size.width,
                    bounds.size.height,
                );
                (Some(png), Some(mask_png), Some(region))
            }
        };

        // A fresh epoch + cancel token, mirroring the clip path's lifecycle.
        self.local_gen_epoch = self.local_gen_epoch.wrapping_add(1);
        let cancel = CancellationToken::new();
        self.local_ai_cancel = Some(cancel.clone());
        self.local_gen_status = JobStatus::Running("starting".to_owned());

        // Mint a fresh seed so the run is reproducible and a real seed lands in
        // the AI-lineage record. Image-to-image / inpaint seed the latent off the
        // init image, so the backend ignores it there; it is still recorded.
        let seed = mint_seed();

        let job = crate::ai::LocalImageJob {
            mode,
            prompt,
            canvas,
            init_png,
            mask_png,
            strength,
            as_new_layer,
            region,
            seed,
        };
        crate::ai::spawn_local_image(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
            cancel,
            self.local_gen_epoch,
        );
    }

    /// The init image for image-to-image / inpaint: the active layer's cel buffer
    /// when one exists, else the cached `display_frame` composite, else a fresh
    /// composite. Encoded to PNG. `None` only when nothing composites.
    fn local_gen_source_png(&self) -> Option<Vec<u8>> {
        if let Some(id) = self.doc.active_buffer_id() {
            if let Some(buf) = self.doc.pixel_buffers.get(&id) {
                return buffer_to_png(buf);
            }
        }
        if let Some(buf) = self.display_frame.as_ref() {
            return buffer_to_png(buf);
        }
        self.doc.composite_active_frame().as_ref().and_then(buffer_to_png)
    }

    /// Lands one editor-mode generation result as exactly one undo entry, keyed by
    /// `mode`:
    ///
    /// - text-to-image -> a new `Layer::raster` + `Cel::raster` + buffer;
    /// - image-to-image -> a new layer (when `as_new_layer`) or a full-canvas
    ///   `PixelRegionEdit` replace;
    /// - inpaint -> a `PixelRegionEdit` over the selection `region`.
    ///
    /// On a decode failure the status is set so the popover surfaces it; on
    /// success the popover closes, the prompt / backend / model / seed are
    /// recorded in the project's AI lineage (the spec's recorded provenance
    /// decision: no watermark, but the lineage is logged as the Create studio
    /// already does for cloud generations), and the status returns to idle.
    pub(crate) fn land_local_gen(&mut self, mode: LocalGenMode, png: &[u8], as_new_layer: bool, region: Option<(u32, u32, u32, u32)>, prompt: &str, seed: u64) {
        let Some(buffer) = crate::app::png_to_pixel_buffer(png) else {
            self.local_gen_status = JobStatus::Failed("could not decode the generated image".to_owned());
            return;
        };
        let landed = match mode {
            LocalGenMode::TextToImage => self.land_local_gen_layer(buffer, "Generate image"),
            LocalGenMode::ImageToImage => {
                if as_new_layer {
                    self.land_local_gen_layer(buffer, "Image to image")
                } else {
                    self.land_local_gen_replace(&buffer, "Image to image")
                }
            }
            LocalGenMode::Inpaint => match region {
                Some((x, y, w, h)) => self.land_local_gen_region(&buffer, x, y, w, h, "Inpaint selection"),
                None => false,
            },
        };
        if landed {
            self.record_local_gen_lineage(mode, prompt, seed);
            self.local_gen_status = JobStatus::Idle;
            self.local_gen = None;
            self.local_ai_cancel = None;
            self.refresh_canvas(true);
        } else {
            self.local_gen_status = JobStatus::Failed("could not land the result on the canvas".to_owned());
        }
    }

    /// Records the prompt / backend / model / seed of an editor-mode local
    /// generation in the project's AI lineage. The watermark is omitted by design
    /// (it would corrupt exact pixel output); the spec instead asks for the same
    /// lineage the Create studio keeps for cloud generations. The
    /// [`PromptHistoryEntry`](pixhaus_core::project::PromptHistoryEntry) schema
    /// holds the prompt, a verb name, and a timestamp, so the backend / model /
    /// mode ride in the verb name (`flux-local:<model>:<mode>`); the seed rides
    /// in the prompt note so it survives a save/reload without a schema change.
    ///
    /// Uses the feature-independent `crate::ai::LOCAL_FLUX_MODEL_ID` so this
    /// compiles and runs the same way with `local-flux` off.
    fn record_local_gen_lineage(&mut self, mode: LocalGenMode, prompt: &str, seed: u64) {
        let mode_tag = match mode {
            LocalGenMode::TextToImage => "text_to_image",
            LocalGenMode::ImageToImage => "image_to_image",
            LocalGenMode::Inpaint => "inpaint",
        };
        let verb_name = format!("flux-local:{}:{mode_tag}", crate::ai::LOCAL_FLUX_MODEL_ID);
        let note = format!("{prompt} [seed {seed}]");
        let timestamp = crate::cockpit::now_secs();
        crate::cockpit::push_prompt_history_for(&mut self.doc.project.library.ai.prompt_history, &verb_name, &note, timestamp);
    }

    /// Lands a generated buffer as a fresh top raster layer + cel, in one undo
    /// entry. Returns false when no sprite is active.
    fn land_local_gen_layer(&mut self, buffer: PixelBuffer, label: &str) -> bool {
        let Some(sprite) = self.doc.active_sprite() else {
            return false;
        };
        let canvas = sprite.canvas;
        let frame = self.doc.active_frame;
        let layer_id = LayerId::new(self.doc.alloc_id());
        let buffer_id = PixelBufferId::new(self.doc.alloc_id());
        push_sprite_edit_with_buffers(&mut self.editor, &mut self.doc, label, vec![(buffer_id, buffer)], |sprite| {
            sprite.layers.push(Layer::raster(layer_id, "Generated"));
            sprite.cels.push(Cel::raster(layer_id, frame, buffer_id, canvas));
        });
        self.doc.active_layer = Some(layer_id);
        true
    }

    /// Replaces the active raster buffer's full canvas with the generated buffer,
    /// recorded as one [`PixelRegionEdit`]. Returns false when there is no active
    /// raster buffer or the sizes do not match.
    fn land_local_gen_replace(&mut self, buffer: &PixelBuffer, label: &str) -> bool {
        let Some(id) = self.doc.active_buffer_id() else {
            return false;
        };
        let (w, h) = (buffer.width(), buffer.height());
        let Some(target) = self.doc.pixel_buffers.get(&id) else {
            return false;
        };
        if target.width() != w || target.height() != h {
            return false;
        }
        let before = crate::commands::extract_region(target, 0, 0, w, h);
        let after = crate::commands::extract_region(buffer, 0, 0, w, h);
        let cmd = PixelRegionEdit {
            buffer_id: id,
            x: 0,
            y: 0,
            w,
            h,
            before,
            after,
            label: label.to_owned(),
        };
        self.editor.history.push(Box::new(cmd), &mut self.doc).is_ok()
    }

    /// Writes the `(x, y, w, h)` region of the generated buffer into the active
    /// raster buffer, recorded as one [`PixelRegionEdit`] — cheaper than a
    /// full-canvas replace for the 8K constraint. Returns false when there is no
    /// active raster buffer, the region falls outside it, or the generated buffer
    /// is smaller than the region.
    fn land_local_gen_region(&mut self, buffer: &PixelBuffer, x: u32, y: u32, w: u32, h: u32, label: &str) -> bool {
        let Some(id) = self.doc.active_buffer_id() else {
            return false;
        };
        let Some(target) = self.doc.pixel_buffers.get(&id) else {
            return false;
        };
        // The generated buffer is canvas-sized; the result region must fit both
        // it and the target.
        if x + w > target.width() || y + h > target.height() || x + w > buffer.width() || y + h > buffer.height() {
            return false;
        }
        let before = crate::commands::extract_region(target, x, y, w, h);
        let after = crate::commands::extract_region(buffer, x, y, w, h);
        let cmd = PixelRegionEdit {
            buffer_id: id,
            x,
            y,
            w,
            h,
            before,
            after,
            label: label.to_owned(),
        };
        self.editor.history.push(Box::new(cmd), &mut self.doc).is_ok()
    }
}

/// Mints a fresh `u64` seed for one editor-mode generation. Dependency-free
/// (no `rand` crate in the shell): an xorshift over the wall clock, mirroring
/// `cockpit::rand_index`. Good enough to vary runs and to record a real seed in
/// the AI lineage — not a cryptographic RNG.
fn mint_seed() -> u64 {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};
    thread_local!(static STATE: Cell<u64> = const { Cell::new(0x2545_f491_4f6c_dd1d) });
    let clock = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    STATE.with(|s| {
        let mut x = s.get() ^ clock;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x
    })
}

/// Encodes a [`PixelBuffer`] to PNG, dropping any row padding. `None` if the
/// rows don't form a tight RGBA image. The local module keeps its own encoder so
/// it depends on no UI-thread studio helper.
fn buffer_to_png(buf: &PixelBuffer) -> Option<Vec<u8>> {
    use std::io::Cursor;
    let (w, h) = (buf.width(), buf.height());
    if w == 0 || h == 0 {
        return None;
    }
    let row_bytes = (w as usize) * 4;
    let mut tight = Vec::with_capacity(row_bytes * h as usize);
    for y in 0..h {
        tight.extend_from_slice(buf.row(y)?.get(..row_bytes)?);
    }
    let img = image::RgbaImage::from_raw(w, h, tight)?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// Rasterizes a [`pixhaus_core::selection::SelectionMask`] to a white-on-black
/// PNG mask: white (255) where the pixel is selected, black where not, full
/// alpha — the `ImageEditRequest::mask` contract (white = repaint). The mask is
/// canvas-sized so it aligns with the canvas-sized init image.
fn selection_mask_to_png(mask: &pixhaus_core::selection::SelectionMask) -> Option<Vec<u8>> {
    use std::io::Cursor;
    let (w, h) = (mask.width(), mask.height());
    if w == 0 || h == 0 {
        return None;
    }
    let mut img = image::RgbaImage::new(w, h);
    for (px, value) in img.pixels_mut().zip(mask.as_bytes().iter()) {
        // Any coverage repaints; treat > 0 as white per the white = repaint rule.
        let v = if *value > 0 { 255 } else { 0 };
        *px = image::Rgba([v, v, v, 255]);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// The app-data model cache root, `<data_dir>/models/<model>`. Mirrors
/// `pixhaus_flux::ModelStore::default_root` without depending on the gated crate,
/// so the settings caption can show a path with the feature off.
#[cfg(feature = "local-flux")]
#[must_use]
pub(crate) fn default_model_cache_root(model: &str) -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("app", "Pixhaus", "Pixhaus")?;
    Some(dirs.data_dir().join("models").join(model))
}

/// Spawns the streaming model download on the tokio runtime. Cumulative byte
/// progress, completion, and failure (including cancellation) arrive over `tx`;
/// `ctx` is woken after each message so the idle settings window repaints.
///
/// The real transfer is gated behind `local-flux` (it pulls in the flux crate's
/// `download` toolbox). With the feature off this reports an immediate failure so
/// the card never appears to hang.
#[cfg(feature = "local-flux")]
pub(crate) fn spawn_model_download(
    handle: &Handle,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    model: String,
    cache_dir: Option<PathBuf>,
    cancel: CancellationToken,
) {
    use pixhaus_ai::backends::local_flux::ModelStore;

    handle.spawn(async move {
        let Some(store) = ModelStore::from_override(cache_dir) else {
            let _ = tx.send(ShellMsg::ModelDownloadFailed {
                model,
                error: "no cache directory could be resolved for this platform".to_owned(),
            });
            ctx.request_repaint();
            return;
        };

        // Read the optional HF token off-thread. A keychain read blocks, so it
        // runs on a blocking worker, never the async reactor.
        let token = tokio::task::spawn_blocking(crate::ai::hf_token).await.unwrap_or(None);

        // Progress closure: forward each cumulative tick as a ShellMsg and wake
        // the UI. Mirrors spawn_clip's progress shape.
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let progress_model = model.clone();
        let progress = move |bytes: u64, total: u64| {
            let fraction = (total > 0).then(|| {
                // total fits well within f32's exact-integer range for a ~16 GB
                // model, so the cast is lossless enough for a progress bar.
                #[allow(clippy::cast_precision_loss)]
                let f = bytes as f32 / total as f32;
                f.clamp(0.0, 1.0)
            });
            let _ = progress_tx.send(ShellMsg::ModelDownloadProgress {
                model: progress_model.clone(),
                fraction,
                bytes,
                total,
                message: "downloading weights".to_owned(),
            });
            progress_ctx.request_repaint();
        };

        let result = store.download(token.as_deref(), progress, &cancel).await;
        let msg = match result {
            Ok(()) => ShellMsg::ModelDownloadDone { model },
            Err(error) => ShellMsg::ModelDownloadFailed {
                model,
                error: error.to_string(),
            },
        };
        let _ = tx.send(msg);
        ctx.request_repaint();
    });
}

/// Spawns a startup presence probe off the UI thread: checks whether the model's
/// weights are present and complete on disk and reports the result over `tx`.
/// Mirrors `spawn_backend_key_op`'s blocking shape — a filesystem walk that must
/// not stall the first paint.
///
/// The real check is gated behind `local-flux` (it consults the flux crate's
/// `ModelStore`). With the feature off it reports "not downloaded", so the card
/// shows the disabled state.
#[cfg(feature = "local-flux")]
pub(crate) fn spawn_model_probe(handle: &Handle, ctx: egui::Context, tx: Sender<ShellMsg>, model: String, cache_dir: Option<PathBuf>) {
    use pixhaus_ai::backends::local_flux::ModelStore;

    handle.spawn_blocking(move || {
        let ready = ModelStore::from_override(cache_dir).is_some_and(|store| store.is_downloaded());
        let _ = tx.send(ShellMsg::LocalModelProbed { model, ready });
        ctx.request_repaint();
    });
}

/// Feature-off stub: the model is never present in a build without on-device
/// generation, so report "not downloaded".
#[cfg(not(feature = "local-flux"))]
pub(crate) fn spawn_model_probe(handle: &Handle, ctx: egui::Context, tx: Sender<ShellMsg>, model: String, _cache_dir: Option<PathBuf>) {
    handle.spawn_blocking(move || {
        let _ = tx.send(ShellMsg::LocalModelProbed { model, ready: false });
        ctx.request_repaint();
    });
}
