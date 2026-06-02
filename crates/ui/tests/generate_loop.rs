//! End-to-end Generate-loop test, headless (bible 25.2 "Generate is ready when…").
//!
//! Drives the real shell path with no window: submit a prompt, drain the job channel
//! until the mock result lands, insert the selected result as a sprite through a
//! command, confirm it composites and the revision advanced, then undo it away.

#![allow(clippy::disallowed_methods, clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use pixhaus_ui::state::Host;
use pixhaus_ui::state::intent::{Intent, apply_intent};
use pixhaus_ui::theme::Theme;

#[tokio::test]
async fn prompt_to_result_to_sprite_to_undo() {
    let mut host = Host::new(&Theme::dark());
    // Register the offline mock provider into the host's provider registry, exactly
    // as the app does at boot.
    pixhaus_mod_providers::register(&mut host.edit.providers);
    let ctx = egui::Context::default();

    // 1. Submit a generation job (spawns a tokio task via the ambient runtime).
    apply_intent(
        &mut host,
        Intent::SubmitGenerateJob {
            prompt: "a small knight".to_owned(),
        },
        &ctx,
    );

    // 2. Drain the job channel until the result lands (the mock has a short delay).
    for _ in 0..2000 {
        pixhaus_ui::shell::drain_background(&mut host, &ctx);
        if host.state.session.result_count > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(host.state.session.result_count, 1, "the mock result landed");
    assert_eq!(host.state.session.selected_result, Some(0), "the first result is selected");

    // 3. Insert the selected result as a new sprite, through a command.
    let revision_before = host.edit.document.revision();
    apply_intent(&mut host, Intent::InsertSelectedResultAsSprite, &ctx);
    assert_eq!(host.edit.document.sprites().len(), 1, "the result became a sprite");
    assert!(
        host.edit.document.revision() > revision_before,
        "applying the command advanced the document revision",
    );
    let composite = pixhaus_core::composite_active(&host.edit.document).expect("the active sprite composites");
    assert_eq!((composite.width(), composite.height()), (64, 64), "the generated sprite is the requested size");

    // 4. Undo removes it.
    apply_intent(&mut host, Intent::Undo, &ctx);
    assert_eq!(host.edit.document.sprites().len(), 0, "undo removed the sprite");
}
