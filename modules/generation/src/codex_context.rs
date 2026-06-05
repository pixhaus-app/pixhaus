//! The Generate-workspace Codex context panel.
//!
//! Generate is Codex-aware (Codex bible section 9): a prompt can name Codex entries
//! by `@handle`, and the artist pins entries into a shared context stack that the
//! prompt compiler weights into the request. This panel surfaces that stack inside
//! Generate - the same `CodexView.context` mirror the Codex workspace's Test panel
//! reads - so the references that steer a generation are visible and editable next to
//! the prompt.
//!
//! It reuses the Codex widgets and intents verbatim; it owns no new state. The
//! prompt field is the panel's scratch carve-out (the one `&mut String` a panel may
//! touch). Compiling routes the text plus the pinned references through the services
//! resolver/compiler via [`Intent::CompileCodexPrompt`], and "Generate from compiled"
//! submits the compiled positive prompt through the existing anchor job pathway - AI
//! proposes, the artist decides.
//!
//! [`Intent::CompileCodexPrompt`]: pixhaus_ui::state::intent::Intent::CompileCodexPrompt

use pixhaus_core::codex::AnchorStrength;
use pixhaus_ui::contrib_api::{MsgKey, Panel, PanelId, PanelMeta, PanelScope};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::intent::Intent;
use pixhaus_ui::{icons, widgets};

/// Resolve an i18n key to display text at render time (the shell's `tr()` contract).
fn tr(key: &str) -> String {
    pixhaus_services::i18n::tr(key)
}

/// The i18n key for an anchor strength (`codex.anchor.strength.*`). The Codex module
/// owns these keys in the shared services locale set; the context selector reuses them
/// so its strength labels read identically to the Codex workspace's.
fn anchor_strength_key(strength: AnchorStrength) -> &'static str {
    match strength {
        AnchorStrength::Loose => "codex.anchor.strength.loose",
        AnchorStrength::Normal => "codex.anchor.strength.normal",
        AnchorStrength::Strong => "codex.anchor.strength.strong",
        AnchorStrength::Locked => "codex.anchor.strength.locked",
    }
}

/// The Generate-side Codex context panel id (generation-owned).
pub const GENERATE_CODEX_CONTEXT: PanelId = PanelId("generate-codex-context");

/// The Codex Context panel: the context stack of pinned references with per-reference
/// strength, an `@`-aware prompt field, a Compile control, and a "Generate from
/// compiled" action. Reads the shared [`CodexView`] mirror; mutates nothing directly.
///
/// [`CodexView`]: pixhaus_ui::state::session::CodexView
pub struct CodexContextPanel;

impl Panel for CodexContextPanel {
    fn id(&self) -> PanelId {
        GENERATE_CODEX_CONTEXT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.generate-codex-context.title"),
            icon: icons::REFERENCE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;

        // The context stack: pinned references as removable chips, each with a strength
        // selector (Codex bible 9.2). Empty until the artist pins an entry from the
        // Codex workspace.
        if view.context.is_empty() {
            ui.label(
                egui::RichText::new(tr("codex.context.empty"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
        } else {
            let mut to_remove = None;
            let mut to_restrength: Option<(_, AnchorStrength)> = None;
            for c in &view.context {
                ui.horizontal(|ui| {
                    if widgets::reference_chip(ui, theme, &format!("@{}", c.handle), c.entry_type, true) {
                        to_remove = Some(c.entry);
                    }
                });
                if let Some(strength) = widgets::strength_selector(ui, theme, c.strength, |s| tr(anchor_strength_key(s))) {
                    to_restrength = Some((c.entry, strength));
                }
                ui.add_space(theme.spacing.xs);
            }
            if let Some(id) = to_remove {
                scope.ctx.intents.push(Intent::RemoveReferenceFromContext(id));
            }
            if let Some((id, strength)) = to_restrength {
                scope.ctx.intents.push(Intent::SetReferenceStrength { id, strength });
            }
        }

        ui.separator();

        // The `@`-aware prompt field (the scratch carve-out). Live suggestions from the
        // Codex search mirror help the artist complete an `@`-mention.
        ui.label(
            egui::RichText::new(tr("codex.context.prompt_label"))
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary),
        );
        ui.add(
            egui::TextEdit::multiline(scope.scratch)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text(tr("codex.context.prompt_placeholder")),
        );

        // Suggestion hints: the entries the Codex search currently surfaces, as inert
        // chips the artist can read off while typing an `@`-mention.
        if !view.suggestions.is_empty() {
            ui.add_space(theme.spacing.xs);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(tr("codex.context.matches"))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                );
                for s in view.suggestions.iter().take(6) {
                    let text = egui::RichText::new(format!("@{}", s.handle))
                        .size(theme.type_scale.label)
                        .color(theme.accent.base);
                    let _ = ui.selectable_label(false, text);
                }
            });
        }

        ui.add_space(theme.spacing.sm);

        // Compile: resolve the text plus the pinned references through the services
        // compiler and store the inspectable preview. Never submits a job.
        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new(format!("{} {}", icons::CODEX, tr("codex.context.compile"))).color(theme.accent.ai))
                .clicked()
            {
                scope.ctx.intents.push(Intent::CompileCodexPrompt {
                    user_text: scope.scratch.clone(),
                });
            }
        });

        // The compiled preview plus a "Generate from compiled" action: submit the
        // compiled positive prompt through the existing anchor job pathway. The submit is
        // disabled while a job is in flight.
        if let Some(compiled) = view.compiled.as_ref()
            && !compiled.positive.is_empty()
        {
            ui.add_space(theme.spacing.xs);
            widgets::section_header(ui, theme, icons::SPARKLE, &tr("codex.context.compiled"));
            ui.label(
                egui::RichText::new(&compiled.positive)
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
            if !compiled.negative.is_empty() {
                ui.label(
                    egui::RichText::new(pixhaus_services::i18n::tr_args("codex.context.negative", &[("text", &compiled.negative)]))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_disabled),
                );
            }
            ui.add_space(theme.spacing.xs);
            let busy = scope.ctx.session.is_generating();
            let positive = compiled.positive.clone();
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(egui::RichText::new(format!("{} {}", icons::SPARKLE, tr("codex.context.generate"))).color(theme.roles.text_primary))
                        .fill(if busy { theme.accent.muted } else { theme.accent.base }),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::SubmitAnchorJob { prompt: positive });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_panel_id_and_region() {
        assert_eq!(CodexContextPanel.id(), GENERATE_CODEX_CONTEXT);
        assert_eq!(CodexContextPanel.meta().default_region, Region::RightDock);
        assert_eq!(CodexContextPanel.meta().title, MsgKey("panel.generate-codex-context.title"));
        assert!(CodexContextPanel.meta().default_open);
    }
}
