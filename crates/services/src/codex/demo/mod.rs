//! The canonical "Bit" demo Codex: a full creative bible, built as a reusable
//! fixture.
//!
//! [`build_bit_demo_codex`] materializes the entire Bit world - 36 entries across 8
//! folders, with rich typed bodies, anchors of every kind, typed relationships,
//! prompt and negative fragments, coverage templates with per-entry slots, aliases,
//! statuses, and `@`-references - by applying the public, undoable `core` Codex
//! commands to a fresh [`Document`]. It doubles as the app's first-run world and the
//! integration-test fixture.
//!
//! Why a builder fn and not a serialized blob: the Codex is the source of truth for
//! the command surface, so building it through the same commands the UI uses keeps
//! the demo honest - if a command's contract changes, this fails to compile or build
//! rather than silently drifting from a stale snapshot.
//!
//! Every fact is grounded in the Bit knowledge base (`modules/generation/src/prompt`
//! and the v2 compose builtins): the name is "Bit" (handle `bit`), the canvas is
//! 512x512, the palette is the crisp 6-colour 8-bit set, and the pose prose matches
//! the established animation set. The strings here are project content stored in
//! `core`, never i18n keys.
//!
//! Why this module is a submodule tree and not one file: the world is ~2500 lines of
//! flat fixture data, too large to navigate as a single file. It splits along its
//! natural section boundaries - the per-entry detail pass, the animation specs, the
//! rule specs, the recipe specs - while the orchestrator, the shared command helpers,
//! the error type, and the small table-driven sections (folders, entries, filing,
//! coverage, relationships) stay here in the parent. The split is pure
//! reorganization: the public API ([`build_bit_demo_codex`], [`BuildError`],
//! [`ENTRY_COUNT`], [`RELATIONSHIP_COUNT`]) and the exact data it produces are
//! unchanged. The detail submodules reach the shared helpers through `use super::*`,
//! so each helper is declared `pub(super)` here.

use std::collections::HashMap;

use pixhaus_core::codex::{
    Anchor, AnchorKind, AnchorStrength, AnimationDetails, AntiAliasingRule, CharacterDetails, ColorRole, CoverageItemStatus, CoverageSlot, DetailLevel,
    GenericDetails, GenericField, LineTreatment, LoopBehavior, PaletteColor, PaletteDetails, PaletteRamp, PoseBeat, PromptFragment, StyleDetails,
};
use pixhaus_core::commands::{
    AddCodexAlias, AddCodexEntry, AddEntryCustomSlot, AddRelationship, ApplyBuiltinCoverageTemplate, ApplyCoverageTemplate, BuiltinCoveragePreset,
    CodexEntryDelta, CodexEntryProto, CreateCodexFolder, CreateCoverageTemplate, SetAnchor, SetAnimationDetails, SetCharacterDetails, SetCodexEntryFolder,
    SetCoverageStatus, SetEntryStatus, SetGenericDetails, SetNegativeFragments, SetPaletteDetails, SetPromptFragments, SetStyleDetails, UpdateCodexEntry,
};
use pixhaus_core::{
    CodexEntryId, CodexFolderId, CodexHandle, Command, CommandError, CoverageTemplateId, Document, EntryStatus, EntryType, HandleError, InclusionPriority,
    RelationKind, Relationship,
};
use thiserror::Error;

// The per-section detail passes live in sibling files; the orchestrator and the
// shared helpers stay here so every section reads the same command surface. The
// modules are private - their fns are reached only through the orchestrator and are
// re-exported nowhere, so the public path stays `codex::demo::<thing>`.
mod animations;
mod entries;
mod recipes;
mod rules;

/// Why [`build_bit_demo_codex`] failed to materialize the world.
///
/// Every failure here is a programming error in the spec data, not a user condition:
/// a malformed handle the validator rejects, or a command that could not apply (a
/// duplicate handle, a missing entry, a wrong detail variant). The builder threads
/// each command's `Result` through `?` rather than swallowing it, so a broken spec
/// surfaces loudly.
#[derive(Debug, Error)]
pub enum BuildError {
    /// A handle literal in the spec was not a valid [`CodexHandle`].
    #[error("invalid codex handle in the demo spec: {0}")]
    Handle(#[from] HandleError),
    /// A command failed to apply while building the world.
    #[error(transparent)]
    Command(#[from] CommandError),
    /// A command that mints an id did not report one after a successful apply (a
    /// `core` contract break; checked defensively so a `None` never becomes a silent
    /// no-op downstream).
    #[error("expected a minted id after apply, but none was reported (subject: {0})")]
    MissingId(&'static str),
    /// The spec referenced a handle that was never created as an entry (a wiring bug
    /// in this builder; checked so a dangling reference fails the build, not the
    /// resolver).
    #[error("the demo spec referenced an unknown handle: {0}")]
    UnknownHandle(&'static str),
}

/// The per-handle id map the builder threads through every phase: handles minted in
/// the entry phase, looked up by relationship and coverage wiring.
type Handles = HashMap<&'static str, CodexEntryId>;

/// Builds the complete canonical Bit demo Codex into a fresh [`Document`].
///
/// The world is materialized in dependency order so every reference resolves: folders
/// first, then all 36 entries (so relationships and coverage that name them can
/// resolve handles to ids), then per-entry detail (header text, aliases, typed
/// bodies, anchors, fragments, status), then coverage templates and statuses, then
/// the relationship graph, then entry filing into folders. Each step is a public,
/// undoable command applied to the document.
///
/// # Errors
/// Returns [`BuildError`] if a handle literal is malformed, a command fails to apply,
/// or the spec references a handle that was never created. None of these should
/// happen for the shipped spec; the `Result` exists so a future edit that breaks the
/// data fails loudly rather than producing a half-built world.
pub fn build_bit_demo_codex() -> Result<Document, BuildError> {
    let mut doc = Document::new();
    let folders = create_folders(&mut doc)?;
    let handles = create_entries(&mut doc)?;
    file_entries(&mut doc, &handles, &folders)?;
    entries::detail_entries(&mut doc, &handles)?;
    wire_coverage(&mut doc, &handles)?;
    wire_relationships(&mut doc, &handles)?;
    Ok(doc)
}

/// The root folders, in spec order. Returns a name->id map for filing.
fn create_folders(doc: &mut Document) -> Result<HashMap<&'static str, CodexFolderId>, BuildError> {
    const NAMES: [&str; 8] = [
        "Characters",
        "Palettes",
        "Styles & vibes",
        "Animations",
        "Props & items",
        "World",
        "Rules",
        "UI",
    ];
    let mut map = HashMap::new();
    for name in NAMES {
        let mut cmd = CreateCodexFolder::new(None, name);
        cmd.apply(doc)?;
        let id = cmd.inserted_id().ok_or(BuildError::MissingId("folder"))?;
        map.insert(name, id);
    }
    Ok(map)
}

/// Every (handle, name, type) the world holds. The single source of the entry set;
/// the filing table and the test both read it via [`build_bit_demo_codex`].
const ENTRIES: &[(&str, &str, EntryType)] = &[
    ("bit", "Bit", EntryType::Character),
    ("byte", "Byte", EntryType::Npc),
    ("bit_default", "Bit Default Palette", EntryType::Palette),
    ("pixel_art", "Pixel Art", EntryType::Style),
    ("retro_arcade", "Retro-tech arcade", EntryType::Vibe),
    ("idle", "Idle", EntryType::Animation),
    ("walk", "Walk", EntryType::Animation),
    ("run", "Run", EntryType::Animation),
    ("jump", "Jump", EntryType::Animation),
    ("fall", "Fall", EntryType::Animation),
    ("attack", "Attack", EntryType::Animation),
    ("hurt", "Hurt", EntryType::Animation),
    ("turnaround", "Turnaround model sheet", EntryType::Pose),
    ("floppy", "Floppy", EntryType::Item),
    ("circuit_tiles", "Circuit Tiles", EntryType::Material),
    ("arcade_world", "The Arcade Cabinet World", EntryType::Location),
    ("readable_at_32px", "Readable at 32px", EntryType::Rule),
    ("transparent_background", "Transparent background", EntryType::Rule),
    ("unified_8bit_palette", "Unified 8-bit palette", EntryType::Rule),
    ("no_extra_limbs", "No extra limbs on Bit", EntryType::Rule),
    ("no_grimdark", "No grimdark tone", EntryType::Rule),
    ("start_button", "Start Button", EntryType::UiElement),
    ("bit_idle_cycle", "Bit idle cycle", EntryType::Recipe),
    ("flat_3d_render", "Flat 3D render", EntryType::Style),
    ("single_subject", "One subject per frame", EntryType::Rule),
    ("identity_lock", "Identity stays on-model", EntryType::Rule),
    ("spatial_stability", "Stable scale and baseline", EntryType::Rule),
    ("clean_silhouette", "Clean readable silhouette", EntryType::Rule),
    ("clean_key", "Clean transparent key", EntryType::Rule),
    ("even_lighting", "Even flat lighting", EntryType::Rule),
    ("flat_side_view", "Flat 2D view", EntryType::Rule),
    ("no_text_or_ui", "No text or UI in the art", EntryType::Rule),
    ("tile_seamless", "Seamless tiling", EntryType::Rule),
    ("single_gait", "One continuous gait", EntryType::Rule),
    ("bit_sprite_sheet", "Bit sprite-sheet recipe", EntryType::Recipe),
    ("circuit_tileset", "Circuit tileset recipe", EntryType::Recipe),
];

/// Creates every entry and returns the handle->id map. Handles are minted here so the
/// later wiring phases resolve names to stable ids.
fn create_entries(doc: &mut Document) -> Result<Handles, BuildError> {
    let mut map = Handles::new();
    for (handle, name, entry_type) in ENTRIES {
        let mut cmd = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(*handle)?,
            name: (*name).to_owned(),
            entry_type: *entry_type,
        });
        cmd.apply(doc)?;
        let id = cmd.inserted_id().ok_or(BuildError::MissingId("entry"))?;
        map.insert(*handle, id);
    }
    Ok(map)
}

/// Looks up a handle's minted id, or fails the build if it was never created.
pub(super) fn id(handles: &Handles, handle: &'static str) -> Result<CodexEntryId, BuildError> {
    handles.get(handle).copied().ok_or(BuildError::UnknownHandle(handle))
}

/// The folder each handle is filed under, by folder name.
const FILING: &[(&str, &str)] = &[
    ("bit", "Characters"),
    ("byte", "Characters"),
    ("bit_default", "Palettes"),
    ("pixel_art", "Styles & vibes"),
    ("retro_arcade", "Styles & vibes"),
    ("idle", "Animations"),
    ("walk", "Animations"),
    ("run", "Animations"),
    ("jump", "Animations"),
    ("fall", "Animations"),
    ("attack", "Animations"),
    ("hurt", "Animations"),
    ("turnaround", "Animations"),
    ("floppy", "Props & items"),
    ("circuit_tiles", "World"),
    ("arcade_world", "World"),
    ("readable_at_32px", "Rules"),
    ("transparent_background", "Rules"),
    ("unified_8bit_palette", "Rules"),
    ("no_extra_limbs", "Rules"),
    ("no_grimdark", "Rules"),
    ("start_button", "UI"),
    ("flat_3d_render", "Styles & vibes"),
    ("single_subject", "Rules"),
    ("identity_lock", "Rules"),
    ("spatial_stability", "Rules"),
    ("clean_silhouette", "Rules"),
    ("clean_key", "Rules"),
    ("even_lighting", "Rules"),
    ("flat_side_view", "Rules"),
    ("no_text_or_ui", "Rules"),
    ("tile_seamless", "Rules"),
    ("single_gait", "Rules"),
    // `bit_idle_cycle`, `bit_sprite_sheet`, and `circuit_tileset` (Recipes) stay at
    // the codex root - no folder.
];

/// Files each entry into its folder. `bit_idle_cycle` is left at the root.
fn file_entries(doc: &mut Document, handles: &Handles, folders: &HashMap<&'static str, CodexFolderId>) -> Result<(), BuildError> {
    for (handle, folder_name) in FILING {
        let entry = id(handles, handle)?;
        let folder = folders.get(folder_name).copied().ok_or(BuildError::MissingId("folder lookup"))?;
        let mut cmd = SetCodexEntryFolder::new(entry, Some(folder));
        cmd.apply(doc)?;
    }
    Ok(())
}

/// A small builder for an entry's header delta, keeping the call sites readable.
pub(super) fn delta(description: &str, lore: &str, visual: &str, tags: &[&str]) -> CodexEntryDelta {
    CodexEntryDelta {
        description: Some(description.to_owned()),
        lore: if lore.is_empty() { None } else { Some(lore.to_owned()) },
        visual_description: if visual.is_empty() { None } else { Some(visual.to_owned()) },
        tags: Some(tags.iter().map(|t| (*t).to_owned()).collect()),
        ..CodexEntryDelta::new()
    }
}

/// Applies a header delta to one entry.
pub(super) fn update(doc: &mut Document, entry: CodexEntryId, d: CodexEntryDelta) -> Result<(), BuildError> {
    let mut cmd = UpdateCodexEntry::new(entry, d);
    cmd.apply(doc)?;
    Ok(())
}

/// Sets one anchor on an entry.
pub(super) fn anchor(doc: &mut Document, entry: CodexEntryId, kind: AnchorKind, strength: AnchorStrength, statement: &str) -> Result<(), BuildError> {
    let mut cmd = SetAnchor::new(entry, Anchor::new(kind, strength, statement));
    cmd.apply(doc)?;
    Ok(())
}

/// Sets the positive prompt fragments on an entry.
pub(super) fn fragments(doc: &mut Document, entry: CodexEntryId, frags: Vec<PromptFragment>) -> Result<(), BuildError> {
    let mut cmd = SetPromptFragments::new(entry, frags);
    cmd.apply(doc)?;
    Ok(())
}

/// Style-scope forbidden list: what crisp 8-bit pixel art never is. The forbidden
/// list is what actually holds a style; the positive description alone drifts.
pub(super) const NEG_STYLE: &[&str] = &[
    "anti-aliasing",
    "smooth gradients",
    "soft or blurry edges",
    "sub-pixel or off-grid detail",
    "3d render",
    "photo-realistic shading",
    "painterly brushwork",
    "jpeg artifacts",
    "colours outside the palette ramp",
];

/// Identity-scope forbidden list for the Bit family: the look that is never Bit. An
/// identity lock that names the features generation must not invent.
pub(super) const NEG_BIT_IDENTITY: &[&str] = &[
    "mouth",
    "human face",
    "teeth",
    "extra antenna",
    "two antennas",
    "extra limbs",
    "organic or blobby body",
    "glossy plastic sheen",
    "grimdark or gritty tone",
];

/// Asset-scope forbidden list for sprites and props on a flat key: the contamination
/// a sprite review rejects - background, shadows, text, fringe.
pub(super) const NEG_ASSET: &[&str] = &[
    "cast shadow",
    "drop shadow",
    "drawn ground line",
    "ground plane",
    "perspective",
    "text",
    "labels",
    "frame numbers",
    "ui or health bars",
    "watermark or signature",
    "key colour halo or fringe",
];

/// The end-of-prompt quality polish line, in the Bit pixel-art register. Optional
/// priority: it is droppable filler relative to identity, placed last.
pub(super) const QUALITY_POLISH: &str = "crisp 8-bit pixel art, clean readable silhouette at 32px, cohesive limited palette, hard pixel edges";

/// Sets the negative fragments on an entry.
pub(super) fn negatives(doc: &mut Document, entry: CodexEntryId, negs: &[&str]) -> Result<(), BuildError> {
    let mut cmd = SetNegativeFragments::new(entry, negs.iter().map(|n| (*n).to_owned()).collect());
    cmd.apply(doc)?;
    Ok(())
}

/// Sets negatives as the union of one or more shared libraries plus per-entry extras,
/// de-duplicated in first-seen order. Lets every entry reuse the forbidden-list
/// discipline without restating it.
pub(super) fn negatives_from(doc: &mut Document, entry: CodexEntryId, libs: &[&[&str]], extra: &[&str]) -> Result<(), BuildError> {
    let mut seen: Vec<&str> = Vec::new();
    for lib in libs {
        for n in *lib {
            if !seen.contains(n) {
                seen.push(n);
            }
        }
    }
    for n in extra {
        if !seen.contains(n) {
            seen.push(n);
        }
    }
    let mut cmd = SetNegativeFragments::new(entry, seen.iter().map(|n| (*n).to_owned()).collect());
    cmd.apply(doc)?;
    Ok(())
}

/// Promotes an entry to a status (every demo entry is Canonical).
pub(super) fn status(doc: &mut Document, entry: CodexEntryId, s: EntryStatus) -> Result<(), BuildError> {
    let mut cmd = SetEntryStatus::new(entry, s);
    cmd.apply(doc)?;
    Ok(())
}

/// A generic key/value body from `(key, value)` pairs.
pub(super) fn generic(doc: &mut Document, entry: CodexEntryId, fields: &[(&str, &str)]) -> Result<(), BuildError> {
    let body = GenericDetails {
        fields: fields
            .iter()
            .map(|(k, v)| GenericField {
                key: (*k).to_owned(),
                value: (*v).to_owned(),
            })
            .collect(),
    };
    let mut cmd = SetGenericDetails::new(entry, body);
    cmd.apply(doc)?;
    Ok(())
}

/// A short positive fragment helper.
pub(super) fn frag(text: &str, priority: InclusionPriority) -> PromptFragment {
    PromptFragment::new(text, priority)
}

/// Wires coverage: a project `platformer_character` template applied to Bit with
/// per-slot statuses and a custom slot, plus disjoint built-in presets on Byte
/// (top-down) and the Start button (UI states).
fn wire_coverage(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    let bit = id(handles, "bit")?;
    let byte = id(handles, "byte")?;
    let button = id(handles, "start_button")?;

    // Bit: a project template (the create-then-apply-by-id path), then per-slot
    // statuses across the production states, then a per-entry custom slot.
    let template = create_platformer_template(doc)?;
    let mut apply = ApplyCoverageTemplate::new(bit, template);
    apply.apply(doc)?;
    let bit_statuses: &[(&str, CoverageItemStatus)] = &[
        ("idle", CoverageItemStatus::Approved),
        ("walk", CoverageItemStatus::Approved),
        ("run", CoverageItemStatus::Approved),
        ("jump", CoverageItemStatus::Generated),
        ("fall", CoverageItemStatus::Generated),
        ("attack", CoverageItemStatus::NeedsReview),
        ("hurt", CoverageItemStatus::NeedsReview),
        ("land", CoverageItemStatus::Missing),
        ("death", CoverageItemStatus::Deprecated),
    ];
    for (slot, item_status) in bit_statuses {
        let mut cmd = SetCoverageStatus::new(bit, *slot, *item_status);
        cmd.apply(doc)?;
    }
    let mut custom = AddEntryCustomSlot::new(bit, CoverageSlot::custom("victory_pose", "Victory pose"));
    custom.apply(doc)?;

    // Byte: a disjoint top-down preset, all slots left Missing.
    let mut byte_preset = ApplyBuiltinCoverageTemplate::new(byte, BuiltinCoveragePreset::TopDownFourDirection);
    byte_preset.apply(doc)?;

    // Start button: a third disjoint preset (UI button states), with a couple set.
    let mut button_preset = ApplyBuiltinCoverageTemplate::new(button, BuiltinCoveragePreset::UiButtonStates);
    button_preset.apply(doc)?;
    let button_statuses: &[(&str, CoverageItemStatus)] = &[
        ("normal", CoverageItemStatus::Approved),
        ("hover", CoverageItemStatus::Generated),
        ("pressed", CoverageItemStatus::Missing),
        ("disabled", CoverageItemStatus::Missing),
    ];
    for (slot, item_status) in button_statuses {
        let mut cmd = SetCoverageStatus::new(button, *slot, *item_status);
        cmd.apply(doc)?;
    }
    Ok(())
}

/// Creates the project `platformer_character` coverage template and returns its id.
/// The slots match the built-in platformer set so the `codex.coverage.slot.*` i18n
/// keys already resolve.
fn create_platformer_template(doc: &mut Document) -> Result<CoverageTemplateId, BuildError> {
    let slots: Vec<CoverageSlot> = ["idle", "walk", "run", "jump", "fall", "land", "attack", "hurt", "death"]
        .iter()
        .map(|k| CoverageSlot::new(*k, format!("codex.coverage.slot.{k}")))
        .collect();
    let mut create = CreateCoverageTemplate::new("platformer_character", slots);
    create.apply(doc)?;
    create.inserted_id().ok_or(BuildError::MissingId("coverage template"))
}

/// Every relationship edge in the world (from-handle, kind, to-handle). The single
/// source of the graph; the test counts against its length.
const RELATIONSHIPS: &[(&str, RelationKind, &str)] = &[
    ("bit", RelationKind::Uses, "bit_default"),
    ("bit", RelationKind::Uses, "pixel_art"),
    ("bit", RelationKind::CompatibleWith, "retro_arcade"),
    ("bit", RelationKind::AppearsIn, "arcade_world"),
    ("bit", RelationKind::BelongsTo, "arcade_world"),
    ("byte", RelationKind::Uses, "bit_default"),
    ("byte", RelationKind::Uses, "pixel_art"),
    ("byte", RelationKind::CompatibleWith, "bit"),
    ("byte", RelationKind::AppearsIn, "arcade_world"),
    ("bit_default", RelationKind::CompatibleWith, "pixel_art"),
    ("pixel_art", RelationKind::CompatibleWith, "retro_arcade"),
    ("idle", RelationKind::AppearsIn, "bit"),
    ("idle", RelationKind::CompatibleWith, "pixel_art"),
    ("walk", RelationKind::AppearsIn, "bit"),
    ("walk", RelationKind::CompatibleWith, "pixel_art"),
    ("run", RelationKind::AppearsIn, "bit"),
    ("run", RelationKind::CompatibleWith, "pixel_art"),
    ("jump", RelationKind::AppearsIn, "bit"),
    ("jump", RelationKind::CompatibleWith, "pixel_art"),
    ("fall", RelationKind::AppearsIn, "bit"),
    ("fall", RelationKind::CompatibleWith, "pixel_art"),
    ("attack", RelationKind::AppearsIn, "bit"),
    ("attack", RelationKind::CompatibleWith, "pixel_art"),
    ("hurt", RelationKind::AppearsIn, "bit"),
    ("hurt", RelationKind::CompatibleWith, "pixel_art"),
    ("turnaround", RelationKind::AppearsIn, "bit"),
    ("turnaround", RelationKind::CompatibleWith, "pixel_art"),
    ("floppy", RelationKind::Uses, "bit_default"),
    ("floppy", RelationKind::BelongsTo, "arcade_world"),
    ("floppy", RelationKind::CompatibleWith, "pixel_art"),
    ("circuit_tiles", RelationKind::Uses, "bit_default"),
    ("circuit_tiles", RelationKind::AppearsIn, "arcade_world"),
    ("circuit_tiles", RelationKind::CompatibleWith, "pixel_art"),
    ("arcade_world", RelationKind::Uses, "bit_default"),
    ("arcade_world", RelationKind::Contains, "circuit_tiles"),
    ("arcade_world", RelationKind::CompatibleWith, "retro_arcade"),
    ("readable_at_32px", RelationKind::CompatibleWith, "pixel_art"),
    ("unified_8bit_palette", RelationKind::Uses, "bit_default"),
    ("no_extra_limbs", RelationKind::AppearsIn, "bit"),
    ("no_grimdark", RelationKind::CompatibleWith, "retro_arcade"),
    ("start_button", RelationKind::Uses, "bit_default"),
    ("start_button", RelationKind::Uses, "pixel_art"),
    ("bit_idle_cycle", RelationKind::Uses, "bit"),
    ("bit_idle_cycle", RelationKind::Uses, "idle"),
    ("bit_idle_cycle", RelationKind::Uses, "pixel_art"),
    ("bit_idle_cycle", RelationKind::Uses, "bit_default"),
    // The house style replaces the forbidden alternative; Bit is incompatible with it.
    ("pixel_art", RelationKind::Replaces, "flat_3d_render"),
    ("bit", RelationKind::IncompatibleWith, "flat_3d_render"),
    // The new review-derived rules, wired like the existing ones.
    ("single_subject", RelationKind::AppearsIn, "bit"),
    ("identity_lock", RelationKind::AppearsIn, "bit"),
    ("spatial_stability", RelationKind::CompatibleWith, "pixel_art"),
    ("clean_silhouette", RelationKind::CompatibleWith, "pixel_art"),
    ("clean_key", RelationKind::CompatibleWith, "pixel_art"),
    ("even_lighting", RelationKind::CompatibleWith, "retro_arcade"),
    ("flat_side_view", RelationKind::CompatibleWith, "pixel_art"),
    ("no_text_or_ui", RelationKind::CompatibleWith, "pixel_art"),
    ("tile_seamless", RelationKind::CompatibleWith, "circuit_tiles"),
    ("single_gait", RelationKind::AppearsIn, "bit"),
    // The two new recipes and what they draw on.
    ("bit_sprite_sheet", RelationKind::Uses, "bit"),
    ("bit_sprite_sheet", RelationKind::Uses, "turnaround"),
    ("bit_sprite_sheet", RelationKind::Uses, "pixel_art"),
    ("bit_sprite_sheet", RelationKind::Uses, "bit_default"),
    ("circuit_tileset", RelationKind::Uses, "circuit_tiles"),
    ("circuit_tileset", RelationKind::Uses, "pixel_art"),
    ("circuit_tileset", RelationKind::Uses, "bit_default"),
];

/// The number of relationship edges the world wires; exposed so the example, boot, and
/// test can assert against it without re-counting the table.
pub const RELATIONSHIP_COUNT: usize = RELATIONSHIPS.len();

/// The number of entries the world holds; exposed for the same reason.
pub const ENTRY_COUNT: usize = ENTRIES.len();

/// Wires every typed relationship edge from the spec table, resolving handles to ids.
/// Run after all entries exist so no endpoint dangles.
fn wire_relationships(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    for (from, kind, to) in RELATIONSHIPS {
        let from_id = id(handles, from)?;
        let to_id = id(handles, to)?;
        let mut cmd = AddRelationship::new(Relationship::new(from_id, *kind, to_id));
        cmd.apply(doc)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{resolve_text, validate_codex};
    use pixhaus_core::codex::EntryDetails;

    #[test]
    fn builds_the_full_world() {
        let doc = build_bit_demo_codex().expect("the demo world builds");
        let codex = doc.codex();
        assert_eq!(codex.entries().len(), ENTRY_COUNT);
        assert_eq!(codex.entries().len(), 36);
        // The itemized relationship table sums to 65 edges.
        assert_eq!(codex.relationships().len(), RELATIONSHIP_COUNT);
        assert_eq!(codex.relationships().len(), 65);
        assert_eq!(codex.folders().len(), 8);
    }

    #[test]
    fn bit_is_canonical_with_its_anchors() {
        let doc = build_bit_demo_codex().expect("builds");
        let codex = doc.codex();
        let bit = codex.resolve_handle(&CodexHandle::new("bit").expect("valid")).expect("bit resolves");
        let entry = codex.entry(bit).expect("entry");
        assert_eq!(entry.status, EntryStatus::Canonical);
        assert_eq!(entry.anchors.len(), 7);
        assert!(
            entry
                .anchors
                .iter()
                .any(|a| a.kind == AnchorKind::Identity && a.strength == AnchorStrength::Locked)
        );
        assert!(
            entry
                .anchors
                .iter()
                .any(|a| a.kind == AnchorKind::Negative && a.strength == AnchorStrength::Locked)
        );
        assert!(!entry.prompt_fragments.is_empty());
        // The alias resolves to the same entry.
        assert_eq!(codex.resolve_handle(&CodexHandle::new("the_mascot").expect("valid")), Some(bit));
    }

    #[test]
    fn palette_carries_six_colors_and_a_ramp() {
        let doc = build_bit_demo_codex().expect("builds");
        let codex = doc.codex();
        let palette = codex.resolve_handle(&CodexHandle::new("bit_default").expect("valid")).expect("resolves");
        let entry = codex.entry(palette).expect("entry");
        match &entry.details {
            EntryDetails::Palette(p) => {
                assert_eq!(p.colors.len(), 6);
                assert_eq!(p.ramps.len(), 2);
                assert_eq!(p.colors[0].rgba, [24, 24, 32, 255]);
            }
            other => panic!("expected a palette body, got {other:?}"),
        }
    }

    #[test]
    fn pixel_art_style_is_art_direction_grade() {
        // The house style carries the load-bearing 8-bit settings: a selective outer
        // outline and manual-only anti-aliasing, with populated rendering rules and a
        // forbidden list.
        let doc = build_bit_demo_codex().expect("builds");
        let codex = doc.codex();
        let style = codex.resolve_handle(&CodexHandle::new("pixel_art").expect("valid")).expect("resolves");
        let entry = codex.entry(style).expect("entry");
        match &entry.details {
            EntryDetails::Style(s) => {
                assert_eq!(s.line_treatment, LineTreatment::Selective);
                assert_eq!(s.anti_aliasing, AntiAliasingRule::Manual);
                assert_eq!(s.detail_level, DetailLevel::Low);
                assert_eq!(s.resolution, Some((512, 512)));
                assert!(!s.rendering_rules.trim().is_empty(), "rendering rules are populated");
                assert!(!s.negative_rules.is_empty(), "the style names a forbidden list");
            }
            other => panic!("expected a style body, got {other:?}"),
        }
    }

    #[test]
    fn forbidden_style_is_deprecated_and_suggests_the_house_style() {
        // `flat_3d_render` exists as the explicit "not this" reference: it is
        // Deprecated, so resolving it reports a Deprecated problem with `pixel_art`
        // offered as the replacement.
        use crate::codex::ResolutionProblem;
        let doc = build_bit_demo_codex().expect("builds");
        let codex = doc.codex();
        let forbidden = codex.resolve_handle(&CodexHandle::new("flat_3d_render").expect("valid")).expect("resolves");
        let pixel_art = codex.resolve_handle(&CodexHandle::new("pixel_art").expect("valid")).expect("resolves");
        assert_eq!(codex.entry(forbidden).expect("entry").status, EntryStatus::Deprecated);
        // The forbidden style carries no positive fragments - it must never enter a prompt.
        assert!(
            codex.entry(forbidden).expect("entry").prompt_fragments.is_empty(),
            "the forbidden style has no positive fragments",
        );
        let report = resolve_text(codex, "@style.flat_3d_render");
        match report.problems.as_slice() {
            [
                ResolutionProblem::Deprecated {
                    entry, suggested_replacement, ..
                },
            ] => {
                assert_eq!(*entry, forbidden);
                assert_eq!(*suggested_replacement, Some(pixel_art));
            }
            other => panic!("expected one Deprecated problem, got {other:?}"),
        }
    }

    #[test]
    fn animations_have_principled_bodies_and_fragments() {
        // The seven Bit animation entries are the teaching set: each carries rich pose
        // beats, a principled fps and frame count, a Critical identity fragment, an
        // Animation anchor, and negatives. The fps and frame counts are per-entry
        // (chosen from the timing KB), not a shared constant.
        let doc = build_bit_demo_codex().expect("builds");
        let codex = doc.codex();
        // (handle, expected fps, expected frame count, minimum pose-beat count).
        let expected: &[(&str, u16, u32, usize)] = &[
            ("idle", 6, 8, 5),
            ("walk", 10, 8, 5),
            ("run", 12, 4, 4),
            ("jump", 12, 12, 5),
            ("fall", 12, 12, 4),
            ("attack", 12, 8, 5),
            ("hurt", 12, 8, 5),
        ];
        for (handle, fps, frames, beats) in expected {
            let entry_id = codex.resolve_handle(&CodexHandle::new(*handle).expect("valid")).expect("resolves");
            let entry = codex.entry(entry_id).expect("entry");
            match &entry.details {
                EntryDetails::Animation(a) => {
                    assert_eq!(a.fps, *fps, "{handle} fps");
                    assert_eq!(a.recommended_frame_count, *frames, "{handle} frame count");
                    assert_eq!(a.pose_beats.len(), *beats, "{handle} pose-beat count");
                    assert!(
                        a.pose_beats.iter().all(|b| !b.label.trim().is_empty() && !b.description.trim().is_empty()),
                        "{handle} beats are filled"
                    );
                    assert!(!a.purpose.trim().is_empty(), "{handle} has a purpose");
                }
                other => panic!("expected an animation body for {handle}, got {other:?}"),
            }
            // A Critical identity+action fragment leads every animation's prompt.
            assert!(
                entry.prompt_fragments.iter().any(|f| f.priority == InclusionPriority::Critical),
                "{handle} has a Critical prompt fragment",
            );
            assert!(!entry.negative_fragments.is_empty(), "{handle} has negatives");
            // The motion-intent Animation anchor locks the timing/poses.
            assert!(
                entry.anchors.iter().any(|a| a.kind == AnchorKind::Animation),
                "{handle} has an Animation anchor",
            );
        }
    }

    #[test]
    fn coverage_is_present_for_bit_and_button() {
        let doc = build_bit_demo_codex().expect("builds");
        let codex = doc.codex();
        let bit = codex.resolve_handle(&CodexHandle::new("bit").expect("valid")).expect("resolves");
        assert_eq!(codex.coverage_status(bit, "idle"), CoverageItemStatus::Approved);
        assert_eq!(codex.entry(bit).expect("entry").custom_slots.len(), 1);
        let button = codex.resolve_handle(&CodexHandle::new("start_button").expect("valid")).expect("resolves");
        assert_eq!(codex.coverage_status(button, "normal"), CoverageItemStatus::Approved);
    }

    #[test]
    fn the_world_is_internally_consistent() {
        let doc = build_bit_demo_codex().expect("builds");
        let codex = doc.codex();
        // Validation: no blocking findings (a clean, well-formed world).
        let report = validate_codex(codex);
        assert!(!report.has_blocking(), "validation: {:?}", report.diagnostics);
        // Every `@`-reference in every prompt fragment resolves cleanly.
        for entry in codex.entries().values() {
            for fragment in &entry.prompt_fragments {
                let resolution = resolve_text(codex, &fragment.text);
                assert!(
                    resolution.is_clean(),
                    "fragment on {} has unresolved references: {:?}",
                    entry.handle.as_str(),
                    resolution.problems
                );
            }
        }
    }
}
