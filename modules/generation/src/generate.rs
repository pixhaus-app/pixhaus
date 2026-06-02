//! The Generate workspace and its panels.
//!
//! Generate is the AI-forward workspace (bible 6.5, 14): the rail is the minimal
//! navigation subset of sprite-edit's shared tools, and the dock is the prompt
//! composer plus the recipe/structure/style libraries and generation settings.
//! Every panel is a `&self` unit struct reading through `scope.ctx` and pushing
//! [`Intent`]s; the Prompt panel is the worked scratch carve-out (the one `&mut
//! String` a panel may touch, because [`egui::TextEdit`] needs a live buffer
//! in-frame). The Console tray panel is owned by sprite-edit and referenced by id,
//! never re-registered.
//!
//! [`Intent`]: pixhaus_ui::state::intent::Intent

use egui::{Key, KeyboardShortcut, Modifiers, Sense, Stroke, StrokeKind, Vec2};
use pixhaus_ui::contrib_api::{
    ActionDesc, ActionId, HostRegistrar, Panel, PanelId, PanelMeta, PanelScope, StatusItem, ToolId, Workspace, WorkspaceId, WorkspaceLayout, WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::intent::Intent;
use pixhaus_ui::theme::Theme;
use pixhaus_ui::{icons, widgets};

/// The Generate workspace id.
pub const GENERATE: WorkspaceId = WorkspaceId("generate");

/// The Prompt dock panel id (generation-owned; the scratch carve-out example).
pub const PROMPT: PanelId = PanelId("prompt");
/// The Recipe dock panel id (generation-owned).
pub const RECIPE: PanelId = PanelId("recipe");
/// The Structure dock panel id (generation-owned).
pub const STRUCTURE: PanelId = PanelId("structure");
/// The Style dock panel id (generation-owned).
pub const STYLE: PanelId = PanelId("style");
/// The Palette Behavior dock panel id (generation-owned).
pub const PALETTE_BEHAVIOR: PanelId = PanelId("palette-behavior");
/// The Advanced Settings dock panel id (generation-owned, collapsed by default).
pub const ADVANCED_SETTINGS: PanelId = PanelId("advanced-settings");
/// The Results tray panel id (generation-owned).
pub const RESULTS: PanelId = PanelId("results");
/// The History tray panel id (generation-owned).
pub const HISTORY: PanelId = PanelId("history");

// Tools owned by sprite-edit, referenced by id - never re-registered here. The
// Generate rail is a navigation subset: pan, zoom, select, and the AI brush.
const HAND: ToolId = ToolId("hand");
const ZOOM: ToolId = ToolId("zoom");
const SELECTION: ToolId = ToolId("selection");
const AI_BRUSH: ToolId = ToolId("ai_brush");

// The Console tray panel is owned by sprite-edit, referenced by id.
const CONSOLE: PanelId = PanelId("console");

// The actions this module's panels dispatch. Namespaced under `gen.` so they never
// collide with sprite-edit's `ai.*` or any other module's action ids.
const GEN_GENERATE: ActionId = ActionId("gen.generate");
const GEN_USE_SELECTED: ActionId = ActionId("gen.use-selected");
const GEN_INSERT_SPRITE: ActionId = ActionId("gen.insert-sprite");
const GEN_CREATE_VARIATIONS: ActionId = ActionId("gen.create-variations");
const GEN_GENERATE_MORE: ActionId = ActionId("gen.generate-more");

/// The Generate workspace: AI-forward sprite generation. Owns layout only, no data
/// (bible rule 2). The rail is the minimal navigation subset of the shared tools.
pub struct GenerateWorkspace;

impl Workspace for GenerateWorkspace {
    fn id(&self) -> WorkspaceId {
        GENERATE
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: "Generate",
            icon: icons::GENERATE,
            purpose: "Generate sprites from a prompt",
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num4),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![PROMPT, RECIPE, STRUCTURE, STYLE, PALETTE_BEHAVIOR, ADVANCED_SETTINGS],
            bottom_tray: vec![RESULTS, HISTORY, CONSOLE],
            primary_tools: vec![HAND, ZOOM, SELECTION, AI_BRUSH],
            default_tool: HAND,
            status_items: vec![
                StatusItem {
                    icon: icons::STATUS_DOT,
                    text: "AI Ready".to_owned(),
                },
                StatusItem {
                    icon: icons::STATUS_DOT,
                    text: "Seed 123456".to_owned(),
                },
            ],
        }
    }
}

/// The Prompt panel. Worked example of the scratch carve-out: the multiline
/// `TextEdit` binds to `scope.scratch` - the only `&mut String` a panel may touch,
/// because `TextEdit` needs a live buffer in-frame. The `{variable}` chips are
/// shown inert below the field, and the accent Generate button pushes a
/// `RunAction` intent (never a model mutation).
pub struct PromptPanel;

impl Panel for PromptPanel {
    fn id(&self) -> PanelId {
        PROMPT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Prompt",
            icon: icons::PROMPT,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;

        // The carve-out: TextEdit needs a live &mut String in-frame. scope.scratch
        // is this panel's own draft buffer - the single, disjoint exception to the
        // intents-only write channel. Never route real mutation through it.
        ui.add(
            egui::TextEdit::multiline(scope.scratch)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text("Describe the sprite. Use {variable} chips."),
        );

        // The {variable} chips: inert markers showing the recognized substitutions.
        ui.horizontal_wrapped(|ui| {
            for chip in ["{subject}", "{style}", "{palette}", "{pose}"] {
                let text = egui::RichText::new(chip).size(theme.type_scale.label).color(theme.accent.base);
                let _ = ui.selectable_label(false, text);
            }
        });

        ui.add_space(theme.spacing.sm);

        // The primary accent Generate button with the sparkle marker.
        let label = egui::RichText::new(format!("{} Generate", icons::SPARKLE)).color(theme.roles.text_primary);
        let button = egui::Button::new(label).fill(theme.accent.base);
        if ui.add_sized([ui.available_width(), 28.0], button).clicked() {
            scope.ctx.intents.push(Intent::RunAction(GEN_GENERATE));
        }
    }
}

/// The Recipe panel. Mock content: a card list of generation recipes, each a
/// thumbnail plus a name and a built-in (locked) vs user badge.
pub struct RecipePanel;

impl Panel for RecipePanel {
    fn id(&self) -> PanelId {
        RECIPE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Recipe",
            icon: icons::HISTORY,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        // (name, built_in): built-ins are locked; user recipes are editable.
        for (name, built_in) in [("Hero sprite", true), ("Top-down tile", true), ("My walk cycle", false)] {
            recipe_row(ui, theme, name, built_in);
        }
    }
}

/// The Structure panel. Mock content: a card list of structure templates with
/// preview thumbnails.
pub struct StructurePanel;

impl Panel for StructurePanel {
    fn id(&self) -> PanelId {
        STRUCTURE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Structure",
            icon: icons::ASSETS,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::mock_thumbnail_grid(ui, theme, 4);
    }
}

/// The Style panel. Mock content: a card list of style references with preview
/// thumbnails.
pub struct StylePanel;

impl Panel for StylePanel {
    fn id(&self) -> PanelId {
        STYLE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Style",
            icon: icons::PALETTE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::mock_thumbnail_grid(ui, theme, 4);
    }
}

/// One recipe card row: a thumbnail patch, the name, and a built-in/user badge.
/// `built_in` reads as a locked `Built-in` chip in disabled text; a user recipe
/// reads as a `User` chip in the accent.
// The 28px thumbnail and the radius token are small bounded constants; the casts
// cannot truncate or lose a sign.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn recipe_row(ui: &mut egui::Ui, theme: &Theme, name: &str, built_in: bool) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, theme.radius.sm, theme.surfaces.inset);
            ui.painter()
                .rect_stroke(rect, theme.radius.sm, Stroke::new(1.0, theme.roles.border), StrokeKind::Inside);
        }
        ui.label(egui::RichText::new(name).color(theme.roles.text_primary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (glyph, text, color) = if built_in {
                (icons::LOCK, "Built-in", theme.roles.text_disabled)
            } else {
                (icons::ADD, "User", theme.accent.base)
            };
            ui.label(egui::RichText::new(format!("{glyph} {text}")).size(theme.type_scale.label).color(color));
        });
    });
}

/// The Palette Behavior panel. Mock content: the inert checkbox set governing how
/// generation interacts with the active palette. The checkboxes bind to throwaway
/// locals that reset each frame - correct for mock content this round.
pub struct PaletteBehaviorPanel;

impl Panel for PaletteBehaviorPanel {
    fn id(&self) -> PanelId {
        PALETTE_BEHAVIOR
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Palette Behavior",
            icon: icons::PALETTE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, _scope: &mut PanelScope<'_>) {
        // (label, default-checked). Inert locals; they reset each frame.
        for (label, default) in [
            ("Use current palette only", true),
            ("Add colors automatically", false),
            ("Suggest palette expansion", false),
            ("Dither to palette", false),
            ("Reduce after generation", false),
        ] {
            let mut checked = default;
            ui.checkbox(&mut checked, label);
        }
    }
}

/// The Advanced Settings panel. Collapsed by default (`default_open: false`). Mock
/// content: the generation parameter fields - Seed, Steps, Strength, Negative
/// prompt, and Model - as inert labels and value widgets.
pub struct AdvancedSettingsPanel;

impl Panel for AdvancedSettingsPanel {
    fn id(&self) -> PanelId {
        ADVANCED_SETTINGS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Advanced Settings",
            icon: icons::SETTINGS,
            default_region: Region::RightDock,
            default_open: false,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        // Inert mock fields; the values reset each frame (drive nothing this round).
        ui.horizontal(|ui| {
            ui.label("Seed");
            let mut seed = 123_456_i32;
            ui.add(egui::DragValue::new(&mut seed).speed(1));
        });
        ui.horizontal(|ui| {
            ui.label("Steps");
            let mut steps = 30_i32;
            ui.add(egui::DragValue::new(&mut steps).range(1..=150));
        });
        ui.horizontal(|ui| {
            ui.label("Strength");
            let mut strength = 0.65_f32;
            ui.add(egui::DragValue::new(&mut strength).speed(0.01).range(0.0..=1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Negative prompt");
            ui.label(egui::RichText::new("blurry, jpeg").color(theme.roles.text_secondary));
        });
        ui.horizontal(|ui| {
            ui.label("Model");
            ui.label(egui::RichText::new("pixhaus-default").color(theme.roles.text_secondary));
        });
    }
}

/// The Results tray panel. Mock content: an 8-card grid of generated results (each
/// shows its number, seed, a sparkle, and a star), the first card ringed as
/// selected with the accent, and an action row (Use selected / Insert as new sprite
/// / Create variations / Generate more) whose buttons push `RunAction` intents.
pub struct ResultsPanel;

impl Panel for ResultsPanel {
    fn id(&self) -> PanelId {
        RESULTS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Results",
            icon: icons::RESULTS,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        ui.horizontal_wrapped(|ui| {
            for i in 0..8 {
                result_card(ui, theme, i, i == 0);
            }
        });
        ui.add_space(theme.spacing.sm);
        ui.horizontal_wrapped(|ui| {
            for (label, action) in [
                ("Use selected", GEN_USE_SELECTED),
                ("Insert as new sprite", GEN_INSERT_SPRITE),
                ("Create variations", GEN_CREATE_VARIATIONS),
                ("Generate more", GEN_GENERATE_MORE),
            ] {
                if ui.button(label).clicked() {
                    scope.ctx.intents.push(Intent::RunAction(action));
                }
            }
        });
    }
}

/// Paint one result card: a checker thumbnail with its number, seed, a sparkle and
/// a star overlay, and an accent selection ring when `selected`.
// The 72px card, the inset offsets, and the radius token are small bounded
// constants; the index-to-f32 casts cannot truncate or lose a sign.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn result_card(ui: &mut egui::Ui, theme: &Theme, index: usize, selected: bool) {
    let card = 72.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(card), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect);
    // The checker thumbnail backdrop so transparency reads.
    let step = card / 4.0;
    for ty in 0..4 {
        for tx in 0..4 {
            let dark = (tx + ty) % 2 == 0;
            let fill = if dark { theme.surfaces.inset } else { theme.surfaces.elevated };
            let min = egui::pos2(rect.left() + tx as f32 * step, rect.top() + ty as f32 * step);
            painter.rect_filled(egui::Rect::from_min_size(min, Vec2::splat(step)), 0.0, fill);
        }
    }
    // The number (top-left), the sparkle (top-right, AI accent), the star
    // (bottom-right), the seed (bottom-left).
    let label_size = theme.type_scale.label;
    painter.text(
        rect.left_top() + Vec2::new(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("#{}", index + 1),
        egui::FontId::proportional(label_size),
        theme.roles.text_primary,
    );
    painter.text(
        rect.right_top() + Vec2::new(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        icons::SPARKLE.to_string(),
        egui::FontId::proportional(label_size),
        theme.accent.ai,
    );
    painter.text(
        rect.right_bottom() + Vec2::new(-4.0, -2.0),
        egui::Align2::RIGHT_BOTTOM,
        icons::STAR.to_string(),
        egui::FontId::proportional(label_size),
        theme.roles.text_secondary,
    );
    painter.text(
        rect.left_bottom() + Vec2::new(4.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        format!("s{}", 123_456 + index),
        egui::FontId::monospace(label_size),
        theme.roles.text_secondary,
    );
    // The selection ring on the selected card, otherwise a hairline border.
    let stroke = if selected {
        Stroke::new(2.0, theme.accent.base)
    } else {
        Stroke::new(1.0, theme.roles.border)
    };
    painter.rect_stroke(rect, theme.radius.sm, stroke, StrokeKind::Inside);
}

/// The History tray panel. Mock content: a list of prior generations, each a
/// monospace row of the prompt summary, the seed, and a timestamp.
pub struct HistoryPanel;

impl Panel for HistoryPanel {
    fn id(&self) -> PanelId {
        HISTORY
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "History",
            icon: icons::HISTORY,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        egui::ScrollArea::vertical().show(ui, |ui| {
            widgets::mock_log(
                ui,
                theme,
                &[
                    "knight idle     s123456  10:02",
                    "knight walk     s123457  10:05",
                    "slime bounce    s123458  10:11",
                    "torch flicker   s123459  10:19",
                ],
            );
        });
    }
}

/// Register the Generate workspace, its six dock panels, the Results/History tray
/// panels, and the `gen.*` actions.
///
/// The shared Console tray panel and the Hand/Zoom/Selection/AI-brush tools are
/// owned by sprite-edit and referenced by id, never re-registered here. Generation
/// contributes no menu group this round.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(GenerateWorkspace));

    // Dock panels.
    host.add_panel(Box::new(PromptPanel));
    host.add_panel(Box::new(RecipePanel));
    host.add_panel(Box::new(StructurePanel));
    host.add_panel(Box::new(StylePanel));
    host.add_panel(Box::new(PaletteBehaviorPanel));
    host.add_panel(Box::new(AdvancedSettingsPanel));

    // Tray panels Generation owns.
    host.add_panel(Box::new(ResultsPanel));
    host.add_panel(Box::new(HistoryPanel));

    // The `gen.*` actions the panels above dispatch. The user-facing ones are
    // command-palette visible.
    for (id, label) in [
        (GEN_GENERATE, "Generate sprite from prompt"),
        (GEN_USE_SELECTED, "Use selected result"),
        (GEN_INSERT_SPRITE, "Insert result as new sprite"),
        (GEN_CREATE_VARIATIONS, "Create variations"),
        (GEN_GENERATE_MORE, "Generate more"),
    ] {
        host.add_action(ActionDesc {
            id,
            label,
            icon: icons::SPARKLE,
            palette_visible: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_layout_matches_the_inventory() {
        let layout = GenerateWorkspace.layout();
        assert_eq!(layout.right_dock, vec![PROMPT, RECIPE, STRUCTURE, STYLE, PALETTE_BEHAVIOR, ADVANCED_SETTINGS]);
        assert_eq!(layout.bottom_tray, vec![RESULTS, HISTORY, CONSOLE]);
        assert_eq!(layout.default_tool, HAND);
        assert_eq!(layout.primary_tools, vec![HAND, ZOOM, SELECTION, AI_BRUSH]);
        assert_eq!(layout.status_items.len(), 2);
        assert_eq!(layout.status_items[0].text, "AI Ready");
        assert_eq!(layout.status_items[1].text, "Seed 123456");
    }

    #[test]
    fn generate_meta_uses_cmd_4() {
        assert_eq!(GenerateWorkspace.id(), GENERATE);
        assert_eq!(GenerateWorkspace.meta().name, "Generate");
        assert_eq!(GenerateWorkspace.meta().shortcut, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num4));
    }

    #[test]
    fn prompt_panel_meta() {
        assert_eq!(PromptPanel.id(), PROMPT);
        assert_eq!(PromptPanel.meta().title, "Prompt");
        assert_eq!(PromptPanel.meta().default_region, Region::RightDock);
        assert!(PromptPanel.meta().default_open);
    }

    #[test]
    fn advanced_settings_is_collapsed_by_default() {
        assert_eq!(AdvancedSettingsPanel.id(), ADVANCED_SETTINGS);
        assert!(!AdvancedSettingsPanel.meta().default_open, "Advanced Settings starts collapsed");
    }

    #[test]
    fn dock_panel_ids_and_regions() {
        assert_eq!(RecipePanel.id(), RECIPE);
        assert_eq!(StructurePanel.id(), STRUCTURE);
        assert_eq!(StylePanel.id(), STYLE);
        assert_eq!(PaletteBehaviorPanel.id(), PALETTE_BEHAVIOR);
        for region in [
            RecipePanel.meta().default_region,
            StructurePanel.meta().default_region,
            StylePanel.meta().default_region,
            PaletteBehaviorPanel.meta().default_region,
            AdvancedSettingsPanel.meta().default_region,
        ] {
            assert_eq!(region, Region::RightDock);
        }
    }

    #[test]
    fn tray_panel_ids_and_regions() {
        assert_eq!(ResultsPanel.id(), RESULTS);
        assert_eq!(HistoryPanel.id(), HISTORY);
        assert_eq!(ResultsPanel.meta().default_region, Region::BottomTray);
        assert_eq!(HistoryPanel.meta().default_region, Region::BottomTray);
    }
}
