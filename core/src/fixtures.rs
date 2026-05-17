//! Reusable sample [`Project`] populated across every public type.
//!
//! Exists so round-trip tests in this crate, future file-format tests
//! in `io`, IPC harnesses in `app`, and the TypeScript-side parse
//! tests in `ui/` all encode the same shape. When you add a new type
//! to the data model, extend [`sample_project`] to exercise it — that
//! single edit propagates coverage to every consumer.
//!
//! The fixture is deterministic. The same `cargo test` run produces
//! byte-identical `MessagePack` and JSON output, which the file-format
//! and IPC streams rely on for snapshot-based assertions.

use std::collections::BTreeMap;

use crate::project::{
    ActiveTarget, AiMetadata, Animation, AnimationId, AssetInfo, BlendMode, BrushShape, BrushState,
    CanvasState, Cel, CelData, ColorMode, Entity, EntityContent, EntityDefaults, EntityGroup,
    EntityId, EntityKind, FeatureFlags, Frame, FrameIndex, FrameRange, FrameTag, GroupId, IVec2,
    Layer, LayerId, LayerKind, Library, LoopDirection, NamedSprite, NineSlice, Palette,
    PaletteEntry, PaletteId, Pivot, PixelBufferId, Project, ProjectAi, ProjectMetadata, Rect,
    ReferenceImage, ReferenceSheet, Rgba, SchemaVersion, SelectionRegion, SelectionState,
    SheetComposition, SheetVariant, SheetVariantId, Size, Slice, SliceId, SliceKey, Sprite,
    SpriteId, StateId, TagDefinition, TagId, TileCell, TileFlags, TileIndex, TilemapData,
    TilemapLayer, TilemapScene, Tileset, TilesetId, TilesetReference, TilesetSource, UserData,
};

/// Builds a fully-populated sample project that touches every type in
/// the data model. Stable across runs — see module docs.
///
/// The body is intentionally one long literal: a reader can audit the
/// entire data model in one scroll. Splitting into helpers would hide
/// the cross-references between layers, cels, frame tags, and slices
/// that this fixture exists to exercise.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn sample_project() -> Project {
    let raster_layer = Layer {
        id: LayerId::new(1),
        name: "background".into(),
        kind: LayerKind::Raster,
        blend_mode: BlendMode::Normal,
        opacity: 255,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData::default(),
    };

    let group_layer = Layer {
        id: LayerId::new(2),
        name: "fx".into(),
        kind: LayerKind::Group { collapsed: false },
        blend_mode: BlendMode::Normal,
        opacity: 200,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData {
            text: Some("non-exported FX".into()),
            color: Some(Rgba::opaque(255, 200, 0)),
        },
    };

    let tilemap_layer = Layer {
        id: LayerId::new(3),
        name: "ground".into(),
        kind: LayerKind::Tilemap {
            tileset: TilesetId::new(1),
        },
        blend_mode: BlendMode::Multiply,
        opacity: 255,
        visible: true,
        locked: false,
        parent: Some(LayerId::new(2)),
        user_data: UserData::default(),
    };

    let reference_layer = Layer {
        id: LayerId::new(4),
        name: "concept".into(),
        kind: LayerKind::Reference {
            image: PixelBufferId::new(1000),
            origin: IVec2::new(-8, -8),
        },
        blend_mode: BlendMode::Normal,
        opacity: 128,
        visible: true,
        locked: true,
        parent: None,
        user_data: UserData::default(),
    };

    let frames = vec![
        Frame {
            duration_ms: 100,
            user_data: UserData::default(),
        },
        Frame::default(),
        Frame {
            duration_ms: 250,
            user_data: UserData {
                text: Some("hold".into()),
                color: None,
            },
        },
    ];

    let mut tilemap_cell_data = TilemapData::empty(2, 2);
    tilemap_cell_data.cells[0] = TileCell {
        index: TileIndex::new(1),
        flags: TileFlags::empty(),
    };
    tilemap_cell_data.cells[3] = TileCell {
        index: TileIndex::new(2),
        flags: TileFlags::FLIP_X.union(TileFlags::FLIP_DIAGONAL),
    };

    let cels = vec![
        Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBufferId::new(10),
            Size::new(32, 32),
        ),
        Cel {
            layer_id: LayerId::new(1),
            frame_index: FrameIndex::new(1),
            position: IVec2::zero(),
            opacity: 255,
            data: CelData::Linked {
                source_frame: FrameIndex::new(0),
            },
            user_data: UserData::default(),
        },
        Cel {
            layer_id: LayerId::new(3),
            frame_index: FrameIndex::new(0),
            position: IVec2::new(0, 0),
            opacity: 255,
            data: CelData::Tilemap {
                data: tilemap_cell_data,
            },
            user_data: UserData::default(),
        },
    ];

    let palette = Palette {
        id: PaletteId::new(1),
        name: "default".into(),
        colors: vec![
            PaletteEntry::new(Rgba::transparent()),
            PaletteEntry {
                color: Rgba::opaque(20, 20, 30),
                name: Some("shadow".into()),
            },
            PaletteEntry {
                color: Rgba::opaque(220, 220, 230),
                name: Some("highlight".into()),
            },
        ],
        user_data: UserData::default(),
    };

    let tileset = Tileset {
        id: TilesetId::new(1),
        name: "dungeon".into(),
        tile_size: Size::new(8, 8),
        tile_count: 16,
        base_index: 1,
        source: TilesetSource::Inline {
            buffer: PixelBufferId::new(2000),
        },
        properties: Vec::new(),
        autotile: None,
        user_data: UserData::default(),
    };

    let frame_tags = vec![FrameTag {
        name: "idle".into(),
        range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(2)),
        loop_direction: LoopDirection::PingPong,
        repeat: 0,
        user_data: UserData::default(),
    }];

    let animations = vec![Animation {
        id: AnimationId::new(1),
        name: "Idle".into(),
        range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(2)),
        loop_direction: LoopDirection::Forward,
        speed_multiplier: 0.75,
        user_data: UserData::default(),
    }];

    let slices = vec![Slice {
        id: SliceId::new(1),
        name: "ui_button".into(),
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::from_xywh(0, 0, 32, 32),
            nine_slice: Some(NineSlice {
                center: Rect {
                    origin: IVec2::new(4, 4),
                    size: Size::new(24, 24),
                },
            }),
            pivot: Some(Pivot {
                offset: IVec2::new(16, 16),
            }),
        }],
        user_data: UserData::default(),
    }];

    let sprite = Sprite {
        id: SpriteId::new(1),
        name: "hero".into(),
        canvas: Size::new(32, 32),
        color_mode: ColorMode::Indexed,
        transparent_color_index: Some(0),
        layers: vec![raster_layer, group_layer, tilemap_layer, reference_layer],
        frames,
        cels,
        palettes: vec![palette],
        palette_frame_overrides: Vec::new(),
        tilesets: vec![tileset],
        frame_tags,
        animations,
        slices,
        user_data: UserData::default(),
    };

    let library = sample_library(sprite);

    Project {
        schema_version: SchemaVersion::current(),
        feature_flags: FeatureFlags::TILEMAPS
            .union(FeatureFlags::REFERENCES)
            .union(FeatureFlags::ANIMATIONS)
            .union(FeatureFlags::SLICES),
        metadata: ProjectMetadata {
            name: "Sample Project".into(),
            description: Some("Exercises every public type in the data model.".into()),
            author: Some("pixhaus".into()),
            // Fixed timestamps keep the fixture deterministic.
            created_at: 1_700_000_000,
            updated_at: 1_700_001_000,
            editor_version: env!("CARGO_PKG_VERSION").into(),
        },
        library,
        canvas: CanvasState {
            active_layer: Some(LayerId::new(1)),
            active_frame: Some(FrameIndex::new(0)),
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom: 4.0,
            onion_skin: true,
            show_tile_grid: false,
        },
        brush: BrushState {
            shape: BrushShape::Circle,
            size: 3,
            foreground_index: 2,
            background_index: 0,
            active_palette: Some(PaletteId::new(1)),
        },
        selection: SelectionState {
            region: Some(SelectionRegion::Rect {
                bounds: Rect::from_xywh(4, 4, 8, 8),
            }),
            anchor_layer: Some(LayerId::new(1)),
        },
        active: ActiveTarget::State {
            entity_id: EntityId::new(1),
            state_id: StateId::new(1),
        },
    }
}

/// Builds the [`Library`] half of [`sample_project`]. Splits out so the
/// long literal in the parent stays scannable while still touching every
/// B9 type. Consumes the hero sprite so the only copy lives inside the
/// library — `sample_project` no longer needs an external `sprites: Vec`
/// after the B9.1 cleanup.
#[allow(clippy::too_many_lines)]
fn sample_library(hero_sprite: Sprite) -> Library {
    let hero_state = NamedSprite {
        id: StateId::new(1),
        state_name: "idle".into(),
        sprite: hero_sprite,
        engine_tags: vec!["loopable".into()],
    };

    let hero = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Character".into()),
        name: "Hero".into(),
        group_id: Some(GroupId::new(1)),
        tags: vec![TagId::new(1)],
        defaults: EntityDefaults {
            canvas_size: Some(Size::new(32, 32)),
            color_mode: Some(ColorMode::Indexed),
            default_palette_id: Some(PaletteId::new(1)),
            default_pivot: None,
            default_fps: Some(12),
        },
        content: EntityContent::Sprites {
            states: vec![hero_state],
            reference_sheet: Some(Box::new(ReferenceSheet {
                canonical: SheetVariant {
                    id: SheetVariantId::new(3),
                    generated_at: 1_700_000_020,
                    image: ReferenceImage {
                        bytes: vec![0x89, 0x50, 0x4E, 0x47],
                        mime: "image/png".into(),
                    },
                    composition: SheetComposition::default(),
                    generation: None,
                    extracted_palette: Vec::new(),
                },
                history: Vec::new(),
                prompts: Vec::new(),
                info: AssetInfo::default(),
            })),
        },
        ai: AiMetadata {
            suggested_tags: Vec::new(),
            vlm_summary: Some("A small armoured hero in front-facing pose.".into()),
            // Populated so downstream snapshots cover the embedding
            // serde path; round-trip tests assert exact bit equality
            // via `to_bits` to catch subtle f32 drift.
            embedding: Some(vec![0.1, 0.2, 0.3]),
            lora_path: None,
        },
        user_data: UserData::default(),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_500,
    };

    let dungeon_tileset = Tileset {
        id: TilesetId::new(2),
        name: "dungeon-shared".into(),
        tile_size: Size::new(8, 8),
        tile_count: 16,
        base_index: 1,
        source: TilesetSource::Inline {
            buffer: PixelBufferId::new(2100),
        },
        properties: Vec::new(),
        autotile: None,
        user_data: UserData::default(),
    };

    let tileset_entity = Entity {
        id: EntityId::new(2),
        kind: EntityKind::Tileset,
        name: "Dungeon".into(),
        group_id: Some(GroupId::new(2)),
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Tileset {
            tileset: dungeon_tileset,
        },
        ai: AiMetadata::default(),
        user_data: UserData::default(),
        created_at: 1_700_000_010,
        updated_at: 1_700_000_010,
    };

    // Tilemap entity references the tileset entity above by id so the
    // fixture exercises cross-entity references and the Tiled-style
    // first_gid model that TilemapScene preserves.
    let mut tilemap_data = TilemapData::empty(4, 4);
    tilemap_data.cells[0] = TileCell {
        index: TileIndex::new(1),
        flags: TileFlags::empty(),
    };
    tilemap_data.cells[5] = TileCell {
        index: TileIndex::new(2),
        flags: TileFlags::FLIP_X,
    };
    let tilemap_entity = Entity {
        id: EntityId::new(4),
        kind: EntityKind::Tilemap,
        name: "Dungeon-1".into(),
        group_id: Some(GroupId::new(2)),
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Tilemap {
            scene: TilemapScene {
                size: Size::new(4, 4),
                tilesets: vec![TilesetReference {
                    tileset_entity_id: EntityId::new(2),
                    first_gid: TileIndex::new(1),
                }],
                layers: vec![TilemapLayer {
                    id: LayerId::new(100),
                    name: "ground".into(),
                    data: tilemap_data,
                    opacity: 255,
                    visible: true,
                }],
                properties: BTreeMap::new(),
            },
        },
        ai: AiMetadata::default(),
        user_data: UserData::default(),
        created_at: 1_700_000_030,
        updated_at: 1_700_000_030,
    };

    Library {
        entities: vec![hero, tileset_entity, tilemap_entity],
        groups: vec![
            EntityGroup {
                id: GroupId::new(1),
                name: "Characters".into(),
                parent_id: None,
                user_data: UserData::default(),
            },
            EntityGroup {
                id: GroupId::new(2),
                name: "Tilesets".into(),
                parent_id: None,
                user_data: UserData::default(),
            },
        ],
        palettes: Vec::new(),
        tags: vec![TagDefinition {
            id: TagId::new(1),
            name: "playable".into(),
            color: Some(Rgba::opaque(40, 200, 80)),
            auto_generated: false,
        }],
        ai: ProjectAi::default(),
    }
}
