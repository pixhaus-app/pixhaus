//! The Draw workspace and the shared sprite-editing panels.
//!
//! Draw owns the panels the other workspaces reuse by id (bible rule 2): the
//! Layers/Sprites/Palette/Selection Actions/AI Assistant dock panels and the
//! Frames/Assets/Console tray panels. They are registered once, here, before any
//! other module, so a later workspace's layout can reference them by id.
//!
//! Every panel is a `&self` unit struct that reads through `scope.ctx` and pushes
//! [`Intent`]s into `scope.ctx.intents`. The mock controls (sliders, checkboxes,
//! toggles) bind to throwaway locals that reset each frame - correct for mock
//! content, which drives nothing this round.

use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_ui::contrib_api::{
    ActionId, HostRegistrar, MenuGroup, MenuItem, MsgKey, Panel, PanelId, PanelMeta, PanelScope, StatusItem, Workspace, WorkspaceId, WorkspaceLayout,
    WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::intent::Intent;
use pixhaus_ui::{icons, widgets};

use crate::tools;

/// The Draw workspace id.
pub const DRAW: WorkspaceId = WorkspaceId("draw");

/// The Layers dock panel id (shared - reused by Animate).
pub const LAYERS: PanelId = PanelId("layers");
/// The Sprites dock panel id (shared - reused by Animate/Tiles).
pub const SPRITES: PanelId = PanelId("sprites");
/// The Palette dock panel id (shared - reused by Tiles/Generate).
pub const PALETTE: PanelId = PanelId("palette");
/// The Selection Actions dock panel id.
pub const SELECTION_ACTIONS: PanelId = PanelId("selection-actions");
/// The AI Assistant dock panel id.
pub const AI_ASSISTANT: PanelId = PanelId("ai-assistant");
/// The Frames tray panel id (shared - reused by Animate, both dock and tray).
pub const FRAMES: PanelId = PanelId("frames");
/// The Assets tray panel id.
pub const ASSETS: PanelId = PanelId("assets");
/// The Console tray panel id (shared - reused by every workspace's tray).
pub const CONSOLE: PanelId = PanelId("console");

// Actions this module's panels and menus dispatch.
const LAYER_NEW: ActionId = ActionId("layer.new");
const LAYER_DELETE: ActionId = ActionId("layer.delete");
const LAYER_MERGE_DOWN: ActionId = ActionId("layer.merge-down");
const SPRITE_NEW: ActionId = ActionId("sprite.new");
const SPRITE_RESIZE: ActionId = ActionId("sprite.resize");
const PALETTE_RAMP: ActionId = ActionId("palette.ramp");
const PALETTE_HARMONY: ActionId = ActionId("palette.harmony");
const PALETTE_REDUCE: ActionId = ActionId("palette.reduce");
const SEL_CUT: ActionId = ActionId("selection.cut");
const SEL_COPY: ActionId = ActionId("selection.copy");
const SEL_PASTE: ActionId = ActionId("selection.paste");
const SEL_INVERT: ActionId = ActionId("selection.invert");
const SEL_CROP: ActionId = ActionId("selection.crop");
const SEL_AI_FILL: ActionId = ActionId("selection.ai-fill");
const SEL_AI_CLEANUP: ActionId = ActionId("selection.ai-clean-up");
const SEL_AI_SEAMLESS: ActionId = ActionId("selection.ai-make-seamless");
const AI_FILL: ActionId = ActionId("ai.fill-selection");
const AI_CLEANUP: ActionId = ActionId("ai.clean-up");
const AI_REDUCE: ActionId = ActionId("ai.reduce-colors");
const AI_RAMP: ActionId = ActionId("ai.suggest-ramp");
const AI_VARIATIONS: ActionId = ActionId("ai.create-variations");
const AI_REMOVE_BG: ActionId = ActionId("ai.remove-background");
const FRAME_ADD: ActionId = ActionId("frame.add");
const FRAME_DUPLICATE: ActionId = ActionId("frame.duplicate");
const FRAME_DELETE: ActionId = ActionId("frame.delete");

/// The Draw workspace: editing a single sprite in space. Layout only - it owns no
/// data (bible rule 2: Draw and Animate are siblings over one editing core).
pub struct DrawWorkspace;

impl Workspace for DrawWorkspace {
    fn id(&self) -> WorkspaceId {
        DRAW
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: MsgKey("workspace.draw.title"),
            icon: icons::PENCIL,
            purpose: MsgKey("workspace.draw.purpose"),
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![LAYERS, SPRITES, PALETTE, SELECTION_ACTIONS, AI_ASSISTANT],
            bottom_tray: vec![FRAMES, ASSETS, CONSOLE],
            primary_tools: tools::ALL.to_vec(),
            default_tool: tools::PENCIL,
            status_items: vec![StatusItem {
                icon: icons::GRID,
                text: MsgKey("workspace.draw.status.grid_on").tr(),
            }],
        }
    }
}

/// The Layers panel. Mock content: a `+ New Layer` affordance, a row per layer
/// with eye/lock toggles, an opacity slider, and a `Normal` blend label; the
/// selected row is tinted `accent.muted`. New Layer pushes a `RunAction` intent.
pub struct LayersPanel;

impl Panel for LayersPanel {
    fn id(&self) -> PanelId {
        LAYERS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.layers.title"),
            icon: icons::LAYERS,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        if ui.button(format!("{} New Layer", icons::ADD)).clicked() {
            scope.ctx.intents.push(Intent::RunAction(LAYER_NEW));
        }

        // The first row reads as selected; selection is mock UI state this round.
        let rows = ["Layer 3", "Layer 2", "Layer 1", "Background"];
        for (i, name) in rows.iter().enumerate() {
            let selected = i == 0;
            let frame = if selected {
                egui::Frame::new().fill(theme.accent.muted)
            } else {
                egui::Frame::new()
            };
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut visible = true;
                    let mut locked = false;
                    ui.toggle_value(&mut visible, icons::EYE.to_string());
                    ui.toggle_value(&mut locked, icons::LOCK.to_string());
                    ui.label(*name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt(("blend", i)).selected_text("Normal").show_ui(ui, |ui| {
                            let mut blend = 0_usize;
                            ui.selectable_value(&mut blend, 0, "Normal");
                            ui.selectable_value(&mut blend, 1, "Multiply");
                            ui.selectable_value(&mut blend, 2, "Screen");
                        });
                    });
                });
                let mut opacity = 255.0_f32;
                // The label rides in its own row at secondary color rather than as the
                // slider's `.text(...)`, which inherits the active-widget accent ink
                // inside a selected row and leaks violet onto a plain label.
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Opacity").size(theme.type_scale.label).color(theme.roles.text_secondary));
                    ui.add(egui::Slider::new(&mut opacity, 0.0..=255.0));
                });
            });
        }
    }
}

/// The Sprites panel. Mock content: a grid of six sprite thumbnails and a
/// `+ New Sprite` button that pushes a `RunAction` intent.
pub struct SpritesPanel;

impl Panel for SpritesPanel {
    fn id(&self) -> PanelId {
        SPRITES
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.sprites.title"),
            icon: icons::SPRITES,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::mock_thumbnail_grid(ui, theme, 6);
        if ui.button(format!("{} New Sprite", icons::ADD)).clicked() {
            scope.ctx.intents.push(Intent::RunAction(SPRITE_NEW));
        }
    }
}

/// The Palette panel. Mock content: the palette name "Bit", an 8x3 swatch grid
/// drawn from the representative `mock.palette` token set, an FG/BG indicator, and
/// the `Ramp` / `Harmony` / `Reduce to palette` buttons.
pub struct PalettePanel;

impl Panel for PalettePanel {
    fn id(&self) -> PanelId {
        PALETTE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.palette.title"),
            icon: icons::PALETTE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        ui.label("Bit");
        swatch_grid(ui, theme);
        ui.label(format!("FG {} BG", icons::CARET_RIGHT));
        ui.horizontal(|ui| {
            if ui.button("Ramp").clicked() {
                scope.ctx.intents.push(Intent::RunAction(PALETTE_RAMP));
            }
            if ui.button("Harmony").clicked() {
                scope.ctx.intents.push(Intent::RunAction(PALETTE_HARMONY));
            }
            if ui.button("Reduce to palette").clicked() {
                scope.ctx.intents.push(Intent::RunAction(PALETTE_REDUCE));
            }
        });
    }
}

/// Paint the 8x3 "Bit" swatch grid from the representative `mock.palette` token set
/// (24 colors). Swatch 0 (the foreground) carries an `accent.base` selection ring;
/// the rest get a hairline border.
fn swatch_grid(ui: &mut egui::Ui, theme: &pixhaus_ui::theme::Theme) {
    let cell = 18.0;
    let cols = 8;
    ui.vertical(|ui| {
        for row in 0..3 {
            ui.horizontal(|ui| {
                for col in 0..cols {
                    let i = row * cols + col;
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(cell), egui::Sense::hover());
                    if ui.is_rect_visible(rect) {
                        ui.painter().rect_filled(rect, theme.radius.sm, theme.mock.palette[i]);
                        // Swatch 0 reads as the selected foreground color.
                        let stroke = if i == 0 {
                            egui::Stroke::new(2.0, theme.accent.base)
                        } else {
                            egui::Stroke::new(1.0, theme.roles.border)
                        };
                        ui.painter().rect_stroke(rect, theme.radius.sm, stroke, egui::StrokeKind::Inside);
                    }
                }
            });
        }
    });
}

/// The Selection Actions panel. Mock content: a manual action row
/// (Cut/Copy/Paste/Invert/Crop) and an AI-marked row (Fill/Clean up/Make
/// seamless). Each button pushes a `RunAction` intent.
pub struct SelectionActionsPanel;

impl Panel for SelectionActionsPanel {
    fn id(&self) -> PanelId {
        SELECTION_ACTIONS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.selection-actions.title"),
            icon: icons::SELECT,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        ui.horizontal_wrapped(|ui| {
            for (label, action) in [
                ("Cut", SEL_CUT),
                ("Copy", SEL_COPY),
                ("Paste", SEL_PASTE),
                ("Invert", SEL_INVERT),
                ("Crop", SEL_CROP),
            ] {
                if ui.button(label).clicked() {
                    scope.ctx.intents.push(Intent::RunAction(action));
                }
            }
        });
        // The AI sub-row: each label leads with the sparkle in the AI accent.
        let ai_tint = theme.accent.ai;
        ui.horizontal_wrapped(|ui| {
            for (label, action) in [("Fill", SEL_AI_FILL), ("Clean up", SEL_AI_CLEANUP), ("Make seamless", SEL_AI_SEAMLESS)] {
                let text = egui::RichText::new(format!("{} {label}", icons::SPARKLE)).color(ai_tint);
                if ui.button(text).clicked() {
                    scope.ctx.intents.push(Intent::RunAction(action));
                }
            }
        });
    }
}

/// The AI Assistant panel: the UX quick-action list. Each row is a full-width
/// button that pushes a distinct `RunAction` intent (mock job + toast this
/// round). The header is sparkle-marked in the AI accent.
pub struct AiAssistantPanel;

impl Panel for AiAssistantPanel {
    fn id(&self) -> PanelId {
        AI_ASSISTANT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.ai-assistant.title"),
            icon: icons::SPARKLE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let actions = [
            ("Fill selection", AI_FILL),
            ("Clean up", AI_CLEANUP),
            ("Reduce colors", AI_REDUCE),
            ("Suggest ramp", AI_RAMP),
            ("Create variations", AI_VARIATIONS),
            ("Remove background", AI_REMOVE_BG),
        ];
        for (label, action) in actions {
            if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(label)).clicked() {
                scope.ctx.intents.push(Intent::RunAction(action));
            }
        }
    }
}

/// The Frames tray panel. Mock content: a horizontal strip of eight frame
/// thumbnails with the current frame (index 0) highlighted `accent.muted`, and
/// add/duplicate/delete controls that push `RunAction` intents.
pub struct FramesPanel;

impl Panel for FramesPanel {
    fn id(&self) -> PanelId {
        FRAMES
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.frames.title"),
            icon: icons::FRAMES,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        ui.horizontal(|ui| {
            if ui.button(format!("{} Add", icons::ADD)).clicked() {
                scope.ctx.intents.push(Intent::RunAction(FRAME_ADD));
            }
            if ui.button("Duplicate").clicked() {
                scope.ctx.intents.push(Intent::RunAction(FRAME_DUPLICATE));
            }
            if ui.button(format!("{} Delete", icons::CLOSE)).clicked() {
                scope.ctx.intents.push(Intent::RunAction(FRAME_DELETE));
            }
        });
        frame_strip(ui, theme);
    }
}

/// Paint a horizontal strip of eight mock frame thumbnails; the current frame
/// (index 0) is tinted with the accent.
fn frame_strip(ui: &mut egui::Ui, theme: &pixhaus_ui::theme::Theme) {
    let cell = 40.0;
    ui.horizontal(|ui| {
        for i in 0..8 {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(cell), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let fill = if i == 0 { theme.accent.muted } else { theme.surfaces.inset };
                ui.painter().rect_filled(rect, theme.radius.sm, fill);
                ui.painter()
                    .rect_stroke(rect, theme.radius.sm, egui::Stroke::new(1.0, theme.roles.border), egui::StrokeKind::Inside);
            }
        }
    });
}

/// The Assets tray panel. Mock content: a row of inert category chips and a grid
/// of asset thumbnails.
pub struct AssetsPanel;

impl Panel for AssetsPanel {
    fn id(&self) -> PanelId {
        ASSETS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.assets.title"),
            icon: icons::ASSETS,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        ui.horizontal(|ui| {
            // Inert category chips; "All" reads as selected.
            for (i, chip) in ["All", "Sprites", "Tiles", "Refs"].iter().enumerate() {
                let mut selected = i == 0;
                ui.selectable_value(&mut selected, true, *chip);
            }
        });
        widgets::mock_thumbnail_grid(ui, theme, 8);
    }
}

/// The Console tray panel: a scrolling mock log in monospace, secondary text.
pub struct ConsolePanel;

impl Panel for ConsolePanel {
    fn id(&self) -> PanelId {
        CONSOLE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.console.title"),
            icon: icons::CONSOLE,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        egui::ScrollArea::vertical().show(ui, |ui| {
            widgets::mock_log(ui, theme, &["info  backend ready", "info  project loaded"]);
        });
    }
}

/// Register the Draw workspace, the shared dock panels, the shared tray panels,
/// and the Sprite/Layer menu groups.
///
/// Order matters: `SpriteEditModule` registers first, so the shared panels exist
/// before any other workspace's layout references them by id (bible rule 2).
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(DrawWorkspace));

    // Shared dock panels.
    host.add_panel(Box::new(LayersPanel));
    host.add_panel(Box::new(SpritesPanel));
    host.add_panel(Box::new(PalettePanel));
    host.add_panel(Box::new(SelectionActionsPanel));
    host.add_panel(Box::new(AiAssistantPanel));

    // Shared tray panels.
    host.add_panel(Box::new(FramesPanel));
    host.add_panel(Box::new(AssetsPanel));
    host.add_panel(Box::new(ConsolePanel));

    // The actions the panels above dispatch (menu items and the AI quick-actions).
    for (id, label, icon) in [
        (LAYER_NEW, MsgKey("command.layer.new"), icons::ADD),
        (LAYER_DELETE, MsgKey("command.layer.delete"), icons::CLOSE),
        (LAYER_MERGE_DOWN, MsgKey("command.layer.merge-down"), icons::LAYERS),
        (SPRITE_NEW, MsgKey("command.sprite.new"), icons::ADD),
        (SPRITE_RESIZE, MsgKey("command.sprite.resize"), icons::TRANSFORM),
        (PALETTE_RAMP, MsgKey("command.palette.ramp"), icons::PALETTE),
        (PALETTE_HARMONY, MsgKey("command.palette.harmony"), icons::PALETTE),
        (PALETTE_REDUCE, MsgKey("command.palette.reduce"), icons::PALETTE),
        (SEL_CUT, MsgKey("command.selection.cut"), icons::CROP),
        (SEL_COPY, MsgKey("command.selection.copy"), icons::CROP),
        (SEL_PASTE, MsgKey("command.selection.paste"), icons::CROP),
        (SEL_INVERT, MsgKey("command.selection.invert"), icons::SELECT),
        (SEL_CROP, MsgKey("command.selection.crop"), icons::CROP),
        (SEL_AI_FILL, MsgKey("command.selection.ai-fill"), icons::SPARKLE),
        (SEL_AI_CLEANUP, MsgKey("command.selection.ai-clean-up"), icons::SPARKLE),
        (SEL_AI_SEAMLESS, MsgKey("command.selection.ai-make-seamless"), icons::SPARKLE),
        (AI_FILL, MsgKey("command.ai.fill-selection"), icons::SPARKLE),
        (AI_CLEANUP, MsgKey("command.ai.clean-up"), icons::SPARKLE),
        (AI_REDUCE, MsgKey("command.ai.reduce-colors"), icons::SPARKLE),
        (AI_RAMP, MsgKey("command.ai.suggest-ramp"), icons::SPARKLE),
        (AI_VARIATIONS, MsgKey("command.ai.create-variations"), icons::SPARKLE),
        (AI_REMOVE_BG, MsgKey("command.ai.remove-background"), icons::SPARKLE),
        (FRAME_ADD, MsgKey("command.frame.add"), icons::ADD),
        (FRAME_DUPLICATE, MsgKey("command.frame.duplicate"), icons::FRAMES),
        (FRAME_DELETE, MsgKey("command.frame.delete"), icons::CLOSE),
    ] {
        host.add_action(pixhaus_ui::contrib_api::ActionDesc {
            id,
            label,
            icon,
            palette_visible: true,
        });
    }

    // The menu groups this module owns (bible: the shared sprite-editing surface).
    host.add_menu_group(MenuGroup {
        label: MsgKey("app.menu.sprite"),
        items: vec![
            MenuItem {
                label: MsgKey("command.sprite.new"),
                shortcut: None,
                action: SPRITE_NEW,
            },
            MenuItem {
                label: MsgKey("command.sprite.resize"),
                shortcut: None,
                action: SPRITE_RESIZE,
            },
        ],
    });
    host.add_menu_group(MenuGroup {
        label: MsgKey("app.menu.layer"),
        items: vec![
            MenuItem {
                label: MsgKey("command.layer.new"),
                shortcut: None,
                action: LAYER_NEW,
            },
            MenuItem {
                label: MsgKey("command.layer.delete"),
                shortcut: None,
                action: LAYER_DELETE,
            },
            MenuItem {
                label: MsgKey("command.layer.merge-down"),
                shortcut: None,
                action: LAYER_MERGE_DOWN,
            },
        ],
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_layout_matches_the_inventory() {
        let layout = DrawWorkspace.layout();
        assert_eq!(layout.right_dock, vec![LAYERS, SPRITES, PALETTE, SELECTION_ACTIONS, AI_ASSISTANT]);
        assert_eq!(layout.bottom_tray, vec![FRAMES, ASSETS, CONSOLE]);
        assert_eq!(layout.default_tool, tools::PENCIL);
        assert_eq!(layout.primary_tools.len(), 15);
        assert_eq!(layout.status_items.len(), 1);
        assert_eq!(layout.status_items[0].text, MsgKey("workspace.draw.status.grid_on").tr());
    }

    #[test]
    fn draw_meta_uses_cmd_1() {
        assert_eq!(DrawWorkspace.id(), DRAW);
        assert_eq!(DrawWorkspace.meta().name, MsgKey("workspace.draw.title"));
        assert_eq!(DrawWorkspace.meta().shortcut, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1));
    }

    #[test]
    fn layers_panel_meta() {
        let meta = LayersPanel.meta();
        assert_eq!(LayersPanel.id(), LAYERS);
        assert_eq!(meta.title, MsgKey("panel.layers.title"));
        assert_eq!(meta.default_region, Region::RightDock);
        assert!(meta.default_open);
    }

    #[test]
    fn shared_dock_panel_ids_and_regions() {
        assert_eq!(SpritesPanel.id(), SPRITES);
        assert_eq!(PalettePanel.id(), PALETTE);
        assert_eq!(SelectionActionsPanel.id(), SELECTION_ACTIONS);
        assert_eq!(AiAssistantPanel.id(), AI_ASSISTANT);
        for p in [
            SpritesPanel.meta().default_region,
            PalettePanel.meta().default_region,
            SelectionActionsPanel.meta().default_region,
            AiAssistantPanel.meta().default_region,
        ] {
            assert_eq!(p, Region::RightDock);
        }
    }

    #[test]
    fn shared_tray_panel_ids_and_regions() {
        assert_eq!(FramesPanel.id(), FRAMES);
        assert_eq!(AssetsPanel.id(), ASSETS);
        assert_eq!(ConsolePanel.id(), CONSOLE);
        for p in [
            FramesPanel.meta().default_region,
            AssetsPanel.meta().default_region,
            ConsolePanel.meta().default_region,
        ] {
            assert_eq!(p, Region::BottomTray);
        }
    }
}
