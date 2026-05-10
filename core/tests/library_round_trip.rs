// Integration tests are their own compilation unit; the workspace
// clippy denies (`unwrap_used`, `expect_used`, `disallowed_methods`,
// `cast_possible_truncation`, `too_many_lines`) don't get the test-mode
// pass that lib.rs sets, so allow them here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::doc_markdown
)]

//! Round-trip coverage for the B9.1 library data model.
//!
//! Every test serialises a Library (or a Project containing one)
//! through `rmp-serde` — the on-disk Pixhaus encoding — and asserts the
//! result deserialises to the same value. The MessagePack path catches
//! `serde` attribute drift that JSON-only round-trips can hide
//! (`skip_serializing_if`, `default`, `tag/content` enums in
//! particular).

use std::collections::BTreeMap;

use pixhaus_core::project::{
    ActiveTarget, AiMetadata, AssetInfo, ColorMode, Entity, EntityContent, EntityDefaults,
    EntityGroup, EntityId, EntityKind, GenerationProvenance, GroupId, IVec2, LayerId, Library,
    NamedSprite, PaletteEntry, PaletteId, Pivot, PixelBufferId, ProjectAi, PromptEntry,
    PromptHistoryEntry, PromptResult, Rect, ReferenceImage, ReferenceSheet, Rgba, SheetComposition,
    SheetPanel, SheetVariant, SheetVariantId, Size, Sprite, SpriteId, StateId, TagDefinition,
    TagId, TileCell, TileFlags, TileIndex, TilemapData, TilemapLayer, TilemapScene, Tileset,
    TilesetId, TilesetReference, TilesetSource, UserData,
};

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    let bytes = rmp_serde::to_vec_named(value).expect("encode");
    rmp_serde::from_slice(&bytes).expect("decode")
}

fn empty_sprite(id: u32, name: &str) -> Sprite {
    Sprite::empty(SpriteId::new(id), name, Size::new(16, 16))
}

#[test]
fn empty_library_round_trips() {
    let l = Library::default();
    assert!(l.is_empty());
    let back: Library = round_trip(&l);
    assert_eq!(l, back);
}

#[test]
fn library_with_one_of_each_kind_round_trips() {
    let custom = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Character".into()),
        name: "Hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![NamedSprite {
                id: StateId::new(1),
                state_name: "idle".into(),
                sprite: empty_sprite(1, "Hero / idle"),
                engine_tags: Vec::new(),
            }],
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
    };

    let tileset = Entity {
        id: EntityId::new(2),
        kind: EntityKind::Tileset,
        name: "Forest".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Tileset {
            tileset: Tileset {
                id: TilesetId::new(10),
                name: "Forest".into(),
                tile_size: Size::new(16, 16),
                tile_count: 32,
                base_index: 1,
                source: TilesetSource::Inline {
                    buffer: PixelBufferId::new(500),
                },
                properties: Vec::new(),
                autotile: None,
                user_data: UserData::default(),
            },
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 1_700_000_001,
        updated_at: 1_700_000_001,
    };

    let mut tilemap_data = TilemapData::empty(2, 2);
    tilemap_data.cells[0] = TileCell {
        index: TileIndex::new(1),
        flags: TileFlags::empty(),
    };
    let tilemap = Entity {
        id: EntityId::new(3),
        kind: EntityKind::Tilemap,
        name: "Forest-1".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Tilemap {
            scene: TilemapScene {
                size: Size::new(16, 16),
                tilesets: vec![TilesetReference {
                    tileset_entity_id: EntityId::new(2),
                    first_gid: TileIndex::new(1),
                }],
                layers: vec![TilemapLayer {
                    id: LayerId::new(1),
                    name: "ground".into(),
                    data: tilemap_data,
                    opacity: 255,
                    visible: true,
                }],
                properties: BTreeMap::from([("music".into(), "forest.ogg".into())]),
            },
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 1_700_000_002,
        updated_at: 1_700_000_002,
    };

    let reference = Entity {
        id: EntityId::new(4),
        kind: EntityKind::Reference,
        name: "Hero sheet".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Reference {
            sheet: ReferenceSheet {
                canonical: SheetVariant {
                    id: SheetVariantId::new(1),
                    generated_at: 1_700_000_010,
                    image: ReferenceImage {
                        bytes: vec![1, 2, 3, 4],
                        mime: "image/png".into(),
                    },
                    composition: SheetComposition::default(),
                    generation: None,
                    extracted_palette: Vec::new(),
                },
                history: Vec::new(),
                prompts: Vec::new(),
                info: AssetInfo::default(),
            },
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 1_700_000_010,
        updated_at: 1_700_000_010,
    };

    let library = Library {
        entities: vec![custom, tileset, tilemap, reference],
        groups: Vec::new(),
        palettes: Vec::new(),
        tags: Vec::new(),
        ai: ProjectAi::default(),
    };

    let back: Library = round_trip(&library);
    assert_eq!(library, back);
}

#[test]
fn custom_entity_with_five_states_preserves_order() {
    let names = ["idle", "walk", "run", "attack", "hurt"];
    let states: Vec<NamedSprite> = names
        .iter()
        .enumerate()
        .map(|(i, n)| NamedSprite {
            id: StateId::new(i as u32 + 1),
            state_name: (*n).into(),
            sprite: empty_sprite(i as u32 + 1, n),
            engine_tags: Vec::new(),
        })
        .collect();

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Character".into()),
        name: "Hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites { states },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let library = Library {
        entities: vec![entity],
        ..Library::default()
    };

    let back: Library = round_trip(&library);
    let EntityContent::Sprites {
        states: back_states,
    } = &back.entities[0].content
    else {
        panic!("expected Sprites content");
    };
    assert_eq!(back_states.len(), 5);
    for (i, n) in names.iter().enumerate() {
        assert_eq!(back_states[i].state_name, *n);
        assert_eq!(back_states[i].id, StateId::new(i as u32 + 1));
    }
}

#[test]
fn anchor_reference_id_round_trips_without_resolution() {
    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Character".into()),
        name: "Hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites { states: Vec::new() },
        ai: AiMetadata::default(),
        // Points at an entity that does not exist in this library on
        // purpose. The data model carries the id; the resolver is
        // someone else's problem.
        anchor_reference_id: Some(EntityId::new(999)),
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let library = Library {
        entities: vec![entity],
        ..Library::default()
    };

    let back: Library = round_trip(&library);
    assert_eq!(
        back.entities[0].anchor_reference_id,
        Some(EntityId::new(999))
    );
}

#[test]
fn reference_entity_with_minimal_sheet_round_trips() {
    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Reference,
        name: "Hero sheet".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Reference {
            sheet: ReferenceSheet {
                canonical: SheetVariant {
                    id: SheetVariantId::new(1),
                    generated_at: 0,
                    image: ReferenceImage {
                        bytes: Vec::new(),
                        mime: "image/png".into(),
                    },
                    composition: SheetComposition::default(),
                    generation: None,
                    extracted_palette: Vec::new(),
                },
                history: Vec::new(),
                prompts: Vec::new(),
                info: AssetInfo::default(),
            },
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let library = Library {
        entities: vec![entity],
        ..Library::default()
    };

    let back: Library = round_trip(&library);
    assert_eq!(library, back);
}

#[test]
fn reference_entity_with_populated_composition_preserves_rects() {
    let composition = SheetComposition {
        views: vec![SheetPanel {
            region: Rect::from_xywh(0, 0, 64, 64),
            label: "front".into(),
        }],
        expressions: vec![SheetPanel {
            region: Rect::from_xywh(64, 0, 32, 32),
            label: "happy".into(),
        }],
        callouts: vec![SheetPanel {
            region: Rect::from_xywh(96, 0, 16, 16),
            label: "scar-over-eye".into(),
        }],
        outfits: vec![SheetPanel {
            region: Rect::from_xywh(0, 64, 32, 64),
            label: "armor-set-A".into(),
        }],
        palette_swatch: Some(Rect::from_xywh(0, 128, 64, 16)),
    };

    let mut info = AssetInfo::default();
    info.fields.insert("name".into(), "Hero".into());
    info.fields.insert("species".into(), "Human".into());
    info.notes.push("Loyal but stubborn.".into());

    let provenance = GenerationProvenance {
        backend: "stability".into(),
        model: "sdxl-1.0".into(),
        prompt: "fantasy hero, full sheet, pixel art".into(),
        seed: Some(42),
        negative_prompt: Some("blurry, low quality".into()),
    };

    let prompt_entry = PromptEntry {
        prompt: "fantasy hero, full sheet, pixel art".into(),
        negative_prompt: Some("blurry, low quality".into()),
        seed: Some(42),
        result: Some(PromptResult::Variant(SheetVariantId::new(1))),
        issued_at: 1_700_000_000,
    };

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Reference,
        name: "Hero sheet".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Reference {
            sheet: ReferenceSheet {
                canonical: SheetVariant {
                    id: SheetVariantId::new(1),
                    generated_at: 1_700_000_000,
                    image: ReferenceImage {
                        bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
                        mime: "image/png".into(),
                    },
                    composition: composition.clone(),
                    generation: Some(provenance.clone()),
                    extracted_palette: vec![PaletteEntry::new(Rgba::opaque(10, 20, 30))],
                },
                history: Vec::new(),
                prompts: vec![prompt_entry],
                info,
            },
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let library = Library {
        entities: vec![entity],
        ..Library::default()
    };

    let back: Library = round_trip(&library);
    let EntityContent::Reference { sheet } = &back.entities[0].content else {
        panic!("expected Reference content");
    };
    assert_eq!(sheet.canonical.composition, composition);
    assert_eq!(sheet.canonical.generation.as_ref(), Some(&provenance));
}

#[test]
fn tilemap_scene_with_three_tilesets_preserves_first_gid() {
    let tilesets = vec![
        TilesetReference {
            tileset_entity_id: EntityId::new(10),
            first_gid: TileIndex::new(1),
        },
        TilesetReference {
            tileset_entity_id: EntityId::new(11),
            first_gid: TileIndex::new(33),
        },
        TilesetReference {
            tileset_entity_id: EntityId::new(12),
            first_gid: TileIndex::new(97),
        },
    ];

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Tilemap,
        name: "Boss-Arena".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Tilemap {
            scene: TilemapScene {
                size: Size::new(64, 32),
                tilesets: tilesets.clone(),
                layers: Vec::new(),
                properties: BTreeMap::new(),
            },
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let library = Library {
        entities: vec![entity],
        ..Library::default()
    };

    let back: Library = round_trip(&library);
    let EntityContent::Tilemap { scene } = &back.entities[0].content else {
        panic!("expected Tilemap content");
    };
    assert_eq!(scene.tilesets, tilesets);
    for (orig, restored) in tilesets.iter().zip(scene.tilesets.iter()) {
        assert_eq!(orig.first_gid, restored.first_gid);
        assert_eq!(orig.tileset_entity_id, restored.tileset_entity_id);
    }
}

#[test]
fn library_with_groups_tags_and_project_ai_round_trips() {
    let library = Library {
        entities: vec![Entity {
            id: EntityId::new(1),
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: Some(GroupId::new(1)),
            tags: vec![TagId::new(1), TagId::new(2)],
            defaults: EntityDefaults {
                canvas_size: Some(Size::new(32, 32)),
                color_mode: Some(ColorMode::Rgba),
                default_palette_id: Some(PaletteId::new(1)),
                default_pivot: Some(Pivot {
                    offset: IVec2::new(16, 16),
                }),
                default_fps: Some(12),
            },
            content: EntityContent::Sprites { states: Vec::new() },
            ai: AiMetadata {
                suggested_tags: vec![TagId::new(3)],
                vlm_summary: Some("hero".into()),
                embedding: Some(vec![0.1, 0.2, 0.3]),
            },
            anchor_reference_id: None,
            user_data: UserData {
                text: Some("playable".into()),
                color: Some(Rgba::opaque(0, 100, 200)),
            },
            created_at: 1,
            updated_at: 2,
        }],
        groups: vec![
            EntityGroup {
                id: GroupId::new(1),
                name: "Characters".into(),
                parent_id: None,
                user_data: UserData::default(),
            },
            EntityGroup {
                id: GroupId::new(2),
                name: "Heroes".into(),
                parent_id: Some(GroupId::new(1)),
                user_data: UserData::default(),
            },
        ],
        palettes: Vec::new(),
        tags: vec![
            TagDefinition {
                id: TagId::new(1),
                name: "playable".into(),
                color: Some(Rgba::opaque(0, 200, 0)),
                auto_generated: false,
            },
            TagDefinition {
                id: TagId::new(2),
                name: "armoured".into(),
                color: None,
                auto_generated: true,
            },
        ],
        ai: ProjectAi {
            style_corpus: vec![EntityId::new(1)],
            project_lora_path: Some("loras/project.safetensors".into()),
            prompt_history: vec![PromptHistoryEntry {
                verb_name: "variant".into(),
                prompt: "more saturated".into(),
                timestamp: 1_700_000_000,
            }],
        },
    };

    let back: Library = round_trip(&library);
    assert_eq!(library, back);
}

#[test]
fn active_target_variants_round_trip() {
    for target in [
        ActiveTarget::None,
        ActiveTarget::State {
            entity_id: EntityId::new(1),
            state_id: StateId::new(2),
        },
        ActiveTarget::Tileset {
            entity_id: EntityId::new(3),
        },
        ActiveTarget::Tilemap {
            entity_id: EntityId::new(4),
        },
        ActiveTarget::Reference {
            entity_id: EntityId::new(5),
        },
    ] {
        let back: ActiveTarget = round_trip(&target);
        assert_eq!(target, back);
    }
}
