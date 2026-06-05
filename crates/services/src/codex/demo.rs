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
    detail_entries(&mut doc, &handles)?;
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
fn id(handles: &Handles, handle: &'static str) -> Result<CodexEntryId, BuildError> {
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
fn delta(description: &str, lore: &str, visual: &str, tags: &[&str]) -> CodexEntryDelta {
    CodexEntryDelta {
        description: Some(description.to_owned()),
        lore: if lore.is_empty() { None } else { Some(lore.to_owned()) },
        visual_description: if visual.is_empty() { None } else { Some(visual.to_owned()) },
        tags: Some(tags.iter().map(|t| (*t).to_owned()).collect()),
        ..CodexEntryDelta::new()
    }
}

/// Applies a header delta to one entry.
fn update(doc: &mut Document, entry: CodexEntryId, d: CodexEntryDelta) -> Result<(), BuildError> {
    let mut cmd = UpdateCodexEntry::new(entry, d);
    cmd.apply(doc)?;
    Ok(())
}

/// Sets one anchor on an entry.
fn anchor(doc: &mut Document, entry: CodexEntryId, kind: AnchorKind, strength: AnchorStrength, statement: &str) -> Result<(), BuildError> {
    let mut cmd = SetAnchor::new(entry, Anchor::new(kind, strength, statement));
    cmd.apply(doc)?;
    Ok(())
}

/// Sets the positive prompt fragments on an entry.
fn fragments(doc: &mut Document, entry: CodexEntryId, frags: Vec<PromptFragment>) -> Result<(), BuildError> {
    let mut cmd = SetPromptFragments::new(entry, frags);
    cmd.apply(doc)?;
    Ok(())
}

/// Style-scope forbidden list: what crisp 8-bit pixel art never is. The forbidden
/// list is what actually holds a style; the positive description alone drifts.
const NEG_STYLE: &[&str] = &[
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
const NEG_BIT_IDENTITY: &[&str] = &[
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
const NEG_ASSET: &[&str] = &[
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
const QUALITY_POLISH: &str = "crisp 8-bit pixel art, clean readable silhouette at 32px, cohesive limited palette, hard pixel edges";

/// Sets the negative fragments on an entry.
fn negatives(doc: &mut Document, entry: CodexEntryId, negs: &[&str]) -> Result<(), BuildError> {
    let mut cmd = SetNegativeFragments::new(entry, negs.iter().map(|n| (*n).to_owned()).collect());
    cmd.apply(doc)?;
    Ok(())
}

/// Sets negatives as the union of one or more shared libraries plus per-entry extras,
/// de-duplicated in first-seen order. Lets every entry reuse the forbidden-list
/// discipline without restating it.
fn negatives_from(doc: &mut Document, entry: CodexEntryId, libs: &[&[&str]], extra: &[&str]) -> Result<(), BuildError> {
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
fn status(doc: &mut Document, entry: CodexEntryId, s: EntryStatus) -> Result<(), BuildError> {
    let mut cmd = SetEntryStatus::new(entry, s);
    cmd.apply(doc)?;
    Ok(())
}

/// A generic key/value body from `(key, value)` pairs.
fn generic(doc: &mut Document, entry: CodexEntryId, fields: &[(&str, &str)]) -> Result<(), BuildError> {
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
fn frag(text: &str, priority: InclusionPriority) -> PromptFragment {
    PromptFragment::new(text, priority)
}

/// Fills in every entry's detail: header text, aliases, typed bodies, anchors,
/// fragments, and Canonical status. One long, flat pass per entry - the world is data,
/// and keeping it linear keeps the fixture auditable against the spec.
#[allow(clippy::too_many_lines)]
fn detail_entries(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    // --- Bit (Character, hero) ---
    let bit = id(handles, "bit")?;
    update(
        doc,
        bit,
        delta(
            "Bit is the Pixhaus mascot: a small, friendly retro robot who guides the player and narrates the world. A boxy CRT/floppy-disk head with a glowing pixel-face screen, a stubby antenna with a blinking pixel, chunky rounded limbs, friendly proportions. Reads cleanly at 32px.",
            "Bit booted up in a forgotten arcade cabinet, wandered out of its own attract loop, and now explores the circuit-board world with stubborn optimism. Small, quick, never grim - it treats every dead end as the next thing to figure out.",
            "Round-over-square silhouette: a boxy CRT head about as tall as the torso, sitting on a chunky rounded biped body roughly two heads tall. The face is a single glowing pixel screen that shows the current expression; one stubby antenna with a blinking pixel on top. Stubby rounded arms and legs. No mouth - the screen carries all expression.",
            &["mascot", "hero", "robot", "retro", "platformer"],
        ),
    )?;
    let mut alias = AddCodexAlias::new(bit, CodexHandle::new("the_mascot")?);
    alias.apply(doc)?;
    let bit_body = CharacterDetails {
        proportions: "Two heads tall; the CRT head is about the same size as the torso. Chunky, rounded, stable stance.".to_owned(),
        silhouette_notes: "Round-over-square: a boxy head on a stubby rounded body reads at any zoom, holds at 32px. The antenna and the screen are the two silhouette landmarks. Body plan: upright biped, two arms and two legs, ~2 heads tall, chunky rounded proportions, legs about half the figure height. In side or three-quarter views, render the near-side arm and leg one value lighter and the far-side one value darker with a dark separation edge so overlapping limbs never merge into one shape.".to_owned(),
        palette_ref: Some(CodexHandle::new("bit_default")?),
        allowed_styles: vec![CodexHandle::new("pixel_art")?],
        forbidden_styles: vec![CodexHandle::new("flat_3d_render")?],
        animation_set: vec![
            CodexHandle::new("idle")?,
            CodexHandle::new("walk")?,
            CodexHandle::new("run")?,
            CodexHandle::new("jump")?,
            CodexHandle::new("fall")?,
            CodexHandle::new("attack")?,
            CodexHandle::new("hurt")?,
        ],
    };
    let mut bit_details = SetCharacterDetails::new(bit, bit_body);
    bit_details.apply(doc)?;
    anchor(
        doc,
        bit,
        AnchorKind::Identity,
        AnchorStrength::Locked,
        "Bit is always friendly and optimistic, never menacing or grimdark; a small retro robot that narrates the world.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Visual,
        AnchorStrength::Locked,
        "Bit is one specific robot in every frame: an upright chunky biped about two heads tall, a boxy CRT/floppy-disk head roughly the size of the torso, a single glowing pixel-face screen as the only expression (no mouth), one stubby antenna with a blinking pixel on top, stubby rounded arms and legs. Legs are about half the figure height, the head about a third; a small hip and shoulder offset keeps near and far limbs separable in side view. Faces right in the canonical view; the near-side limb reads one value lighter, the far-side one value darker, with a dark separation edge.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Uses the Bit Default 6-colour 8-bit palette: charcoal body, cyan screen glow, off-white highlights, warm rust and sage-green accents.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Style,
        AnchorStrength::Strong,
        "Clean crisp pixel art, flat solid colour, strong silhouette, no anti-aliasing.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Scale,
        AnchorStrength::Normal,
        "Two heads tall, proportions held as fractions of total height so the figure stays on-model at any zoom; the round-over-square silhouette, the antenna, and the screen must each read at 32px on the 512x512 canvas.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Lore,
        AnchorStrength::Normal,
        "Booted from a forgotten arcade cabinet; explores a circuit-board world with stubborn optimism.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Negative,
        AnchorStrength::Locked,
        "No extra limbs, no mouth, no sharp teeth, no grimdark tone, no motion blur.",
    )?;
    fragments(
        doc,
        bit,
        vec![
            frag(
                "Bit, the Pixhaus mascot: an upright chunky retro robot about two heads tall with a boxy CRT/floppy-disk head, a single glowing pixel-face screen as its only expression (no mouth), one stubby antenna with a blinking pixel on top, stubby rounded arms and legs - a friendly round-over-square silhouette.",
                InclusionPriority::Critical,
            ),
            frag(
                "clean readable silhouette, the antenna and screen as the two landmarks, near-side limbs lighter and far-side darker with a dark separation edge so limbs stay clear of the body; reads at 32px",
                InclusionPriority::Important,
            ),
            frag("using @palette.bit_default in @style.pixel_art", InclusionPriority::Normal),
            frag("in the @vibe.retro_arcade world", InclusionPriority::Normal),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(doc, bit, &[NEG_BIT_IDENTITY, NEG_STYLE], &["motion blur", "duplicate character or second Bit"])?;
    status(doc, bit, EntryStatus::Canonical)?;

    // --- Byte (Npc, companion) ---
    let byte = id(handles, "byte")?;
    update(
        doc,
        byte,
        delta(
            "Byte, Bit's companion - a small floating drone bot with a single round glowing lens-eye, a little spinning propeller on top, and slim arms. Shares Bit's crisp 8-bit palette.",
            "Where Bit walks, Byte hovers. A quieter machine, mostly lens and propeller, it tags along to light the dark corners of the circuit-board world and hand Bit the occasional floppy.",
            "A compact floating drone: one big round glowing lens-eye dominating the body, a small spinning propeller on top keeping it aloft, two slim arms. No legs - it never touches the floor.",
            &["companion", "npc", "drone", "robot", "retro"],
        ),
    )?;
    // Byte is an Npc, whose model body is generic (the rich Character body is reserved
    // for the Character type), so its character-style notes go in as generic fields.
    generic(
        doc,
        byte,
        &[
            ("proportions", "Compact and lens-dominated, about one and a half heads tall, smaller than Bit."),
            (
                "silhouette_notes",
                "A round lens body with a propeller nub on top and two slim arms - a floating circle reads instantly against Bit's boxy head.",
            ),
            (
                "body_plan",
                "Floating drone, no legs: hovers permanently, never planted to a ground baseline. The propeller reads as the propulsion surface - a filled disc with a clear leading edge, near side lighter, far side darker.",
            ),
            (
                "rest_state",
                "Resting state is a hover with a gentle vertical bob and a slight nose-up tilt; any flap or spin is low-amplitude in place, contrasting Bit's grounded idle.",
            ),
            ("palette_ref", "bit_default"),
            ("allowed_styles", "pixel_art"),
            ("animation_set", "idle"),
        ],
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Identity,
        AnchorStrength::Locked,
        "Byte is Bit's friendly floating companion drone - calm, helpful, never threatening.",
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Visual,
        AnchorStrength::Strong,
        "Byte is a compact floating drone: one big round glowing lens-eye, a small spinning propeller on top, two slim arms, no legs. It hovers - never planted on a ground line - resting with a gentle bob and a slight nose-up tilt.",
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Same Bit Default 8-bit palette as Bit; the lens glows in the cyan screen-glow colour.",
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "No legs, no feet on the ground, no menacing red eye.",
    )?;
    fragments(
        doc,
        byte,
        vec![
            frag(
                "Byte, Bit's companion: a small floating drone bot with a single round glowing lens-eye, a little spinning propeller on top, two slim arms, no legs - it hovers, never touching the floor.",
                InclusionPriority::Critical,
            ),
            frag(
                "rests in a hover with a gentle bob and slight nose-up tilt; the propeller reads as a filled disc with a leading edge, near side lighter and far side darker",
                InclusionPriority::Important,
            ),
            frag(
                "the same crisp 8-bit palette as @character.bit, using @palette.bit_default in @style.pixel_art",
                InclusionPriority::Normal,
            ),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(
        doc,
        byte,
        &[NEG_BIT_IDENTITY, NEG_STYLE],
        &["legs", "feet on the ground", "menacing red eye", "motion blur"],
    )?;
    status(doc, byte, EntryStatus::Canonical)?;

    // --- Bit Default Palette ---
    let palette = id(handles, "bit_default")?;
    update(
        doc,
        palette,
        delta(
            "The crisp 6-colour 8-bit palette the whole Bit world shares: a dark charcoal neutral, a cyan screen glow, an off-white highlight, a warm rust accent, a sage-green accent, and a near-black outline.",
            "",
            "Six flat, saturated 8-bit colours. Charcoal reads as the body, cyan as the live screen/lens glow, off-white as highlights and rim, rust and sage as the two warm/cool accents, and a near-black outline keeps every shape crisp.",
            &["palette", "8bit", "retro", "canonical"],
        ),
    )?;
    let palette_body = PaletteDetails {
        colors: vec![
            PaletteColor::new([24, 24, 32, 255], ColorRole::Shadow),
            PaletteColor::new([64, 200, 220, 255], ColorRole::MagicGlow),
            PaletteColor::new([240, 240, 245, 255], ColorRole::Highlight),
            PaletteColor::new([220, 90, 70, 255], ColorRole::Danger),
            PaletteColor::new([120, 200, 90, 255], ColorRole::Healing),
            PaletteColor::new([12, 12, 16, 255], ColorRole::Outline),
        ],
        ramps: vec![
            PaletteRamp {
                name: "Body charcoal ramp (outline -> shadow -> highlight)".to_owned(),
                color_indices: vec![5, 0, 2],
            },
            PaletteRamp {
                name: "Screen glow ramp (shadow -> cyan glow -> highlight)".to_owned(),
                color_indices: vec![0, 1, 2],
            },
        ],
        allow_generated_colors: false,
    };
    let mut palette_details = SetPaletteDetails::new(palette, palette_body);
    palette_details.apply(doc)?;
    anchor(
        doc,
        palette,
        AnchorKind::Palette,
        AnchorStrength::Locked,
        "Exactly these six 8-bit colours, by role: charcoal body, cyan screen glow, off-white highlight, warm rust accent, sage-green accent, near-black outline. Shade along the named ramps only. One cohesive limited palette, locked across every asset in the world - no new colours, no gradients.",
    )?;
    fragments(
        doc,
        palette,
        vec![
            frag(
                "a crisp 6-colour 8-bit palette by role: charcoal body, cyan screen glow, off-white highlights, warm rust and sage-green accents, a near-black outline",
                InclusionPriority::Important,
            ),
            frag(
                "shade by stepping along the ramp (shadow to base to highlight), not by blending; one cohesive limited palette locked across the whole world",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        palette,
        &[
            "off-palette colours",
            "gradients",
            "more than six colours",
            "blended or dithered mid-tones outside the ramp",
        ],
    )?;
    status(doc, palette, EntryStatus::Canonical)?;

    // --- Pixel Art (Style) ---
    let pixel_art = id(handles, "pixel_art")?;
    update(
        doc,
        pixel_art,
        delta(
            "The house look: clean crisp pixel art with a limited palette, strong silhouette, flat solid colour, crisp edges, and consistent lighting. No anti-aliasing.",
            "",
            "Single-weight clean outlines, flat fills, hard pixel edges, even lighting across every frame. The silhouette does the work; detail stays minimal so sprites read at small sizes.",
            &["style", "pixel-art", "8bit", "canonical"],
        ),
    )?;
    let style_body = StyleDetails {
        rendering_rules: "Clean 8-bit pixel art on a fixed grid. Selective dark outline on the outer silhouette only; interior form reads by value, not by line. Flat solid fills from a limited palette, no more colours than the ramp allows. Even, flat lighting across every frame - no directional cast shadow, no rim light, no spotlight. In side and three-quarter views, overlapping limbs carry a near/far value split (near limb one step lighter, far limb one step darker) with a dark separation edge so the two never merge into one shape. Proportions hold as fractions of total height so the figure stays on-model at any zoom and reads at 32px.".to_owned(),
        line_treatment: LineTreatment::Selective,
        detail_level: DetailLevel::Low,
        anti_aliasing: AntiAliasingRule::Manual,
        resolution: Some((512, 512)),
        negative_rules: vec![
            "automatic anti-aliasing".to_owned(),
            "smooth gradients or soft shading".to_owned(),
            "blur or motion blur".to_owned(),
            "3d render or photo-realism".to_owned(),
            "off-grid sub-pixel detail".to_owned(),
            "interior outline scribble (outline the silhouette, not every interior shape)".to_owned(),
            "more colours than the palette ramp".to_owned(),
        ],
    };
    let mut style_details = SetStyleDetails::new(pixel_art, style_body);
    style_details.apply(doc)?;
    anchor(
        doc,
        pixel_art,
        AnchorKind::Style,
        AnchorStrength::Locked,
        "Clean 8-bit pixel art on a fixed grid: a selective dark outline on the outer silhouette only, interior read by value, flat solid fills from a limited palette, even flat lighting, hard pixel edges, manual hand-placed anti-aliasing only - never automatic.",
    )?;
    anchor(
        doc,
        pixel_art,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "No painterly gradients, no automatic anti-aliasing, no soft shading, no interior outline scribble, no off-grid sub-pixel detail.",
    )?;
    fragments(
        doc,
        pixel_art,
        vec![
            frag(
                "clean 8-bit pixel art on a fixed grid: a selective dark outline on the outer silhouette only, interior form read by value not by line, flat solid fills, hard pixel edges, even flat lighting",
                InclusionPriority::Important,
            ),
            frag(
                "overlapping limbs carry a near/far value split with a dark separation edge so they never merge; limited palette, manual anti-aliasing only",
                InclusionPriority::Normal,
            ),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(
        doc,
        pixel_art,
        &[NEG_STYLE],
        &["interior outline scribble", "same-value overlapping limbs reading as a blob"],
    )?;
    status(doc, pixel_art, EntryStatus::Canonical)?;

    // --- Retro-tech arcade (Vibe) ---
    let vibe = id(handles, "retro_arcade")?;
    update(
        doc,
        vibe,
        delta(
            "The mood of Bit's world: friendly retro-tech arcade. Glowing neon pixel screens, a circuit-board world, warm and optimistic - never grimdark.",
            "Everything hums with low-fi electricity: attract-mode glow, blinking node lights, the soft whine of a CRT. The feeling is a friendly arcade after hours, not a dystopia.",
            "Glowing cyan screens against charcoal, blueprint-grid floors, blinking node junctions. Light is warm and even; the palette stays bright and friendly.",
            &["vibe", "retro-tech", "arcade", "friendly", "canonical"],
        ),
    )?;
    generic(
        doc,
        vibe,
        &[
            ("mood", "friendly, optimistic, playful retro-tech - an arcade after hours, never a dystopia"),
            (
                "palette_cues",
                "saturated but limited: charcoal grounds, cyan screen glow, off-white highlights, warm rust and sage-green accents - the Bit Default set",
            ),
            (
                "lighting",
                "even and flat with a soft CRT bloom around lit screens and nodes; horizontally uniform on backgrounds so a layer can tile left-to-right without a hot side",
            ),
            (
                "setting",
                "the interior circuit-board world of a forgotten arcade cabinet: blueprint-grid floors, blinking node junctions, banks of pixel screens",
            ),
            ("era", "1980s-90s arcade register - scanline glow, attract-mode shimmer, chunky pixels"),
            ("tone_forbidden", "grimdark, horror, dystopian, gritty, photo-real"),
        ],
    )?;
    anchor(
        doc,
        vibe,
        AnchorKind::Lore,
        AnchorStrength::Strong,
        "Friendly retro-tech arcade: the interior circuit-board world of a forgotten cabinet, charcoal grounds under even flat light with a soft CRT bloom on cyan screens and nodes, saturated-but-limited 8-bit colour, warm and optimistic.",
    )?;
    anchor(
        doc,
        vibe,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "Never grimdark, never horror, never dystopian.",
    )?;
    fragments(
        doc,
        vibe,
        vec![
            frag(
                "friendly retro-tech arcade mood: a circuit-board world inside an old cabinet, glowing cyan pixel screens against charcoal, blinking node junctions, warm and optimistic",
                InclusionPriority::Normal,
            ),
            frag(
                "even flat lighting with a soft CRT bloom; saturated but limited 8-bit colour from @palette.bit_default; backgrounds lit horizontally evenly so they tile without a hot side",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        vibe,
        &["grimdark", "horror", "dystopian", "dark and gritty", "photo-real", "neon-noir rain"],
    )?;
    status(doc, vibe, EntryStatus::Canonical)?;

    // --- The seven animations ---
    detail_animations(doc, handles)?;

    // --- Turnaround (Pose / reference entry) ---
    //
    // The turnaround is a Pose entry, which holds a Generic body, so its four-view
    // breakdown and timing notes go in as generic fields (SetAnimationDetails would
    // reject a Pose entry). The description, fragments, negatives, and the three
    // Locked anchors carry the authored model-sheet spec. This is the identity
    // reference every other Bit animation is checked against (solid drawing).
    let turnaround = id(handles, "turnaround")?;
    update(
        doc,
        turnaround,
        delta(
            "Bit's model-sheet turnaround - front, three-quarter, side, and back views at identical scale and volume. Not a motion cycle: the identity reference every animation entry is built against.",
            "",
            "Four orthographic views in a row at identical scale and volume - front, three-quarter, side profile, back - the master reference the directional sprites and every animation derive from. Only the viewing angle changes (solid drawing).",
            &["pose", "turnaround", "reference", "model-sheet"],
        ),
    )?;
    generic(
        doc,
        turnaround,
        &[
            (
                "purpose",
                "Lock Bit's identity across viewing angles so every other animation stays on-model (solid drawing): identical volume and proportion across all views, one clear view per cell, a strong silhouette per view, the profile rule for legibility. A static multi-view reference, not a motion cycle.",
            ),
            ("loop_behavior", "Once - a reference sheet, not a played clip"),
            ("recommended_frame_count", "4 - one canonical view per cell"),
            ("fps", "2 - if ever cycled, a slow view-to-view flip, not motion"),
            (
                "view.front",
                "Bit square to camera, both feet planted, arms relaxed at the sides but clear of the torso, antenna straight up with a slight natural lean, the screen showing the neutral level eyes. Baseline proportions: ~2 heads tall, boxy CRT head over a chunky rounded biped body, a round-over-square silhouette.",
            ),
            (
                "view.three_quarter",
                "Rotated ~45 degrees, showing the depth of the CRT head and the body volume; antenna and screen visible; same height and mass as the front (solid drawing - volume constant).",
            ),
            (
                "view.side",
                "Full side profile: the profile rule for legibility, the depth of the head and the stubby antenna's attachment clear, one arm and one leg reading against the body, the screen edge-on or angled to stay readable. Same height line as the other views. Apply the near/far value split on the visible arm and leg (near lighter, far darker) with a dark separation edge.",
            ),
            (
                "view.back",
                "Rear view: head and body shape from behind, antenna from the back, no screen (the back of the CRT head), proportions identical to the front. Confirms the silhouette closes from every angle.",
            ),
        ],
    )?;
    anchor(
        doc,
        turnaround,
        AnchorKind::Animation,
        AnchorStrength::Locked,
        "Turnaround is a static four-view reference (front / three-quarter / side / back), not a motion cycle: identical height, scale, volume, and proportion across all views, a neutral pose, only the angle changes.",
    )?;
    anchor(
        doc,
        turnaround,
        AnchorKind::Scale,
        AnchorStrength::Locked,
        "Every view must read as a closed, on-model silhouette at 32px; this sheet is the identity all other Bit animations are checked against.",
    )?;
    anchor(
        doc,
        turnaround,
        AnchorKind::Style,
        AnchorStrength::Locked,
        "@style.pixel_art on @palette.bit_default - crisp 8-bit, no anti-aliasing, consistent across all four views.",
    )?;
    fragments(
        doc,
        turnaround,
        vec![
            frag(
                "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen as the only expression, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall, round-over-square silhouette), model-sheet turnaround.",
                InclusionPriority::Critical,
            ),
            frag(
                "Four views - front, three-quarter, side profile, back - at identical height, scale, volume, and proportion; only the viewing angle changes (solid drawing).",
                InclusionPriority::Important,
            ),
            frag(
                "A neutral standing pose in every view, arms relaxed but clear of the torso silhouette, antenna upright; the side view follows the profile rule for legibility; the back view shows the CRT head from behind with no screen.",
                InclusionPriority::Important,
            ),
            frag(
                "Each view is a strong, closed, readable silhouette at 32px; consistent feature placement (head, screen, antenna, limbs) across all four.",
                InclusionPriority::Important,
            ),
            frag(
                "Reference sheet: 4 views in a left-to-right grid, identical cell size, identical character scale, identical front lighting and camera distance per cell, a shared ground line.",
                InclusionPriority::Normal,
            ),
            frag(
                "crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        turnaround,
        &[
            "scale, height, or proportion change between views - identical volume across angles",
            "an expression change or action pose - neutral standing only",
            "a screen visible on the back view",
            "extra limbs, a duplicate character beyond the four intended views, background",
            "motion blur, action smears",
            "mouth, facial features beyond the pixel-face screen",
            "anti-aliasing, painterly gradients, soft shading",
            "perspective or lighting drift between cells",
        ],
    )?;
    status(doc, turnaround, EntryStatus::Canonical)?;

    // --- Floppy (Item) ---
    let floppy = id(handles, "floppy")?;
    update(
        doc,
        floppy,
        delta(
            "A retro floppy-disk power-up from Bit's world - a chunky 3.5-inch floppy disk with a glowing label and a pixel shine.",
            "Scattered through the circuit-board world, a Floppy is a fragment of forgotten data Bit can pick up. Byte often hands them over.",
            "A chunky 3.5-inch floppy disk seen front-on, sliding metal shutter at the top, a bright label across the middle with a soft glow, and a single pixel shine on one corner. 8-bit palette.",
            &["item", "power-up", "collectible", "retro", "floppy"],
        ),
    )?;
    generic(
        doc,
        floppy,
        &[
            ("kind", "power-up"),
            ("rarity", "common"),
            ("effect", "data fragment pickup"),
            (
                "silhouette",
                "a square 3.5-inch floppy-disk shell, sliding metal shutter across the top, a label strip across the middle",
            ),
            ("material", "matte plastic shell with a brushed-metal shutter, flat 8-bit fills - not glossy"),
            (
                "composition",
                "one object centred, filling about three-quarters of the frame, clear margin all around, not touching the edges",
            ),
            ("view", "flat 2D front-on, even ambient lighting, a single pixel shine - no perspective"),
        ],
    )?;
    anchor(
        doc,
        floppy,
        AnchorKind::Visual,
        AnchorStrength::Strong,
        "A chunky 3.5-inch floppy disk seen flat and front-on: square shell, a sliding metal shutter across the top, a glowing label strip across the middle, one pixel shine on a corner. One clean readable silhouette, centred, no perspective.",
    )?;
    anchor(
        doc,
        floppy,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Bit Default 8-bit palette; the label glows in the cyan screen-glow colour.",
    )?;
    fragments(
        doc,
        floppy,
        vec![
            frag(
                "a chunky 3.5-inch floppy-disk power-up, flat and front-on: square shell, a sliding metal shutter across the top, a glowing cyan label strip, one pixel shine",
                InclusionPriority::Important,
            ),
            frag(
                "one object centred and filling about three-quarters of the frame with a clear margin, even ambient lighting, a clean keyable silhouette",
                InclusionPriority::Important,
            ),
            frag(
                "from @character.bit's world, using @palette.bit_default in @style.pixel_art",
                InclusionPriority::Normal,
            ),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(
        doc,
        floppy,
        &[NEG_STYLE, NEG_ASSET],
        &[
            "modern USB drive",
            "object touching the frame edge",
            "more than one object",
            "glossy reflection",
        ],
    )?;
    status(doc, floppy, EntryStatus::Canonical)?;

    // --- Circuit Tiles (Material) ---
    let tiles = id(handles, "circuit_tiles")?;
    update(
        doc,
        tiles,
        delta(
            "A top-down circuit-board floor tileset for Bit's world - blueprint-grid lines, solder traces, glowing node junctions, seamless edges.",
            "",
            "A seamless top-down floor: blueprint-grid lines on charcoal, copper-style solder traces routing between glowing cyan node junctions. Tiles align on a grid with seamless edges so they repeat without seams.",
            &["material", "tileset", "circuit-board", "floor", "seamless"],
        ),
    )?;
    generic(
        doc,
        tiles,
        &[
            (
                "tiling",
                "seamless on all four edges: top matches bottom, left matches right, so tiles repeat with no visible seam",
            ),
            ("surface", "top-down circuit-board floor: a dark PCB-green substrate over charcoal"),
            (
                "detail",
                "fine copper-style solder traces routing between small silver solder-pad nodes that glow cyan; detail spread evenly so any patch looks interchangeable - no hero chip, no full-width trace run",
            ),
            (
                "node_language",
                "nodes sit at trace junctions, evenly distributed; vias glow in the cyan screen-glow colour",
            ),
            ("lighting", "even ambient light, no directional cast shadow, no center hotspot"),
            ("edges", "crisp anti-alias-free tile edges, no colour fringe"),
        ],
    )?;
    anchor(
        doc,
        tiles,
        AnchorKind::Visual,
        AnchorStrength::Strong,
        "A seamless top-down circuit-board floor: a dark PCB-green substrate, fine copper solder traces routing between small silver solder-pad nodes that glow cyan, blueprint-grid lines. Trace and node detail spread evenly so any region looks interchangeable - no hero chip, one continuous board.",
    )?;
    anchor(
        doc,
        tiles,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Bit Default 8-bit palette; node junctions glow in the cyan screen-glow colour.",
    )?;
    anchor(
        doc,
        tiles,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "No visible seam or grid line between tiles, no cell-sized panel, no full-width or full-height trace run, no single dominant chip, no edge fringe, no directional shadow.",
    )?;
    fragments(
        doc,
        tiles,
        vec![
            frag(
                "a seamless top-down circuit-board floor tile: dark PCB-green substrate, fine copper solder traces, small silver solder-pad nodes glowing cyan, blueprint-grid lines",
                InclusionPriority::Important,
            ),
            frag(
                "tiles seamlessly on all four edges (top matches bottom, left matches right); trace and node detail spread evenly so any patch is interchangeable - no hero chip, no full-width trace run, one continuous board",
                InclusionPriority::Important,
            ),
            frag(
                "even ambient lighting, crisp pixel edges; for @location.arcade_world, using @palette.bit_default in @style.pixel_art",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives_from(
        doc,
        tiles,
        &[NEG_STYLE],
        &[
            "visible tile seam",
            "grid line between tiles",
            "cell-sized panel or beveled block",
            "full-width or full-height trace run",
            "single dominant hero chip",
            "uneven detail density across the surface",
            "directional cast shadow",
            "edge fringe or halo",
        ],
    )?;
    status(doc, tiles, EntryStatus::Canonical)?;

    // --- The Arcade Cabinet World (Location) ---
    let world = id(handles, "arcade_world")?;
    update(
        doc,
        world,
        delta(
            "The circuit-board world inside a forgotten arcade cabinet where Bit lives - glowing screens, blueprint-grid floors, and humming node junctions.",
            "Behind the cabinet's dark screen is a whole world built from its own circuitry: floors of circuit-board, walls of stacked components, and the steady hum of a machine that never quite powered down. Bit calls it home; Byte lights its corners.",
            "Interiors of charcoal and glowing cyan: circuit-board floors (the Circuit Tiles), banks of pixel screens, blinking node junctions, all under warm even arcade light. Friendly, not grim.",
            &["location", "arcade", "circuit-board", "world", "canonical"],
        ),
    )?;
    generic(
        doc,
        world,
        &[
            ("type", "interior circuit-board world inside a forgotten arcade cabinet"),
            (
                "scene_brief",
                "Inside a dark cabinet, a whole world built from its own circuitry. Even flat light with a soft CRT bloom on cyan screens and nodes; charcoal grounds, saturated-but-limited 8-bit colour; warm, friendly, optimistic - never grim.",
            ),
            (
                "layer_sky",
                "opaque charcoal back wall with a faint blueprint grid; tonal gradient runs top-to-bottom only so it tiles horizontally without a hot side",
            ),
            ("layer_far", "distant silhouettes of stacked components and screen banks, dim cyan glow"),
            ("layer_mid", "rows of arcade-cabinet shapes and node junctions, the readable middle band"),
            ("layer_near", "foreground circuit-board floor (the Circuit Tiles) and nearby props"),
            ("lighting", "ambient and horizontally even; no sun or single light source on one side"),
        ],
    )?;
    anchor(
        doc,
        world,
        AnchorKind::Lore,
        AnchorStrength::Strong,
        "A circuit-board world inside a forgotten arcade cabinet, read in depth bands: a charcoal back wall, distant component silhouettes, a middle band of cabinets and node junctions, a foreground circuit-board floor. Even flat light with a soft CRT bloom, warm and friendly, never grim.",
    )?;
    anchor(doc, world, AnchorKind::Palette, AnchorStrength::Normal, "Bit Default 8-bit palette throughout.")?;
    fragments(
        doc,
        world,
        vec![
            frag(
                "the circuit-board world inside a forgotten arcade cabinet: a charcoal back wall, distant component silhouettes, a middle band of cabinets and blinking node junctions, a foreground floor of @material.circuit_tiles",
                InclusionPriority::Normal,
            ),
            frag(
                "ambient, horizontally even lighting with a soft CRT bloom (no light on one side); @palette.bit_default in @style.pixel_art, the @vibe.retro_arcade mood",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        world,
        &[
            "grimdark",
            "horror",
            "outdoor landscape",
            "realistic photo",
            "a single directional light source",
            "characters baked into the background",
        ],
    )?;
    status(doc, world, EntryStatus::Canonical)?;

    // --- Rules ---
    detail_rules(doc, handles)?;

    // --- Start Button (UiElement) ---
    let button = id(handles, "start_button")?;
    update(
        doc,
        button,
        delta(
            "The title-screen Start button - a chunky pixel-art button with a glowing label, in Bit's house look.",
            "",
            "A chunky rounded rectangular button with a near-black outline, charcoal fill, an off-white pixel label reading START, and a soft cyan glow on its active state. Hard pixel edges.",
            &["ui", "button", "hud", "title-screen"],
        ),
    )?;
    generic(doc, button, &[("shape", "chunky rounded rectangle"), ("label", "START")])?;
    anchor(
        doc,
        button,
        AnchorKind::Style,
        AnchorStrength::Strong,
        "Clean pixel-art button, hard edges, flat fills, cyan glow on the active state.",
    )?;
    anchor(
        doc,
        button,
        AnchorKind::Palette,
        AnchorStrength::Normal,
        "Bit Default 8-bit palette; glow in the cyan screen-glow colour.",
    )?;
    fragments(
        doc,
        button,
        vec![frag(
            "a chunky pixel-art Start button with a glowing label, using @palette.bit_default in @style.pixel_art",
            InclusionPriority::Normal,
        )],
    )?;
    negatives(doc, button, &["3d bevel", "gradient fill", "drop shadow blur"])?;
    status(doc, button, EntryStatus::Canonical)?;

    // --- Bit idle cycle (Recipe) ---
    let recipe = id(handles, "bit_idle_cycle")?;
    update(
        doc,
        recipe,
        delta(
            "A reusable recipe for generating Bit's idle breathing loop: the idle animation of Bit in the house style and palette.",
            "",
            "An 8-frame idle loop sheet of Bit breathing in place, consistent scale, transparent background.",
            &["recipe", "idle", "animation", "workflow"],
        ),
    )?;
    generic(
        doc,
        recipe,
        &[
            ("character", "bit"),
            ("animation", "idle"),
            ("style", "pixel_art"),
            ("palette", "bit_default"),
            ("frames", "8"),
            ("fps", "8"),
            ("canvas", "512x512 cells, pin the resolution explicitly"),
            (
                "step_1_anchor",
                "Lock identity first: one neutral Bit reference on a flat key (the turnaround front view), the canonical on-model image every cell is matched against.",
            ),
            (
                "step_2_pose_table",
                "Author the 8 idle pose beats (rest, inhale rise, top-of-breath moving hold, blink, exhale settle) as the pose map the model skins - do not let the model invent the motion.",
            ),
            (
                "step_3_skin",
                "Render the 8-frame sheet with the anchor attached so every cell stays the same robot; lowest temperature for an identity-critical multi-cell sheet.",
            ),
            (
                "step_4_normalize",
                "Align and scale-normalize the cells to one baseline and one scale; key the background to transparent.",
            ),
            (
                "step_5_review",
                "Check the sheet against the Rules folder: on-model identity, in-place stability, clean keyed silhouette, no off-palette colour.",
            ),
        ],
    )?;
    anchor(
        doc,
        recipe,
        AnchorKind::Animation,
        AnchorStrength::Normal,
        "One full slow breath across an 8-frame loop; feet planted, antenna lag.",
    )?;
    fragments(
        doc,
        recipe,
        vec![frag(
            "generate @character.bit doing @animation.idle in @style.pixel_art using @palette.bit_default, an 8-frame loop on a 512x512 transparent canvas, every cell matched to the @pose.turnaround front view, checked against @rule.identity_lock and @rule.clean_silhouette",
            InclusionPriority::Normal,
        )],
    )?;
    negatives(doc, recipe, &["drift between frames", "scale change", "background"])?;
    status(doc, recipe, EntryStatus::Canonical)?;

    // --- The forbidden alternative style, the new rules, and the two recipes ---
    detail_forbidden_style(doc, handles)?;
    detail_new_recipes(doc, handles)?;

    Ok(())
}

/// Fills in `flat_3d_render`: the explicitly forbidden alternative style, kept as a
/// concrete "not this" reference. It is Deprecated so the resolver suggests
/// `pixel_art` as its replacement, carries a single Negative anchor, and has no
/// positive fragments so it can never enter a prompt.
fn detail_forbidden_style(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    let forbidden = id(handles, "flat_3d_render")?;
    update(
        doc,
        forbidden,
        delta(
            "A smooth 3D-render look - soft gradients, ambient-occlusion shading, anti-aliased edges. The opposite of the house style; kept only as the forbidden reference so 'not this' is concrete.",
            "",
            "Soft shaded volumes, gradient fills, blurred edges, baked lighting - everything @style.pixel_art forbids.",
            &["style", "forbidden", "reference"],
        ),
    )?;
    let body = StyleDetails {
        rendering_rules: "Smooth gradient shading, ambient occlusion, anti-aliased edges, baked directional lighting.".to_owned(),
        line_treatment: LineTreatment::None,
        detail_level: DetailLevel::High,
        anti_aliasing: AntiAliasingRule::Allowed,
        resolution: None,
        negative_rules: vec![],
    };
    let mut details = SetStyleDetails::new(forbidden, body);
    details.apply(doc)?;
    anchor(
        doc,
        forbidden,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "Never render Bit's world this way; this is the explicitly forbidden style, not an option.",
    )?;
    // Deprecated last: it still resolves so `forbidden_styles` and relationships point
    // at it, and the resolver offers `pixel_art` as the suggested replacement.
    status(doc, forbidden, EntryStatus::Deprecated)?;
    Ok(())
}

/// One reusable generation recipe beyond the idle cycle: a handle, header text, the
/// generic step spine, one positive fragment, and a single anchor.
struct RecipeSpec {
    handle: &'static str,
    description: &'static str,
    visual: &'static str,
    tags: &'static [&'static str],
    /// The ordered `(key, value)` step fields of the recipe body.
    steps: &'static [(&'static str, &'static str)],
    /// The single compiled-prompt fragment, at Normal priority.
    fragment: &'static str,
    /// The recipe's single anchor: kind, strength, statement.
    anchor: (AnchorKind, AnchorStrength, &'static str),
}

/// Fills in the two reusable recipes: a character sprite-sheet recipe and a tileset
/// recipe. Both stay at the codex root (no folder), like `bit_idle_cycle`. Each is the
/// anchor-then-skin / lock-then-paint spine distilled into a recipe entry.
fn detail_new_recipes(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    let specs = [
        RecipeSpec {
            handle: "bit_sprite_sheet",
            description: "The reusable recipe for any Bit action sprite sheet: anchor the identity, author the pose table, skin onto it, align, then review.",
            visual: "",
            tags: &["recipe", "sprite-sheet", "character", "workflow"],
            steps: &[
                ("step_1_anchor", "Neutral Bit on a flat key from @pose.turnaround as the on-model anchor."),
                (
                    "step_2_pose_table",
                    "Pick the action's pose beats from the matching animation entry as the pose map.",
                ),
                (
                    "step_3_skin",
                    "Render the sheet with the anchor attached; lowest temperature for identity-critical multi-cell work.",
                ),
                ("step_4_normalize", "Same baseline, same scale, in-place; key to transparent."),
                ("step_5_review", "Check against the Rules folder."),
                ("canvas", "512x512 cells"),
            ],
            fragment: "generate a @character.bit action sheet in @style.pixel_art using @palette.bit_default, every cell matched to the @pose.turnaround anchor, same scale and baseline, transparent background, checked against @rule.single_subject and @rule.identity_lock",
            anchor: (
                AnchorKind::Animation,
                AnchorStrength::Normal,
                "Anchor-then-skin: lock Bit's identity from the turnaround, render every cell against it, normalize to one baseline and scale.",
            ),
        },
        RecipeSpec {
            handle: "circuit_tileset",
            description: "The reusable recipe for the Circuit Tiles set: lock the palette across the set, paint the body for seamless tiling, then review the edges.",
            visual: "",
            tags: &["recipe", "tileset", "material", "workflow"],
            steps: &[
                ("step_1_lock", "Lock @palette.bit_default across the whole set so no tile drifts."),
                (
                    "step_2_paint",
                    "Paint the body tile for seamless tiling: top matches bottom, left matches right, even detail, no hero chip.",
                ),
                ("step_3_edges", "Heal the seams and check edge-cap consistency."),
                ("step_4_review", "Check against @rule.tile_seamless and @rule.even_lighting."),
                ("temperature", "moderate - lower than free generation, higher than a character sheet"),
            ],
            fragment: "generate a @material.circuit_tiles set in @style.pixel_art using @palette.bit_default, seamless on all four edges, even detail with no hero chip, checked against @rule.tile_seamless",
            anchor: (
                AnchorKind::Style,
                AnchorStrength::Normal,
                "Lock the palette across the set, paint for seamlessness, heal the seams, review the edges.",
            ),
        },
    ];

    for spec in specs {
        let entry = id(handles, spec.handle)?;
        update(doc, entry, delta(spec.description, "", spec.visual, spec.tags))?;
        generic(doc, entry, spec.steps)?;
        let (kind, strength, statement) = spec.anchor;
        anchor(doc, entry, kind, strength, statement)?;
        fragments(doc, entry, vec![frag(spec.fragment, InclusionPriority::Normal)])?;
        status(doc, entry, EntryStatus::Canonical)?;
    }
    Ok(())
}

/// One animation's full specification, grounded in the animation-principles knowledge
/// base so the demo Codex teaches as it generates. Each field maps to a public Codex
/// command: `description` to [`UpdateCodexEntry`], the timing/loop/`beats` to
/// [`SetAnimationDetails`], `fragments` to [`SetPromptFragments`], `negatives` to
/// [`SetNegativeFragments`], and the three anchors to [`SetAnchor`].
struct AnimSpec {
    handle: &'static str,
    description: &'static str,
    purpose: &'static str,
    loop_behavior: LoopBehavior,
    frames: u32,
    fps: u16,
    /// The key poses, in playback order; each beat names the principle it embodies.
    beats: &'static [(&'static str, &'static str)],
    /// Priority-tagged positive fragments: a Critical identity+action line, the
    /// Important motion/timing/arc/weight lines, then Normal sheet-framing and
    /// `@`-references that resolve against the world.
    fragments: &'static [(&'static str, InclusionPriority)],
    /// Per-entry negative fragments (drift, off-model, clipping, scale-change, etc.).
    negatives: &'static [&'static str],
    /// The motion-intent Animation anchor, at the given strength.
    animation_anchor: (AnchorStrength, &'static str),
    /// The Scale anchor that keeps the motion readable at 32px.
    scale_anchor: (AnchorStrength, &'static str),
    /// The Style anchor pinning the house look.
    style_anchor: (AnchorStrength, &'static str),
}

/// Fills in the seven Bit animations as a best-in-class, principle-grounded teaching
/// set. Each spec is self-contained: its own principled fps and frame count (from the
/// timing/spacing KB, not a shared constant), pose beats that walk the key poses and
/// name the principle each one embodies, priority-tagged prompt fragments (a Critical
/// identity+action line, Important motion/timing/arc/weight/anticipation/overlap
/// lines, Normal sheet-framing and `@`-references), per-entry negatives, and three
/// anchors (Animation motion-intent, Scale 32px-read, Style house-look).
///
/// Two families share discipline: the locomotion family (idle / walk / run) shares
/// ground-contact, weight read through the up/down of mass, and antenna-lag overlap;
/// the impact/reaction family (attack / hurt) shares the AAR shape (anticipation,
/// snap action, overshoot, held settle) and the screen-switch-on-settle rule.
///
/// One long body on purpose: the seven specs are flat data with no branching to
/// factor out, and keeping them in one table keeps the fixture auditable against the
/// authored spec.
#[allow(clippy::too_many_lines)]
fn detail_animations(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    use AnchorStrength::{Locked, Strong};
    use InclusionPriority::{Critical, Important, Normal};

    let specs = [
        AnimSpec {
            handle: "idle",
            description: "Bit at rest, alive but still - a slow breathing oscillation with a periodic blink and a trailing antenna sway. The base loop every other state returns to.",
            purpose: "Keep Bit reading as alive while idle by replacing a dead static hold with a moving hold (a static hold past ~8 frames looks dead). Governing principles: moving-hold timing, slow-in/slow-out on the breath, secondary action and overlap/follow-through on the antenna, appeal. The body breathes; the antenna lags and settles; the screen blinks as punctuation.",
            loop_behavior: LoopBehavior::Loop,
            frames: 8,
            fps: 6,
            beats: &[
                (
                    "Rest low (neutral)",
                    "Settled square on both feet, shoulders at their lowest, screen showing soft level eyes (content glyph), antenna upright with a hair of lean. Line of action is a gentle resting S-curve (line-of-action).",
                ),
                (
                    "Inhale rise (slow-out)",
                    "Torso eases upward, shoulders lift, the body stretches a touch taller without changing volume; spacing tightens at the start of the rise (squash/stretch volume-preserved, slow-out).",
                ),
                (
                    "Top of breath (moving hold)",
                    "Highest point, briefly held; the antenna still catches up, leaning back as it lags the rise by ~2-3 frames (wave action, overlap). No dead freeze - this is the moving hold, not a stop.",
                ),
                (
                    "Blink accent",
                    "A single-frame screen blink as punctuation, placed at the top so it reads as a calm beat (acting: blinks mark beats).",
                ),
                (
                    "Exhale settle low (slow-in)",
                    "Torso eases back down to rest, decelerating into the low pose; the antenna overshoots forward slightly then settles (slow-in, follow-through with decaying amplitude).",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen as the only expression, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), standing idle and breathing in place.",
                    Critical,
                ),
                (
                    "Slow breathing oscillation: torso rises and falls a few pixels over the loop, slow-in and slow-out, never a frozen pose (a moving hold, not a static hold).",
                    Important,
                ),
                (
                    "Antenna lags the breath by 2-3 frames and overshoots slightly before settling - overlap and follow-through; the blinking pixel rides the tip.",
                    Important,
                ),
                (
                    "Pixel-face screen holds a soft, level, content expression with one single-frame blink as punctuation; expression states switch crisply, never blur or morph.",
                    Important,
                ),
                (
                    "Feet stay planted flat on the ground line across all frames; weight even on both feet; a gentle resting S-curve in the body silhouette, readable at 32px.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera in every cell.",
                    Normal,
                ),
                (
                    "crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background for clean keying",
                    Normal,
                ),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "static frozen pose, dead hold",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, a second Bit in a cell",
                "background scenery, background changes between cells",
                "motion blur, smear frames",
                "mouth, facial features beyond the pixel-face screen",
                "anti-aliasing, painterly gradients, soft shading",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Locked,
                "Idle is an 8-frame loop at 6 fps: breathe up on slow-out, hold at the top as a moving hold, settle down on slow-in; the antenna lags 2-3 frames and overshoots; one blink per loop. The loop seam is seamless - the last frame eases into the first.",
            ),
            scale_anchor: (
                Strong,
                "The breathing rise and antenna sway must stay readable in the 32px silhouette; no detail that only reads when zoomed in.",
            ),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default - crisp 8-bit, no anti-aliasing."),
        },
        AnimSpec {
            handle: "walk",
            description: "Bit's standard walk loop - a brisk, energetic four-pose step cycle with a clear weight shift, counter-swinging arms, and a bobbing head the antenna trails behind. The locomotion backbone.",
            purpose: "A readable, weight-bearing walk on the four-pose foundation (contact / down / passing / up), conveying weight through the head/body bob and the belt-line tilt. Governing principles: arcs, slow-in/slow-out, follow-through on the antenna, timing, weight-shift on the belt line, arm counter-swing in opposition. Encodes contralateral motion and avoids the lockstep-arms robot tell.",
            loop_behavior: LoopBehavior::Loop,
            frames: 8,
            fps: 10,
            beats: &[
                (
                    "Contact (right lead)",
                    "Right foot lands extended forward, left foot extended behind, body just above mid-height. Left arm forward, right arm back (opposition). Belt line tilts toward the off-weight leg (weight-shift). Feet travel on a clear arc, not a straight slide.",
                ),
                (
                    "Down / recoil",
                    "Weight transfers onto the front leg, knee bends, body at its lowest point - where weight reads. The arm reaches its peak swing here, one frame after contact (the arm lags the leg). Belt line tilts strongly toward the bent weight-bearing leg.",
                ),
                (
                    "Passing",
                    "The free leg passes the standing leg, the standing leg straight, body at/near its highest. Belt line briefly level - the personality pose. Light S-curve through the spine, shoulders counter-tilting the hips.",
                ),
                (
                    "Up / push-off",
                    "The standing leg straightens fully, pushing the body to peak height, the free foot reaching forward into the next contact. Belt line tilts toward the push-off leg. The antenna trails the bob, lagging ~2-4 frames, overshooting at the top before settling (overlap/follow-through). Screen neutral-content, eyes forward.",
                ),
                (
                    "Contact (left lead)",
                    "Mirror of the first contact for the second step: left foot lands, right behind, arms swap to the opposite opposition. The remaining beats mirror down / passing / up.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), walking in a side-view step cycle.",
                    Critical,
                ),
                (
                    "Four-pose walk per step - contact, down (lowest, knee bent, weight reads here), passing (highest, body lifts), up (push-off) - over two steps, left and right mirrored.",
                    Important,
                ),
                (
                    "Arms swing in opposition to the legs (left arm forward when the right leg leads) and peak one frame after the foot contacts; never in lockstep with the legs.",
                    Important,
                ),
                (
                    "A clear head and body up-down bob conveys weight; the belt line tilts toward the weight-bearing leg every step and is never level for long - no sliding feet.",
                    Important,
                ),
                (
                    "Feet travel on arcs and plant firmly on the ground line at contact; the antenna trails the head bob by 2-4 frames and overshoots at the top (overlap and follow-through).",
                    Important,
                ),
                (
                    "A light S-curve through the body, shoulders counter-tilting the hips; an asymmetric pose, never twinned left-right; silhouette readable at 32px with limbs clear of the torso.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "floating or skating feet, a sliding walk",
                "a level belt line, flat hips across the cycle",
                "arms moving in lockstep with the legs",
                "twinned symmetric left-right poses",
                "straight-line, arc-less limb paths",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Locked,
                "Walk is an 8-frame loop at 10 fps: two steps, four poses each (contact / down / passing / up), arms in opposition peaking one frame after foot contact, the belt line tilting toward the weight leg, the antenna lagging 2-4 frames. Body lowest at down, highest at passing.",
            ),
            scale_anchor: (
                Strong,
                "The bob, weight shift, and stride must read in the 32px silhouette; limbs held clear of the torso outline.",
            ),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "run",
            description: "Bit's run loop - a fast, leaning four-pose cycle with a longer stride and an airborne passing frame where both feet leave the ground. The high-energy sibling of the walk.",
            purpose: "A believable cartoon run that obeys the one rule separating a run from a fast walk: at least one frame with both feet off the ground at passing. Governing principles: timing (fewer frames, on ones), arcs, exaggeration (the forward lean), weight, follow-through (the antenna streams back). Minimal head bob - over-bobbing reads as jumping.",
            loop_behavior: LoopBehavior::Loop,
            frames: 4,
            fps: 12,
            beats: &[
                (
                    "Contact",
                    "Front foot lands, knee deeply bent absorbing, body low and pitched forward ~20-30 degrees (the run lean; exaggeration sells speed). Arms bent ~90 degrees in fists, in opposition, kept close to the body.",
                ),
                (
                    "Down / recoil",
                    "The knee absorbs and the push-off begins, body starting to rise, the rear leg loading. The stride is longer than the walk's.",
                ),
                (
                    "Passing - AIRBORNE (the money frame)",
                    "Both feet off the ground, body at its highest, briefly weightless; the front leg extending forward, the back leg trailing fully extended. This is the frame that makes it a run, not a walk. Antenna pinned back, streaming with momentum (~4-8 frame lag).",
                ),
                (
                    "Up / push-off",
                    "The back leg extends explosively, launching into the next stride, the front foot reaching forward. The head rises only a third-to-half a head height - minimal bob. Screen determined/focused, eyes fixed forward. The antenna whips on the push-off then trails.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), running in a fast side-view cycle.",
                    Critical,
                ),
                (
                    "Four-pose run on ones - contact, down, airborne passing, push-off - with at least one frame where BOTH feet are off the ground at passing; this is what makes it a run, not a fast walk.",
                    Important,
                ),
                (
                    "A strong forward body lean (20-30 degrees) sells the speed; a longer stride than a walk; arms bent ~90 degrees in loose fists, a short swing, kept close, in opposition to the legs.",
                    Important,
                ),
                (
                    "Minimal head bob - the head rises only a third to half a head height; over-bobbing reads as jumping, not running.",
                    Important,
                ),
                (
                    "The antenna streams backward with momentum, pinned back through the airborne frame and whipping on push-off (overlap/follow-through, more lag than the walk); the blinking pixel trails the tip.",
                    Important,
                ),
                (
                    "An asymmetric leg pose, feet on arcs, a dynamic silhouette readable at 32px with limbs clear of the torso.",
                    Important,
                ),
                (
                    "Sprite sheet: 4 frames in a left-to-right grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a both-feet-down run - the passing frame must be airborne",
                "an upright vertical body, a missing forward lean",
                "excessive head bob",
                "arms in lockstep with the legs, wide loose walk-arms",
                "twinned symmetric poses",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
            ],
            animation_anchor: (
                Locked,
                "Run is a 4-frame loop at 12 fps on ones: contact, down, AIRBORNE passing (both feet off the ground), push-off. Forward lean 20-30 degrees, minimal head bob, arms bent ~90 degrees in opposition, the antenna streaming back.",
            ),
            scale_anchor: (Strong, "The airborne passing frame and the forward lean must be unambiguous at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "jump",
            description: "Bit's standing jump - anticipation crouch, explosive stretch launch, weightless apex, heavy squash landing, and a settle. A full five-phase weight study, played once.",
            purpose: "Teach weight and force through the five jump phases, where the anticipation crouch depth telegraphs the jump size before it happens and the landing squash plus held compression sells the impact. Governing principles: anticipation, squash/stretch (volume-preserved), arcs (parabolic, symmetric about the apex), follow-through, the contact-before-squash weight trick, decay easing on the fall.",
            loop_behavior: LoopBehavior::Once,
            frames: 12,
            fps: 12,
            beats: &[
                (
                    "Anticipation crouch",
                    "Knees bend deep, the body drops and squashes wider (volume preserved), arms draw back and down, the head dips, the screen narrows to a loaded/focused glyph. Forward C-curve. The crouch depth telegraphs the jump height - the soul of the jump. The antenna whips down/back.",
                ),
                (
                    "Launch / push-off stretch",
                    "Legs extend explosively, the body stretches tall, arms swing up. A straight-diagonal line of action (power). Hold one foot in contact one extra frame while extending (the contact-before-squash trick) for push-off weight. The antenna snaps up and trails the stretch.",
                ),
                (
                    "Apex",
                    "Both feet off the ground, the body extended and reaching up, briefly weightless; the top of a parabolic arc symmetric about the apex. Screen bright/wide (excited). Hands lead. The antenna streams.",
                ),
                (
                    "Landing contact (squash)",
                    "Feet plant, one frame of contact with the body still extended BEFORE the knees squash (contact-before-squash makes it land heavy), then the knees absorb and the body squashes down and wider, arms forward for balance, head/screen bobs down, screen squints on impact. Decay easing - gravity accelerates the fall, slow-out only, no slow-in.",
                ),
                (
                    "Recover / settle",
                    "Hold the low landing pose a beat (chunky weight), then rise back to standing with follow-through; the antenna overshoots down past the body then oscillates up with decaying amplitude and settles. Skip the recovery and the jump feels glued.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), performing a standing jump.",
                    Critical,
                ),
                (
                    "Five phases in order - anticipation crouch, launch stretch, weightless apex, landing squash, recover - each a distinct key pose; no phase skipped.",
                    Important,
                ),
                (
                    "Anticipation: a deep knee bend and body crouch (forward C-curve) before launch; the crouch depth telegraphs the jump height. No jump without the crouch.",
                    Important,
                ),
                (
                    "Squash and stretch with volume preserved - squash wider on the crouch and the landing, stretch taller on the launch and apex; the rigid head and limbs hold their shape, never gummy.",
                    Important,
                ),
                (
                    "The body follows a parabolic arc symmetric about the apex; the fall accelerates (slow-out only, decay easing); land heavy with one contact frame before the knees squash, then hold the low pose a beat.",
                    Important,
                ),
                (
                    "The antenna whips down on the crouch, snaps up trailing the launch, streams at the apex, overshoots down on impact and oscillates to a settle (overlap/follow-through); the screen reads loaded on the crouch, bright-wide at the apex, squint on impact.",
                    Important,
                ),
                (
                    "Feet are off the ground at the apex (airborne); a strong readable silhouette at 32px in every phase.",
                    Important,
                ),
                (
                    "Sprite sheet: 12 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "feet still grounded at the apex - Bit must be airborne at the top",
                "a missing anticipation crouch, a launch without a wind-up",
                "gummy or rubber-hose limbs, volume change on the head",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Jump plays once across 12 frames at 12 fps through five phases: crouch (telegraph), launch stretch, airborne apex, landing squash (contact-before-squash, a held low beat), recover. A parabolic arc symmetric about the apex; the fall on decay easing.",
            ),
            scale_anchor: (Strong, "The crouch depth, the airborne apex, and the landing squash must each read at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "fall",
            description: "Bit losing balance and dropping - a tip past the point of no return, a flailing descent, a heavy impact, and an overshoot settle. Built from balance loss and counter-reaction, played once.",
            purpose: "Show an involuntary fall reading as weight surrendered to gravity, with the centre of gravity tipping past the foot base, counter-reaction staggering the parts, and a heavy impact-then-settle. Governing principles: balance/counterbalance, counter-reaction (body leads, head lags, antenna lags most), arcs, follow-through, decay easing (constant gravity, slow-out only, no slow-in), contact-before-squash on impact.",
            loop_behavior: LoopBehavior::Once,
            frames: 12,
            fps: 12,
            beats: &[
                (
                    "Off-balance tip",
                    "The centre-of-gravity line passes outside the foot base; the body leans past its support, one leg extends back in a last-moment balance attempt, arms windmill forward and out (not opposing the legs). The screen flashes to alarm - wide/dilated eyes. The CoG outside the base is what reads as about to fall.",
                ),
                (
                    "Free-fall descent",
                    "Arms out for balance, legs trailing, the body tipping along its line of action and accelerating (decay, slow-out only). Counter-reaction: the head/screen lags the body, the antenna lags most, trailing up and back. A forward/tumbling C-curve.",
                ),
                (
                    "Impact contact (squash)",
                    "The body hits, one contact frame still extended BEFORE the squash (contact-before-squash for weight), then the knees/body absorb and squash down, arms come forward. The screen squints on impact.",
                ),
                (
                    "Settle / recovery",
                    "Overshoot then settle; the antenna whips forward/down past the body (overshoot 20-30%), swings back, oscillates twice with diminishing amplitude before rest. The screen does a small recovery blink back toward neutral. A stop is an event: decelerate, overshoot, settle - never a hard freeze.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), losing balance and falling.",
                    Critical,
                ),
                (
                    "Sequence - off-balance tip, free-fall descent, heavy impact, settle. The tip reads because the centre of gravity leans past the feet; one leg kicks back, arms windmill out for balance.",
                    Important,
                ),
                (
                    "The descent accelerates under gravity (decay easing, slow-out only, no slow-in); counter-reaction staggers the parts - the body leads, the head/screen lags, the antenna lags the most, trailing back.",
                    Important,
                ),
                (
                    "Land heavy: one contact frame with the body still extended before it squashes, then absorb and squash down (volume preserved); arms forward.",
                    Important,
                ),
                (
                    "The settle is an event, not a freeze - overshoot then settle; the antenna whips past the body 20-30% and oscillates down to rest; the screen flashes alarm (wide eyes) on the tip and descent, squints on impact, blinks on recovery.",
                    Important,
                ),
                (
                    "A forward/tumbling C-curve line of action; a silhouette readable at 32px through the tip and the impact.",
                    Important,
                ),
                (
                    "Sprite sheet: 12 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a balanced upright pose during the fall - the centre of gravity must be past the feet",
                "a constant-speed descent - the fall accelerates",
                "a hard frozen stop on landing - it overshoots and settles",
                "arms opposing the legs like a walk - arms windmill out",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Fall plays once across 12 frames at 12 fps: off-balance tip (CoG past the feet), accelerating descent (decay easing), heavy impact (contact-before-squash), overshoot settle. Counter-reaction staggers body, head, antenna; the antenna overshoots and oscillates to rest.",
            ),
            scale_anchor: (Strong, "The tip past balance and the heavy impact must read at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "attack",
            description: "Bit's melee swing - a coiled wind-up away from the target, a one-to-two-frame strike, a recoil bounce where the hit lands for the viewer, and a long settle. A hard accent wrapped in anticipation-action-reaction, played once.",
            purpose: "Land a decisive melee strike as a hard accent inside the AAR formula, where the audience feels the hit on the recoil/bounce, not at contact. Governing principles: anticipation (opposite direction, ~2x the action), the hard-accent bounce-back, snap action (no slow-in, 1-2 frames), arcs, follow-through, a held settle (6+ frames at speed) so the accent reads, exaggeration. Pairs with hurt as the impact/reaction family.",
            loop_behavior: LoopBehavior::Once,
            frames: 8,
            fps: 12,
            beats: &[
                (
                    "Ready / neutral",
                    "Settled, weight centred, the screen showing a focused glyph (small constricted pupils = focus/aggression).",
                ),
                (
                    "Wind-up anticipation (the longest beat)",
                    "The body coils AWAY from the strike target, the striking limb drawn back, weight shifts to the back foot, the screen narrows to a determined squint. A backward C-curve. The antic runs ~2x the action time - go back before you go forward.",
                ),
                (
                    "Strike / contact (1-2 frames, fast)",
                    "The limb drives through along a straight-diagonal line of action (power); shown having passed through the target, not dwelling on contact. A single elongated streak on this frame reads as speed. No slow-in - snap from the held wind-up.",
                ),
                (
                    "Overshoot / bounce-back",
                    "The limb passes max extension then recoils slightly toward the body; this is where the hit lands for the viewer (a hard accent - the snap is the bounce). Any impact spark sits one frame after contact, on the bounce. The antenna whips hardest here.",
                ),
                (
                    "Settle (held, long)",
                    "The body returns to balance over several frames; the screen relaxes from squint back to neutral/satisfied (switching on the settle frame, never a crossfade); the antenna does one last overshoot-and-settle. A hard accent must hold or the hit feels unimportant.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), performing a decisive melee attack swing.",
                    Critical,
                ),
                (
                    "Anticipation-action-reaction: a long wind-up coiling AWAY from the target (about twice the strike's length), a fast 1-2 frame strike, then a recoil bounce - the hit reads on the bounce, not at contact.",
                    Important,
                ),
                (
                    "The wind-up is a backward C-curve with weight on the back foot; the strike drives through on a straight-diagonal line of action and is shown having passed through the target, never dwelling on the contact frame.",
                    Important,
                ),
                (
                    "Snap the strike with no slow-in (held wind-up, then sudden release); a single elongated streak on the strike frame reads as speed; the limb overshoots past full extension then recoils toward the body.",
                    Important,
                ),
                (
                    "The antenna whips hardest on the strike-and-bounce and settles last; any impact spark appears one frame after contact, on the recoil (overlap/follow-through).",
                    Important,
                ),
                (
                    "The screen reads focused (small pupils) through the wind-up and switches crisply to neutral/satisfied on the held settle frame - never a blur or morph; the settle is held long enough that the hit feels important.",
                    Important,
                ),
                (
                    "A strong silhouette at 32px in the wind-up and the strike, limbs clear of the torso.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a strike without a wind-up - a teleporting attack",
                "dwelling on the contact frame, ending on the strike with no settle",
                "slow-in on the strike - it snaps",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur as a soft smear across cells (a single crisp streak frame only), painterly gradients",
                "mouth, anti-aliasing",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Attack plays once across 8 frames at 12 fps as a hard accent in AAR: a long wind-up away from the target (~2x the action), a 1-2 frame snap strike on a straight diagonal, a recoil bounce where the hit lands, a held settle. The screen switches focused to neutral on the settle frame.",
            ),
            scale_anchor: (Strong, "The wind-up coil and the strike extension must read at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "hurt",
            description: "Bit taking a hit - a brief compress into the blow, a sharp recoil away, an overshoot past a believable position, and a decaying settle back toward neutral. A take driven by contrast, played once. The reaction sibling of the attack.",
            purpose: "Sell an impact reaction as a take built on contrast - the recoil reads hardest springing from a content/neutral state, following the take's universal DOWN-then-UP pattern. Governing principles: anticipation (the down-compress), takes/accents (a 1-2 frame snap), follow-through/overshoot, contrast (start opposite the destination), the moving-hold settle, a restrained volume-preserving squash (a chunky robot, not gummy). Pairs with attack as the impact/reaction family.",
            loop_behavior: LoopBehavior::Once,
            frames: 8,
            fps: 12,
            beats: &[
                (
                    "Pre-hit contrast pose",
                    "Caught content/neutral, the screen soft and level. The wider the swing from fine to hurt, the harder the recoil lands (look for contrasts).",
                ),
                (
                    "Down anticipation (compress)",
                    "The body crouches and compresses INTO the hit direction for a beat before reacting; the take is DOWN then UP. A restrained squash, volume preserved (a chunky robot - widen what flattens, do not shrink or grow him).",
                ),
                (
                    "Accent / impact (1-2 frames)",
                    "A sharp recoil AWAY from the hit: the head and body snap back, the screen flashes to an alarm glyph (wide/dilated eyes, or a jagged glyph). A backward C-curve. A single smear or doubled silhouette sells the violence. The hardest single frame.",
                ),
                (
                    "Overshoot",
                    "The body travels PAST a believable recoil position; the antenna whips back hardest of all (whip action, a one-frame crack to an impossibly-far lean).",
                ),
                (
                    "Settle back (held, decaying)",
                    "The body springs back through balance with a small decaying oscillation; the screen switches (never a crossfade) from alarm back toward neutral/dazed on the settle frame; the antenna does one last overshoot-and-settle. A moving hold under the settle keeps it alive.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), recoiling from taking a hit.",
                    Critical,
                ),
                (
                    "A take built on contrast: start in a content/neutral pose, compress DOWN into the hit for a beat, then snap UP and back in a sharp 1-2 frame recoil - the down-then-up pattern.",
                    Important,
                ),
                (
                    "The recoil is a backward C-curve away from the hit; the body overshoots past a believable position before springing back; a single smear or doubled silhouette on the accent frame sells the violence.",
                    Important,
                ),
                (
                    "The squash is restrained and volume-preserved - widen what flattens, never shrink or grow Bit; the chunky rigid body is not gummy.",
                    Important,
                ),
                (
                    "The antenna whips back hardest of all on the accent (a one-frame crack to an extreme lean) and settles last; the screen flashes an alarm glyph (wide/dilated eyes) on impact and switches crisply back toward neutral/dazed on the held settle frame - never a morph.",
                    Important,
                ),
                (
                    "The settle springs back with a small decaying oscillation and a moving hold underneath (no dead freeze); a strong silhouette at 32px through the recoil.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a recoil without the down-compress beat - a flat snap from neutral",
                "scale change between cells - Bit does not shrink or grow when squashed",
                "gummy or rubber-hose deformation on the rigid body",
                "a morphing or crossfading screen expression - it switches crisply",
                "a hard frozen settle - it oscillates and decays under a moving hold",
                "extra limbs, duplicate character, background",
                "motion blur as a soft smear across cells (a single crisp streak frame only), painterly gradients",
                "mouth, anti-aliasing",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Hurt plays once across 8 frames at 12 fps as a take built on contrast: a content pose, a down-compress into the hit, a 1-2 frame snap recoil (backward C), an overshoot, a decaying settle under a moving hold. The screen switches content to alarm to neutral on key frames, never morphs.",
            ),
            scale_anchor: (Strong, "The compress and the recoil overshoot must read at 32px without scale drift."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
    ];

    for spec in specs {
        let entry = id(handles, spec.handle)?;
        update(doc, entry, delta(spec.description, "", "", &["animation", "bit", spec.handle]))?;
        let body = AnimationDetails {
            purpose: spec.purpose.to_owned(),
            loop_behavior: spec.loop_behavior,
            recommended_frame_count: spec.frames,
            fps: spec.fps,
            pose_beats: spec
                .beats
                .iter()
                .map(|(label, description)| PoseBeat {
                    label: (*label).to_owned(),
                    description: (*description).to_owned(),
                })
                .collect(),
            character_compatibility: vec![CodexHandle::new("bit")?],
        };
        let mut details = SetAnimationDetails::new(entry, body);
        details.apply(doc)?;
        let (anim_strength, anim_statement) = spec.animation_anchor;
        anchor(doc, entry, AnchorKind::Animation, anim_strength, anim_statement)?;
        let (scale_strength, scale_statement) = spec.scale_anchor;
        anchor(doc, entry, AnchorKind::Scale, scale_strength, scale_statement)?;
        let (style_strength, style_statement) = spec.style_anchor;
        anchor(doc, entry, AnchorKind::Style, style_strength, style_statement)?;
        let frags: Vec<PromptFragment> = spec.fragments.iter().map(|(text, priority)| frag(text, *priority)).collect();
        fragments(doc, entry, frags)?;
        negatives(doc, entry, spec.negatives)?;
        status(doc, entry, EntryStatus::Canonical)?;
    }
    Ok(())
}

/// One project rule: handle, constraint field value, and its single anchor.
struct RuleSpec {
    handle: &'static str,
    description: &'static str,
    constraint: &'static str,
    anchor_kind: AnchorKind,
    anchor_statement: &'static str,
    /// An optional positive fragment (only a couple of rules carry one).
    fragment: Option<&'static str>,
}

/// Fills in the project rules: the five original constraints plus the review-derived
/// rules adapted from sprite- and tile-review criteria. Each is Canonical, carries a
/// one-field generic body and a single Locked anchor, and a couple add a prompt
/// fragment. One long, flat table on purpose - the rules are data with no branching.
#[allow(clippy::too_many_lines)]
fn detail_rules(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    let specs = [
        RuleSpec {
            handle: "readable_at_32px",
            description: "Every sprite's silhouette must read clearly at 32px.",
            constraint: "silhouette must read at 32px",
            anchor_kind: AnchorKind::Scale,
            anchor_statement: "Every sprite must hold a clear, recognizable silhouette at 32px.",
            fragment: None,
        },
        RuleSpec {
            handle: "transparent_background",
            description: "Sprites are generated and exported on a transparent background.",
            constraint: "transparent background for all sprites",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "No baked-in background behind a sprite; the background stays transparent.",
            fragment: None,
        },
        RuleSpec {
            handle: "unified_8bit_palette",
            description: "The whole world shares one crisp 8-bit palette - the Bit Default Palette.",
            constraint: "all assets use the Bit Default 8-bit palette",
            anchor_kind: AnchorKind::Palette,
            anchor_statement: "Every asset uses the Bit Default 6-colour 8-bit palette; no off-palette colour.",
            fragment: Some("unified crisp 8-bit palette across the whole world, @palette.bit_default"),
        },
        RuleSpec {
            handle: "no_extra_limbs",
            description: "Bit has exactly two arms, two legs, one antenna, and no mouth.",
            constraint: "no extra limbs, no mouth on Bit",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "Bit never has extra limbs, a mouth, or sharp teeth.",
            fragment: None,
        },
        RuleSpec {
            handle: "no_grimdark",
            description: "The Bit world stays friendly and optimistic - no dark, gritty, or grimdark tone.",
            constraint: "friendly, optimistic; never grimdark",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "No grimdark, horror, or dystopian tone anywhere in the Bit world.",
            fragment: None,
        },
        RuleSpec {
            handle: "single_subject",
            description: "Exactly one Bit per frame - no twin, clone, mirror, ghost, or second figure.",
            constraint: "one subject per frame; no duplicates",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "Never more than one character in a cell - no twin, clone, mirror, reflection, ghost echo, or stray second figure.",
            fragment: None,
        },
        RuleSpec {
            handle: "identity_lock",
            description: "Bit stays on-model across every frame and every asset - same head, screen, antenna, palette, and proportions; nothing appears or disappears.",
            constraint: "on-model identity across all frames; no flicker",
            anchor_kind: AnchorKind::Identity,
            anchor_statement: "Every frame and every asset is unmistakably the same Bit: same head shape, same pixel-face screen, same single antenna, same palette and proportions; no part appears or disappears between frames.",
            fragment: None,
        },
        RuleSpec {
            handle: "spatial_stability",
            description: "Same scale, same baseline, same horizontal centre - locomotion animates in place, not sliding across the frame.",
            constraint: "consistent scale and baseline; animate in place",
            anchor_kind: AnchorKind::Scale,
            anchor_statement: "Keep one scale, one baseline, and one horizontal centre across a sheet; ground motion happens in place (on an invisible treadmill), with the airborne frames the only exception.",
            fragment: None,
        },
        RuleSpec {
            handle: "clean_silhouette",
            description: "Read by value, not by outline - keep a clean, closed silhouette at 32px with limbs clear of the body.",
            constraint: "clean readable silhouette at 32px; read by value",
            anchor_kind: AnchorKind::Visual,
            anchor_statement: "Interior form reads by value, not by interior line; the outer silhouette stays clean and closed and reads at 32px, with overlapping limbs separated by a near/far value split and a dark edge.",
            fragment: None,
        },
        RuleSpec {
            handle: "clean_key",
            description: "Crisp keyed silhouette - no halo, fringe, or stray pixels, and transparent gaps inside the silhouette too.",
            constraint: "clean transparent key; no halo or fringe",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "Everything that is not the subject is fully transparent, including the gaps inside the silhouette; no key-colour halo, no colour fringe, no stray pixels, no drawn ground line.",
            fragment: None,
        },
        RuleSpec {
            handle: "even_lighting",
            description: "Even, flat lighting everywhere - no directional cast shadow, rim light, spotlight, or vignette.",
            constraint: "even flat lighting; no directional shadow",
            anchor_kind: AnchorKind::Style,
            anchor_statement: "Light every asset evenly and flatly with at most a soft CRT bloom; no directional cast shadow, no rim light, no centre spotlight, no vignette.",
            fragment: None,
        },
        RuleSpec {
            handle: "flat_side_view",
            description: "Flat 2D side or front view - not 3D, isometric, or top-down (except tiles, which are top-down); no perspective.",
            constraint: "flat 2D view; no perspective or isometric",
            anchor_kind: AnchorKind::Style,
            anchor_statement: "Render flat in 2D - side or front view for characters and props, top-down for floor tiles; never 3D, isometric, or perspective.",
            fragment: None,
        },
        RuleSpec {
            handle: "no_text_or_ui",
            description: "No text, labels, frame numbers, health bars, watermarks, or signatures in the art.",
            constraint: "no text, ui, or watermark contamination",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "No text, labels, frame numbers, health bars, UI chrome, watermarks, or signatures anywhere in the generated art.",
            fragment: None,
        },
        RuleSpec {
            handle: "tile_seamless",
            description: "Tiles read as one continuous surface - top matches bottom, left matches right, even density, no grid line or hero blob.",
            constraint: "seamless tiling; even density; no hero feature",
            anchor_kind: AnchorKind::Negative,
            anchor_statement: "A tile body reads as one continuous surface: its top edge matches its bottom and its left matches its right, detail density is uniform everywhere, and there is no visible grid line and no repeating hero feature the eye snaps to.",
            fragment: Some("seamless tiling on all four edges, uniform detail density, no visible grid line or hero feature, @material.circuit_tiles"),
        },
        RuleSpec {
            handle: "single_gait",
            description: "A whole locomotion sheet is one cycle at one energy - the bottom row continues the top, not a second, calmer animation.",
            constraint: "one continuous gait across the sheet",
            anchor_kind: AnchorKind::Animation,
            anchor_statement: "A locomotion sheet is a single continuous cycle at one energy level; the bottom row continues the top row's gait - never split into two different animations.",
            fragment: None,
        },
    ];

    for spec in specs {
        let entry = id(handles, spec.handle)?;
        update(doc, entry, delta(spec.description, "", "", &["rule", "constraint"]))?;
        generic(doc, entry, &[("constraint", spec.constraint)])?;
        anchor(doc, entry, spec.anchor_kind, AnchorStrength::Locked, spec.anchor_statement)?;
        if let Some(text) = spec.fragment {
            fragments(doc, entry, vec![frag(text, InclusionPriority::Important)])?;
        }
        status(doc, entry, EntryStatus::Canonical)?;
    }
    Ok(())
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
