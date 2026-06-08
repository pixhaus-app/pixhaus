//! Codex-workspace shared widgets: reference chips, anchor/status badges, the
//! strength selector, the coverage checklist, the entry card, and the mini hover card
//! (bible 13.5, 21.2). All paint from theme tokens only - no hex - so a theme swap
//! recolors them for free. They take plain `core` values (not session structs) so
//! both the Codex panels and the Generate context panel can reuse them.

use pixhaus_core::codex::details::{ColorRole, PaletteColor};
use pixhaus_core::codex::{AnchorKind, AnchorStrength, CoverageLabel, EntryStatus, EntryType};

use crate::icons;
use crate::theme::Theme;

/// Resolve a [`CoverageLabel`] to display text: a [`CoverageLabel::Key`] goes through
/// the localization service (`tr`), a [`CoverageLabel::Literal`] is returned as-is. This
/// is the one rule for rendering a slot label - a literal is never passed to `tr()`, so
/// user-created labels do not log missing-key spam. Every slot-label render goes through
/// it.
#[must_use]
pub fn resolve_coverage_label(label: &CoverageLabel) -> String {
    match label {
        CoverageLabel::Key(key) => pixhaus_services::i18n::tr(key),
        CoverageLabel::Literal(text) => text.clone(),
    }
}

/// What the slot editor's row reports back to the caller: which control was clicked.
/// `None` means no interaction this frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SlotEditAction {
    /// The rename (pencil) control was clicked.
    Rename,
    /// The remove (trash) control was clicked.
    Remove,
    /// The move-up control was clicked.
    MoveUp,
    /// The move-down control was clicked.
    MoveDown,
}

/// A corner radius from a theme radius token. The radius tokens are small bounded
/// positive constants (2/3 px), so the f32 -> u8 conversion is exact and never
/// truncates or loses a sign.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn radius(px: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(px as u8)
}

/// A symmetric margin from theme spacing tokens. The spacing scale is small bounded
/// positive constants (2..16 px), so the f32 -> i8 conversion is exact.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn margin(x: f32, y: f32) -> egui::Margin {
    egui::Margin::symmetric(x as i8, y as i8)
}

/// A decorative type-color for an entry type, drawn from the theme's mock thumbnail
/// tints (decorative, not a semantic role) so the type chips read with distinct color
/// without minting new role tokens. Stable per type: the same type always maps to the
/// same tint.
#[must_use]
pub fn type_color(theme: &Theme, entry_type: EntryType) -> egui::Color32 {
    let tints = &theme.mock.thumbnails;
    // Index by the type's position in `EntryType::all`, wrapped into the tint set.
    let idx = EntryType::all().iter().position(|t| *t == entry_type).unwrap_or(0);
    tints[idx % tints.len()]
}

/// The phosphor glyph that represents an entry type in the type tree and chips.
#[must_use]
pub fn type_icon(entry_type: EntryType) -> char {
    match entry_type {
        EntryType::Character | EntryType::Npc | EntryType::Enemy | EntryType::Creature => icons::CHARACTER,
        EntryType::Palette => icons::PALETTE,
        EntryType::Style | EntryType::Vibe | EntryType::Vfx => icons::SPARKLE,
        EntryType::Animation | EntryType::Pose => icons::TIMELINE,
        EntryType::Location | EntryType::Biome => icons::LOCATION,
        EntryType::Material => icons::MATERIAL,
        EntryType::Rule => icons::RULE,
        EntryType::Recipe => icons::HISTORY,
        EntryType::ReferenceBoard => icons::BOARD,
        EntryType::Faction => icons::TAG,
        EntryType::Prop | EntryType::Item | EntryType::Weapon | EntryType::UiElement => icons::ASSETS,
    }
}

/// The role color for an entry status: canonical reads as success, deprecated/archived
/// as warning, rejected as error, and draft/candidate as muted secondary text.
#[must_use]
pub fn status_color(theme: &Theme, status: EntryStatus) -> egui::Color32 {
    match status {
        EntryStatus::Canonical => theme.roles.success,
        EntryStatus::Candidate => theme.accent.base,
        EntryStatus::Draft => theme.roles.text_secondary,
        EntryStatus::Deprecated | EntryStatus::Archived => theme.roles.warning,
        EntryStatus::Rejected => theme.roles.error,
    }
}

/// The color for an anchor strength: Loose reads as disabled, Normal as secondary,
/// Strong as the accent, Locked as the full accent (the firmest hold).
#[must_use]
pub fn strength_color(theme: &Theme, strength: AnchorStrength) -> egui::Color32 {
    match strength {
        AnchorStrength::Loose => theme.roles.text_disabled,
        AnchorStrength::Normal => theme.roles.text_secondary,
        AnchorStrength::Strong => theme.accent.hover,
        AnchorStrength::Locked => theme.accent.base,
    }
}

/// A removable reference chip: a type-colored dot, the `@handle`, and an `x`. Returns
/// `true` when the remove control was clicked. Used by the Generate context stack and
/// the Codex inspector's references list (bible 6.2).
pub fn reference_chip(ui: &mut egui::Ui, theme: &Theme, handle: &str, entry_type: EntryType, removable: bool) -> bool {
    let mut removed = false;
    let fill = theme.surface(crate::theme::tokens::SurfaceTier::Inset);
    let frame = egui::Frame::new()
        .fill(fill)
        .inner_margin(margin(theme.spacing.sm, theme.spacing.xs))
        .corner_radius(radius(theme.radius.md))
        .stroke(egui::Stroke::new(1.0, theme.roles.border));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            // The type-colored mention marker.
            ui.label(
                egui::RichText::new(icons::REFERENCE.to_string())
                    .size(theme.type_scale.label)
                    .color(type_color(theme, entry_type)),
            );
            ui.label(egui::RichText::new(handle).size(theme.type_scale.label).color(theme.roles.text_primary));
            if removable
                && ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(icons::CLOSE.to_string())
                                .size(theme.type_scale.label)
                                .color(theme.roles.text_secondary),
                        )
                        .frame(false),
                    )
                    .clicked()
            {
                removed = true;
            }
        });
    });
    removed
}

/// An anchor badge: the kind + strength names in the strength color, with the anchor
/// glyph. Passive (no `Response`); the inspector composes these in a wrapped row. The
/// `kind_label` and `strength_label` are already localized by the caller (the module
/// owns the `codex.anchor.*` keys); the strength still drives the color.
pub fn anchor_badge(ui: &mut egui::Ui, theme: &Theme, strength: AnchorStrength, kind_label: &str, strength_label: &str) {
    let color = strength_color(theme, strength);
    // Arrange the two labels through a bundle template (`codex.anchor.badge` =
    // "%{kind} %{strength}") rather than a Rust `format!`, so word order and separator are
    // localizable - a grammar wanting strength-first or a different separator expresses it in
    // the bundle. The shell resolving a module-owned key at render time is the normal i18n path.
    let text = pixhaus_services::i18n::tr_args("codex.anchor.badge", &[("kind", kind_label), ("strength", strength_label)]);
    badge(ui, theme, icons::ANCHOR, &text, color);
}

/// A status badge: the localized status label in its role color. The caller localizes
/// the label (the module owns the `codex.status.*` keys); the status drives the color.
pub fn status_badge(ui: &mut egui::Ui, theme: &Theme, status: EntryStatus, label: &str) {
    badge(ui, theme, icons::STATUS_DOT, label, status_color(theme, status));
}

/// A small pill: a glyph + text in `color`, on the inset surface. The shared shape the
/// anchor and status badges paint.
fn badge(ui: &mut egui::Ui, theme: &Theme, glyph: char, text: &str, color: egui::Color32) {
    let frame = egui::Frame::new()
        .fill(theme.surface(crate::theme::tokens::SurfaceTier::Inset))
        .inner_margin(margin(theme.spacing.sm, theme.spacing.xs))
        .corner_radius(radius(theme.radius.sm));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            ui.label(egui::RichText::new(glyph.to_string()).size(theme.type_scale.label).color(color));
            ui.label(egui::RichText::new(text).size(theme.type_scale.label).color(color));
        });
    });
}

/// A four-step strength selector (Loose / Normal / Strong / Locked). Returns the
/// newly-chosen strength when the user clicks a different step, else `None`. The
/// `current` step reads in the accent. `label_for` resolves a step to its localized name
/// (the module owns the `codex.anchor.strength.*` keys); this widget holds no literals.
pub fn strength_selector(ui: &mut egui::Ui, theme: &Theme, current: AnchorStrength, label_for: impl Fn(AnchorStrength) -> String) -> Option<AnchorStrength> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        for step in [AnchorStrength::Loose, AnchorStrength::Normal, AnchorStrength::Strong, AnchorStrength::Locked] {
            let active = step == current;
            let color = if active { theme.accent.base } else { theme.roles.text_secondary };
            let label = egui::RichText::new(label_for(step)).size(theme.type_scale.label).color(color);
            if ui.selectable_label(active, label).clicked() && !active {
                chosen = Some(step);
            }
        }
    });
    chosen
}

/// One coverage slot row for the checklist: a check (complete) or a hollow circle
/// (missing) plus the slot label, and a trailing generate button on missing slots.
/// `generate_label` is the already-localized button text (the caller owns the key, as
/// the sibling widgets do). Returns `true` when that button is clicked.
pub fn coverage_slot_row(ui: &mut egui::Ui, theme: &Theme, label: &str, complete: bool, allow_generate: bool, generate_label: &str) -> bool {
    let mut generate = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.sm;
        let (glyph, color) = if complete {
            (icons::CHECK, theme.roles.success)
        } else {
            (icons::MISSING, theme.roles.text_disabled)
        };
        ui.label(egui::RichText::new(glyph.to_string()).size(theme.type_scale.body).color(color));
        let text_color = if complete { theme.roles.text_primary } else { theme.roles.text_secondary };
        ui.label(egui::RichText::new(label).size(theme.type_scale.body).color(text_color));
        if !complete && allow_generate {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let btn = egui::Button::new(
                    egui::RichText::new(format!("{} {generate_label}", icons::SPARKLE))
                        .size(theme.type_scale.label)
                        .color(theme.accent.ai),
                )
                .frame(false);
                if ui.add(btn).clicked() {
                    generate = true;
                }
            });
        }
    });
    generate
}

/// A coverage checklist: a header line with the completion ratio, then one row per
/// slot. `slots` is `(label, complete)` pairs. Returns the index of any slot whose
/// generate button was clicked. `allow_generate` gates the per-row button;
/// `generate_label` is the already-localized button text passed through to each row.
pub fn coverage_checklist(ui: &mut egui::Ui, theme: &Theme, slots: &[(String, bool)], allow_generate: bool, generate_label: &str) -> Option<usize> {
    let total = slots.len();
    let complete = slots.iter().filter(|(_, done)| *done).count();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(icons::COVERAGE.to_string())
                .size(theme.type_scale.section_header)
                .color(theme.accent.base),
        );
        ui.label(
            egui::RichText::new(format!("{complete} / {total}"))
                .size(theme.type_scale.section_header)
                .color(theme.roles.text_primary)
                .strong(),
        );
    });
    ui.add_space(theme.spacing.xs);
    let mut generate_index = None;
    for (i, (label, done)) in slots.iter().enumerate() {
        if coverage_slot_row(ui, theme, label, *done, allow_generate, generate_label) {
            generate_index = Some(i);
        }
    }
    generate_index
}

/// One slot row in the template slot editor: the slot label, then trailing
/// rename / remove controls and (when `can_move_up`/`can_move_down`) reorder carets.
/// Returns the [`SlotEditAction`] the user took this frame, or `None`. Reorder controls
/// are hidden at the ends so the list cannot move a slot out of range. The label renders
/// from `label_text`, which the caller resolves through [`resolve_coverage_label`], so
/// this widget holds no localization rule of its own.
pub fn slot_editor_row(ui: &mut egui::Ui, theme: &Theme, label_text: &str, can_move_up: bool, can_move_down: bool) -> Option<SlotEditAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.sm;
        ui.label(
            egui::RichText::new(icons::COVERAGE.to_string())
                .size(theme.type_scale.body)
                .color(theme.roles.text_secondary),
        );
        ui.label(egui::RichText::new(label_text).size(theme.type_scale.body).color(theme.roles.text_primary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // The trailing controls read right-to-left: trash, pencil, then the carets.
            let icon_button = |ui: &mut egui::Ui, glyph: char, color: egui::Color32| -> bool {
                ui.add(egui::Button::new(egui::RichText::new(glyph.to_string()).size(theme.type_scale.label).color(color)).frame(false))
                    .clicked()
            };
            if icon_button(ui, icons::TRASH, theme.roles.warning) {
                action = Some(SlotEditAction::Remove);
            }
            if icon_button(ui, icons::RENAME, theme.roles.text_secondary) {
                action = Some(SlotEditAction::Rename);
            }
            if can_move_down && icon_button(ui, icons::MOVE_DOWN, theme.roles.text_secondary) {
                action = Some(SlotEditAction::MoveDown);
            }
            if can_move_up && icon_button(ui, icons::MOVE_UP, theme.roles.text_secondary) {
                action = Some(SlotEditAction::MoveUp);
            }
        });
    });
    action
}

/// The fields an entry card paints (bible 21.2), bundled so the function stays under
/// the argument-count lint.
pub struct EntryCardData<'a> {
    /// Display name (project content).
    pub name: &'a str,
    /// Primary handle without `@` (project content).
    pub handle: &'a str,
    /// The entry's type.
    pub entry_type: EntryType,
    /// The entry's status.
    pub status: EntryStatus,
    /// Strongest anchor strength present, for the anchor indicator dot (`None` if no
    /// anchor).
    pub top_anchor: Option<AnchorStrength>,
    /// Coverage completion ratio in `0.0..=1.0`.
    pub coverage_ratio: f32,
    /// Whether this card is the current selection (accent ring).
    pub selected: bool,
}

/// An entry card: a type-colored thumbnail placeholder, the name and `@handle`, a
/// status dot, an anchor-strength dot, and a coverage bar (bible 21.2). Returns the
/// click `Response` so the Navigator can select on click. Uses a placeholder thumbnail
/// (no real sprite yet); a real thumbnail slots in here later.
// The card geometry and the ratio (clamped 0..1) are small bounded values; the casts
// for the thumbnail rect and the bar width cannot truncate or lose a sign.
#[allow(clippy::cast_precision_loss)]
pub fn entry_card(ui: &mut egui::Ui, theme: &Theme, data: &EntryCardData<'_>) -> egui::Response {
    let card_w = ui.available_width().min(220.0);
    let frame = egui::Frame::new()
        .fill(theme.surface(crate::theme::tokens::SurfaceTier::Elevated))
        .inner_margin(theme.spacing.sm)
        .corner_radius(radius(theme.radius.md))
        .stroke(egui::Stroke::new(
            if data.selected { 2.0 } else { 1.0 },
            if data.selected { theme.accent.base } else { theme.roles.border },
        ));
    let inner = frame.show(ui, |ui| {
        ui.set_width(card_w);
        ui.horizontal(|ui| {
            // The type-colored thumbnail placeholder.
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(40.0), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                painter.rect_filled(rect, radius(theme.radius.sm), theme.surface(crate::theme::tokens::SurfaceTier::Inset));
                let blob = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(22.0));
                painter.rect_filled(blob, radius(theme.radius.sm), type_color(theme, data.entry_type));
            }
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(data.name)
                            .size(theme.type_scale.body)
                            .color(theme.roles.text_primary)
                            .strong(),
                    );
                    // The status dot.
                    ui.label(
                        egui::RichText::new(icons::STATUS_DOT.to_string())
                            .size(theme.type_scale.label)
                            .color(status_color(theme, data.status)),
                    );
                    // The anchor dot, if any anchor is present.
                    if let Some(strength) = data.top_anchor {
                        ui.label(
                            egui::RichText::new(icons::ANCHOR.to_string())
                                .size(theme.type_scale.label)
                                .color(strength_color(theme, strength)),
                        );
                    }
                });
                ui.label(
                    egui::RichText::new(format!("@{}", data.handle))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                );
                // The coverage bar: a thin track with an accent fill proportional to the
                // ratio.
                let ratio = data.coverage_ratio.clamp(0.0, 1.0);
                let (bar, _) = ui.allocate_exact_size(egui::vec2(card_w * 0.6, 4.0), egui::Sense::hover());
                if ui.is_rect_visible(bar) {
                    let painter = ui.painter();
                    painter.rect_filled(bar, 2u8, theme.surface(crate::theme::tokens::SurfaceTier::Inset));
                    let fill = egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * ratio, bar.height()));
                    painter.rect_filled(fill, 2u8, theme.roles.success);
                }
            });
        });
    });
    inner.response.interact(egui::Sense::click())
}

/// One anchor entry for [`hover_card`]: the strength (drives the badge color) and the
/// already-localized kind and strength labels (the caller owns the `codex.anchor.*`
/// keys).
pub struct HoverAnchor<'a> {
    /// The anchor strength, which colors the badge.
    pub strength: AnchorStrength,
    /// The localized anchor-kind name.
    pub kind_label: &'a str,
    /// The localized strength name.
    pub strength_label: &'a str,
}

/// The referenced entry a [`hover_card`] describes: name, type, status (the badge color)
/// and its localized status label, and a short description. The labels are localized by
/// the caller (the module owns the `codex.*` keys); this widget holds no display
/// literals.
pub struct HoverEntry<'a> {
    /// The entry's display name (project content, not localized).
    pub name: &'a str,
    /// The entry type, which picks the type glyph and tint.
    pub entry_type: EntryType,
    /// The entry status, which colors the status badge.
    pub status: EntryStatus,
    /// The localized status label.
    pub status_label: &'a str,
    /// A short description line (project content); empty hides the line.
    pub description: &'a str,
}

/// A mini hover card for a referenced entry (bible 13.5): name, type, status badge, a
/// short description, and the anchor badges. Drawn inside an `on_hover_ui` closure, so
/// it returns nothing.
pub fn hover_card(ui: &mut egui::Ui, theme: &Theme, entry: &HoverEntry<'_>, anchors: &[HoverAnchor<'_>]) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(type_icon(entry.entry_type).to_string())
                .size(theme.type_scale.body)
                .color(type_color(theme, entry.entry_type)),
        );
        ui.label(
            egui::RichText::new(entry.name)
                .size(theme.type_scale.body)
                .color(theme.roles.text_primary)
                .strong(),
        );
    });
    ui.horizontal(|ui| {
        status_badge(ui, theme, entry.status, entry.status_label);
    });
    if !entry.description.is_empty() {
        ui.label(
            egui::RichText::new(entry.description)
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary),
        );
    }
    if !anchors.is_empty() {
        ui.add_space(theme.spacing.xs);
        ui.horizontal_wrapped(|ui| {
            for anchor in anchors {
                anchor_badge(ui, theme, anchor.strength, anchor.kind_label, anchor.strength_label);
            }
        });
    }
}

/// An action returned by [`editable_list`]: add the buffered text, or remove the item
/// at an index. The caller maps these to the matching intents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListAction {
    /// Add a new item carrying the trimmed buffer text.
    Add(String),
    /// Remove the item at this index.
    Remove(usize),
}

/// A small editable list of short strings (aliases, tags, single-line fragments): one
/// removable row per item, then an add field bound to `buffer` with an Add button.
///
/// Returns a [`ListAction`] when the user adds (Enter in the field or the Add button,
/// with non-empty trimmed text) or removes a row. The caller owns the list (it lives in
/// the model) and the `buffer` (a draft string); this widget only reports the intent.
pub fn editable_list(ui: &mut egui::Ui, theme: &Theme, items: &[String], buffer: &mut String, placeholder: &str) -> Option<ListAction> {
    let mut action = None;
    for (i, item) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            ui.label(egui::RichText::new(item).size(theme.type_scale.label).color(theme.roles.text_primary));
            let remove = egui::Button::new(
                egui::RichText::new(icons::CLOSE.to_string())
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            )
            .frame(false);
            if ui.add(remove).clicked() {
                action = Some(ListAction::Remove(i));
            }
        });
    }
    ui.horizontal(|ui| {
        let field = ui.add(egui::TextEdit::singleline(buffer).hint_text(placeholder).desired_width(140.0));
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let add = ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} ", icons::ADD))
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            ))
            .clicked();
        if (submitted || add) && !buffer.trim().is_empty() {
            action = Some(ListAction::Add(buffer.trim().to_owned()));
        }
    });
    action
}

/// A kind picker for adding an anchor: one selectable per [`AnchorKind`] the entry does
/// not already carry. Returns the chosen kind on click. `existing` is the kinds already
/// present, which are omitted (one anchor per kind). `label_for` resolves a kind to its
/// localized name (the module owns the `codex.anchor.kind.*` keys); this widget never
/// holds display literals.
pub fn add_anchor_picker(ui: &mut egui::Ui, theme: &Theme, existing: &[AnchorKind], label_for: impl Fn(AnchorKind) -> String) -> Option<AnchorKind> {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        for kind in [
            AnchorKind::Identity,
            AnchorKind::Visual,
            AnchorKind::Palette,
            AnchorKind::Style,
            AnchorKind::Animation,
            AnchorKind::Scale,
            AnchorKind::Lore,
            AnchorKind::Negative,
        ] {
            if existing.contains(&kind) {
                continue;
            }
            let label = egui::RichText::new(format!("{} {}", icons::ADD, label_for(kind)))
                .size(theme.type_scale.label)
                .color(theme.accent.base);
            if ui.add(egui::Button::new(label).frame(true)).clicked() {
                chosen = Some(kind);
            }
        }
    });
    chosen
}

/// Every palette color role, for the role picker. Mirrors `ColorRole`'s variants; a
/// missing variant would only drop a choice from the menu, never miscompile.
pub const ALL_COLOR_ROLES: [ColorRole; 11] = [
    ColorRole::Shadow,
    ColorRole::Midtone,
    ColorRole::Highlight,
    ColorRole::Outline,
    ColorRole::Skin,
    ColorRole::Cloth,
    ColorRole::Metal,
    ColorRole::MagicGlow,
    ColorRole::Danger,
    ColorRole::Healing,
    ColorRole::UiAccent,
];

/// What [`palette_color_row`] reports the caller should do to one color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorRowAction {
    /// Toggle the color's locked flag.
    ToggleLocked,
    /// Toggle the color's optional flag.
    ToggleOptional,
    /// Set the color's RGBA value (the color picker changed).
    SetRgba([u8; 4]),
    /// Set the color's role.
    SetRole(ColorRole),
    /// Remove this color.
    Remove,
}

/// One palette-color editor row: an editable swatch (a color picker over the value), the
/// role as a picker menu, lock and optional toggles, and a remove control. Returns the
/// action the user took, if any. The caller mutates its working `PaletteDetails` copy and
/// commits it as a `SetPaletteDetails` intent. `color` is a small `Copy` value passed by
/// value. `role_label` and `optional_label` are localized by the caller (the module owns
/// the `codex.color_role.*` and `codex.palette.optional_short` keys); `role_label_for`
/// resolves a role to its localized name for the picker. This widget never holds display
/// literals. Editing the RGBA value is the one allowed `Color32` use in widget code: it is
/// rendering and editing user palette data, not theme chrome.
pub fn palette_color_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    color: PaletteColor,
    role_label: &str,
    optional_label: &str,
    role_label_for: impl Fn(ColorRole) -> String,
) -> Option<ColorRowAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.sm;
        // The swatch is an editable color button over the color's own RGBA (user data).
        let [r, g, b, a] = color.rgba;
        let mut srgba = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        if ui.color_edit_button_srgba(&mut srgba).changed() {
            action = Some(ColorRowAction::SetRgba(srgba.to_array()));
        }
        // The role as a picker menu: the current role is the button label; choosing a
        // different role reports it.
        ui.menu_button(
            egui::RichText::new(role_label).size(theme.type_scale.label).color(theme.roles.text_primary),
            |ui| {
                for role in ALL_COLOR_ROLES {
                    if role != color.role && ui.button(role_label_for(role)).clicked() {
                        action = Some(ColorRowAction::SetRole(role));
                        ui.close();
                    }
                }
            },
        );
        if ui
            .selectable_label(
                color.locked,
                egui::RichText::new(if color.locked { icons::LOCK } else { icons::LOCK_OPEN }.to_string())
                    .size(theme.type_scale.label)
                    .color(if color.locked { theme.accent.base } else { theme.roles.text_secondary }),
            )
            .clicked()
        {
            action = Some(ColorRowAction::ToggleLocked);
        }
        if ui
            .selectable_label(
                color.optional,
                egui::RichText::new(optional_label).size(theme.type_scale.label).color(if color.optional {
                    theme.accent.base
                } else {
                    theme.roles.text_secondary
                }),
            )
            .clicked()
        {
            action = Some(ColorRowAction::ToggleOptional);
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(icons::TRASH.to_string())
                        .size(theme.type_scale.label)
                        .color(theme.roles.warning),
                )
                .frame(false),
            )
            .clicked()
        {
            action = Some(ColorRowAction::Remove);
        }
    });
    action
}

/// What [`folder_row`] reports the user did to a folder.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FolderRowAction {
    /// Toggle the folder expanded/collapsed.
    ToggleExpanded,
    /// Begin renaming the folder (the caller opens an inline rename field).
    Rename,
    /// Delete the folder.
    Delete,
    /// Create a new folder under this one.
    NewChild,
}

/// One folder header row in the Navigator tree: a caret + folder glyph, the name, and
/// trailing rename / new-child / delete controls. `expanded` drives the caret and the
/// open/closed folder glyph. Returns the action the user took, if any. The caller draws
/// the folder's entries and children below when `expanded`.
// `depth` is the folder nesting level, a tiny integer; the usize -> f32 cast for the
// indent cannot lose precision in any realistic tree.
#[allow(clippy::cast_precision_loss)]
pub fn folder_row(ui: &mut egui::Ui, theme: &Theme, name: &str, expanded: bool, depth: usize) -> Option<FolderRowAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        // Indent by depth so nested folders read as a tree.
        ui.add_space(theme.spacing.md * depth as f32);
        let caret = if expanded { icons::CARET_DOWN } else { icons::CARET_RIGHT };
        let glyph = if expanded { icons::FOLDER_OPEN } else { icons::FOLDER };
        let header = ui.add(
            egui::Button::new(
                egui::RichText::new(format!("{caret} {glyph} {name}"))
                    .size(theme.type_scale.body)
                    .color(theme.roles.text_primary),
            )
            .frame(false),
        );
        if header.clicked() {
            action = Some(FolderRowAction::ToggleExpanded);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::TRASH.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.warning),
                    )
                    .frame(false),
                )
                .clicked()
            {
                action = Some(FolderRowAction::Delete);
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::ADD.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.accent.base),
                    )
                    .frame(false),
                )
                .clicked()
            {
                action = Some(FolderRowAction::NewChild);
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::RENAME.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(false),
                )
                .clicked()
            {
                action = Some(FolderRowAction::Rename);
            }
        });
    });
    action
}

// ---------------------------------------------------------------------------
// Production-cockpit widgets (the polish pass). Token-bound, accent-restrained:
// violet only for the active tab / selected row / `@handle` / primary-AI action;
// every health / readiness / status signal uses `roles.success`/`warning`/`error`.
// ---------------------------------------------------------------------------

/// A read-only tag pill: text on the inset surface, secondary ink, no interaction.
/// Used for an entry's tag chips in the header and the Overview tags card.
pub fn chip(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    let frame = egui::Frame::new()
        .fill(theme.surface(crate::theme::tokens::SurfaceTier::Inset))
        .inner_margin(margin(theme.spacing.sm, theme.spacing.xs))
        .corner_radius(radius(theme.radius.sm));
    frame.show(ui, |ui| {
        ui.label(egui::RichText::new(text).size(theme.type_scale.label).color(theme.roles.text_secondary));
    });
}

/// The identity `@handle` chip: the `@` glyph and the handle in the accent, on the
/// accent-muted fill. This is the one place a handle reads in violet - the entry's
/// identity marker in the mockup - which the accent-restraint rule allows.
pub fn handle_chip(ui: &mut egui::Ui, theme: &Theme, handle: &str) {
    let frame = egui::Frame::new()
        .fill(theme.accent.muted)
        .inner_margin(margin(theme.spacing.sm, theme.spacing.xs))
        .corner_radius(radius(theme.radius.sm));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            ui.label(
                egui::RichText::new(icons::REFERENCE.to_string())
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            );
            ui.label(egui::RichText::new(handle).size(theme.type_scale.label).color(theme.accent.base).strong());
        });
    });
}

/// A type chip: the type glyph in its type color, the type's dot, and a `label`
/// (the caller passes the localized type name). On the inset surface, neutral ink.
pub fn type_chip(ui: &mut egui::Ui, theme: &Theme, entry_type: EntryType, label: &str) {
    let frame = egui::Frame::new()
        .fill(theme.surface(crate::theme::tokens::SurfaceTier::Inset))
        .inner_margin(margin(theme.spacing.sm, theme.spacing.xs))
        .corner_radius(radius(theme.radius.sm));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            ui.label(
                egui::RichText::new(type_icon(entry_type).to_string())
                    .size(theme.type_scale.label)
                    .color(type_color(theme, entry_type)),
            );
            ui.label(egui::RichText::new(label).size(theme.type_scale.label).color(theme.roles.text_primary));
        });
    });
}

/// One label/value row for an info strip: the label in secondary ink, the value in
/// primary. `mono` renders the value in the monospace scale (for ids and versions).
pub fn stat_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str, mono: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.sm;
        ui.label(egui::RichText::new(label).size(theme.type_scale.label).color(theme.roles.text_secondary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let size = if mono { theme.type_scale.mono } else { theme.type_scale.label };
            let mut text = egui::RichText::new(value).size(size).color(theme.roles.text_primary);
            if mono {
                text = text.monospace();
            }
            ui.label(text);
        });
    });
}

/// An info strip: a stack of `(label, value)` rows on the inset surface. The header
/// key-info band and the Overview Key-info card both draw from this. `mono_values`
/// renders every value in the monospace scale (used for the id/version band).
pub fn info_strip(ui: &mut egui::Ui, theme: &Theme, rows: &[(String, String)], mono_values: bool) {
    let frame = egui::Frame::new()
        .fill(theme.surface(crate::theme::tokens::SurfaceTier::Inset))
        .inner_margin(margin(theme.spacing.md, theme.spacing.sm))
        .corner_radius(radius(theme.radius.sm));
    frame.show(ui, |ui| {
        ui.vertical(|ui| {
            for (label, value) in rows {
                stat_row(ui, theme, label, value, mono_values);
            }
        });
    });
}

/// One detail-tab spec: its localized label, its glyph, and the value it selects.
/// `T` is the panel's tab enum (`CodexDetailTab`), kept generic so the widget owns
/// no state-crate dependency.
pub struct DetailTab<'a, T> {
    /// The localized tab label (the module passes a `tr()` string).
    pub label: &'a str,
    /// The phosphor glyph for the tab.
    pub glyph: char,
    /// The tab this entry selects.
    pub value: T,
}

/// A detail tab bar: one selectable per tab, the active one in the accent with an
/// accent underline. Returns the newly-clicked tab, else `None`. Accent on the
/// active tab is the allowed active-state use.
pub fn detail_tab_bar<T: Copy + PartialEq>(ui: &mut egui::Ui, theme: &Theme, tabs: &[DetailTab<'_, T>], active: T) -> Option<T> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.md;
        for tab in tabs {
            let is_active = tab.value == active;
            let color = if is_active { theme.accent.base } else { theme.roles.text_secondary };
            let label = egui::RichText::new(format!("{} {}", tab.glyph, tab.label))
                .size(theme.type_scale.body)
                .color(color);
            let resp = ui.selectable_label(is_active, label);
            if is_active {
                // The accent underline beneath the active tab.
                let r = resp.rect;
                let y = r.bottom() + 1.0;
                ui.painter()
                    .line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], egui::Stroke::new(2.0, theme.accent.base));
            }
            if resp.clicked() && !is_active {
                chosen = Some(tab.value);
            }
        }
    });
    chosen
}

/// The status-role color for a `0.0..=1.0` ratio: `success` >= 0.8, `warning`
/// 0.4..0.8, `error` below. The shared band used by the health and readiness rings.
#[must_use]
pub fn ratio_color(theme: &Theme, ratio: f32) -> egui::Color32 {
    if ratio >= 0.8 {
        theme.roles.success
    } else if ratio >= 0.4 {
        theme.roles.warning
    } else {
        theme.roles.error
    }
}

/// A flat score ring: a circular track in the inset surface with an arc swept
/// `ratio` of the way around in the band color, the percentage centered, and a
/// `label` beneath. No gradient, no glow - the mockup's glossy gauge reconciled to a
/// flat token arc. `glyph` marks the ring (health pulse / readiness gauge).
// The ring geometry is small bounded values; the casts for the arc segments cannot
// truncate or lose a sign.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn score_ring(ui: &mut egui::Ui, theme: &Theme, glyph: char, ratio: f32, percent: u8, label: &str) {
    let ratio = ratio.clamp(0.0, 1.0);
    let arc_color = ratio_color(theme, ratio);
    let diameter = 64.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(diameter, diameter), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = rect.center();
        let r = diameter / 2.0 - 4.0;
        let track = theme.surface(crate::theme::tokens::SurfaceTier::Inset);
        // The full track, then the swept arc, both as short line segments around the
        // circle (egui has no native arc primitive; segments read clean at this size).
        let steps = 48_usize;
        let lit = (ratio * steps as f32).round() as usize;
        for i in 0..steps {
            let a0 = std::f32::consts::TAU * (i as f32 / steps as f32) - std::f32::consts::FRAC_PI_2;
            let a1 = std::f32::consts::TAU * ((i + 1) as f32 / steps as f32) - std::f32::consts::FRAC_PI_2;
            let p0 = center + egui::vec2(a0.cos(), a0.sin()) * r;
            let p1 = center + egui::vec2(a1.cos(), a1.sin()) * r;
            let color = if i < lit { arc_color } else { track };
            painter.line_segment([p0, p1], egui::Stroke::new(3.0, color));
        }
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            format!("{percent}%"),
            egui::FontId::proportional(theme.type_scale.body),
            theme.roles.text_primary,
        );
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        ui.label(egui::RichText::new(glyph.to_string()).size(theme.type_scale.label).color(arc_color));
        ui.label(egui::RichText::new(label).size(theme.type_scale.label).color(theme.roles.text_secondary));
    });
}

/// One health-checklist row: a pass/warn/fail glyph in its role color, the `label`,
/// and an optional trailing `detail` (e.g. a coverage `N/M`) in secondary ink.
/// `state` is `Some(true)` for pass, `Some(false)` for fail, `None` for warn. A
/// generalization of [`coverage_slot_row`] without the Generate button.
pub fn status_check_row(ui: &mut egui::Ui, theme: &Theme, state: Option<bool>, label: &str, detail: Option<&str>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.sm;
        let (glyph, color) = match state {
            Some(true) => (icons::CHECK, theme.roles.success),
            Some(false) => (icons::CLOSE, theme.roles.error),
            None => (icons::WARN, theme.roles.warning),
        };
        ui.label(egui::RichText::new(glyph.to_string()).size(theme.type_scale.body).color(color));
        ui.label(egui::RichText::new(label).size(theme.type_scale.body).color(theme.roles.text_primary));
        if let Some(detail) = detail {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(detail).size(theme.type_scale.label).color(theme.roles.text_secondary));
            });
        }
    });
}

/// An info card: an elevated `card`-style frame with its own `glyph + title` section
/// header and an optional trailing edit affordance (a pencil). Returns `true` when
/// the edit affordance was clicked. The Overview grid composes these. (A sibling to
/// [`card`](fn@crate::widgets::card), which is panel-meta-driven; this one takes a plain
/// glyph + title so a center tab can lay out many cards without a `PanelMeta` each.)
pub fn info_card(ui: &mut egui::Ui, theme: &Theme, glyph: char, title: &str, editable: bool, body: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut edit = false;
    let frame = egui::Frame::new()
        .fill(theme.surfaces.elevated)
        .inner_margin(margin(theme.spacing.md, theme.spacing.sm))
        .corner_radius(radius(theme.radius.md))
        .stroke(egui::Stroke::new(1.0, theme.roles.border));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            let size = theme.type_scale.section_header;
            ui.label(egui::RichText::new(glyph.to_string()).size(size).color(theme.accent.base));
            ui.label(egui::RichText::new(title).size(size).color(theme.roles.text_primary).strong());
            if editable {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pencil = egui::Button::new(
                        egui::RichText::new(icons::RENAME.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(false);
                    if ui.add(pencil).clicked() {
                        edit = true;
                    }
                });
            }
        });
        ui.add_space(theme.spacing.xs);
        body(ui);
    });
    edit
}

/// A quick-link chip to a related entry: a type-colored mention glyph + the entry's
/// label, on the inset surface. Returns `true` on click (the caller selects the
/// entry). Used by the Overview quick-links card and the relations list.
pub fn quick_link_chip(ui: &mut egui::Ui, theme: &Theme, label: &str, entry_type: EntryType) -> bool {
    let frame = egui::Frame::new()
        .fill(theme.surface(crate::theme::tokens::SurfaceTier::Inset))
        .inner_margin(margin(theme.spacing.sm, theme.spacing.xs))
        .corner_radius(radius(theme.radius.sm))
        .stroke(egui::Stroke::new(1.0, theme.roles.border));
    let inner = frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            ui.label(
                egui::RichText::new(icons::LINK.to_string())
                    .size(theme.type_scale.label)
                    .color(type_color(theme, entry_type)),
            );
            ui.label(egui::RichText::new(label).size(theme.type_scale.label).color(theme.roles.text_primary));
        });
    });
    inner.response.interact(egui::Sense::click()).clicked()
}

/// One "used in" count row: a glyph, a label, and a right-aligned count pill. Passive.
pub fn used_in_row(ui: &mut egui::Ui, theme: &Theme, glyph: char, label: &str, count: usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.sm;
        ui.label(
            egui::RichText::new(glyph.to_string())
                .size(theme.type_scale.body)
                .color(theme.roles.text_secondary),
        );
        ui.label(egui::RichText::new(label).size(theme.type_scale.body).color(theme.roles.text_primary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            count_pill(ui, theme, count);
        });
    });
}

/// A small count pill: the number on the inset surface, secondary ink. The shared
/// shape for nav-node counts and used-in counts.
pub fn count_pill(ui: &mut egui::Ui, theme: &Theme, count: usize) {
    let frame = egui::Frame::new()
        .fill(theme.surface(crate::theme::tokens::SurfaceTier::Inset))
        .inner_margin(margin(theme.spacing.sm, theme.spacing.xs))
        .corner_radius(radius(theme.radius.sm));
    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(count.to_string())
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary),
        );
    });
}

/// A MOCK linked-asset thumbnail strip: up to `max_visible` checker tiles, then a
/// `+N` overflow tile when `count` exceeds the cap. Stands in for a real asset store
/// (none exists this pass); the tiles are placeholders, not real thumbnails.
// The tile geometry is small bounded values; the casts cannot truncate or lose a sign.
#[allow(clippy::cast_precision_loss)]
pub fn asset_thumbnail_strip(ui: &mut egui::Ui, theme: &Theme, count: usize, max_visible: usize) {
    let tile = 36.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        let shown = count.min(max_visible);
        for _ in 0..shown {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(tile), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // The MOCK checker placeholder (no real sprite store yet).
                let [light, dark] = theme.mock.checker;
                painter.rect_filled(rect, radius(theme.radius.sm), dark);
                let half = egui::Vec2::splat(tile / 2.0);
                painter.rect_filled(egui::Rect::from_min_size(rect.min, half), 0u8, light);
                painter.rect_filled(egui::Rect::from_min_size(rect.center(), half), 0u8, light);
            }
        }
        let overflow = count.saturating_sub(shown);
        if overflow > 0 {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(tile), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                painter.rect_filled(rect, radius(theme.radius.sm), theme.surface(crate::theme::tokens::SurfaceTier::Inset));
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("+{overflow}"),
                    egui::FontId::proportional(theme.type_scale.label),
                    theme.roles.text_secondary,
                );
            }
        }
    });
}

/// One history-timeline row: a version tag, the author and a relative-ordering dot,
/// and the change summary. `version`/`author`/`summary` are project content the
/// caller passes (the module localizes any wrapping label). Passive.
pub fn history_timeline_row(ui: &mut egui::Ui, theme: &Theme, version: u32, author: &str, summary: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.sm;
        ui.label(
            egui::RichText::new(icons::HISTORY.to_string())
                .size(theme.type_scale.label)
                .color(theme.accent.base),
        );
        ui.label(
            egui::RichText::new(format!("v{version}"))
                .size(theme.type_scale.label)
                .color(theme.roles.text_primary)
                .strong(),
        );
        if !author.is_empty() {
            ui.label(egui::RichText::new(author).size(theme.type_scale.label).color(theme.roles.text_secondary));
        }
        if !summary.is_empty() {
            ui.label(egui::RichText::new(summary).size(theme.type_scale.label).color(theme.roles.text_secondary));
        }
    });
}

/// The data one Navigator tree node row paints, bundled so the function stays under
/// the argument-count lint.
// The four flags are independent presentation facets of one row (selection, whether
// it expands, its caret state, and whether the glyph reads in the accent), not a
// state machine - so they stay plain bools rather than collapsing into enums.
#[allow(clippy::struct_excessive_bools)]
pub struct NavNodeData<'a> {
    /// The leading glyph (type-group glyph, smart-filter glyph, or a pinned-row dot).
    pub glyph: char,
    /// The row label (handle, group name, or filter name).
    pub label: &'a str,
    /// A trailing count badge, or `None` for no badge.
    pub count: Option<usize>,
    /// Nesting depth, for the indent.
    pub depth: usize,
    /// Whether the row is the current selection (accent fill + left bar).
    pub selected: bool,
    /// Whether this node can expand (draws a caret); `false` for leaf rows.
    pub expandable: bool,
    /// The caret state when `expandable`.
    pub expanded: bool,
    /// Whether the glyph should read in the accent (a pinned/active marker) rather
    /// than the type/secondary color.
    pub accent_glyph: bool,
}

/// What a [`nav_tree_node`] click reported.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct NavNodeResponse {
    /// The row body was clicked (select the node / activate the filter).
    pub clicked: bool,
    /// The caret was clicked (toggle expansion). Only set for expandable rows.
    pub toggled: bool,
}

/// One Navigator tree-node row: an optional caret, a glyph, a label, and an optional
/// count pill, indented by depth. The selected row gets an accent-muted fill and a
/// 2px accent left-edge bar (the allowed selection accent). Returns which part the
/// user clicked. Used for the WORLD type-group rows, the pinned rows, and the
/// COLLECTIONS smart-filter rows.
// `depth` is a tiny nesting integer; the usize -> f32 indent cast cannot lose
// precision in any realistic tree.
#[allow(clippy::cast_precision_loss)]
pub fn nav_tree_node(ui: &mut egui::Ui, theme: &Theme, data: &NavNodeData<'_>) -> NavNodeResponse {
    let mut out = NavNodeResponse::default();
    let fill = if data.selected { theme.accent.muted } else { egui::Color32::TRANSPARENT };
    let frame = egui::Frame::new().fill(fill).inner_margin(margin(theme.spacing.xs, theme.spacing.xs));
    let inner = frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            ui.add_space(theme.spacing.md * data.depth as f32);
            if data.expandable {
                let caret = if data.expanded { icons::CARET_DOWN } else { icons::CARET_RIGHT };
                let btn = egui::Button::new(
                    egui::RichText::new(caret.to_string())
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                )
                .frame(false);
                if ui.add(btn).clicked() {
                    out.toggled = true;
                }
            }
            let glyph_color = if data.accent_glyph { theme.accent.base } else { theme.roles.text_secondary };
            ui.label(egui::RichText::new(data.glyph.to_string()).size(theme.type_scale.body).color(glyph_color));
            let text_color = if data.selected { theme.accent.base } else { theme.roles.text_primary };
            ui.label(egui::RichText::new(data.label).size(theme.type_scale.body).color(text_color));
            if let Some(count) = data.count {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    count_pill(ui, theme, count);
                });
            }
        });
    });
    let row = inner.response.interact(egui::Sense::click());
    if row.clicked() && !out.toggled {
        out.clicked = true;
    }
    // The 2px accent left-edge bar on the selected row.
    if data.selected && ui.is_rect_visible(row.rect) {
        let r = row.rect;
        ui.painter().line_segment(
            [egui::pos2(r.left(), r.top()), egui::pos2(r.left(), r.bottom())],
            egui::Stroke::new(2.0, theme.accent.base),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_color_is_stable_per_type() {
        let theme = Theme::dark();
        assert_eq!(type_color(&theme, EntryType::Character), type_color(&theme, EntryType::Character));
    }

    #[test]
    fn resolve_coverage_label_passes_a_literal_through_untouched() {
        // A literal is user content: it must come back verbatim, never routed through
        // tr() (which would log a missing-key warning for arbitrary text).
        let literal = CoverageLabel::Literal("Crouch".to_owned());
        assert_eq!(resolve_coverage_label(&literal), "Crouch");
    }

    #[test]
    fn resolve_coverage_label_resolves_a_known_key() {
        // A known preset key resolves to its translation, not the raw key.
        let key = CoverageLabel::Key("codex.coverage.slot.idle".to_owned());
        let resolved = resolve_coverage_label(&key);
        assert_ne!(resolved, "codex.coverage.slot.idle", "a known key resolves to display text");
        assert!(!resolved.is_empty(), "the resolved label is non-empty");
    }

    #[test]
    fn status_color_distinguishes_canonical_from_rejected() {
        let theme = Theme::dark();
        assert_ne!(status_color(&theme, EntryStatus::Canonical), status_color(&theme, EntryStatus::Rejected));
    }

    #[test]
    fn strength_color_rises_from_loose_to_locked() {
        let theme = Theme::dark();
        // Loose maps to disabled, Locked to the accent base - they must differ.
        assert_ne!(strength_color(&theme, AnchorStrength::Loose), strength_color(&theme, AnchorStrength::Locked));
    }

    #[test]
    fn type_icon_is_not_the_replacement_char() {
        for t in EntryType::all() {
            assert_ne!(type_icon(*t), '\u{fffd}', "every type maps to a real glyph");
        }
    }

    #[test]
    fn list_action_round_trips() {
        // The action enum is plain data the caller maps to intents; check equality.
        assert_eq!(ListAction::Add("bit".to_owned()), ListAction::Add("bit".to_owned()));
        assert_ne!(ListAction::Remove(0), ListAction::Remove(1));
    }

    #[test]
    fn ratio_color_bands_by_status_role() {
        let theme = Theme::dark();
        assert_eq!(ratio_color(&theme, 0.95), theme.roles.success, "high ratios read as success");
        assert_eq!(ratio_color(&theme, 0.5), theme.roles.warning, "mid ratios read as warning");
        assert_eq!(ratio_color(&theme, 0.1), theme.roles.error, "low ratios read as error");
    }

    #[test]
    fn ratio_color_never_uses_the_accent() {
        // Accent restraint: a health/readiness band must never be violet.
        let theme = Theme::dark();
        for r in [0.0, 0.3, 0.6, 0.85, 1.0] {
            assert_ne!(ratio_color(&theme, r), theme.accent.base, "status bands stay off the accent");
        }
    }

    #[test]
    fn nav_node_response_defaults_to_no_click() {
        let r = NavNodeResponse::default();
        assert!(!r.clicked, "a default response is not a click");
        assert!(!r.toggled, "a default response is not a toggle");
    }

    #[test]
    fn detail_tab_carries_its_value() {
        // The tab spec is plain data the bar reads; check the value round-trips.
        let tab = DetailTab {
            label: "Overview",
            glyph: icons::INSPECTOR,
            value: 3_u8,
        };
        assert_eq!(tab.value, 3, "the tab keeps the value it selects");
        assert!(!tab.label.is_empty(), "a tab carries a label");
    }

    #[test]
    fn nav_node_data_holds_its_fields() {
        let data = NavNodeData {
            glyph: icons::WORLD,
            label: "Characters",
            count: Some(4),
            depth: 1,
            selected: true,
            expandable: true,
            expanded: false,
            accent_glyph: false,
        };
        assert_eq!(data.count, Some(4));
        assert!(data.selected);
        assert_eq!(data.label, "Characters");
    }
}
