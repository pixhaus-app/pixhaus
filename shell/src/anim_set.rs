//! The directional-cascade **coverage grid**: a read-only view over an entity's
//! [`CharacterAnchor`] that shows, per animation kind and facing direction, which
//! derived sheets exist and which are stale against the current canonical.
//!
//! The grid is a studio overlay toggled by `anim_set_open`, paralleling
//! `studio_library_open`. It reads the active entity's
//! `EntityContent::Sprites -> ReferenceSheet.anchor` and the canonical variant,
//! and classifies every cell with the staleness predicates already on
//! [`CharacterAnchor`] (`is_neutral_stale`, `is_directional_stale`,
//! `is_sheet_stale`). It writes nothing: clicking a cell pre-seeds the studio
//! (kind, facing, and the source anchor selected) and the actual generation/
//! integrate writes land in later cascade tasks (C2–C5).
//!
//! The classification is pure logic — [`build_grid`] takes a `&CharacterAnchor`
//! and the current canonical and returns a [`CoverageGrid`] with no egui or
//! document access — so it is unit-tested directly against constructed anchors.

use eframe::egui;

use pixhaus_core::project::{
    AnchorDirection, AnimationKind, CharacterAnchor, EntityContent, ReferenceSheet, SheetVariant,
};

use crate::app::ShellApp;
use crate::studio::{AnimKind, Facing};

/// The freshness of one coverage cell against the current canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellState {
    /// No derived sheet / anchor exists for this slot.
    Missing,
    /// Present and derived from the current upstream — up to date.
    Fresh,
    /// Present but an upstream anchor changed; needs a re-roll.
    Stale,
}

/// East is never generated or stored — it is the horizontal flip of west (see
/// [`pixhaus_core::project::DirectionalAnchors::east_from_west`]). Its column
/// shows a flip badge rather than its own freshness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EastState {
    /// East-from-west is off; east is not covered.
    Disabled,
    /// East-from-west is on; east mirrors the west anchor's freshness.
    Enabled(CellState),
}

/// One classified cell in the kind x direction body of the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DerivedCell {
    /// The animation kind this cell's row holds.
    pub kind: AnimationKind,
    /// The facing this cell's column holds.
    pub direction: AnchorDirection,
    /// The cell's freshness. East cells mirror west's staleness when
    /// `east_from_west` is set, else `Missing`.
    pub state: CellState,
}

/// The anchor header row: the neutral anchor plus the directional anchors. East
/// is a flip of west, so it carries an [`EastState`] rather than a [`CellState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AnchorRow {
    /// The effect-stripped neutral anchor's freshness.
    pub neutral: CellState,
    /// The south directional anchor's freshness.
    pub south: CellState,
    /// The west directional anchor's freshness (east flips this).
    pub west: CellState,
    /// The north directional anchor's freshness.
    pub north: CellState,
    /// East: a flip of west, enabled or not.
    pub east: EastState,
}

/// The full coverage grid: the anchor header row and one classified
/// [`DerivedCell`] per (kind, direction) body slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CoverageGrid {
    /// The neutral + directional anchor header.
    pub anchors: AnchorRow,
    /// The kind x direction body, row-major over [`Self::KINDS`] then
    /// [`Self::DIRECTIONS`].
    pub cells: Vec<DerivedCell>,
}

impl CoverageGrid {
    /// The grid's rows, in display order.
    pub(crate) const KINDS: [AnimationKind; 3] = [AnimationKind::Idle, AnimationKind::Walk, AnimationKind::Attack];

    /// The grid's columns, in display order.
    pub(crate) const DIRECTIONS: [AnchorDirection; 4] = [AnchorDirection::South, AnchorDirection::West, AnchorDirection::North, AnchorDirection::East];

    /// The classified cell for a given kind and direction, or `None` if it is
    /// outside the fixed grid (it never is for the canonical kinds/directions).
    #[must_use]
    pub(crate) fn cell(&self, kind: AnimationKind, direction: AnchorDirection) -> Option<DerivedCell> {
        self.cells.iter().copied().find(|c| c.kind == kind && c.direction == direction)
    }
}

/// Classifies a single derived-sheet slot. A slot is `Missing` when no
/// `DerivedSheet` matches the kind+direction, else `Stale`/`Fresh` via
/// [`CharacterAnchor::is_sheet_stale`].
fn classify_derived(anchor: &CharacterAnchor, kind: AnimationKind, direction: AnchorDirection, canonical: &SheetVariant) -> CellState {
    match anchor.derived_sheets.iter().find(|s| s.animation_kind == kind && s.direction == direction) {
        None => CellState::Missing,
        Some(sheet) => {
            if anchor.is_sheet_stale(sheet, canonical) {
                CellState::Stale
            } else {
                CellState::Fresh
            }
        }
    }
}

/// Classifies a directional anchor slot: `Missing` when absent, else
/// `Stale`/`Fresh` via [`CharacterAnchor::is_directional_stale`].
fn classify_directional(anchor: &CharacterAnchor, dir: AnchorDirection, canonical: &SheetVariant) -> CellState {
    if anchor.directional.get(dir).is_none() {
        return CellState::Missing;
    }
    if anchor.is_directional_stale(dir, canonical) {
        CellState::Stale
    } else {
        CellState::Fresh
    }
}

/// Builds the coverage grid model for an anchor against the current canonical.
///
/// Pure: no egui, no document access. The neutral and directional cells classify
/// with the anchor's own staleness predicates; East mirrors west's freshness
/// when `east_from_west` is set, and the body cells classify each derived sheet.
#[must_use]
pub(crate) fn build_grid(anchor: &CharacterAnchor, canonical: &SheetVariant) -> CoverageGrid {
    let neutral = if anchor.neutral.is_none() {
        CellState::Missing
    } else if anchor.is_neutral_stale(canonical) {
        CellState::Stale
    } else {
        CellState::Fresh
    };

    let south = classify_directional(anchor, AnchorDirection::South, canonical);
    let west = classify_directional(anchor, AnchorDirection::West, canonical);
    let north = classify_directional(anchor, AnchorDirection::North, canonical);
    // East is a flip of west: enabled tracks `east_from_west`, and its freshness
    // mirrors west (east never has its own stored variant).
    let east = if anchor.directional.east_from_west {
        EastState::Enabled(classify_directional(anchor, AnchorDirection::East, canonical))
    } else {
        EastState::Disabled
    };

    let mut cells = Vec::with_capacity(CoverageGrid::KINDS.len() * CoverageGrid::DIRECTIONS.len());
    for kind in CoverageGrid::KINDS {
        for direction in CoverageGrid::DIRECTIONS {
            let state = classify_derived(anchor, kind, direction, canonical);
            cells.push(DerivedCell { kind, direction, state });
        }
    }

    CoverageGrid {
        anchors: AnchorRow { neutral, south, west, north, east },
        cells,
    }
}

/// The label for an animation-kind grid row.
fn kind_label(kind: AnimationKind) -> &'static str {
    match kind {
        AnimationKind::Idle => "Idle",
        AnimationKind::Walk => "Walk",
        AnimationKind::Attack => "Attack",
    }
}

/// The label for a direction grid column.
fn direction_label(dir: AnchorDirection) -> &'static str {
    match dir {
        AnchorDirection::South => "South",
        AnchorDirection::West => "West",
        AnchorDirection::North => "North",
        AnchorDirection::East => "East",
    }
}

/// Maps a core [`AnimationKind`] to the studio's [`AnimKind`] for pre-seeding.
fn kind_to_studio(kind: AnimationKind) -> AnimKind {
    match kind {
        AnimationKind::Idle => AnimKind::Idle,
        AnimationKind::Walk => AnimKind::Walk,
        AnimationKind::Attack => AnimKind::Attack,
    }
}

/// Maps a core [`AnchorDirection`] to the studio's [`Facing`] for pre-seeding.
fn direction_to_facing(dir: AnchorDirection) -> Facing {
    match dir {
        AnchorDirection::South => Facing::South,
        AnchorDirection::West => Facing::West,
        AnchorDirection::North => Facing::North,
        AnchorDirection::East => Facing::East,
    }
}

/// The icon glyph and hover text for a cell state.
fn state_glyph(state: CellState) -> (&'static str, &'static str) {
    match state {
        CellState::Missing => (crate::icons::EMPTY, "Missing — no sheet derived for this slot"),
        CellState::Fresh => (crate::icons::CHECK, "Fresh — derived from the current canonical"),
        CellState::Stale => (crate::icons::STALE, "Stale — an upstream anchor changed; re-roll"),
    }
}

impl ShellApp {
    /// Reads the active entity's [`CharacterAnchor`] and approved canonical, or
    /// `None` when no entity is active or its sheet has no canonical variant.
    fn active_anchor(&self) -> Option<(&CharacterAnchor, &SheetVariant)> {
        let entity_id = self.doc.active_entity_id()?;
        let entity = self.doc.project.library.entities.iter().find(|e| e.id == entity_id)?;
        let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &entity.content
        else {
            return None;
        };
        let ReferenceSheet { canonical: Some(canonical), anchor, .. } = sheet.as_ref() else {
            return None;
        };
        Some((anchor, canonical))
    }

    /// The directional-cascade coverage grid overlay. Read-only: it reflects the
    /// active entity's cascade state and pre-seeds the studio on a cell click,
    /// writing nothing to the document.
    pub(crate) fn anim_set_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading(format!("{} Coverage", crate::icons::FILM));
            ui.label(egui::RichText::new("directional cascade").weak());
        });
        ui.add_space(4.0);

        let Some(grid) = self.active_anchor().map(|(anchor, canonical)| build_grid(anchor, canonical)) else {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Approve a canonical reference sheet to plan its directional cascade.").weak());
            return;
        };

        // The anchor header row: neutral + directional anchors, east as a flip
        // badge. No actions yet — C2/C3 fill these slots.
        ui.label(egui::RichText::new("Anchors").strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            anchor_chip(ui, "Neutral", grid.anchors.neutral);
            anchor_chip(ui, "South", grid.anchors.south);
            anchor_chip(ui, "West", grid.anchors.west);
            anchor_chip(ui, "North", grid.anchors.north);
            east_chip(ui, grid.anchors.east);
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Animations").strong());
        ui.add_space(4.0);

        // The kind x direction body. A click pre-seeds the studio; no writes.
        let mut seed: Option<(AnimationKind, AnchorDirection)> = None;
        egui::Grid::new("anim_set_grid").striped(true).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("");
            for dir in CoverageGrid::DIRECTIONS {
                ui.label(egui::RichText::new(direction_label(dir)).strong());
            }
            ui.end_row();

            for kind in CoverageGrid::KINDS {
                ui.label(egui::RichText::new(kind_label(kind)).strong());
                for dir in CoverageGrid::DIRECTIONS {
                    let state = grid.cell(kind, dir).map_or(CellState::Missing, |c| c.state);
                    let (glyph, hover) = state_glyph(state);
                    let badge = if dir == AnchorDirection::East && matches!(grid.anchors.east, EastState::Enabled(_)) {
                        format!("{glyph} {}", crate::icons::FLIP_H)
                    } else {
                        glyph.to_owned()
                    };
                    if ui.button(badge).on_hover_text(hover).clicked() {
                        seed = Some((kind, dir));
                    }
                }
                ui.end_row();
            }
        });

        if let Some((kind, dir)) = seed {
            self.seed_studio_from_cell(kind_to_studio(kind), direction_to_facing(dir));
        }
    }

    /// Pre-seeds the studio for a coverage cell and closes the overlay: set the
    /// kind and facing, refresh the prompt scaffolds, and return to the studio
    /// stages. No document write — generation/integrate land in later tasks.
    fn seed_studio_from_cell(&mut self, kind: AnimKind, facing: Facing) {
        self.studio.kind = kind;
        self.studio.facing = facing;
        self.anim_set_open = false;
    }
}

/// Renders one anchor-header chip: a label and its state glyph.
fn anchor_chip(ui: &mut egui::Ui, label: &str, state: CellState) {
    let (glyph, hover) = state_glyph(state);
    ui.label(format!("{glyph} {label}")).on_hover_text(hover);
}

/// Renders the east anchor chip: a flip badge when enabled, else a missing chip.
fn east_chip(ui: &mut egui::Ui, east: EastState) {
    match east {
        EastState::Disabled => {
            let (glyph, _) = state_glyph(CellState::Missing);
            ui.label(format!("{glyph} East")).on_hover_text("East is a flip of west; not enabled");
        }
        EastState::Enabled(state) => {
            let (glyph, _) = state_glyph(state);
            ui.label(format!("{glyph} {} East", crate::icons::FLIP_H))
                .on_hover_text("East mirrors west (flip of west)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{DerivedSheet, ReferenceImage, SheetVariantId};

    /// A variant with the given id and parent, enough to drive the staleness
    /// predicates.
    fn variant(id: u32, parent: Option<u32>) -> SheetVariant {
        let mut v = SheetVariant::from_image(
            SheetVariantId::new(id),
            0,
            ReferenceImage {
                bytes: vec![id as u8],
                mime: "image/png".into(),
            },
        );
        v.parent_variant_id = parent.map(SheetVariantId::new);
        v
    }

    /// An anchor with a fresh neutral and a fresh west directional derived from
    /// it, plus one fresh and one stale derived sheet, against canonical id 1.
    fn fixture() -> (CharacterAnchor, SheetVariant) {
        let canonical = variant(1, None);
        let neutral = variant(2, Some(1));
        let west = variant(3, Some(2));
        let mut anchor = CharacterAnchor {
            neutral: Some(neutral),
            ..Default::default()
        };
        anchor.directional.set(AnchorDirection::West, west);
        // A fresh walk/west sheet, derived from the current west anchor (id 3).
        anchor.derived_sheets.push(DerivedSheet {
            animation_kind: AnimationKind::Walk,
            direction: AnchorDirection::West,
            tag_name: "walk".into(),
            derived_from: SheetVariantId::new(3),
        });
        // A stale idle/south sheet: south has no directional anchor, and the
        // sheet claims to derive from a now-gone upstream id.
        anchor.derived_sheets.push(DerivedSheet {
            animation_kind: AnimationKind::Idle,
            direction: AnchorDirection::South,
            tag_name: "idle".into(),
            derived_from: SheetVariantId::new(99),
        });
        (anchor, canonical)
    }

    #[test]
    fn neutral_and_directional_header_classify() {
        let (anchor, canonical) = fixture();
        let grid = build_grid(&anchor, &canonical);
        assert_eq!(grid.anchors.neutral, CellState::Fresh, "neutral derived from canonical");
        assert_eq!(grid.anchors.west, CellState::Fresh, "west derived from neutral");
        assert_eq!(grid.anchors.south, CellState::Missing, "no south anchor");
        assert_eq!(grid.anchors.north, CellState::Missing, "no north anchor");
        assert_eq!(grid.anchors.east, EastState::Disabled, "east not enabled");
    }

    #[test]
    fn derived_cells_classify_fresh_stale_missing() {
        let (anchor, canonical) = fixture();
        let grid = build_grid(&anchor, &canonical);
        assert_eq!(grid.cell(AnimationKind::Walk, AnchorDirection::West).unwrap().state, CellState::Fresh);
        assert_eq!(grid.cell(AnimationKind::Idle, AnchorDirection::South).unwrap().state, CellState::Stale);
        // Every untouched slot is missing.
        assert_eq!(grid.cell(AnimationKind::Attack, AnchorDirection::North).unwrap().state, CellState::Missing);
        assert_eq!(grid.cell(AnimationKind::Walk, AnchorDirection::South).unwrap().state, CellState::Missing);
    }

    #[test]
    fn grid_covers_every_kind_and_direction() {
        let (anchor, canonical) = fixture();
        let grid = build_grid(&anchor, &canonical);
        assert_eq!(grid.cells.len(), CoverageGrid::KINDS.len() * CoverageGrid::DIRECTIONS.len());
        for kind in CoverageGrid::KINDS {
            for dir in CoverageGrid::DIRECTIONS {
                assert!(grid.cell(kind, dir).is_some(), "cell {kind:?}/{dir:?} present");
            }
        }
    }

    #[test]
    fn re_rolling_canonical_makes_the_whole_cascade_stale() {
        let (anchor, _) = fixture();
        // A new canonical the neutral was never derived from: everything below
        // it goes stale, and the fresh derived sheet flips to stale too.
        let rerolled = variant(7, None);
        let grid = build_grid(&anchor, &rerolled);
        assert_eq!(grid.anchors.neutral, CellState::Stale);
        assert_eq!(grid.anchors.west, CellState::Stale);
        assert_eq!(grid.cell(AnimationKind::Walk, AnchorDirection::West).unwrap().state, CellState::Stale);
    }

    #[test]
    fn east_enabled_mirrors_west_freshness() {
        let (mut anchor, canonical) = fixture();
        // Enabling east is a no-op store that flips `east_from_west`; east then
        // mirrors west's freshness (west is fresh here).
        anchor.directional.set(AnchorDirection::East, variant(0, None));
        let grid = build_grid(&anchor, &canonical);
        assert_eq!(grid.anchors.east, EastState::Enabled(CellState::Fresh));

        // A stale neutral cascades to west and therefore to east.
        let rerolled = variant(7, None);
        let stale_grid = build_grid(&anchor, &rerolled);
        assert_eq!(stale_grid.anchors.east, EastState::Enabled(CellState::Stale));
    }

    #[test]
    fn missing_neutral_makes_directionals_stale_not_missing_when_present() {
        // A west anchor present but a missing neutral: the west cell reads stale
        // (its upstream is gone), not fresh.
        let canonical = variant(1, None);
        let mut anchor = CharacterAnchor::default();
        anchor.directional.set(AnchorDirection::West, variant(3, Some(2)));
        let grid = build_grid(&anchor, &canonical);
        assert_eq!(grid.anchors.neutral, CellState::Missing);
        assert_eq!(grid.anchors.west, CellState::Stale);
    }
}
