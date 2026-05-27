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
    /// API-key entry and provider status.
    AiBackends,
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
            ui.selectable_value(&mut self.settings_tab, SettingsTab::AiBackends, "AI backends");
            ui.selectable_value(&mut self.settings_tab, SettingsTab::Graphics, "Graphics");
        });
        ui.separator();
        ui.add_space(4.0);
        match self.settings_tab {
            SettingsTab::General => self.general_tab(ui),
            SettingsTab::Keybinds => self.keybinds_tab(ui),
            SettingsTab::AiBackends => self.ai_backends_tab(ui),
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

    /// AI backends tab: one status/set/clear row per ported provider.
    fn ai_backends_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("AI backends");
        ui.label(egui::RichText::new("Keys are stored in the OS keychain, never on disk.").small().weak());
        ui.add_space(8.0);
        self.backend_row(ui, "OpenAI", ai::OPENAI_BACKEND_ID, "Generates reference sheets (gpt-image).");
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);
        self.backend_row(ui, "FAL", ai::FAL_BACKEND_ID, "Reference sheets, plus image-to-video animation.");
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

/// Reads the first non-modifier key pressed this frame and builds a chord from
/// it plus the current modifiers. Returns `None` when no key is down.
fn capture_chord(ctx: &egui::Context) -> Option<Chord> {
    ctx.input(|i| {
        let key = Key::ALL.iter().copied().find(|&k| i.key_pressed(k))?;
        Some(Chord::from_modifiers(i.modifiers, key))
    })
}
