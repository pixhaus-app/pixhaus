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

use pixhaus_ai::compose::{VariableAxis, identity_lock_clause};
use pixhaus_core::project::{
    AnchorDirection, AnimationKind, CharacterAnchor, DerivedSheet, EntityContent, EntityId, ReferenceImage, ReferenceSheet, SheetVariant, SheetVariantId,
    VariantOrigin,
};

use crate::app::{JobStatus, ShellApp};
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
        anchors: AnchorRow {
            neutral,
            south,
            west,
            north,
            east,
        },
        cells,
    }
}

/// Builds the neutral-pose prompt for the Tier-1 derivation pass.
///
/// The neutral anchor is the canonical stripped of baked-in effects: a clean,
/// effect-free, front-facing rest pose. We cannot truly invert an effect stack
/// (Tier 2, deferred), so Tier 1 regenerates a clean pose conditioned on the
/// canonical sheet, layering a neutral-pose instruction over the canonical's own
/// subject prompt so the regenerated neutral keeps the character's identity.
#[must_use]
fn neutral_pose_prompt(canonical_prompt: &str) -> String {
    const NEUTRAL: &str = "neutral rest pose, relaxed stance, front view, no special effects, no glow, no particles, flat even lighting";
    // The pass conditions on the canonical anchor, so lock identity and free only
    // the pose. The clause rides the tail, after the axis prose.
    let lock = identity_lock_clause(VariableAxis::Pose);
    let subject = canonical_prompt.trim();
    if subject.is_empty() {
        format!("{NEUTRAL}, {lock}")
    } else {
        format!("{subject}, {NEUTRAL}, {lock}")
    }
}

/// Builds the neutral [`SheetVariant`] from a derived image and the canonical it
/// descends from. Stamps the cascade contract: `origin = NeutralReset` and
/// `parent_variant_id = Some(canonical.id)`, so [`CharacterAnchor::is_neutral_stale`]
/// reads the result as fresh against that canonical.
#[must_use]
fn neutral_variant(id: SheetVariantId, created_at: i64, canonical: &SheetVariant, image: Vec<u8>) -> SheetVariant {
    let mut variant = SheetVariant::from_image(
        id,
        created_at,
        ReferenceImage {
            bytes: image,
            mime: "image/png".to_owned(),
        },
    );
    variant.origin = VariantOrigin::NeutralReset;
    variant.parent_variant_id = Some(canonical.id);
    variant
}

/// Stores a derived neutral anchor onto the entity's [`CharacterAnchor`] as one
/// undoable library edit. The store step of the Tier-1 derivation: it mints the
/// variant via [`neutral_variant`] (stamping the `NeutralReset` origin and the
/// `parent_variant_id` edge back to the current canonical) and writes it onto
/// `anchor.neutral` through [`crate::commands::push_library_edit`], so undo
/// removes it and the grid's neutral cell goes fresh.
///
/// Takes the editor and document directly (not `&mut ShellApp`) so the store
/// step is unit-testable against a constructed [`crate::document::DocumentStore`]
/// without a full app. No-op when the entity has no reference sheet — the caller
/// only reaches this with an approved canonical in hand.
fn store_neutral(
    editor: &mut crate::editor::EditorState,
    doc: &mut crate::document::DocumentStore,
    entity_id: EntityId,
    canonical: &SheetVariant,
    image: Vec<u8>,
) {
    let variant = neutral_variant(SheetVariantId::new(doc.alloc_id()), now_secs(), canonical, image);
    crate::commands::push_library_edit(editor, doc, "Derive neutral anchor", entity_id, move |entity| {
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet), ..
        } = &mut entity.content
        {
            sheet.anchor.neutral = Some(variant);
        }
    });
}

/// Builds the directional-view prompt for a directional-anchor pass. Layers the
/// view instruction for `dir` over the neutral's subject prompt so the directional
/// anchor keeps the character's identity. East is never passed here — it is the
/// flip of west, enabled rather than generated.
#[must_use]
fn directional_pose_prompt(neutral_prompt: &str, dir: AnchorDirection) -> String {
    let view = match dir {
        AnchorDirection::South => "front view, facing the camera",
        // East is the flip of west and is never generated; if a stray call reaches
        // here it falls back to west's view rather than panicking.
        AnchorDirection::West | AnchorDirection::East => "side view, facing left",
        AnchorDirection::North => "back view, facing away",
    };
    const COMMON: &str = "neutral rest pose, no special effects, flat even lighting";
    // The pass conditions on the neutral anchor, so lock identity and free only
    // the facing direction. The clause rides the tail, after the axis prose.
    let lock = identity_lock_clause(VariableAxis::Facing);
    let subject = neutral_prompt.trim();
    if subject.is_empty() {
        format!("{view}, {COMMON}, {lock}")
    } else {
        format!("{subject}, {view}, {COMMON}, {lock}")
    }
}

/// Builds a directional [`SheetVariant`] from a derived image and the neutral it
/// descends from. Stamps the cascade contract: `origin = DirectionalAnchor` and
/// `parent_variant_id = Some(neutral.id)`, so
/// [`CharacterAnchor::is_directional_stale`] reads the result fresh against that
/// neutral.
#[must_use]
fn directional_variant(id: SheetVariantId, created_at: i64, neutral: &SheetVariant, image: Vec<u8>) -> SheetVariant {
    let mut variant = SheetVariant::from_image(
        id,
        created_at,
        ReferenceImage {
            bytes: image,
            mime: "image/png".to_owned(),
        },
    );
    variant.origin = VariantOrigin::DirectionalAnchor;
    variant.parent_variant_id = Some(neutral.id);
    variant
}

/// Stores a derived directional anchor onto the entity's [`CharacterAnchor`] as
/// one undoable library edit. The store step of a south/west/north derivation: it
/// mints the variant via [`directional_variant`] (stamping the `DirectionalAnchor`
/// origin and the `parent_variant_id` edge back to the neutral) and writes it onto
/// the matching directional slot via [`DirectionalAnchors::set`], through
/// [`crate::commands::push_library_edit`], so undo removes it and the grid's cell
/// goes fresh.
///
/// East must not reach here — it is the flip of west and carries no stored variant
/// (see [`enable_east`]). Calling with [`AnchorDirection::East`] would no-op the
/// variant and only flip `east_from_west`, which is not this function's contract,
/// so the caller is responsible for routing East to [`enable_east`] instead.
///
/// Takes the editor and document directly (not `&mut ShellApp`) so the store step
/// is unit-testable against a constructed [`crate::document::DocumentStore`]
/// without a full app. No-op when the entity has no reference sheet.
fn store_directional(
    editor: &mut crate::editor::EditorState,
    doc: &mut crate::document::DocumentStore,
    entity_id: EntityId,
    dir: AnchorDirection,
    neutral: &SheetVariant,
    image: Vec<u8>,
) {
    let variant = directional_variant(SheetVariantId::new(doc.alloc_id()), now_secs(), neutral, image);
    let label = match dir {
        AnchorDirection::South => "Derive south anchor",
        AnchorDirection::West => "Derive west anchor",
        AnchorDirection::North => "Derive north anchor",
        AnchorDirection::East => "Derive east anchor",
    };
    crate::commands::push_library_edit(editor, doc, label, entity_id, move |entity| {
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet), ..
        } = &mut entity.content
        {
            sheet.anchor.directional.set(dir, variant);
        }
    });
}

/// Enables east-as-flip-of-west on the entity's [`CharacterAnchor`] as one
/// undoable library edit. East is never generated: this calls
/// [`DirectionalAnchors::set`] with [`AnchorDirection::East`], a no-op store that
/// flips `east_from_west` on so Land mirrors the west loop for east. No variant is
/// stored. Idempotent — when `east_from_west` is already set,
/// [`crate::commands::push_library_edit`] sees no change and records no entry.
///
/// Takes the editor and document directly so it is unit-testable without a full
/// app. No-op when the entity has no reference sheet.
fn enable_east(editor: &mut crate::editor::EditorState, doc: &mut crate::document::DocumentStore, entity_id: EntityId) {
    crate::commands::push_library_edit(editor, doc, "Enable east (flip of west)", entity_id, |entity| {
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet), ..
        } = &mut entity.content
        {
            // A placeholder variant: `set(East, _)` discards it and only flips the
            // flag, so its contents never reach the document.
            let placeholder = SheetVariant::from_image(
                SheetVariantId::new(0),
                0,
                ReferenceImage {
                    bytes: Vec::new(),
                    mime: "image/png".to_owned(),
                },
            );
            sheet.anchor.directional.set(AnchorDirection::East, placeholder);
        }
    });
}

/// Resolves the anchor variant id a clip in `direction` was seeded from, for a
/// [`DerivedSheet`]'s `derived_from` edge. Prefers the directional anchor for
/// that direction (east returns the west variant via
/// [`pixhaus_core::project::DirectionalAnchors::get`]); falls back to the neutral
/// when no directional anchor exists, since the studio seeds from the neutral
/// before any directional anchor is derived. `None` only when neither is present.
#[must_use]
fn seed_variant_id(anchor: &CharacterAnchor, direction: AnchorDirection) -> Option<SheetVariantId> {
    anchor
        .directional
        .get(direction)
        .map(|v| v.id)
        .or_else(|| anchor.neutral.as_ref().map(|n| n.id))
}

/// One unit of cascade re-generation, in the dependency order a stale anchor must
/// be rebuilt: the neutral first (everything derives from it), then the directional
/// anchors (they derive from the neutral), then the derived animation sheets (they
/// derive from the directional anchors). [`plan_reroll`] returns these already
/// ordered, so a driver can run them front-to-back without re-checking dependencies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RerollStep {
    /// Re-derive the effect-stripped neutral anchor from the current canonical.
    Neutral,
    /// Re-derive a directional anchor from the (re-derived) neutral. East never
    /// appears here — it is the flip of west and carries no stored variant.
    Directional(AnchorDirection),
    /// Re-run the i2v + integrate for one stale derived sheet, re-pointing its
    /// `derived_from` at the new upstream anchor.
    Sheet {
        /// The animation the stale sheet holds.
        animation_kind: AnimationKind,
        /// The facing the stale sheet was generated for.
        direction: AnchorDirection,
    },
}

/// Enumerates the stale dependents of `anchor` against the current `canonical`, in
/// dependency order: the neutral (if stale) first, then each stale directional
/// anchor (south / west / north — east is the flip of west and never re-derived),
/// then each stale derived sheet.
///
/// Pure: no egui, no document, no spawning. A driver walks the returned steps
/// front-to-back, queuing each re-run through the durable queue (A1) so the cascade
/// rebuilds bottom-up — neutral before directionals before sheets — and a sheet's
/// `derived_from` re-points at an upstream that is itself already fresh.
///
/// A fresh dependent is omitted: re-rolling the canonical marks everything below it
/// stale, but re-rolling only a single directional anchor leaves the neutral and the
/// other directionals fresh, so only the affected branch is enqueued. The body order
/// matches the coverage grid ([`CoverageGrid::KINDS`] x [`CoverageGrid::DIRECTIONS`])
/// so the plan reads in the same order the grid renders.
#[must_use]
pub(crate) fn plan_reroll(anchor: &CharacterAnchor, canonical: &SheetVariant) -> Vec<RerollStep> {
    let mut steps = Vec::new();

    // The neutral is the root of the cascade: re-derive it first so the
    // directional anchors below have a fresh upstream to condition on.
    if anchor.is_neutral_stale(canonical) {
        steps.push(RerollStep::Neutral);
    }

    // Directional anchors derive from the neutral. East is the flip of west and is
    // never generated, so it is not a re-derive step — refreshing west refreshes
    // east. Only enqueue a directional that exists and reads stale (a missing
    // directional is not a dependent to re-roll; it was never derived).
    for dir in [AnchorDirection::South, AnchorDirection::West, AnchorDirection::North] {
        if anchor.directional.get(dir).is_some() && anchor.is_directional_stale(dir, canonical) {
            steps.push(RerollStep::Directional(dir));
        }
    }

    // Derived sheets sit at the bottom of the cascade. Walk them in grid order so
    // the plan matches the coverage surface; enqueue each that reads stale.
    for kind in CoverageGrid::KINDS {
        for direction in CoverageGrid::DIRECTIONS {
            if let Some(sheet) = anchor.derived_sheets.iter().find(|s| s.animation_kind == kind && s.direction == direction) {
                if anchor.is_sheet_stale(sheet, canonical) {
                    steps.push(RerollStep::Sheet {
                        animation_kind: kind,
                        direction,
                    });
                }
            }
        }
    }

    steps
}

/// Records a [`DerivedSheet`] edge onto the entity's [`CharacterAnchor`] at Land,
/// as one undoable library edit. This is the cascade write that integrate's frame
/// landing pairs with: the frames land as a [`crate::commands::SpriteBufferEdit`]
/// ("Integrate animation"), and this records the edge ("Record cascade edge") so
/// the coverage grid reads the new (kind, direction) cell as covered and undo
/// reverses each independently.
///
/// `derived_from` is the directional anchor (or neutral) the clip was seeded from
/// — when it changes id, the new sheet reads stale, which is the cascade's whole
/// point. East shares west's seed id (the flip is not its own variant).
///
/// Takes the editor and document directly (not `&mut ShellApp`) so the write is
/// unit-testable against a constructed [`crate::document::DocumentStore`]. No-op
/// when the entity has no reference sheet.
fn record_derived_sheet(
    editor: &mut crate::editor::EditorState,
    doc: &mut crate::document::DocumentStore,
    entity_id: EntityId,
    animation_kind: AnimationKind,
    direction: AnchorDirection,
    tag_name: String,
    derived_from: SheetVariantId,
) {
    let sheet = DerivedSheet {
        animation_kind,
        direction,
        tag_name,
        derived_from,
    };
    crate::commands::push_library_edit(editor, doc, "Record cascade edge", entity_id, move |entity| {
        if let EntityContent::Sprites {
            reference_sheet: Some(ref_sheet),
            ..
        } = &mut entity.content
        {
            ref_sheet.anchor.derived_sheets.push(sheet);
        }
    });
}

/// UTC seconds since the epoch, for stamping the derived variant's `created_at`.
/// Returns 0 if the clock is before the epoch.
fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
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
            reference_sheet: Some(sheet), ..
        } = &entity.content
        else {
            return None;
        };
        let ReferenceSheet {
            canonical: Some(canonical),
            anchor,
            ..
        } = sheet.as_ref()
        else {
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
        // badge. The neutral exposes the C2 "derive neutral" action; south / west /
        // north each derive from the neutral, and east is enabled as a flip of west.
        ui.label(egui::RichText::new("Anchors").strong());
        ui.add_space(2.0);
        let mut do_derive_neutral = false;
        ui.horizontal(|ui| {
            anchor_chip(ui, "Neutral", grid.anchors.neutral);
            let deriving = matches!(self.neutral_status, JobStatus::Running(_));
            let button = egui::Button::new(format!("{} Derive", crate::icons::WAND));
            if ui
                .add_enabled(!deriving, button)
                .on_hover_text("Regenerate the canonical as a clean, effect-free neutral pose")
                .clicked()
            {
                do_derive_neutral = true;
            }
            anchor_chip(ui, "South", grid.anchors.south);
            anchor_chip(ui, "West", grid.anchors.west);
            anchor_chip(ui, "North", grid.anchors.north);
            east_chip(ui, grid.anchors.east);
        });
        match &self.neutral_status {
            JobStatus::Running(msg) => {
                ui.label(egui::RichText::new(format!("{} {msg}…", crate::icons::SPARKLE)).weak());
            }
            JobStatus::Failed(err) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("{} {err}", crate::icons::STALE));
            }
            JobStatus::Idle => {}
        }

        // Directional generate actions. South / west / north each derive from the
        // neutral, so they are only available once a neutral exists. East is never
        // generated — it is enabled as the flip of west.
        ui.add_space(2.0);
        let mut derive_dir: Option<AnchorDirection> = None;
        let mut do_enable_east = false;
        ui.horizontal(|ui| {
            let has_neutral = !matches!(grid.anchors.neutral, CellState::Missing);
            let deriving = matches!(self.directional_status, JobStatus::Running(_));
            for (dir, state) in [
                (AnchorDirection::South, grid.anchors.south),
                (AnchorDirection::West, grid.anchors.west),
                (AnchorDirection::North, grid.anchors.north),
            ] {
                let verb = if matches!(state, CellState::Missing) { "Generate" } else { "Re-roll" };
                let label = format!("{} {verb} {}", crate::icons::WAND, direction_label(dir));
                let hover = if has_neutral {
                    format!("Derive the {} anchor from the neutral pose", direction_label(dir).to_lowercase())
                } else {
                    "Derive a neutral anchor first".to_owned()
                };
                if ui
                    .add_enabled(has_neutral && !deriving, egui::Button::new(label))
                    .on_hover_text(hover)
                    .clicked()
                {
                    derive_dir = Some(dir);
                }
            }
            // East: enable the flip, do not generate. Disabled once enabled
            // (idempotent), and badged as a flip rather than a generation.
            let east_on = matches!(grid.anchors.east, EastState::Enabled(_));
            let east_label = format!("{} Enable East (flip of West)", crate::icons::FLIP_H);
            if ui
                .add_enabled(!east_on, egui::Button::new(east_label))
                .on_hover_text("East mirrors the west loop at Land. Correct only for left-right-symmetric subjects.")
                .clicked()
            {
                do_enable_east = true;
            }
        });
        match &self.directional_status {
            JobStatus::Running(msg) => {
                ui.label(egui::RichText::new(format!("{} {msg}…", crate::icons::SPARKLE)).weak());
            }
            JobStatus::Failed(err) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("{} {err}", crate::icons::STALE));
            }
            JobStatus::Idle => {}
        }

        if do_derive_neutral {
            self.derive_neutral();
        }
        if let Some(dir) = derive_dir {
            self.derive_directional(dir);
        }
        if do_enable_east {
            self.enable_east_for_active();
        }

        // Re-roll stale dependents. The plan is the count of stale neutral /
        // directional / sheet slots; the action drives one queued re-run at a time
        // (a remote provider plus the 8K constraint rule out unbounded fan-out), so
        // it stays disabled while a derivation or clip is already running.
        ui.add_space(4.0);
        let plan = self
            .active_anchor()
            .map(|(anchor, canonical)| plan_reroll(anchor, canonical))
            .unwrap_or_default();
        let busy = matches!(self.neutral_status, JobStatus::Running(_))
            || matches!(self.directional_status, JobStatus::Running(_))
            || matches!(self.anim_status, JobStatus::Running(_));
        let mut do_reroll = false;
        ui.horizontal(|ui| {
            let stale = plan.len();
            let label = format!("{} Re-roll stale dependents", crate::icons::WAND);
            let hover = if stale == 0 {
                "Nothing stale — the cascade is up to date".to_owned()
            } else {
                format!("Regenerate {stale} stale anchor(s)/sheet(s) in dependency order, one queued job at a time")
            };
            if ui.add_enabled(stale > 0 && !busy, egui::Button::new(label)).on_hover_text(hover).clicked() {
                do_reroll = true;
            }
            if stale > 0 {
                ui.label(egui::RichText::new(format!("{stale} stale")).weak());
            }
        });
        if do_reroll {
            self.reroll_stale_dependents();
        }

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

    /// Kicks off the Tier-1 neutral-anchor derivation: a text-to-image pass that
    /// regenerates the canonical pose as a clean, effect-free neutral via the
    /// existing [`crate::ai::FirstFrameJob::Generate`] path, conditioned on the
    /// canonical sheet so the regenerated neutral keeps the character's identity.
    /// The result lands through [`Self::land_neutral`], which stamps the
    /// `NeutralReset` origin and the `parent_variant_id` edge and stores it
    /// undoably.
    ///
    /// No-op when no entity is active or its sheet has no approved canonical —
    /// the header only exposes the action with a canonical in hand.
    fn derive_neutral(&mut self) {
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        // Capture the canonical's id and prompt up front; the spawned task carries
        // the id back so the stored variant's parent edge matches the canonical
        // the pass derived from.
        let Some((canonical_id, canonical_prompt)) = self.active_anchor().map(|(_, c)| (c.id, c.user_prompt.clone())) else {
            return;
        };
        // The approved anchor PNG conditions the pass so the neutral stays
        // on-model. Without it (the headless path) the pass still runs prompt-only.
        let reference_images = self.doc.active_anchor().map(<[u8]>::to_vec).into_iter().collect();
        let Some(canvas) = self.doc.active_sprite().map(|s| (s.canvas.width, s.canvas.height)) else {
            return;
        };

        let job = crate::ai::FirstFrameJob::Generate {
            reference_images,
            canvas,
            prompt: neutral_pose_prompt(&canonical_prompt),
            num_variants: 1,
            seed: None,
        };

        self.neutral_epoch += 1;
        self.neutral_status = JobStatus::Running("deriving neutral".to_owned());
        let cancel = tokio_util::sync::CancellationToken::new();
        crate::ai::spawn_neutral(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
            cancel,
            self.neutral_epoch,
            entity_id,
            canonical_id,
        );
    }

    /// Lands a derived neutral image: re-resolves the live canonical (it may have
    /// changed since the pass started), stamps `NeutralReset` and the parent edge
    /// via [`store_neutral`], and clears the job status. A no-op when the canonical
    /// moved out from under the run (the stored `canonical_id` no longer matches),
    /// so a stale derivation never writes a wrong edge.
    pub(crate) fn land_neutral(&mut self, entity_id: EntityId, canonical_id: SheetVariantId, image: Vec<u8>) {
        self.neutral_status = JobStatus::Idle;
        let canonical = match self.active_anchor() {
            Some((_, canonical)) if canonical.id == canonical_id => canonical.clone(),
            _ => {
                self.neutral_status = JobStatus::Failed("the canonical changed during derivation; re-run".to_owned());
                return;
            }
        };
        store_neutral(&mut self.editor, &mut self.doc, entity_id, &canonical, image);
    }

    /// Kicks off a directional-anchor derivation for `dir` (south / west / north):
    /// a text-to-image pass that regenerates the neutral seen from that facing via
    /// the existing [`crate::ai::FirstFrameJob::Generate`] path, conditioned on the
    /// neutral so the directional anchor stays on-model. The result lands through
    /// [`Self::land_directional`], which stamps the `DirectionalAnchor` origin and
    /// the `parent_variant_id` edge to the neutral and stores it undoably.
    ///
    /// East is never derived here — it is the flip of west; route it through
    /// [`Self::enable_east_for_active`] instead. No-op when no entity is active,
    /// its sheet has no approved canonical, or it has no neutral yet (the
    /// directional anchors derive from the neutral, so a missing neutral has no
    /// upstream to condition on).
    fn derive_directional(&mut self, dir: AnchorDirection) {
        if dir == AnchorDirection::East {
            return;
        }
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        // The directional anchor derives from the neutral: capture its id (for the
        // parent edge) and prompt (to stay on-model). Without a neutral there is no
        // upstream, so the action is unavailable.
        let Some((neutral_id, neutral_prompt)) = self
            .active_anchor()
            .and_then(|(anchor, _)| anchor.neutral.as_ref().map(|n| (n.id, n.user_prompt.clone())))
        else {
            self.directional_status = JobStatus::Failed("derive a neutral anchor first".to_owned());
            return;
        };
        let reference_images = self.doc.active_anchor().map(<[u8]>::to_vec).into_iter().collect();
        let Some(canvas) = self.doc.active_sprite().map(|s| (s.canvas.width, s.canvas.height)) else {
            return;
        };

        let job = crate::ai::FirstFrameJob::Generate {
            reference_images,
            canvas,
            prompt: directional_pose_prompt(&neutral_prompt, dir),
            num_variants: 1,
            seed: None,
        };

        self.directional_epoch += 1;
        self.directional_status = JobStatus::Running(format!("deriving {}", direction_label(dir).to_lowercase()));
        let cancel = tokio_util::sync::CancellationToken::new();
        crate::ai::spawn_directional(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
            cancel,
            self.directional_epoch,
            entity_id,
            dir,
            neutral_id,
        );
    }

    /// Lands a derived directional image: re-resolves the live neutral (it may
    /// have changed since the pass started), stamps `DirectionalAnchor` and the
    /// parent edge via [`store_directional`], and clears the job status. A no-op
    /// when the neutral moved out from under the run (the stored `neutral_id` no
    /// longer matches), so a stale derivation never writes a wrong edge.
    pub(crate) fn land_directional(&mut self, entity_id: EntityId, dir: AnchorDirection, neutral_id: SheetVariantId, image: Vec<u8>) {
        self.directional_status = JobStatus::Idle;
        let neutral = match self.active_anchor().and_then(|(anchor, _)| anchor.neutral.clone()) {
            Some(neutral) if neutral.id == neutral_id => neutral,
            _ => {
                self.directional_status = JobStatus::Failed("the neutral changed during derivation; re-run".to_owned());
                return;
            }
        };
        store_directional(&mut self.editor, &mut self.doc, entity_id, dir, &neutral, image);
    }

    /// Enables east-as-flip-of-west on the active entity: a no-op store that flips
    /// `east_from_west` so Land mirrors the west loop for east. No generation, no
    /// stored variant. No-op when no entity is active.
    fn enable_east_for_active(&mut self) {
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        enable_east(&mut self.editor, &mut self.doc, entity_id);
    }

    /// Records the directional-cascade [`DerivedSheet`] edge for the just-landed
    /// clip, the second of Land's two undo entries (the first being the frames
    /// integrated by [`crate::commands::integrate_frames_undoable`]). The edge
    /// reads the studio's kind and facing and points `derived_from` at the
    /// directional anchor (or neutral) the clip was seeded from, so the coverage
    /// grid's matching cell reads covered and re-rolling the upstream marks it
    /// stale.
    ///
    /// `tag_name` is the frame tag integrate created — the motion label — so the
    /// recorded edge names the same range that landed on the timeline.
    ///
    /// [`AnimKind::Custom`] has no [`AnimationKind`] in the cascade, so a custom
    /// animation records no edge; the Land inspector notes that custom animations
    /// are not tracked in the cascade grid. Also a no-op when no entity is active,
    /// its sheet has no anchor to seed from, or the seed variant cannot be
    /// resolved (no directional anchor and no neutral yet).
    pub(crate) fn record_cascade_edge(&mut self, tag_name: &str) {
        let Some(animation_kind) = self.studio.kind.to_animation_kind() else {
            // Custom animations have no cascade kind: land the frames, skip the
            // edge, and tell the user why so the grid's silence is explained.
            self.anim_status = JobStatus::Failed("custom animations are not tracked in the cascade grid".to_owned());
            return;
        };
        let direction = self.studio.facing.to_anchor_direction();
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        let Some(derived_from) = self.active_anchor().and_then(|(anchor, _)| seed_variant_id(anchor, direction)) else {
            return;
        };
        record_derived_sheet(
            &mut self.editor,
            &mut self.doc,
            entity_id,
            animation_kind,
            direction,
            tag_name.to_owned(),
            derived_from,
        );
    }

    /// Re-rolls the active entity's stale cascade dependents, one queued job at a
    /// time. Reads the plan via [`plan_reroll`] (neutral -> directional -> sheets in
    /// dependency order) and starts the first step that is not already in flight:
    ///
    /// - a stale neutral re-derives via [`Self::derive_neutral`];
    /// - else the first stale directional anchor re-derives via
    ///   [`Self::derive_directional`];
    /// - else the first stale derived sheet seeds the studio (kind + facing) and
    ///   starts its i2v clip through the durable queue ([`Self::start_clip`]).
    ///
    /// One step per call is deliberate: a remote provider plus the 8K constraint
    /// rule out unbounded fan-out, so the driver gates concurrency to a single
    /// in-flight job. Each landed re-derive refreshes its slot and clears it from the
    /// next plan, so repeated invocations (or a re-call when a job lands) walk the
    /// cascade bottom-up until [`plan_reroll`] is empty and the grid reads all-fresh.
    /// While a derivation is already running the call is a no-op, so the bound holds.
    ///
    /// No-op when no entity is active, its sheet has no canonical, or nothing is
    /// stale.
    pub(crate) fn reroll_stale_dependents(&mut self) {
        // Respect the one-in-flight bound: never stack a second derivation.
        if matches!(self.neutral_status, JobStatus::Running(_))
            || matches!(self.directional_status, JobStatus::Running(_))
            || matches!(self.anim_status, JobStatus::Running(_))
        {
            return;
        }
        let Some(step) = self
            .active_anchor()
            .map(|(anchor, canonical)| plan_reroll(anchor, canonical))
            .and_then(|steps| steps.into_iter().next())
        else {
            return;
        };
        match step {
            RerollStep::Neutral => self.derive_neutral(),
            RerollStep::Directional(dir) => self.derive_directional(dir),
            RerollStep::Sheet { animation_kind, direction } => {
                // Seed the studio for the stale sheet's framing, then start the i2v
                // clip; Land re-records the edge with the fresh `derived_from`. The
                // overlay stays open (unlike a manual cell click) so the grid still
                // shows the batch's remaining stale cells.
                self.studio.kind = kind_to_studio(animation_kind);
                self.studio.facing = direction_to_facing(direction);
                self.start_clip();
            }
        }
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
    use pixhaus_core::project::{DerivedSheet, Size};

    use crate::document::DocumentStore;
    use crate::editor::EditorState;

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

    // ── C2: neutral-anchor derivation (Tier 1 store step) ─────────────────────

    /// A tiny valid PNG, decode-clean so [`SheetVariant::from_image`] reads real
    /// dimensions rather than the template defaults.
    fn tiny_png(w: u32, h: u32) -> Vec<u8> {
        use std::io::Cursor;
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encode test png");
        out
    }

    /// Reads the active entity's neutral anchor, if one was stored.
    fn neutral_of(doc: &DocumentStore, entity_id: EntityId) -> Option<SheetVariant> {
        let entity = doc.project.library.entities.iter().find(|e| e.id == entity_id)?;
        let EntityContent::Sprites {
            reference_sheet: Some(sheet), ..
        } = &entity.content
        else {
            return None;
        };
        sheet.anchor.neutral.clone()
    }

    #[test]
    fn neutral_pose_prompt_layers_over_the_subject() {
        let p = neutral_pose_prompt("a small retro robot");
        assert!(p.starts_with("a small retro robot, "), "the subject leads: {p}");
        assert!(p.contains("neutral rest pose"), "the neutral instruction is appended: {p}");
        // The pass conditions on the anchor, so it carries the identity-lock clause
        // freeing only the pose, after the axis prose.
        assert!(p.contains("keep the same character from the reference image"), "the identity-lock clause is appended: {p}");
        assert!(p.contains("change only the pose"), "the neutral pass frees only the pose: {p}");
        // An empty canonical prompt falls back to the bare neutral instruction.
        assert!(!neutral_pose_prompt("  ").is_empty());
        assert!(!neutral_pose_prompt("  ").starts_with(','), "no leading comma when the subject is empty");
        assert!(neutral_pose_prompt("  ").contains("keep the same character"), "the lock survives an empty subject");
    }

    #[test]
    fn neutral_variant_stamps_the_neutral_reset_contract() {
        let canonical = variant(1, None);
        let v = neutral_variant(SheetVariantId::new(2), 1_700_000_000, &canonical, tiny_png(16, 16));
        assert_eq!(v.origin, VariantOrigin::NeutralReset);
        assert_eq!(v.parent_variant_id, Some(canonical.id));
        assert_eq!(v.created_at, 1_700_000_000);
    }

    #[test]
    fn store_step_lands_a_fresh_neutral_under_one_undo() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let entity_id = doc.active_entity_id().expect("active entity");
        let mut editor = EditorState::default();

        // Approve a canonical so the entity carries an anchor to derive from.
        let canonical = SheetVariant::from_image(
            SheetVariantId::new(doc.alloc_id()),
            1,
            ReferenceImage {
                bytes: tiny_png(24, 24),
                mime: "image/png".to_owned(),
            },
        );
        let canonical_id = canonical.id;
        {
            let canonical = canonical.clone();
            crate::commands::push_library_edit(&mut editor, &mut doc, "Approve anchor", entity_id, move |entity| {
                if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                    let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                    sheet.canonical = Some(canonical);
                }
            });
        }

        // The store step under test.
        store_neutral(&mut editor, &mut doc, entity_id, &canonical, tiny_png(16, 16));

        let neutral = neutral_of(&doc, entity_id).expect("neutral landed");
        assert_eq!(neutral.parent_variant_id, Some(canonical_id), "parent edge points at the canonical");
        assert_eq!(neutral.origin, VariantOrigin::NeutralReset, "origin is NeutralReset");

        // The grid's freshness predicate reads the stored neutral as fresh.
        let stored = CharacterAnchor {
            neutral: Some(neutral),
            ..Default::default()
        };
        assert!(!stored.is_neutral_stale(&canonical), "the derived neutral is fresh against its canonical");

        // The write is one undoable entry: undo clears the neutral.
        editor.history.undo(&mut doc).expect("undo the neutral");
        assert!(neutral_of(&doc, entity_id).is_none(), "undo removes the derived neutral");
    }

    // ── C3: directional-anchor derivation + east-as-flip-of-west ──────────────

    /// Reads the active entity's [`DirectionalAnchors`], if a sheet exists.
    fn directional_of(doc: &DocumentStore, entity_id: EntityId) -> Option<pixhaus_core::project::DirectionalAnchors> {
        let entity = doc.project.library.entities.iter().find(|e| e.id == entity_id)?;
        let EntityContent::Sprites {
            reference_sheet: Some(sheet), ..
        } = &entity.content
        else {
            return None;
        };
        Some(sheet.anchor.directional.clone())
    }

    /// Builds a document with one sprite whose entity carries a canonical and a
    /// neutral, returning the doc, editor, entity id, and the neutral variant the
    /// directional anchors derive from.
    fn doc_with_neutral() -> (DocumentStore, EditorState, EntityId, SheetVariant) {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let entity_id = doc.active_entity_id().expect("active entity");
        let mut editor = EditorState::default();

        let canonical = SheetVariant::from_image(
            SheetVariantId::new(doc.alloc_id()),
            1,
            ReferenceImage {
                bytes: tiny_png(24, 24),
                mime: "image/png".to_owned(),
            },
        );
        {
            let canonical = canonical.clone();
            crate::commands::push_library_edit(&mut editor, &mut doc, "Approve anchor", entity_id, move |entity| {
                if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                    let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                    sheet.canonical = Some(canonical);
                }
            });
        }
        store_neutral(&mut editor, &mut doc, entity_id, &canonical, tiny_png(16, 16));
        let neutral = neutral_of(&doc, entity_id).expect("neutral landed");
        (doc, editor, entity_id, neutral)
    }

    #[test]
    fn directional_pose_prompt_layers_the_view_over_the_subject() {
        let west = directional_pose_prompt("a knight", AnchorDirection::West);
        assert!(west.starts_with("a knight, "), "the subject leads: {west}");
        assert!(west.contains("facing left"), "west is the side view facing left: {west}");
        // The pass conditions on the neutral anchor, so it carries the identity-lock
        // clause freeing only the facing direction.
        assert!(west.contains("keep the same character from the reference image"), "the identity-lock clause is appended: {west}");
        assert!(west.contains("change only the facing direction"), "the directional pass frees only the facing: {west}");
        let north = directional_pose_prompt("a knight", AnchorDirection::North);
        assert!(north.contains("back view"), "north is the back view: {north}");
        // Empty subject falls back to the bare view instruction, no leading comma.
        assert!(!directional_pose_prompt("  ", AnchorDirection::South).starts_with(','));
        assert!(
            directional_pose_prompt("  ", AnchorDirection::South).contains("change only the facing direction"),
            "the facing lock survives an empty subject"
        );
    }

    /// The composed cascade -> `run_first_frame` Generate path must not
    /// double-append or cross axes. The cascade prompts (`neutral_pose_prompt`,
    /// `directional_pose_prompt`) already carry the axis-appropriate lock; the
    /// Generate arm adds the framing suffix via `generate_frame_prompt` and then
    /// runs `lock_when_conditioned`, whose marker guard leaves a pre-locked prompt
    /// alone. This runs both transforms (with a non-empty reference, as the
    /// cascade always conditions) to guard the seam the isolated prompt tests
    /// miss: each side is correct alone, but the prompt the backend actually sees
    /// is the composition of the two.
    #[test]
    fn cascade_prompt_through_generate_arm_locks_each_axis_once() {
        let lock_count = |haystack: &str, needle: &str| haystack.matches(needle).count();
        // The Generate arm: framing suffix, then the conditioned Pose lock guarded
        // by the marker. The cascade always conditions on an anchor.
        let through_generate_arm = |base: &str| crate::ai::lock_when_conditioned(crate::ai::generate_frame_prompt(base), true);

        // Neutral: the cascade prompt carries the Pose lock; the Generate arm's
        // marker guard does not re-add it, so the final prompt locks the pose once.
        let neutral = through_generate_arm(&neutral_pose_prompt("a small retro robot"));
        assert_eq!(
            lock_count(&neutral, "keep the same character from the reference image"),
            1,
            "the identity-lock clause appears exactly once on the composed neutral prompt: {neutral}"
        );
        assert_eq!(lock_count(&neutral, "change only the pose"), 1, "the neutral prompt frees the pose exactly once: {neutral}");
        assert!(!neutral.contains("change only the facing direction"), "the neutral prompt must not free facing: {neutral}");
        assert!(neutral.contains(", single sprite frame, side view"), "the framing suffix is present: {neutral}");

        // Directional: the cascade prompt carries the Facing lock; the Generate
        // arm's marker guard does not add a Pose lock, so the final prompt frees
        // facing — and only facing. A stray Pose lock would contradict the free axis.
        let directional = through_generate_arm(&directional_pose_prompt("a knight", AnchorDirection::West));
        assert_eq!(
            lock_count(&directional, "keep the same character from the reference image"),
            1,
            "the identity-lock clause appears exactly once on the composed directional prompt: {directional}"
        );
        assert!(
            directional.contains("change only the facing direction"),
            "the directional prompt frees the facing direction: {directional}"
        );
        assert!(
            !directional.contains("change only the pose"),
            "the directional prompt must not also free the pose — contradictory axes: {directional}"
        );
    }

    #[test]
    fn directional_variant_stamps_the_directional_contract() {
        let neutral = variant(2, Some(1));
        let v = directional_variant(SheetVariantId::new(5), 1_700_000_000, &neutral, tiny_png(16, 16));
        assert_eq!(v.origin, VariantOrigin::DirectionalAnchor);
        assert_eq!(v.parent_variant_id, Some(neutral.id));
        assert_eq!(v.created_at, 1_700_000_000);
    }

    #[test]
    fn store_directional_lands_a_fresh_west_under_one_undo() {
        let (mut doc, mut editor, entity_id, neutral) = doc_with_neutral();
        let canonical = SheetVariant::from_image(
            SheetVariantId::new(0),
            1,
            ReferenceImage {
                bytes: tiny_png(24, 24),
                mime: "image/png".to_owned(),
            },
        );

        store_directional(&mut editor, &mut doc, entity_id, AnchorDirection::West, &neutral, tiny_png(16, 16));

        let directional = directional_of(&doc, entity_id).expect("directional present");
        let west = directional.get(AnchorDirection::West).expect("west stored");
        assert_eq!(west.origin, VariantOrigin::DirectionalAnchor, "origin is DirectionalAnchor");
        assert_eq!(west.parent_variant_id, Some(neutral.id), "parent edge points at the neutral");

        // The grid's freshness predicate reads the stored west as fresh against a
        // matching neutral/canonical pair.
        let stored = CharacterAnchor {
            neutral: Some(neutral.clone()),
            directional: directional.clone(),
            ..Default::default()
        };
        // `is_directional_stale` walks back to the canonical via the neutral; here
        // the neutral's parent is the approved canonical, so reconstruct it.
        let canonical = SheetVariant {
            id: neutral.parent_variant_id.expect("neutral has a parent"),
            ..canonical
        };
        assert!(!stored.is_directional_stale(AnchorDirection::West, &canonical), "the derived west is fresh");

        // One undoable entry: undo clears the west anchor.
        editor.history.undo(&mut doc).expect("undo the west anchor");
        let after = directional_of(&doc, entity_id).expect("sheet still present");
        assert!(after.get(AnchorDirection::West).is_none(), "undo removes the derived west");
    }

    #[test]
    fn enable_east_flips_the_flag_without_storing_a_variant() {
        let (mut doc, mut editor, entity_id, _neutral) = doc_with_neutral();

        enable_east(&mut editor, &mut doc, entity_id);

        let directional = directional_of(&doc, entity_id).expect("directional present");
        assert!(directional.east_from_west, "east_from_west is set");
        // East stores no variant of its own — the placeholder is discarded by
        // `DirectionalAnchors::set(East, _)`, and no other slot is touched.
        assert!(directional.south.is_none(), "south untouched");
        assert!(directional.west.is_none(), "west untouched");
        assert!(directional.north.is_none(), "north untouched");

        // One undoable entry: undo clears the flag.
        editor.history.undo(&mut doc).expect("undo enable-east");
        assert!(
            !directional_of(&doc, entity_id).expect("sheet present").east_from_west,
            "undo clears east_from_west"
        );
    }

    #[test]
    fn enable_east_is_idempotent_no_second_undo_entry() {
        let (mut doc, mut editor, entity_id, _neutral) = doc_with_neutral();

        enable_east(&mut editor, &mut doc, entity_id);
        let nodes_after_first = editor.history.node_count();
        // A second enable is a no-op store: `east_from_west` is already true, so the
        // edit sees no change and records nothing.
        enable_east(&mut editor, &mut doc, entity_id);
        assert_eq!(editor.history.node_count(), nodes_after_first, "a redundant enable records no undo entry");
        assert!(directional_of(&doc, entity_id).expect("sheet present").east_from_west);
    }

    // ── C4: DerivedSheet edge recording at Land ───────────────────────────────

    /// Reads the active entity's derived sheets, if a reference sheet exists.
    fn derived_sheets_of(doc: &DocumentStore, entity_id: EntityId) -> Vec<DerivedSheet> {
        let Some(entity) = doc.project.library.entities.iter().find(|e| e.id == entity_id) else {
            return Vec::new();
        };
        let EntityContent::Sprites {
            reference_sheet: Some(sheet), ..
        } = &entity.content
        else {
            return Vec::new();
        };
        sheet.anchor.derived_sheets.clone()
    }

    #[test]
    fn seed_variant_id_prefers_directional_then_neutral() {
        // West directional present: its id is the seed.
        let neutral = variant(2, Some(1));
        let mut anchor = CharacterAnchor {
            neutral: Some(neutral),
            ..Default::default()
        };
        anchor.directional.set(AnchorDirection::West, variant(3, Some(2)));
        assert_eq!(
            seed_variant_id(&anchor, AnchorDirection::West),
            Some(SheetVariantId::new(3)),
            "west uses its directional anchor"
        );
        // East shares west's seed (the flip is not its own variant).
        assert_eq!(
            seed_variant_id(&anchor, AnchorDirection::East),
            Some(SheetVariantId::new(3)),
            "east shares west's seed"
        );
        // South has no directional anchor: it falls back to the neutral.
        assert_eq!(
            seed_variant_id(&anchor, AnchorDirection::South),
            Some(SheetVariantId::new(2)),
            "south falls back to the neutral"
        );
        // No directional and no neutral: nothing to seed from.
        assert_eq!(seed_variant_id(&CharacterAnchor::default(), AnchorDirection::South), None);
    }

    #[test]
    fn record_derived_sheet_lands_the_edge_with_expected_fields_under_one_undo() {
        let (mut doc, mut editor, entity_id, neutral) = doc_with_neutral();
        // Seed from the neutral (no directional anchor yet) — the studio's pre-
        // directional case. `derived_from` must match the neutral's id.
        let derived_from = neutral.id;

        record_derived_sheet(
            &mut editor,
            &mut doc,
            entity_id,
            AnimationKind::Idle,
            AnchorDirection::South,
            "idle".to_owned(),
            derived_from,
        );

        let sheets = derived_sheets_of(&doc, entity_id);
        assert_eq!(sheets.len(), 1, "one edge recorded");
        let sheet = &sheets[0];
        assert_eq!(sheet.animation_kind, AnimationKind::Idle);
        assert_eq!(sheet.direction, AnchorDirection::South);
        assert_eq!(sheet.tag_name, "idle", "tag names the landed range");
        assert_eq!(sheet.derived_from, derived_from, "derived_from points at the seed anchor");

        // One undoable entry: undo removes the edge.
        editor.history.undo(&mut doc).expect("undo the edge");
        assert!(derived_sheets_of(&doc, entity_id).is_empty(), "undo removes the recorded edge");
        editor.history.redo(&mut doc).expect("redo the edge");
        assert_eq!(derived_sheets_of(&doc, entity_id).len(), 1, "redo restores the edge");
    }

    #[test]
    fn land_records_two_undo_entries_and_undo_reverses_each_independently() {
        use pixhaus_core::canvas::PixelBuffer;
        use pixhaus_core::project::{LoopDirection, Rgba};

        let (mut doc, mut editor, entity_id, neutral) = doc_with_neutral();
        let frames_before = doc.frame_count();
        let nodes_before = editor.history.node_count();

        // First Land entry: integrate the frames (mirrors `integrate_picked`).
        let frames: Vec<PixelBuffer> = (0..3).map(|_| PixelBuffer::filled(16, 16, Rgba::new(7, 7, 7, 255)).expect("frame")).collect();
        crate::commands::integrate_frames_undoable(&mut editor, &mut doc, frames, 100, "walk", LoopDirection::Forward, None).expect("integrated range");
        assert_eq!(doc.frame_count(), 3, "three frames landed");

        // Second Land entry: record the cascade edge, seeded from the neutral.
        record_derived_sheet(
            &mut editor,
            &mut doc,
            entity_id,
            AnimationKind::Walk,
            AnchorDirection::South,
            "walk".to_owned(),
            neutral.id,
        );
        assert_eq!(derived_sheets_of(&doc, entity_id).len(), 1, "edge recorded");
        assert_eq!(editor.history.node_count(), nodes_before + 2, "Land is two distinct undo entries");

        // The first undo reverses only the edge; the frames stay on the timeline.
        editor.history.undo(&mut doc).expect("undo the edge");
        assert!(derived_sheets_of(&doc, entity_id).is_empty(), "first undo removes the edge");
        assert_eq!(doc.frame_count(), 3, "the frames are still landed after undoing only the edge");

        // The second undo reverses the frame integration.
        editor.history.undo(&mut doc).expect("undo the frames");
        assert_eq!(doc.frame_count(), frames_before, "second undo removes the integrated frames");
        assert!(derived_sheets_of(&doc, entity_id).is_empty(), "no edge remains");
    }

    // ── C5: re-roll stale dependents (the planner) ────────────────────────────

    /// An anchor with a fresh neutral and a fresh west directional, two derived
    /// sheets (one fresh walk/west, one stale idle/south), against canonical id 1 —
    /// the C1 fixture is reused here so the planner is checked against the same
    /// staleness model the grid renders.
    #[test]
    fn plan_reroll_lists_nothing_when_everything_is_fresh() {
        let (anchor, canonical) = fixture();
        // The idle/south sheet in the fixture derives from a gone upstream, so it is
        // stale even against the live canonical — strip it to get an all-fresh anchor.
        let mut fresh = anchor;
        fresh.derived_sheets.retain(|s| s.direction != AnchorDirection::South);
        assert_eq!(plan_reroll(&fresh, &canonical), Vec::new(), "an all-fresh cascade has no stale dependents");
    }

    #[test]
    fn plan_reroll_enumerates_two_stale_sheets_in_dependency_order() {
        // A neutral and a west directional, both fresh against canonical id 1, with
        // two derived sheets that each claim a gone upstream — so the two sheets are
        // stale but the anchors are not. The planner must list exactly the two
        // stale sheets, in grid (kind x direction) order, and nothing above them.
        let canonical = variant(1, None);
        let neutral = variant(2, Some(1));
        let west = variant(3, Some(2));
        let mut anchor = CharacterAnchor {
            neutral: Some(neutral),
            ..Default::default()
        };
        anchor.directional.set(AnchorDirection::West, west);
        // Two stale sheets: idle/south and walk/west, each derived from a now-gone id.
        anchor.derived_sheets.push(DerivedSheet {
            animation_kind: AnimationKind::Walk,
            direction: AnchorDirection::West,
            tag_name: "walk".into(),
            derived_from: SheetVariantId::new(88),
        });
        anchor.derived_sheets.push(DerivedSheet {
            animation_kind: AnimationKind::Idle,
            direction: AnchorDirection::South,
            tag_name: "idle".into(),
            derived_from: SheetVariantId::new(99),
        });

        let plan = plan_reroll(&anchor, &canonical);
        // Exactly the two stale sheets, no neutral or directional step (both fresh),
        // and in grid order: Idle/South precedes Walk/West.
        assert_eq!(
            plan,
            vec![
                RerollStep::Sheet {
                    animation_kind: AnimationKind::Idle,
                    direction: AnchorDirection::South,
                },
                RerollStep::Sheet {
                    animation_kind: AnimationKind::Walk,
                    direction: AnchorDirection::West,
                },
            ],
            "exactly the two stale sheets, in grid order, with no fresh upstream step"
        );
    }

    #[test]
    fn plan_reroll_orders_neutral_then_directional_then_sheets() {
        // Re-roll the canonical: the neutral, the west directional, and the derived
        // sheets all go stale. The plan must rebuild bottom-up — neutral first, then
        // the directional, then the sheets — so each sheet re-points at a fresh
        // upstream.
        let (anchor, _) = fixture();
        let rerolled = variant(7, None);
        let plan = plan_reroll(&anchor, &rerolled);

        assert_eq!(plan.first(), Some(&RerollStep::Neutral), "the neutral re-derives first");
        assert_eq!(
            plan.get(1),
            Some(&RerollStep::Directional(AnchorDirection::West)),
            "the directional re-derives after the neutral"
        );
        // The remaining steps are sheets; the fixture's two sheets both go stale.
        assert!(plan[2..].iter().all(|s| matches!(s, RerollStep::Sheet { .. })), "sheets re-roll last");
        let sheet_count = plan.iter().filter(|s| matches!(s, RerollStep::Sheet { .. })).count();
        assert_eq!(sheet_count, 2, "both fixture sheets re-roll under a canonical re-roll");
    }

    #[test]
    fn plan_reroll_skips_missing_directionals_and_never_lists_east() {
        // A stale neutral with no directionals derived and one stale derived sheet:
        // the plan re-derives the neutral and the sheet, but never a missing
        // directional (it was never derived, so it is not a dependent), and never
        // east (east is the flip of west, not a re-derive step).
        let rerolled = variant(7, None);
        let neutral = variant(2, Some(1)); // parent id 1, not 7 -> stale against rerolled
        let mut anchor = CharacterAnchor {
            neutral: Some(neutral),
            ..Default::default()
        };
        // Enable east-from-west on top of a (missing) west so east could in
        // principle be reported — the planner must still never emit an east step.
        anchor.directional.east_from_west = true;
        anchor.derived_sheets.push(DerivedSheet {
            animation_kind: AnimationKind::Attack,
            direction: AnchorDirection::North,
            tag_name: "attack".into(),
            derived_from: SheetVariantId::new(5),
        });

        let plan = plan_reroll(&anchor, &rerolled);
        assert_eq!(plan.first(), Some(&RerollStep::Neutral), "the stale neutral re-derives first");
        assert!(
            !plan.iter().any(|s| matches!(s, RerollStep::Directional(_))),
            "no directional step: none was derived, so none is a dependent"
        );
        assert!(
            !plan.iter().any(|s| matches!(s, RerollStep::Directional(AnchorDirection::East))),
            "east is the flip of west and is never a re-derive step"
        );
        assert_eq!(
            plan.iter().filter(|s| matches!(s, RerollStep::Sheet { .. })).count(),
            1,
            "the one stale sheet re-rolls"
        );
    }
}
