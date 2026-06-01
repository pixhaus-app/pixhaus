//! Top bar region: the menu strip, the workspace tab strip, and a thin global
//! status strip, in one elevated frame.

use crate::contrib_api::ids::ActionId;
use crate::region::region_id;
use crate::shell::menus::{ACTION_VIEW_THEME, ACTION_VIEW_TOGGLE_GRID, ACTION_WINDOW_COMMAND_PALETTE};
use crate::state::Host;
use crate::state::intent::{Intent, IntentSink};
use crate::state::ui_state::GridMode;
use crate::theme::tokens::{SurfaceTier, ThemeVariant};
use crate::widgets;

/// Render the top-bar region.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Elevated)).inner_margin(theme.spacing.sm);

    egui::Panel::top(region_id::TOP_BAR).frame(frame).show_inside(ui, |ui| {
        // Row 1: menu strip. Shell groups, then module-contributed groups.
        ui.horizontal(|ui| {
            for group in &registries.menus {
                ui.menu_button(group.label, |ui| {
                    for menu_item in &group.items {
                        if menu_item.action == ACTION_VIEW_THEME {
                            ui.menu_button("Theme", |ui| {
                                for (label, variant) in [
                                    ("Dark", ThemeVariant::Dark),
                                    ("Light", ThemeVariant::Light),
                                    ("Accent", ThemeVariant::AccentHighContrast),
                                ] {
                                    if ui.button(label).clicked() {
                                        intents.push(Intent::SetThemeVariant(variant));
                                        ui.close();
                                    }
                                }
                            });
                        } else if ui.button(menu_item.label).clicked() {
                            push_menu_intent(intents, menu_item.action, state.ui.grid);
                            ui.close();
                        }
                    }
                });
            }
        });

        ui.add_space(theme.spacing.xs);

        // Row 2: workspace tab strip. Active = accent pill + underline.
        ui.horizontal(|ui| {
            let active = state.session.active_workspace;
            for ws in registries.workspaces.iter() {
                let meta = ws.meta();
                let id = ws.id();
                if widgets::workspace_tab(ui, theme, meta.name, id == active).clicked() {
                    intents.push(Intent::SelectWorkspace(id));
                }
            }
        });

        ui.add_space(theme.spacing.xs);

        // Row 3: a thin global-status strip.
        ui.horizontal(|ui| {
            ui.colored_label(theme.roles.text_secondary, if state.session.dirty { "Unsaved changes" } else { "Saved" });
        });
    });
}

/// Map a menu item to its intent. The live items (`Toggle Grid`, `Command
/// Palette`) carry real intents; everything else is a mock `RunAction`.
fn push_menu_intent(intents: &mut IntentSink, action: ActionId, current_grid: GridMode) {
    match action {
        ACTION_VIEW_TOGGLE_GRID => intents.push(Intent::SetGrid(toggle_grid(current_grid))),
        ACTION_WINDOW_COMMAND_PALETTE => intents.push(Intent::OpenCommandPalette),
        other => intents.push(Intent::RunAction(other)),
    }
}

/// Toggle Grid flips between off and the 8px minor grid (the default on-state).
fn toggle_grid(current: GridMode) -> GridMode {
    match current {
        GridMode::Off => GridMode::Px8,
        GridMode::Px8 | GridMode::Px16 => GridMode::Off,
    }
}
