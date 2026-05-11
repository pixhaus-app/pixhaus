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
use pixhaus_io::aseprite::{decode_from_file, document_to_archive};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("io crate has parent")
        .join("examples")
        .join("aseprite-roundtrip")
        .join(name)
}

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
