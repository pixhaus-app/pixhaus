//! Backwards-compatibility test for the S54 schema MINOR bump.
//!
//! Confirms that a `.pixhaus` file written before the addition of palette
//! pages and palette animation (schema 4.0) still loads cleanly on the
//! current reader and that the new [`Palette::pages`] / [`Palette::animation`]
//! fields default to empty / `None`.
//!
//! The fixture under `tests/fixtures/legacy_v4_0/palette_no_pages.pixhaus`
//! is generated once via the `regenerate_legacy_v4_0_fixture` test below
//! (gated by `PIXHAUS_REGEN_LEGACY_FIXTURES=1`) and committed to the repo.
//! On every other CI run the regenerator is a no-op so the committed
//! bytes are what the loader exercises.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation
)]

use std::path::Path;

use pixhaus_core::project::{
    ActiveTarget, AiMetadata, Entity, EntityContent, EntityDefaults, EntityId, EntityKind,
    NamedSprite, Palette, PaletteEntry, PaletteId, Project, Rgba, SchemaVersion, Size, Sprite,
    SpriteId, StateId, UserData,
};
use pixhaus_io::pixhaus::{PixhausArchive, decode, encode};

const FIXTURE_PATH: &str = "tests/fixtures/legacy_v4_0/palette_no_pages.pixhaus";
const REGEN_ENV: &str = "PIXHAUS_REGEN_LEGACY_FIXTURES";

/// Constructs the in-memory archive whose serialized bytes form the legacy
/// fixture. Because `Palette::pages` and `Palette::animation` carry
/// `#[serde(skip_serializing_if = ...)]`, an empty / `None` value produces
/// the same wire shape as a pre-S54 writer that never knew those fields
/// existed. Setting `schema_version` to 4.0 finishes the legacy disguise.
fn build_legacy_archive() -> PixhausArchive {
    let mut project = Project::new("legacy-v4-0");
    project.schema_version = SchemaVersion { major: 4, minor: 0 };

    // One sprite carrying one palette with two unnamed entries. Both
    // new palette fields stay at their default (empty pages, no
    // animation) — these will be skipped by the writer.
    let palette = Palette {
        id: PaletteId::new(1),
        name: "legacy".into(),
        colors: vec![
            PaletteEntry::new(Rgba::transparent()),
            PaletteEntry::new(Rgba::opaque(255, 0, 0)),
        ],
        user_data: UserData::default(),
        pages: Vec::new(),
        animation: None,
    };

    let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
    sprite.palettes.push(palette);

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Sprite".into()),
        name: "Hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![NamedSprite {
                id: StateId::new(1),
                state_name: "idle".into(),
                sprite,
                engine_tags: Vec::new(),
            }],
            reference_sheet: None,
        },
        ai: AiMetadata::default(),
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };
    project.library.entities.push(entity);
    project.active = ActiveTarget::State {
        entity_id: EntityId::new(1),
        state_id: StateId::new(1),
    };

    PixhausArchive::new(project)
}

#[test]
fn regenerate_legacy_v4_0_fixture() {
    if std::env::var_os(REGEN_ENV).is_none() {
        // CI path: regenerator is gated off, do nothing.
        return;
    }
    let archive = build_legacy_archive();
    let bytes = encode(&archive).expect("encode legacy archive");
    let path = Path::new(FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture dir");
    }
    std::fs::write(path, bytes).expect("write fixture");
}

#[test]
fn legacy_v4_0_palette_loads_with_empty_pages_and_none_animation() {
    let path = Path::new(FIXTURE_PATH);
    let bytes = std::fs::read(path).expect(
        "legacy v4.0 fixture is missing; \
         regenerate with PIXHAUS_REGEN_LEGACY_FIXTURES=1 cargo nextest run -p pixhaus-io \
         regenerate_legacy_v4_0_fixture",
    );
    let archive = decode(&bytes).expect("legacy v4.0 archive must load cleanly");

    // Schema version is preserved as written: 4.0.
    assert_eq!(archive.project.schema_version.major, 4);
    assert_eq!(archive.project.schema_version.minor, 0);

    // The sprite's palette comes back with default empty / none for the
    // S54 fields — proving the reader fills the missing wire keys via
    // serde defaults rather than reaching for a stale stand-in.
    let (named, _entity_id) = archive
        .project
        .sprites_iter()
        .next()
        .expect("legacy fixture carries one sprite");
    let palette = named
        .sprite
        .palettes
        .first()
        .expect("legacy fixture carries one palette");
    assert!(palette.pages.is_empty(), "pages must default to empty");
    assert!(
        palette.animation.is_none(),
        "animation must default to None"
    );
}
