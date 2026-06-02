//! Integration test: the Generate loop through the public `History` surface.

// Separate crate: the lib's `cfg_attr(test, ...)` does not reach here.
#![allow(clippy::disallowed_methods, clippy::unwrap_used, clippy::expect_used)]

use pixhaus_core::{ApplyGeneratedAsset, Document, composite_sprite};
use pixhaus_services::History;

#[test]
fn execute_apply_generated_asset_then_undo() {
    let mut doc = Document::new();
    let mut history = History::new();
    let pixels = vec![0, 128, 255, 255]; // one opaque pixel

    history
        .execute(&mut doc, Box::new(ApplyGeneratedAsset::new("bit".to_owned(), 1, 1, 4, pixels.clone())))
        .unwrap();

    let id = doc.active_sprite().expect("a sprite is active");
    assert_eq!(composite_sprite(&doc, id).unwrap().as_bytes(), pixels.as_slice());
    assert!(history.can_undo());

    history.undo(&mut doc).unwrap();
    assert_eq!(doc.sprites().len(), 0);
    assert!(history.can_redo());
}
