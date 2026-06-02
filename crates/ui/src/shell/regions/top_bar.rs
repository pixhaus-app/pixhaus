//! Top bar region: the menu strip, the workspace tab strip, and a thin global
//! status strip, in one elevated frame.

use pixhaus_services::i18n;

use crate::contrib_api::ids::{ActionId, MsgKey};
use crate::contrib_api::module::MenuGroup;
use crate::region::region_id;
use crate::shell::menus::{
    ACTION_PIXHAUS_ABOUT, ACTION_VIEW_THEME, ACTION_VIEW_TOGGLE_GRID, ACTION_VIEW_TOGGLE_KEYS, ACTION_WINDOW_COMMAND_PALETTE, ordered_menu_groups,
};
use crate::state::Host;
use crate::state::intent::{Intent, IntentSink};
use crate::state::ui_state::GridMode;
use crate::theme::Theme;
use crate::theme::tokens::{SurfaceTier, ThemeVariant};
use crate::widgets;

/// The brand group's stable key. Matched (not displayed) to pick the Pixhaus
/// group out of the menu bar so it renders as the app mark + wordmark, not a flat
/// item. The wordmark text itself is the `app.ui.brand.wordmark` key.
const BRAND_GROUP: MsgKey = MsgKey("app.menu.pixhaus");

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
        // Row 1: the menu strip - brand mark + wordmark on the left, then the menu
        // groups as flat text. The shell decides display order; modules just
        // contribute their groups into the registry in any order.
        ui.horizontal(|ui| {
            // Flat menu items: no resting frame, only a faint neutral hover/open
            // lift. Scoped so the override never leaks past this strip. Reserve
            // `accent` for the active tab/tool/AI - the hover stays neutral.
            ui.scope(|ui| {
                flatten_menu_visuals(ui, theme);

                let groups = ordered_menu_groups(&registries.menus);
                for group in groups {
                    if group.label == BRAND_GROUP {
                        brand(ui, theme, group, intents);
                    } else {
                        menu_group(ui, group, intents, state.ui.grid);
                    }
                }
            });
        });

        ui.add_space(theme.spacing.xs);

        // Row 2: workspace tab strip. Active = accent pill + underline.
        ui.horizontal(|ui| {
            let active = state.session.active_workspace;
            for ws in registries.workspaces.iter() {
                let meta = ws.meta();
                let id = ws.id();
                if widgets::workspace_tab(ui, theme, &meta.name.tr(), id == active).clicked() {
                    intents.push(Intent::SelectWorkspace(id));
                }
            }
        });

        ui.add_space(theme.spacing.xs);

        // Row 3: a thin global-status strip.
        ui.horizontal(|ui| {
            let status = if state.session.dirty {
                "app.ui.status.unsaved"
            } else {
                "app.ui.status.saved"
            };
            ui.colored_label(theme.roles.text_secondary, i18n::tr(status));
        });
    });
}

/// Strip the resting frame off menu buttons so the strip reads as one flat bar.
///
/// A `Button` (which `menu_button` is) paints `weak_bg_fill` + `bg_stroke` at rest,
/// giving each label its own box. Clearing those for the `inactive` state makes the
/// items frameless; `hovered`/`active`/`open` get a faint neutral lift (`surfaces.hover`)
/// so the pointer target still reads. `expansion = 0` keeps the hover from nudging the
/// label. Tightening `item_spacing.x` sits the labels closer, like a real menu bar.
// The radius token is a small, bounded positive constant; the f32 -> u8 cast cannot
// truncate or lose a sign here.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn flatten_menu_visuals(ui: &mut egui::Ui, theme: &Theme) {
    let widgets = &mut ui.style_mut().visuals.widgets;
    let radius = egui::CornerRadius::same(theme.radius.sm as u8);

    widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
    widgets.inactive.bg_stroke = egui::Stroke::NONE;
    widgets.inactive.expansion = 0.0;
    widgets.inactive.corner_radius = radius;

    for state in [&mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
        state.weak_bg_fill = theme.surfaces.hover;
        state.bg_stroke = egui::Stroke::NONE;
        state.expansion = 0.0;
        state.corner_radius = radius;
    }

    ui.spacing_mut().item_spacing.x = theme.spacing.xs;
}

/// Draw the left brand: the Bit icon mark + the "Pixhaus" wordmark, as a single menu
/// button that opens the Pixhaus app menu (About / Preferences).
fn brand(ui: &mut egui::Ui, theme: &Theme, group: &MenuGroup, intents: &mut IntentSink) {
    // The Bit face is detailed, so the mark needs more room than the 15px title size
    // the flat-text labels use; 20px reads cleanly against the elevated bar.
    let mark_side = BRAND_MARK_SIDE;
    let label = egui::RichText::new(i18n::tr("app.ui.brand.wordmark")).strong().size(theme.type_scale.body);

    // The mark sits flush against the wordmark, so drop the inter-item gap just here.
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        brand_mark(ui, theme, mark_side);
        ui.menu_button(label, |ui| menu_items(ui, group, intents, GridMode::Off));
    });
}

/// The brand-mark side in points. Tuned against the headless render: the Bit face is
/// detailed, so 20px keeps it legible where the ~15px title size goes muddy.
const BRAND_MARK_SIDE: f32 = 20.0;

/// Draw the Bit icon mark at `side` points, on a faint backing chip.
///
/// NEAREST sampling is load-bearing: it keeps the pixel art crisp instead of letting
/// egui's default bilinear filter soften the hard edges. A faint rounded chip behind
/// the mark lifts the dark sprite outline off the elevated bar. The rect is allocated
/// first so the chip paints under the image in one painter pass.
// Radius token is a small, bounded positive constant; the f32 -> u8 cast cannot
// truncate or lose a sign here.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn brand_mark(ui: &mut egui::Ui, theme: &Theme, side: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, theme.radius.sm as u8, theme.surfaces.inset);
        egui::Image::new(crate::brand::ICON)
            .texture_options(egui::TextureOptions::NEAREST)
            .maintain_aspect_ratio(true)
            .paint_at(ui, rect);
    }
}

/// Draw one menu group as a flat-text `menu_button` with its dropdown items.
fn menu_group(ui: &mut egui::Ui, group: &MenuGroup, intents: &mut IntentSink, current_grid: GridMode) {
    ui.menu_button(group.label.tr(), |ui| menu_items(ui, group, intents, current_grid));
}

/// Render a group's dropdown items. `View > Theme` expands to the variant submenu;
/// every other item maps to its intent and closes the menu. The dropdown opens in
/// its own popup area off the context style, so the flattened bar visuals scoped
/// onto the strip's `ui` don't reach here - menu items keep their normal look.
fn menu_items(ui: &mut egui::Ui, group: &MenuGroup, intents: &mut IntentSink, current_grid: GridMode) {
    for menu_item in &group.items {
        if menu_item.action == ACTION_VIEW_THEME {
            ui.menu_button(menu_item.label.tr(), |ui| {
                for (key, variant) in [
                    ("app.ui.theme.dark", ThemeVariant::Dark),
                    ("app.ui.theme.light", ThemeVariant::Light),
                    ("app.ui.theme.accent", ThemeVariant::AccentHighContrast),
                ] {
                    if ui.button(i18n::tr(key)).clicked() {
                        intents.push(Intent::SetThemeVariant(variant));
                        ui.close();
                    }
                }
            });
        } else if ui.button(menu_item.label.tr()).clicked() {
            push_menu_intent(intents, menu_item.action, current_grid);
            ui.close();
        }
    }
}

/// Map a menu item to its intent. The live items (`Toggle Grid`, `Command
/// Palette`) carry real intents; everything else is a mock `RunAction`.
fn push_menu_intent(intents: &mut IntentSink, action: ActionId, current_grid: GridMode) {
    match action {
        ACTION_PIXHAUS_ABOUT => intents.push(Intent::OpenAbout),
        ACTION_VIEW_TOGGLE_GRID => intents.push(Intent::SetGrid(toggle_grid(current_grid))),
        ACTION_VIEW_TOGGLE_KEYS => intents.push(Intent::ToggleI18nKeys),
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
