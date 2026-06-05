//! The Inspector panel (right dock): entry health, linked assets, used-in counts, and
//! quick actions.
//!
//! The former anchors/relations/preview moved to the center tabs (spec 5). A `&self`
//! panel reading `CodexEntryDetail` and pushing `Intent`s.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is exactly the panel-trait surface, the intent enum, the
// design-system glyphs/widgets, the i18n helper, and this panel's id.
use super::{INSPECTOR, Intent, MsgKey, Panel, PanelId, PanelMeta, PanelScope, Region, icons, tr, widgets};

/// The Inspector panel (right dock): entry health (ring + checklist), linked assets,
/// used-in counts, and quick actions. The former anchors/relations/preview moved to the
/// center tabs (spec 5).
pub struct InspectorPanel;

impl Panel for InspectorPanel {
    fn id(&self) -> PanelId {
        INSPECTOR
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-inspector.title"),
            icon: icons::INSPECTOR,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;
        let Some(detail) = view.detail.as_ref() else {
            ui.label(
                egui::RichText::new(tr("codex.inspector.empty"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
            return;
        };
        let id = detail.summary.id;

        // --- Entry health: a ring with the tier label, then the checklist. ---
        widgets::section_header(ui, theme, icons::HEALTH, &tr("codex.health.title"));
        let tier_key = if detail.health >= 0.8 {
            "codex.health.excellent"
        } else if detail.health >= 0.4 {
            "codex.health.good"
        } else {
            "codex.health.needs_work"
        };
        widgets::score_ring(ui, theme, icons::HEALTH, detail.health, detail.health_percent, &tr(tier_key));
        ui.add_space(theme.spacing.xs);
        for check in &detail.health_checks {
            widgets::status_check_row(ui, theme, check.state, &tr(&check.label_key), check.detail.as_deref());
        }

        // --- Linked assets (MOCK strip sized by the real handle count). ---
        ui.add_space(theme.spacing.sm);
        widgets::section_header(ui, theme, icons::ASSETS, &tr("codex.inspector.linked_assets"));
        widgets::asset_thumbnail_strip(ui, theme, detail.linked_asset_count, 4);

        // --- Used in (DERIVED counts). ---
        ui.add_space(theme.spacing.sm);
        widgets::section_header(ui, theme, icons::LINK, &tr("codex.inspector.used_in"));
        widgets::used_in_row(ui, theme, icons::RECIPE, &tr("codex.used_in.recipes"), detail.used_in.recipes);
        widgets::used_in_row(ui, theme, icons::TIMELINE, &tr("codex.used_in.animations"), detail.used_in.animations);
        widgets::used_in_row(ui, theme, icons::SPARKLE, &tr("codex.used_in.fx"), detail.used_in.fx);
        widgets::used_in_row(ui, theme, icons::SCENE, &tr("codex.used_in.scenes"), detail.used_in.scenes);

        // --- Quick actions. ---
        ui.add_space(theme.spacing.sm);
        widgets::section_header(ui, theme, icons::SLIDERS, &tr("codex.editor.section.identity"));
        ui.horizontal_wrapped(|ui| {
            // Pin / unpin.
            if view.is_pinned(id) {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(format!("{} {}", icons::PIN, tr("codex.action.remove")))
                                .size(theme.type_scale.label)
                                .color(theme.roles.text_secondary),
                        )
                        .frame(true),
                    )
                    .clicked()
                {
                    scope.ctx.intents.push(Intent::UnpinCodexEntry(id));
                }
            } else if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::PIN, tr("codex.action.pin_selected")))
                            .size(theme.type_scale.label)
                            .color(theme.accent.base),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::PinCodexEntry(id));
            }
            // Duplicate.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::DUPLICATE, tr("codex.action.duplicate")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::DuplicateCodexEntry(id));
            }
            // Promote to canonical.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::PROMOTE, tr("codex.action.promote")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::PromoteCodexEntry(id));
            }
            // Archive.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::ARCHIVE, tr("codex.action.archive")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::ArchiveCodexEntry(id));
            }
            // Delete (the destructive action, in the error role).
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::TRASH, tr("codex.action.delete_entry")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.error),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::DeleteCodexEntry(id));
            }
        });
    }
}
