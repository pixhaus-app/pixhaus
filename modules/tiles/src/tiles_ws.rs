//! The Tiles workspace and its panels.
//!
//! Tile editing is the shared sprite-editing core with tile-aware overlays and
//! validation (bible rule 2), so the Tiles workspace reuses the full 15-tool rail
//! and references sprite-edit's shared Assets and Console panels by id - they are
//! never re-registered here. This module adds the Tileset / Rule Type / Material /
//! Seam QA / AI Tile Assistant dock panels, the Tile Variants tray panel, and the
//! AI-tile actions.

use egui::{Key, KeyboardShortcut, Modifiers, Sense, Vec2};
use pixhaus_ui::contrib_api::{
    ActionDesc, ActionId, HostRegistrar, MsgKey, PENCIL, Panel, PanelId, PanelMeta, PanelScope, StatusItem, TOOL_RAIL, Workspace, WorkspaceId, WorkspaceLayout,
    WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::intent::Intent;
use pixhaus_ui::theme::Theme;
use pixhaus_ui::{icons, widgets};

/// The Tiles workspace id.
pub const TILES: WorkspaceId = WorkspaceId("tiles");

/// The Tileset dock panel id (tiles-owned).
pub const TILESET: PanelId = PanelId("tileset");
/// The Rule Type dock panel id (tiles-owned).
pub const RULE_TYPE: PanelId = PanelId("rule-type");
/// The Material dock panel id (tiles-owned).
pub const MATERIAL: PanelId = PanelId("material");
/// The Seam QA dock panel id (tiles-owned).
pub const SEAM_QA: PanelId = PanelId("seam-qa");
/// The AI Tile Assistant dock panel id (tiles-owned).
pub const AI_TILE_ASSISTANT: PanelId = PanelId("ai-tile-assistant");
/// The Tile Variants tray panel id (tiles-owned).
pub const TILE_VARIANTS: PanelId = PanelId("tile-variants");

// Shared panels owned by sprite-edit, referenced by id - never re-registered here.
const ASSETS: PanelId = PanelId("assets");
const CONSOLE: PanelId = PanelId("console");

// AI Tile Assistant quick-actions. Namespaced under `tile.` so they never collide
// with sprite-edit's `ai.*` action ids.
const TILE_MAKE_SEAMLESS: ActionId = ActionId("tile.make-seamless");
const TILE_VARIATIONS: ActionId = ActionId("tile.generate-variations");
const TILE_SUGGEST_MATERIAL: ActionId = ActionId("tile.suggest-material");
const TILE_FIX_SEAMS: ActionId = ActionId("tile.fix-seams");

// The Tileset "+ New Tile" affordance.
const TILE_NEW: ActionId = ActionId("tile.new");

/// The Tiles workspace: tileset and terrain authoring atop the shared editing core
/// (bible rule 2). Owns layout only, no data.
pub struct TilesWorkspace;

impl Workspace for TilesWorkspace {
    fn id(&self) -> WorkspaceId {
        TILES
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: MsgKey("workspace.tiles.title"),
            icon: icons::TILES,
            purpose: MsgKey("workspace.tiles.purpose"),
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![TILESET, RULE_TYPE, MATERIAL, SEAM_QA, AI_TILE_ASSISTANT],
            bottom_tray: vec![TILE_VARIANTS, ASSETS, CONSOLE],
            primary_tools: TOOL_RAIL.to_vec(),
            default_tool: PENCIL,
            status_items: vec![
                StatusItem {
                    icon: icons::GRID,
                    text: "Tile 16px".to_owned(),
                },
                StatusItem {
                    icon: icons::CHECK,
                    text: MsgKey("workspace.tiles.status.seams_ok").tr(),
                },
            ],
        }
    }
}

/// The Tileset dock panel. Mock content: a 4x4 checkerboard tile grid and a
/// `+ New Tile` button that pushes a `RunAction` intent.
pub struct TilesetPanel;

impl Panel for TilesetPanel {
    fn id(&self) -> PanelId {
        TILESET
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.tileset.title"),
            icon: icons::TILESET,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        tile_grid(ui, theme, 4, 4);
        if ui.button(format!("{} New Tile", icons::ADD)).clicked() {
            scope.ctx.intents.push(Intent::RunAction(TILE_NEW));
        }
    }
}

/// Paint a `cols`x`rows` grid of mock tile patches (alternating surface tiers).
// The grid dimensions and the radius token are small bounded constants; the f32
// casts of the loop indices and `theme.radius.sm` cannot truncate or lose a sign.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn tile_grid(ui: &mut egui::Ui, theme: &Theme, cols: usize, rows: usize) {
    let cell = 32.0;
    ui.vertical(|ui| {
        for row in 0..rows {
            ui.horizontal(|ui| {
                for col in 0..cols {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
                    if ui.is_rect_visible(rect) {
                        let fill = if (row + col) % 2 == 0 {
                            theme.surfaces.inset
                        } else {
                            theme.surfaces.elevated
                        };
                        ui.painter().rect_filled(rect, theme.radius.sm, fill);
                        ui.painter()
                            .rect_stroke(rect, theme.radius.sm, egui::Stroke::new(1.0, theme.roles.border), egui::StrokeKind::Inside);
                    }
                }
            });
        }
    });
}

/// The Rule Type dock panel. Mock content: a radio group over the four tiling
/// rules. The selection is throwaway local state (resets each frame; drives
/// nothing this round).
pub struct RuleTypePanel;

impl Panel for RuleTypePanel {
    fn id(&self) -> PanelId {
        RULE_TYPE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.rule-type.title"),
            icon: icons::GRID,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, _scope: &mut PanelScope<'_>) {
        // Throwaway local selection - mock radios that drive nothing.
        let mut rule = 0_usize;
        ui.radio_value(&mut rule, 0, "Single");
        ui.radio_value(&mut rule, 1, "Seamless");
        ui.radio_value(&mut rule, 2, "3x3 Autotile");
        ui.radio_value(&mut rule, 3, "47-blob");
    }
}

/// The Material dock panel. Mock content: a row of inert material chips.
pub struct MaterialPanel;

impl Panel for MaterialPanel {
    fn id(&self) -> PanelId {
        MATERIAL
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.material.title"),
            icon: icons::PALETTE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, _scope: &mut PanelScope<'_>) {
        ui.horizontal_wrapped(|ui| {
            // Inert chips; "Grass" reads as selected.
            for (i, chip) in ["Grass", "Stone", "Water", "Sand"].iter().enumerate() {
                let mut selected = i == 0;
                ui.selectable_value(&mut selected, true, *chip);
            }
        });
    }
}

/// The Seam QA dock panel. Mock content: a per-edge checklist with success and
/// warning badges drawn as colored dots (`roles.success` / `roles.warning`).
pub struct SeamQaPanel;

impl Panel for SeamQaPanel {
    fn id(&self) -> PanelId {
        SEAM_QA
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.seam-qa.title"),
            icon: icons::CHECK,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        seam_row(ui, theme, true, "Top");
        seam_row(ui, theme, true, "Left");
        seam_row(ui, theme, false, "Bottom seam");
    }
}

/// One seam-QA checklist row: a success or warning dot plus a label. `ok` picks
/// `roles.success`/`CHECK` vs `roles.warning`/`WARN`.
fn seam_row(ui: &mut egui::Ui, theme: &Theme, ok: bool, label: &str) {
    ui.horizontal(|ui| {
        let (glyph, color, text) = if ok {
            (icons::CHECK, theme.roles.success, format!("OK {label}"))
        } else {
            (icons::WARN, theme.roles.warning, format!("WARN {label}"))
        };
        ui.label(egui::RichText::new(glyph.to_string()).color(color));
        ui.label(egui::RichText::new(text).color(theme.roles.text_secondary));
    });
}

/// The AI Tile Assistant dock panel: the UX quick-action list. Each row is a
/// full-width button pushing a distinct `RunAction` intent (mock job this round).
/// The header is sparkle-marked in the AI accent.
pub struct AiTileAssistantPanel;

impl Panel for AiTileAssistantPanel {
    fn id(&self) -> PanelId {
        AI_TILE_ASSISTANT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.ai-tile-assistant.title"),
            icon: icons::SPARKLE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let actions = [
            ("Make seamless", TILE_MAKE_SEAMLESS),
            ("Generate variations", TILE_VARIATIONS),
            ("Suggest material", TILE_SUGGEST_MATERIAL),
            ("Fix seams", TILE_FIX_SEAMS),
        ];
        for (label, action) in actions {
            if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(label)).clicked() {
                scope.ctx.intents.push(Intent::RunAction(action));
            }
        }
    }
}

/// The Tile Variants tray panel. Mock content: a row of tile patches and a
/// seamless-tiling preview (a 2x2 repeated checkerboard block).
pub struct TileVariantsPanel;

impl Panel for TileVariantsPanel {
    fn id(&self) -> PanelId {
        TILE_VARIANTS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.tile-variants.title"),
            icon: icons::TILESET,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        ui.horizontal(|ui| {
            // A row of mock tile patches.
            widgets::mock_thumbnail_grid(ui, theme, 4);
            ui.separator();
            // The seamless-tiling preview: a 2x2 repeat of the same patch.
            seamless_preview(ui, theme);
        });
    }
}

/// Paint a seamless-tiling preview: a 2x2 grid of the same mock checker patch,
/// so the repeat reads as continuous.
// The 2x2 layout and the checker step are small bounded constants; the f32 casts
// cannot truncate or lose a sign.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn seamless_preview(ui: &mut egui::Ui, theme: &Theme) {
    let tile = 36.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(tile * 2.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect);
    let step = tile / 4.0;
    // Repeat one 4x4 checker across the 2x2 tile preview so seams align.
    for ty in 0..8 {
        for tx in 0..8 {
            let dark = (tx + ty) % 2 == 0;
            let fill = if dark { theme.surfaces.inset } else { theme.surfaces.elevated };
            let min = egui::pos2(rect.left() + tx as f32 * step, rect.top() + ty as f32 * step);
            painter.rect_filled(egui::Rect::from_min_size(min, Vec2::splat(step)), 0.0, fill);
        }
    }
    painter.rect_stroke(rect, theme.radius.sm, egui::Stroke::new(1.0, theme.roles.border), egui::StrokeKind::Inside);
}

/// Register the Tiles workspace, its six panels, and the AI-tile actions.
///
/// The shared Assets and Console panels are owned by sprite-edit and referenced by
/// id, not re-registered. Tiles contributes no menu group.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(TilesWorkspace));

    host.add_panel(Box::new(TilesetPanel));
    host.add_panel(Box::new(RuleTypePanel));
    host.add_panel(Box::new(MaterialPanel));
    host.add_panel(Box::new(SeamQaPanel));
    host.add_panel(Box::new(AiTileAssistantPanel));
    host.add_panel(Box::new(TileVariantsPanel));

    // The tile actions the panels above dispatch.
    for (id, label) in [
        (TILE_NEW, MsgKey("command.tile.new")),
        (TILE_MAKE_SEAMLESS, MsgKey("command.tile.make-seamless")),
        (TILE_VARIATIONS, MsgKey("command.tile.generate-variations")),
        (TILE_SUGGEST_MATERIAL, MsgKey("command.tile.suggest-material")),
        (TILE_FIX_SEAMS, MsgKey("command.tile.fix-seams")),
    ] {
        let icon = if id == TILE_NEW { icons::ADD } else { icons::SPARKLE };
        host.add_action(ActionDesc {
            id,
            label,
            icon,
            palette_visible: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_layout_matches_the_inventory() {
        let layout = TilesWorkspace.layout();
        assert_eq!(layout.right_dock, vec![TILESET, RULE_TYPE, MATERIAL, SEAM_QA, AI_TILE_ASSISTANT]);
        assert_eq!(layout.bottom_tray, vec![TILE_VARIANTS, ASSETS, CONSOLE]);
        assert_eq!(layout.default_tool, PENCIL);
        assert_eq!(layout.primary_tools.len(), 15);
        assert_eq!(layout.status_items.len(), 2);
        assert_eq!(layout.status_items[0].text, "Tile 16px");
        assert_eq!(layout.status_items[1].text, "Seams OK");
    }

    #[test]
    fn tiles_meta_uses_cmd_3() {
        assert_eq!(TilesWorkspace.id(), TILES);
        assert_eq!(TilesWorkspace.meta().name, MsgKey("workspace.tiles.title"));
        assert_eq!(TilesWorkspace.meta().shortcut, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3));
    }

    #[test]
    fn tile_panel_ids_and_regions() {
        assert_eq!(TilesetPanel.id(), TILESET);
        assert_eq!(RuleTypePanel.id(), RULE_TYPE);
        assert_eq!(MaterialPanel.id(), MATERIAL);
        assert_eq!(SeamQaPanel.id(), SEAM_QA);
        assert_eq!(AiTileAssistantPanel.id(), AI_TILE_ASSISTANT);
        assert_eq!(TileVariantsPanel.id(), TILE_VARIANTS);
        for region in [
            TilesetPanel.meta().default_region,
            RuleTypePanel.meta().default_region,
            MaterialPanel.meta().default_region,
            SeamQaPanel.meta().default_region,
            AiTileAssistantPanel.meta().default_region,
        ] {
            assert_eq!(region, Region::RightDock);
        }
        assert_eq!(TileVariantsPanel.meta().default_region, Region::BottomTray);
    }
}
