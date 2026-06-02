//! Integration test: the Generate loop in miniature, driven through the public API.
//!
//! Build a document, apply a generated asset as a sprite, confirm it composites to
//! the input, undo it away, and confirm the document is empty again. This is exactly
//! the sequence the services `History` and the UI will run.

// Integration tests are a separate crate, so the lib's `cfg_attr(test, ...)` does not
// reach here; allow the test-only conveniences explicitly (matches the repo pattern in
// crates/ui/tests/resolve_layout_snapshot.rs).
#![allow(clippy::disallowed_methods, clippy::unwrap_used, clippy::expect_used)]

use pixhaus_core::{ApplyGeneratedAsset, Command, Document, composite_sprite};

fn checker_4x4() -> Vec<u8> {
    // 4x4 RGBA, alternating opaque magenta / transparent — distinctive bytes.
    let mut bytes = Vec::with_capacity(4 * 4 * 4);
    for i in 0..16 {
        if i % 2 == 0 {
            bytes.extend_from_slice(&[255, 0, 255, 255]);
        } else {
            bytes.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    bytes
}

#[test]
fn apply_generated_asset_round_trips_through_undo() {
    let mut doc = Document::new();
    let source = checker_4x4();
    let mut cmd = ApplyGeneratedAsset::new("bit".to_owned(), 4, 4, 16, source.clone());

    let rev_before = doc.revision();
    cmd.apply(&mut doc).unwrap();

    // The sprite exists, is active, and composites back to the source bytes.
    assert_eq!(doc.sprites().len(), 1);
    let id = doc.active_sprite().expect("a sprite is active after apply");
    assert_eq!(doc.active_sprite_size(), Some((4, 4)));
    assert!(doc.revision() > rev_before);
    let composite = composite_sprite(&doc, id).unwrap();
    assert_eq!(composite.as_bytes(), source.as_slice());

    // Undo removes the sprite and its buffer; the document is empty again.
    cmd.undo(&mut doc).unwrap();
    assert_eq!(doc.sprites().len(), 0);
    assert_eq!(doc.active_sprite(), None);
    assert_eq!(doc.buffers().len(), 0);
}
