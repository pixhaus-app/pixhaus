//! The settings window: a separate OS window hosting the General, Keybinds, and
//! AI-backends tabs.
//!
//! The window is an egui *immediate* viewport (`show_viewport_immediate`) rather
//! than an in-canvas modal, so it gets its own OS window, decorations, and close
//! button. Immediate (not deferred) is the deliberate choice: the viewport UI
//! closure runs synchronously inside [`crate::app::ShellApp::ui`] and borrows
//! `&mut self`, so the tabs mutate app state, the keymap, and the keychain
//! directly — no `Arc<Mutex<…>>` handshake. The X button is detected through the
//! viewport's `close_requested` and clears `settings_open`.

use eframe::egui::{self, Key};

use crate::ai;
use crate::app::ShellApp;
use crate::keymap::{Chord, CommandCategory, CommandId, KeybindPreset};

/// Which settings tab is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    /// Theme and other app-global preferences.
    #[default]
    General,
    /// Keyboard rebinding.
    Keybinds,
    /// Cloud-backend API keys plus the on-device model manager.
    Ai,
    /// GPU adapter selection.
    Graphics,
}

impl ShellApp {
    /// Shows the settings window when `settings_open` is set. Called once per
    /// frame from [`ShellApp::ui`]; a no-op while closed.
    pub(crate) fn show_settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            // Don't carry a half-finished rebind capture into the next opening.
            self.capturing = None;
            return;
        }

        // A separate handle so the closure can borrow `&mut self` without also
        // borrowing the `ctx` argument. The immediate-viewport callback hands us
        // a root `&mut Ui` for the child window (not a `Context`), so the body
        // goes in a `CentralPanel::show_inside`.
        let viewport_ctx = ctx.clone();
        viewport_ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("pixhaus_settings"),
            egui::ViewportBuilder::default()
                .with_title("Pixhaus settings")
                .with_inner_size([560.0, 600.0])
                .with_min_inner_size([420.0, 380.0]),
            |ui, _class| {
                egui::CentralPanel::default().show_inside(ui, |ui| self.settings_body(ui));
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    self.settings_open = false;
                    self.capturing = None;
                }
            },
        );
    }

    /// The tab row and the active tab's body.
    fn settings_body(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.settings_tab, SettingsTab::General, "General");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Keybinds, "Keybinds");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Ai, "AI");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Graphics, "Graphics");
        });
        ui.separator();
        ui.add_space(4.0);
        match self.settings_tab {
            SettingsTab::General => self.general_tab(ui),
            SettingsTab::Keybinds => self.keybinds_tab(ui),
            SettingsTab::Ai => self.ai_tab(ui),
            SettingsTab::Graphics => self.graphics_tab(ui),
        }
    }

    /// Graphics tab: pick the GPU adapter the editor runs on. The choice is
    /// saved and applied on the next launch (eframe creates the device once at
    /// startup), so a change shows a restart hint rather than switching live.
    fn graphics_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("GPU");
        if let Some(active) = &self.active_adapter {
            ui.label(egui::RichText::new(format!("Active: {}", crate::gpu::label(active))).small().weak());
        }
        ui.add_space(6.0);

        if self.available_adapters.len() <= 1 {
            ui.label(egui::RichText::new("Only one GPU is available.").small().weak());
            return;
        }

        // "Automatic" plus one row per adapter. Defer the mutation until after
        // the loop so the click handlers don't borrow `self` while iterating.
        let mut pick: Option<Option<wgpu::AdapterInfo>> = None; // Some(None) = automatic
        let automatic = self.gpu_pref.is_none();
        if ui.radio(automatic, "Automatic (recommended)").clicked() && !automatic {
            pick = Some(None);
        }
        for info in &self.available_adapters {
            let selected = self.gpu_pref.as_ref().is_some_and(|p| p.matches(info));
            if ui.radio(selected, crate::gpu::label(info)).clicked() && !selected {
                pick = Some(Some(info.clone()));
            }
        }
        match pick {
            Some(Some(info)) => self.select_gpu(&info),
            Some(None) => self.clear_gpu_pref(),
            None => {}
        }

        // A restart is pending whenever the saved choice differs from what is
        // actually running.
        let pending = match (&self.gpu_pref, &self.active_adapter) {
            (Some(pref), Some(active)) => !pref.matches(active),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if pending {
            ui.add_space(6.0);
            let palette = crate::theme::Palette::for_theme(ui.ctx().theme());
            ui.label(
                egui::RichText::new("Takes effect on the next launch — restart Pixhaus.")
                    .small()
                    .color(palette.warning),
            );
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(6.0);
        ui.heading("Effects");
        ui.checkbox(&mut self.reveal_effect_enabled, "Generation reveal animation");
        ui.label(
            egui::RichText::new("Play a particle reveal in the studio while images generate. Off shows a static view.")
                .small()
                .weak(),
        );
    }

    /// General tab: the theme control. The home for later app-global prefs
    /// (restore window size, open last project, confirm on quit).
    fn general_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Theme");
        ui.label(egui::RichText::new("Follows the system appearance by default.").small().weak());
        ui.add_space(4.0);
        let mut pref = self.theme_preference;
        ui.radio_value(&mut pref, egui::ThemePreference::System, "System");
        ui.radio_value(&mut pref, egui::ThemePreference::Dark, "Dark");
        ui.radio_value(&mut pref, egui::ThemePreference::Light, "Light");
        if pref != self.theme_preference {
            self.set_theme_preference(ui.ctx(), pref);
        }
    }

    /// AI tab: the cloud backend rows, then the on-device model manager. Scrolls
    /// because the local section's download card and override sliders can outrun
    /// the window's height.
    fn ai_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Cloud backends");
            ui.label(egui::RichText::new("Keys are stored in the OS keychain, never on disk.").small().weak());
            ui.add_space(8.0);
            self.backend_row(ui, "OpenAI", ai::OPENAI_BACKEND_ID, "Generates reference sheets (gpt-image).");
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            self.backend_row(ui, "FAL", ai::FAL_BACKEND_ID, "Reference sheets, plus image-to-video animation.");

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            self.local_model_section(ui);
        });
    }

    /// The on-device model manager: device, cache dir, the download/manage card,
    /// the optional HF token, and the distilled-params override. The real section
    /// is compiled only with `local-flux`; the feature-off stub renders one weak
    /// line so the tab never looks broken.
    #[cfg(feature = "local-flux")]
    fn local_model_section(&mut self, ui: &mut egui::Ui) {
        use crate::ai::DevicePref;
        use crate::app::LocalModelStatus;

        let palette = crate::theme::Palette::for_theme(ui.ctx().theme());

        ui.heading("Local model (FLUX.2 klein)");
        ui.label(
            egui::RichText::new("Runs on this machine. No API key, no per-image cost. One-time ~16 GB download.")
                .small()
                .weak(),
        );
        ui.add_space(8.0);

        // --- Device ---------------------------------------------------------
        ui.strong("Device");
        // Deferred mutation: collect the click, apply after the borrow ends so
        // the radio closures don't hold `&mut self`.
        let mut new_device: Option<DevicePref> = None;
        ui.horizontal(|ui| {
            for (pref, label) in device_choices() {
                let selected = self.local_ai.device == pref;
                if ui.radio(selected, label).clicked() && !selected {
                    new_device = Some(pref);
                }
            }
        });
        if let Some(pref) = new_device {
            self.local_ai.device = pref;
            // Device choice only takes effect on the next model load; re-register
            // so a ready backend picks up the new device on its next invoke.
            self.refresh_local_backend();
        }
        if self.local_ai.device == DevicePref::Auto {
            ui.label(egui::RichText::new(format!("Auto resolves to: {}", resolved_device_label())).small().weak());
        }
        ui.add_space(8.0);

        // --- Cache directory ------------------------------------------------
        ui.strong("Cache directory");
        let cache_caption = self
            .local_model_cache_root()
            .map_or_else(|| "no cache directory available".to_owned(), |p| p.display().to_string());
        ui.label(egui::RichText::new(cache_caption).small().weak());
        let mut reset_cache = false;
        let mut choose_cache = false;
        ui.horizontal(|ui| {
            if ui.button("Choose…").clicked() {
                choose_cache = true;
            }
            if ui
                .add_enabled(self.local_ai.cache_dir.is_some(), egui::Button::new("Reset to default"))
                .clicked()
            {
                reset_cache = true;
            }
        });
        if choose_cache {
            self.spawn_cache_dir_picker();
        }
        if reset_cache {
            self.local_ai.cache_dir = None;
            self.local_ai_status = LocalModelStatus::Unknown;
            crate::local_ai::spawn_model_probe(
                self.runtime.handle(),
                self.egui_ctx.clone(),
                self.tx.clone(),
                self.local_ai.default_model.clone(),
                None,
            );
            self.refresh_local_backend();
        }
        ui.add_space(10.0);

        // --- Model card -----------------------------------------------------
        let mut download = false;
        let mut cancel = false;
        let mut delete = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("FLUX.2 klein 4B");
                match &self.local_ai_status {
                    LocalModelStatus::Ready => {
                        ui.label(egui::RichText::new("ready").small().color(palette.success));
                    }
                    LocalModelStatus::Failed(_) => {
                        ui.label(egui::RichText::new("failed").small().color(palette.error));
                    }
                    LocalModelStatus::Downloading { .. } => {
                        ui.label(egui::RichText::new("downloading").small().color(palette.warning));
                    }
                    LocalModelStatus::NotDownloaded => {
                        ui.label(egui::RichText::new("not downloaded").small().weak());
                    }
                    LocalModelStatus::Unknown => {
                        ui.label(egui::RichText::new("checking…").small().weak());
                    }
                }
            });
            match self.local_ai_status.clone() {
                LocalModelStatus::NotDownloaded => {
                    if ui.button("Download").clicked() {
                        download = true;
                    }
                }
                LocalModelStatus::Unknown => {
                    // Presence probe still in flight — offer no action until it
                    // resolves, so a click can't race the probe.
                    ui.add_enabled(false, egui::Button::new("Download"));
                }
                LocalModelStatus::Downloading { fraction, bytes, total } => {
                    ui.add(egui::ProgressBar::new(fraction).show_percentage());
                    ui.label(egui::RichText::new(format!("{} / {}", gib(bytes), gib(total))).small().weak());
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                }
                LocalModelStatus::Ready => {
                    if ui.button("Delete").clicked() {
                        delete = true;
                    }
                }
                LocalModelStatus::Failed(err) => {
                    ui.label(egui::RichText::new(err).small().color(palette.error));
                    if ui.button("Retry").clicked() {
                        download = true;
                    }
                }
            }
        });
        if download {
            self.start_model_download();
        }
        if cancel {
            self.cancel_model_download();
        }
        if delete {
            self.delete_model();
        }
        ui.add_space(6.0);

        // --- Total disk usage ----------------------------------------------
        ui.label(
            egui::RichText::new(format!(
                "Total disk usage when downloaded: ~16 GB. Cached under: {}",
                self.local_model_cache_caption()
            ))
            .small()
            .weak(),
        );
        ui.add_space(10.0);

        // --- HF token -------------------------------------------------------
        ui.separator();
        ui.add_space(6.0);
        ui.strong("Hugging Face token (optional)");
        ui.label(
            egui::RichText::new("Only needed for gated or rate-limited downloads. klein-4B is public.")
                .small()
                .weak(),
        );
        if self.hf_token_configured {
            ui.label(egui::RichText::new("token stored").small().color(palette.success));
        }
        let mut save_token: Option<String> = None;
        let mut clear_token = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.hf_token_input)
                    .password(true)
                    .hint_text("paste HF token")
                    .desired_width(220.0),
            );
            if ui.button("Save").clicked() && !self.hf_token_input.trim().is_empty() {
                save_token = Some(self.hf_token_input.trim().to_owned());
                self.hf_token_input.clear();
            }
            if ui.add_enabled(self.hf_token_configured, egui::Button::new("Clear")).clicked() {
                clear_token = true;
            }
        });
        if let Some(token) = save_token {
            self.save_key(ai::HF_TOKEN_BACKEND_ID, &token);
            // Optimistic; the off-thread refresh confirms it.
            self.hf_token_configured = true;
        }
        if clear_token {
            self.clear_backend(ai::HF_TOKEN_BACKEND_ID);
            self.hf_token_configured = false;
        }
        ui.add_space(10.0);

        // --- Distilled params ----------------------------------------------
        ui.separator();
        ui.add_space(6.0);
        ui.strong("Sampling");
        ui.label(egui::RichText::new("Steps: 4 · Guidance: 1.0 (distilled defaults)").small().weak());
        let mut advanced = self.advanced_open;
        if ui.checkbox(&mut advanced, "Advanced override").changed() {
            self.advanced_open = advanced;
            // Opening the override seeds the distilled posture; closing it pins
            // the distilled defaults again.
            self.local_ai.overrides = advanced.then(|| self.local_ai.overrides.unwrap_or_default());
        }
        if self.advanced_open {
            let mut overrides = self.local_ai.overrides.unwrap_or_default();
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Steps");
                changed |= ui.add(egui::Slider::new(&mut overrides.steps, 1..=8)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Guidance");
                changed |= ui.add(egui::Slider::new(&mut overrides.guidance, 0.0..=4.0)).changed();
            });
            if changed {
                self.local_ai.overrides = Some(overrides);
            }
        }
    }

    /// Feature-off stub: one weak line stating on-device generation is absent.
    #[cfg(not(feature = "local-flux"))]
    #[allow(clippy::unused_self)]
    fn local_model_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Local model");
        ui.label(egui::RichText::new("This build was compiled without on-device generation.").small().weak());
    }

    /// The cache-path caption for the "total disk usage" line: the override path
    /// or the resolved app-data default, or a plain note when neither resolves.
    #[cfg(feature = "local-flux")]
    fn local_model_cache_caption(&self) -> String {
        self.local_model_cache_root()
            .map_or_else(|| "the platform app-data directory".to_owned(), |p| p.display().to_string())
    }

    /// Spawns the native folder picker off the UI thread (it blocks) and routes
    /// the chosen directory back over [`crate::app::ShellMsg::ModelCacheDirChosen`].
    #[cfg(feature = "local-flux")]
    fn spawn_cache_dir_picker(&self) {
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        self.runtime.handle().spawn_blocking(move || {
            let chosen = rfd::FileDialog::new().set_title("Choose the model cache directory").pick_folder();
            let _ = tx.send(crate::app::ShellMsg::ModelCacheDirChosen(chosen));
            ctx.request_repaint();
        });
    }

    /// One provider row: a status badge, a password field with Save, and Clear.
    fn backend_row(&mut self, ui: &mut egui::Ui, display: &str, backend_id: &'static str, hint: &str) {
        let configured = self.key_configured(backend_id);
        let registered = ai::backend_registered(&self.verb_runtime, backend_id);
        let palette = crate::theme::Palette::for_theme(ui.ctx().theme());

        ui.horizontal(|ui| {
            ui.strong(display);
            let (text, color) = if registered {
                ("registered", palette.success)
            } else if configured {
                ("configured, not registered", palette.warning)
            } else {
                ("not configured", palette.warning)
            };
            ui.label(egui::RichText::new(text).small().color(color));
        });
        ui.label(egui::RichText::new(hint).small().weak());

        // Defer the mutable-self actions until after the draft field's borrow
        // ends, so Save/Clear can call back into `&mut self`.
        let mut save_key: Option<String> = None;
        let mut clear = false;
        ui.horizontal(|ui| {
            let draft = if backend_id == ai::OPENAI_BACKEND_ID {
                &mut self.openai_key_input
            } else {
                &mut self.fal_key_input
            };
            ui.add(egui::TextEdit::singleline(draft).password(true).hint_text("paste API key").desired_width(220.0));
            if ui.button("Save").clicked() && !draft.trim().is_empty() {
                save_key = Some(draft.trim().to_owned());
                draft.clear();
            }
            if ui.add_enabled(configured, egui::Button::new("Clear")).clicked() {
                clear = true;
            }
        });

        if let Some(key) = save_key {
            self.save_key(backend_id, &key);
        }
        if clear {
            self.clear_backend(backend_id);
        }
    }

    /// Keybinds tab: a preset dropdown plus a rebindable row per command.
    fn keybinds_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Preset");
            egui::ComboBox::from_id_salt("keybind_preset")
                .selected_text(self.keymap.preset.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.keymap.preset, KeybindPreset::Aseprite, KeybindPreset::Aseprite.label());
                    ui.selectable_value(&mut self.keymap.preset, KeybindPreset::Photoshop, KeybindPreset::Photoshop.label());
                    ui.selectable_value(&mut self.keymap.preset, KeybindPreset::Custom, KeybindPreset::Custom.label());
                });
            let has_custom = !self.keymap.custom.is_empty();
            if ui.add_enabled(has_custom, egui::Button::new("Reset all")).clicked() {
                self.keymap.reset_all();
                self.capturing = None;
            }
        });
        ui.label(
            egui::RichText::new("Rebinding a command records a custom override that wins over the preset.")
                .small()
                .weak(),
        );
        ui.separator();

        // Resolve a pending capture: the first non-modifier key pressed becomes
        // the new binding. Reading the settings viewport's own input keeps this
        // independent of the main window's dispatch.
        if let Some(command) = self.capturing {
            if let Some(chord) = capture_chord(ui.ctx()) {
                self.keymap.set_override(command, chord);
                self.capturing = None;
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for &category in CommandCategory::ALL {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(category.label()).strong());
                for &command in CommandId::ALL {
                    if command.category() != category {
                        continue;
                    }
                    self.keybind_row(ui, command);
                }
            }
        });
    }

    /// One rebindable command row: label, effective chord (or capture prompt),
    /// and a reset button when overridden.
    fn keybind_row(&mut self, ui: &mut egui::Ui, command: CommandId) {
        // Unique id per row so the buttons don't collide on call-site location.
        ui.push_id(command.label(), |ui| {
            ui.horizontal(|ui| {
                ui.add_sized([180.0, 18.0], egui::Label::new(command.label()).truncate());

                let capturing_this = self.capturing == Some(command);
                let label = if capturing_this {
                    "press keys…".to_owned()
                } else {
                    self.keymap.resolve(command).map_or_else(|| "—".to_owned(), |c| c.to_string())
                };
                if ui.add_sized([140.0, 20.0], egui::Button::new(label)).clicked() {
                    self.capturing = if capturing_this { None } else { Some(command) };
                }

                let overridden = self.keymap.is_overridden(command);
                if ui.add_enabled(overridden, egui::Button::new("Reset")).clicked() {
                    self.keymap.reset(command);
                    if capturing_this {
                        self.capturing = None;
                    }
                }
            });
        });
    }
}

/// The device-preference radios to offer for this build and platform: always
/// `Auto` and `CPU`, plus the GPU variants the build compiled in. Only offering
/// supported variants keeps the artist from picking a backend that silently
/// falls back to CPU.
#[cfg(feature = "local-flux")]
fn device_choices() -> Vec<(crate::ai::DevicePref, &'static str)> {
    use crate::ai::DevicePref;
    let mut out = vec![(DevicePref::Auto, "Auto")];
    if cfg!(feature = "local-flux-cuda") {
        out.push((DevicePref::Cuda(0), "CUDA"));
    }
    if cfg!(feature = "local-flux-metal") {
        out.push((DevicePref::Metal, "Metal"));
    }
    out.push((DevicePref::Cpu, "CPU"));
    out
}

/// The device `Auto` resolves to for this build, for the caption under the radios.
/// CPU-only builds always resolve to CPU; GPU builds name the GPU family.
#[cfg(feature = "local-flux")]
fn resolved_device_label() -> &'static str {
    if cfg!(feature = "local-flux-cuda") {
        "CUDA (GPU)"
    } else if cfg!(feature = "local-flux-metal") {
        "Metal (GPU)"
    } else {
        "CPU (slow — minutes per image)"
    }
}

/// Format a byte count as gibibytes with one decimal, for the download caption.
#[cfg(feature = "local-flux")]
fn gib(bytes: u64) -> String {
    // f64 carries a ~16 GB byte count without precision loss for a one-decimal
    // display.
    #[allow(clippy::cast_precision_loss)]
    let g = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    format!("{g:.1} GB")
}

/// Reads the first non-modifier key pressed this frame and builds a chord from
/// it plus the current modifiers. Returns `None` when no key is down.
fn capture_chord(ctx: &egui::Context) -> Option<Chord> {
    ctx.input(|i| {
        let key = Key::ALL.iter().copied().find(|&k| i.key_pressed(k))?;
        Some(Chord::from_modifiers(i.modifiers, key))
    })
}
