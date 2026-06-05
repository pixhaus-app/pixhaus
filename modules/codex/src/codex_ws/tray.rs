//! The remaining bottom-tray panels: Test Generation and History.
//!
//! Test entries without leaving the workspace (bible 8.8, 14); the History panel reads
//! the selected entry's version-history mirror (no hardcoded log). Both are `&self`
//! panels that read `CodexView` and push `Intent`s.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is the panel-trait surface, the intent enum, the
// design-system glyphs/widgets, the i18n helper, these two panels' ids, and the shared
// `history_body` defined in the editor area.
use super::{HISTORY, Intent, MsgKey, Panel, PanelId, PanelMeta, PanelScope, Region, TEST_GENERATION, history_body, icons, tr, widgets};

/// The Test Generation panel (bottom tray): a prompt field (the scratch carve-out),
/// the context stack as removable reference chips, a Compile button, and the compiled
/// preview - test entries without leaving the workspace (bible 8.8, 14).
pub struct TestGenerationPanel;

impl Panel for TestGenerationPanel {
    fn id(&self) -> PanelId {
        TEST_GENERATION
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-test-generation.title"),
            icon: icons::SPARKLE,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;

        // The context stack: pinned references as removable chips (bible 9.2).
        if view.context.is_empty() {
            ui.label(
                egui::RichText::new(tr("codex.test.empty"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
        } else {
            ui.horizontal_wrapped(|ui| {
                let mut to_remove = None;
                for c in &view.context {
                    if widgets::reference_chip(ui, theme, &format!("@{}", c.handle), c.entry_type, true) {
                        to_remove = Some(c.entry);
                    }
                }
                if let Some(id) = to_remove {
                    scope.ctx.intents.push(Intent::RemoveReferenceFromContext(id));
                }
            });
        }

        // Add the selected entry to the context stack.
        if let Some(selected) = view.selected
            && !view.is_pinned(selected)
            && ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::PIN, tr("codex.action.pin_selected")))
                            .size(theme.type_scale.label)
                            .color(theme.accent.base),
                    )
                    .frame(false),
                )
                .clicked()
        {
            scope.ctx.intents.push(Intent::AddReferenceToContext(selected));
        }

        ui.add_space(theme.spacing.sm);

        // The prompt field: the scratch carve-out.
        ui.add(
            egui::TextEdit::multiline(scope.scratch)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text(tr("codex.test.placeholder")),
        );

        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new(format!("{} {}", icons::SPARKLE, tr("codex.action.compile"))).color(theme.accent.ai))
                .clicked()
            {
                scope.ctx.intents.push(Intent::CompileCodexPrompt {
                    user_text: scope.scratch.clone(),
                });
            }
        });

        // The compiled preview.
        if let Some(compiled) = view.compiled.as_ref()
            && !compiled.positive.is_empty()
        {
            ui.add_space(theme.spacing.xs);
            widgets::section_header(ui, theme, icons::CODEX, &tr("codex.test.compiled"));
            ui.label(
                egui::RichText::new(&compiled.positive)
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
        }
    }
}

/// The History panel (bottom tray): the selected entry's version timeline (or a hint to
/// select an entry). Reads the version-history mirror; no hardcoded log.
pub struct CodexHistoryPanel;

impl Panel for CodexHistoryPanel {
    fn id(&self) -> PanelId {
        HISTORY
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-history.title"),
            icon: icons::HISTORY,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;
        match view.detail.as_ref() {
            Some(detail) => history_body(ui, theme, detail),
            None => {
                ui.label(
                    egui::RichText::new(tr("codex.inspector.empty"))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                );
            }
        }
    }
}
