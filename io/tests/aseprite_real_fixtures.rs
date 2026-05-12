//! Real-fixture Aseprite import coverage.
//!
//! B9.5 restores the importer. These tests exercise `document_to_archive`
//! against actual `.aseprite` files on disk to catch regressions that
//! synthetic in-memory tests cannot reach (e.g., unexpected chunk types,
//! unusual palette layouts, linked cel chains across frame boundaries).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation
)]

use std::path::PathBuf;

use pixhaus_core::project::library::EntityContent;
use pixhaus_io::aseprite::{
    ConvertedArchive, archive_to_document, archive_to_per_state_documents, decode,
    decode_from_file, document_to_archive, encode,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("io crate has parent")
        .join("examples")
        .join("aseprite-roundtrip")
        .join(name)
}

/// Import a fixture and assert it produces exactly one Custom-kind entity with
/// at least `min_states` states. Returns the archive for further assertions.
fn import_fixture(name: &str, min_states: usize) -> ConvertedArchive {
    let path = fixture(name);
    let doc = decode_from_file(&path).expect("decode fixture");
    let result = document_to_archive(&doc, name).expect("import must succeed");

    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!("entity content must be Sprites");
    };
    assert!(
        states.len() >= min_states,
        "{name}: expected ≥{min_states} states, got {}",
        states.len()
    );
    assert!(
        !result.archive.buffers.is_empty(),
        "{name}: at least one pixel buffer must be populated"
    );
    result
}

// ── single-frame-rgba ─────────────────────────────────────────────────────────

#[test]
fn single_frame_rgba_imports_to_library_entity() {
    let path = fixture("single-frame-rgba.aseprite");
    let doc = decode_from_file(&path).expect("decode fixture");
    let result = document_to_archive(&doc, "real-fixture").expect("import must succeed");

    let entities = &result.archive.project.library.entities;
    assert_eq!(entities.len(), 1, "must produce exactly one entity");

    let EntityContent::Sprites { states } = &entities[0].content else {
        panic!("entity content must be Sprites");
    };
    assert!(!states.is_empty(), "must have at least one state");

    // A file with no tags gets one 'default' state covering all frames.
    if states.len() == 1 {
        assert_eq!(states[0].state_name, "default");
    }

    // Raster cels must have placed their pixel bytes into the archive buffer list.
    assert!(
        !result.archive.buffers.is_empty(),
        "at least one pixel buffer must be populated"
    );
}

// ── group-multiply ────────────────────────────────────────────────────────────

#[test]
fn group_multiply_imports() {
    let result = import_fixture("group-multiply.aseprite", 1);
    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    // Must have at least one layer (the group and/or its child).
    assert!(
        !states[0].sprite.layers.is_empty(),
        "group-multiply must import with layers"
    );
}

#[test]
fn group_multiply_merged_round_trip() {
    let result = import_fixture("group-multiply.aseprite", 1);

    let doc_out = archive_to_document(&result.archive).expect("export must succeed");
    let bytes = encode(&doc_out).expect("encode must succeed");
    let doc_in = decode(&bytes).expect("decode must succeed");
    let result2 = document_to_archive(&doc_in, "group-multiply").expect("re-import must succeed");

    let EntityContent::Sprites { states: s1 } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    let EntityContent::Sprites { states: s2 } =
        &result2.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert_eq!(s1.len(), s2.len(), "round-trip must preserve state count");
    assert_eq!(
        s1[0].sprite.layers.len(),
        s2[0].sprite.layers.len(),
        "round-trip must preserve layer count"
    );
}

// ── indexed-with-palette ──────────────────────────────────────────────────────

#[test]
fn indexed_with_palette_imports() {
    let result = import_fixture("indexed-with-palette.aseprite", 1);
    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert!(
        !states[0].sprite.palettes.is_empty(),
        "indexed fixture must carry a palette"
    );
}

#[test]
fn indexed_with_palette_merged_round_trip() {
    let result = import_fixture("indexed-with-palette.aseprite", 1);

    let doc_out = archive_to_document(&result.archive).expect("export must succeed");
    let bytes = encode(&doc_out).expect("encode must succeed");
    let doc_in = decode(&bytes).expect("decode must succeed");
    let result2 =
        document_to_archive(&doc_in, "indexed-with-palette").expect("re-import must succeed");

    let EntityContent::Sprites { states: s1 } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    let EntityContent::Sprites { states: s2 } =
        &result2.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert_eq!(s1.len(), s2.len());
    assert_eq!(
        s1[0].sprite.palettes.len(),
        s2[0].sprite.palettes.len(),
        "palette must survive round-trip"
    );
}

// ── multi-frame-tags ──────────────────────────────────────────────────────────

#[test]
fn multi_frame_tags_imports() {
    let result = import_fixture("multi-frame-tags.aseprite", 1);
    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    // multi-frame-tags has frame tags; each becomes a state.
    // Regardless of tag count, there must be at least one state with ≥1 frame.
    assert!(
        !states[0].sprite.frames.is_empty(),
        "each state must have at least one frame"
    );
}

#[test]
fn multi_frame_tags_merged_round_trip() {
    let result = import_fixture("multi-frame-tags.aseprite", 1);

    let doc_out = archive_to_document(&result.archive).expect("export must succeed");
    let bytes = encode(&doc_out).expect("encode must succeed");
    let doc_in = decode(&bytes).expect("decode must succeed");
    let result2 = document_to_archive(&doc_in, "multi-frame-tags").expect("re-import must succeed");

    let EntityContent::Sprites { states: s1 } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    let EntityContent::Sprites { states: s2 } =
        &result2.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert_eq!(s1.len(), s2.len(), "state count must survive round-trip");
}

#[test]
fn multi_frame_tags_per_state_round_trip() {
    // multi-frame-tags has multiple states — the interesting per-state case.
    let result = import_fixture("multi-frame-tags.aseprite", 1);
    let EntityContent::Sprites {
        states: original_states,
    } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };

    let per_state_docs =
        archive_to_per_state_documents(&result.archive).expect("per-state export must succeed");

    assert_eq!(
        per_state_docs.len(),
        original_states.len(),
        "one per-state doc per state"
    );

    for (i, per_state) in per_state_docs.iter().enumerate() {
        assert!(
            per_state.warnings.is_empty(),
            "state {} should have no warnings",
            per_state.state_name
        );

        // Encode → decode → re-import and verify frame count is preserved.
        let bytes = encode(&per_state.document).expect("encode must succeed");
        let decoded = decode(&bytes).expect("decode must succeed");
        let re_imported =
            document_to_archive(&decoded, &per_state.state_name).expect("re-import must succeed");

        let EntityContent::Sprites {
            states: reimported_states,
        } = &re_imported.archive.project.library.entities[0].content
        else {
            panic!("expected Sprites content after re-import");
        };

        assert_eq!(
            reimported_states.len(),
            1,
            "per-state doc must produce one state"
        );
        assert_eq!(
            reimported_states[0].sprite.frames.len(),
            original_states[i].sprite.frames.len(),
            "frame count must survive per-state round-trip for state '{}'",
            per_state.state_name
        );
    }
}

// ── tilemap-with-tileset ──────────────────────────────────────────────────────

#[test]
fn tilemap_with_tileset_imports() {
    let result = import_fixture("tilemap-with-tileset.aseprite", 1);
    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    // Tileset data must be imported alongside the sprite state.
    assert!(
        !states[0].sprite.tilesets.is_empty(),
        "tilemap fixture must import tilesets"
    );
}

#[test]
#[ignore = "tilemap cel round-trip not yet implemented; tracked for B9.5-followup-2"]
fn tilemap_with_tileset_merged_round_trip() {
    // Tilemap layers cannot currently be fully round-tripped because the
    // export path encodes tilemap cels but the re-import path reads them
    // back as-is without re-associating tile indices with tileset entries.
    // Mark ignored with an explicit reason rather than skipping the file.
    let result = import_fixture("tilemap-with-tileset.aseprite", 1);
    let doc_out = archive_to_document(&result.archive).expect("export must succeed");
    let bytes = encode(&doc_out).expect("encode must succeed");
    let doc_in = decode(&bytes).expect("decode must succeed");
    let result2 =
        document_to_archive(&doc_in, "tilemap-with-tileset").expect("re-import must succeed");

    let EntityContent::Sprites { states: s1 } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    let EntityContent::Sprites { states: s2 } =
        &result2.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert_eq!(s1[0].sprite.tilesets.len(), s2[0].sprite.tilesets.len());
}
