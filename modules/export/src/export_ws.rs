//! The Export workspace and its panels.
//!
//! Export is production discipline (bible 6.7, 19): pick a format and an engine
//! preset, set the animation metadata, and clear the QA checklist before writing.
//! The rail is the minimal navigation subset of sprite-edit's shared tools (Hand,
//! Zoom). Every panel is a `&self` unit struct reading through `scope.ctx` and
//! pushing [`Intent`]s; the Console tray panel is owned by sprite-edit and
//! referenced by id, never re-registered. The engine target is Unity only.
//!
//! [`Intent`]: pixhaus_ui::state::intent::Intent

use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_ui::contrib_api::{
    ActionDesc, ActionId, HostRegistrar, MsgKey, Panel, PanelId, PanelMeta, PanelScope, StatusItem, ToolId, Workspace, WorkspaceId, WorkspaceLayout,
    WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::intent::Intent;
use pixhaus_ui::theme::Theme;
use pixhaus_ui::{icons, widgets};

/// The Export workspace id.
pub const EXPORT: WorkspaceId = WorkspaceId("export");

/// The Export Format dock panel id (export-owned).
pub const EXPORT_FORMAT: PanelId = PanelId("export-format");
/// The Engine Preset dock panel id (export-owned).
pub const ENGINE_PRESET: PanelId = PanelId("engine-preset");
/// The Animation Metadata dock panel id (export-owned).
pub const ANIMATION_METADATA: PanelId = PanelId("animation-metadata");
/// The QA Warnings dock panel id (export-owned).
pub const QA_WARNINGS: PanelId = PanelId("qa-warnings");
/// The Export Log tray panel id (export-owned).
pub const EXPORT_LOG: PanelId = PanelId("export-log");

// Tools owned by sprite-edit, referenced by id - never re-registered here. The
// Export rail is a navigation subset: pan and zoom only.
const HAND: ToolId = ToolId("hand");
const ZOOM: ToolId = ToolId("zoom");

// The Console tray panel is owned by sprite-edit, referenced by id.
const CONSOLE: PanelId = PanelId("console");

// The actions this module's panels dispatch. Namespaced under `export.` so they
// never collide with any other module's action ids.
const EXPORT_RUN: ActionId = ActionId("export.run");
const EXPORT_FIX: ActionId = ActionId("export.fix");
const EXPORT_IGNORE: ActionId = ActionId("export.ignore");

/// The Export workspace: production output for the Unity engine target. Owns layout
/// only, no data (bible rule 2). The rail is pan and zoom only.
pub struct ExportWorkspace;

impl Workspace for ExportWorkspace {
    fn id(&self) -> WorkspaceId {
        EXPORT
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: MsgKey("workspace.export.title"),
            icon: icons::EXPORT_WS,
            purpose: MsgKey("workspace.export.purpose"),
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num5),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![EXPORT_FORMAT, ENGINE_PRESET, ANIMATION_METADATA, QA_WARNINGS],
            bottom_tray: vec![EXPORT_LOG, CONSOLE],
            primary_tools: vec![HAND, ZOOM],
            default_tool: HAND,
            status_items: vec![
                StatusItem {
                    icon: icons::EXPORT,
                    text: MsgKey("workspace.export.status.format").tr(),
                },
                StatusItem {
                    icon: icons::CHECK,
                    text: "0 warnings".to_owned(),
                },
            ],
        }
    }
}

/// The Export Format panel. Mock content: a radio group over the output formats.
/// The selection is throwaway local state (resets each frame; drives nothing).
pub struct ExportFormatPanel;

impl Panel for ExportFormatPanel {
    fn id(&self) -> PanelId {
        EXPORT_FORMAT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.export-format.title"),
            icon: icons::EXPORT,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::section_header(ui, theme, icons::EXPORT, "Export Format");
        // Throwaway local selection - mock radios that drive nothing this round.
        let mut format = 0_usize;
        for (i, label) in ["PNG", "Spritesheet", "GIF", "APNG", "JSON"].iter().enumerate() {
            ui.radio_value(&mut format, i, *label);
        }
    }
}

/// The Engine Preset panel. Mock content: Unity is the only enabled, selected
/// preset (the engine target is Unity only); the rest are listed as future targets
/// in disabled text and read as inert.
pub struct EnginePresetPanel;

impl Panel for EnginePresetPanel {
    fn id(&self) -> PanelId {
        ENGINE_PRESET
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.engine-preset.title"),
            icon: icons::SETTINGS,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::section_header(ui, theme, icons::SETTINGS, "Engine Preset");
        // Unity is selected and enabled; the rest read as future targets.
        let frame = egui::Frame::new().fill(theme.accent.muted).inner_margin(theme.spacing.xs);
        frame.show(ui, |ui| {
            ui.label(egui::RichText::new("Unity").color(theme.roles.text_primary).strong());
        });
        for engine in ["Godot", "Unreal Paper2D", "Phaser", "PixiJS", "Love2D", "Bevy", "Generic", "Custom"] {
            ui.label(egui::RichText::new(format!("{engine} (soon)")).color(theme.roles.text_disabled));
        }
    }
}

/// The Animation Metadata panel. Mock content: the per-animation export toggle,
/// trim, padding, and pivot controls. The widgets bind to throwaway locals that
/// reset each frame - correct for mock content this round.
pub struct AnimationMetadataPanel;

impl Panel for AnimationMetadataPanel {
    fn id(&self) -> PanelId {
        ANIMATION_METADATA
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.animation-metadata.title"),
            icon: icons::FRAMES,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::section_header(ui, theme, icons::FRAMES, "Animation Metadata");
        // Inert mock controls; the values reset each frame (drive nothing).
        let mut per_animation = true;
        ui.checkbox(&mut per_animation, "Per-animation export");
        let mut trim = true;
        ui.checkbox(&mut trim, "Trim");
        ui.horizontal(|ui| {
            ui.label("Padding");
            let mut padding = 2_i32;
            ui.add(egui::DragValue::new(&mut padding).range(0..=64));
        });
        ui.horizontal(|ui| {
            ui.label("Pivot");
            ui.label(egui::RichText::new("Center").color(theme.roles.text_secondary));
        });
    }
}

/// The QA Warnings panel. Mock content: the pre-export validation checklist with
/// success/warning badges. The OK rows pass; each WARN row carries Fix and Ignore
/// buttons that push `RunAction` intents (validate before writing, bible 19.4).
pub struct QaWarningsPanel;

impl Panel for QaWarningsPanel {
    fn id(&self) -> PanelId {
        QA_WARNINGS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.qa-warnings.title"),
            icon: icons::CHECK,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::section_header(ui, theme, icons::CHECK, "QA Warnings");
        for label in ["All frames same size", "Transparent bg", "Palette < 32"] {
            qa_ok_row(ui, theme, label);
        }
        for label in ["\"jump\" does not loop", "2 missing animations"] {
            qa_warn_row(ui, theme, label, scope);
        }
    }
}

/// One passing QA row: a success dot and the label.
fn qa_ok_row(ui: &mut egui::Ui, theme: &Theme, label: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icons::CHECK.to_string()).color(theme.roles.success));
        ui.label(egui::RichText::new(format!("OK {label}")).color(theme.roles.text_secondary));
    });
}

/// One failing QA row: a warning badge, the label, and Fix/Ignore buttons that push
/// `RunAction` intents.
fn qa_warn_row(ui: &mut egui::Ui, theme: &Theme, label: &str, scope: &mut PanelScope<'_>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icons::WARN.to_string()).color(theme.roles.warning));
        ui.label(egui::RichText::new(format!("WARN {label}")).color(theme.roles.text_secondary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Ignore").clicked() {
                scope.ctx.intents.push(Intent::RunAction(EXPORT_IGNORE));
            }
            if ui.button("Fix").clicked() {
                scope.ctx.intents.push(Intent::RunAction(EXPORT_FIX));
            }
        });
    });
}

/// The Export Log tray panel: a scrolling mock log in monospace, secondary text.
pub struct ExportLogPanel;

impl Panel for ExportLogPanel {
    fn id(&self) -> PanelId {
        EXPORT_LOG
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.export-log.title"),
            icon: icons::CONSOLE,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        egui::ScrollArea::vertical().show(ui, |ui| {
            widgets::mock_log(ui, theme, &["info  exporter ready", "info  Unity preset selected"]);
        });
    }
}

/// Register the Export workspace, its four dock panels, the Export Log tray panel,
/// and the `export.*` actions.
///
/// The shared Console tray panel and the Hand/Zoom tools are owned by sprite-edit
/// and referenced by id, never re-registered here. Export contributes no menu group
/// this round.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(ExportWorkspace));

    // Dock panels.
    host.add_panel(Box::new(ExportFormatPanel));
    host.add_panel(Box::new(EnginePresetPanel));
    host.add_panel(Box::new(AnimationMetadataPanel));
    host.add_panel(Box::new(QaWarningsPanel));

    // The tray panel Export owns.
    host.add_panel(Box::new(ExportLogPanel));

    // The `export.*` actions the panels above dispatch. Run is command-palette
    // visible; the per-warning Fix/Ignore are not (they only make sense in context).
    host.add_action(ActionDesc {
        id: EXPORT_RUN,
        label: MsgKey("command.export.run"),
        icon: icons::EXPORT,
        palette_visible: true,
    });
    for (id, label) in [(EXPORT_FIX, MsgKey("command.export.fix")), (EXPORT_IGNORE, MsgKey("command.export.ignore"))] {
        host.add_action(ActionDesc {
            id,
            label,
            icon: icons::WARN,
            palette_visible: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_layout_matches_the_inventory() {
        let layout = ExportWorkspace.layout();
        assert_eq!(layout.right_dock, vec![EXPORT_FORMAT, ENGINE_PRESET, ANIMATION_METADATA, QA_WARNINGS]);
        assert_eq!(layout.bottom_tray, vec![EXPORT_LOG, CONSOLE]);
        assert_eq!(layout.default_tool, HAND);
        assert_eq!(layout.primary_tools, vec![HAND, ZOOM]);
        assert_eq!(layout.status_items.len(), 2);
        assert_eq!(layout.status_items[0].text, MsgKey("workspace.export.status.format").tr());
        assert_eq!(layout.status_items[1].text, "0 warnings");
    }

    #[test]
    fn export_meta_uses_cmd_5() {
        assert_eq!(ExportWorkspace.id(), EXPORT);
        assert_eq!(ExportWorkspace.meta().name, MsgKey("workspace.export.title"));
        assert_eq!(ExportWorkspace.meta().shortcut, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num5));
    }

    #[test]
    fn dock_panel_ids_and_regions() {
        assert_eq!(ExportFormatPanel.id(), EXPORT_FORMAT);
        assert_eq!(EnginePresetPanel.id(), ENGINE_PRESET);
        assert_eq!(AnimationMetadataPanel.id(), ANIMATION_METADATA);
        assert_eq!(QaWarningsPanel.id(), QA_WARNINGS);
        for region in [
            ExportFormatPanel.meta().default_region,
            EnginePresetPanel.meta().default_region,
            AnimationMetadataPanel.meta().default_region,
            QaWarningsPanel.meta().default_region,
        ] {
            assert_eq!(region, Region::RightDock);
        }
    }

    #[test]
    fn export_log_is_a_tray_panel() {
        assert_eq!(ExportLogPanel.id(), EXPORT_LOG);
        assert_eq!(ExportLogPanel.meta().default_region, Region::BottomTray);
    }
}
