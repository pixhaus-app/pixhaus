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

/// Drains the job channel each tick until `done` holds or the budget elapses.
async fn drain_until(host: &mut Host, ctx: &egui::Context, done: impl Fn(&Host) -> bool) {
    for _ in 0..2000 {
        pixhaus_ui::shell::drain_background(host, ctx);
        if done(host) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

#[tokio::test]
// This test exists because Generate was built as one thin vertical slice -
// core model, Command, job, mock provider, render path driven end to end -
// rather than front-end-first against mocks. The payoff is the closed loop
// (prompt -> job -> result -> apply -> canvas -> undo), so the loop is proven
// here first; the slice also lays ~80% of the spine Draw and Animate reuse.
async fn prompt_to_result_to_sprite_to_undo() {
    let mut host = Host::new(&Theme::dark());
    // Register the offline mock provider into the host's provider registry, exactly
    // as the app does at boot.
    pixhaus_mod_providers::register(&mut host.edit.providers);
    let ctx = egui::Context::default();

    // 1. Submit an anchor generation job (spawns a tokio task via the ambient runtime).
    apply_intent(
        &mut host,
        Intent::SubmitAnchorJob {
            prompt: "a small knight".to_owned(),
        },
        &ctx,
    );
    assert!(host.state.session.is_generating(), "submitting a job marks the session generating");

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
    assert!(!host.state.session.is_generating(), "a completed job clears the generating state");

    // 3. Insert the selected result as a new sprite, through a command.
    let revision_before = host.edit.document.revision();
    apply_intent(&mut host, Intent::InsertSelectedResultAsSprite, &ctx);
    assert_eq!(host.edit.document.sprites().len(), 1, "the result became a sprite");
    assert!(
        host.edit.document.revision() > revision_before,
        "applying the command advanced the document revision",
    );
    let composite = pixhaus_core::composite_active(&host.edit.document).expect("the active sprite composites");
    assert_eq!(
        (composite.width(), composite.height()),
        pixhaus_core::DEFAULT_CANVAS_SIZE,
        "the generated sprite is the default canvas size",
    );

    // 4. Undo removes it.
    apply_intent(&mut host, Intent::Undo, &ctx);
    assert_eq!(host.edit.document.sprites().len(), 0, "undo removed the sprite");
}

#[tokio::test]
async fn anchor_then_idle_animation_to_animated_sprite_to_undo() {
    let mut host = Host::new(&Theme::dark());
    pixhaus_mod_providers::register(&mut host.edit.providers);
    let ctx = egui::Context::default();

    // 1. Generate the anchor (first pass).
    apply_intent(
        &mut host,
        Intent::SubmitAnchorJob {
            prompt: "Bit, side view".to_owned(),
        },
        &ctx,
    );
    drain_until(&mut host, &ctx, |h| h.state.session.result_count == 1).await;
    assert_eq!(host.state.session.selected_result, Some(0), "the anchor is selected");
    assert!(host.state.session.selected_is_anchor(), "the first result is a still anchor");

    // 2. Generate the idle animation from the selected anchor (second pass).
    apply_intent(
        &mut host,
        Intent::SubmitIdleAnimationJob {
            prompt: "Bit idle loop".to_owned(),
            from_result: 0,
            cols: 4,
            rows: 2,
            fps: 12,
            clip_name: "idle".to_owned(),
        },
        &ctx,
    );
    drain_until(&mut host, &ctx, |h| h.state.session.result_count == 2).await;

    // 3. Select the animation result; the read-only mirror reports eight frames.
    apply_intent(&mut host, Intent::SelectResult(1), &ctx);
    assert_eq!(host.state.session.result_frame_count(1), Some(8), "the mock idle has eight frames");
    assert!(host.state.session.selected_is_animation(), "the second result is an animation");

    // 4. Insert it as an animated sprite, through a command.
    let revision_before = host.edit.document.revision();
    apply_intent(&mut host, Intent::InsertSelectedAsAnimatedSprite, &ctx);
    assert_eq!(host.edit.document.sprites().len(), 1, "the animation became a sprite");
    assert!(host.edit.document.revision() > revision_before, "the command advanced the document revision");

    let id = host.edit.document.active_sprite().expect("a sprite is active");
    let sprite = host.edit.document.sprite(id).expect("the sprite is present");
    assert_eq!(sprite.frames().len(), 8, "one frame per generated cell");
    assert_eq!(sprite.clips().len(), 1, "the sprite has one clip");
    assert_eq!(sprite.clips()[0].name, "idle", "the clip is named idle");
    assert_eq!(sprite.clips()[0].frame_count(), 8, "the idle clip spans all eight frames");

    // Each frame composites to a buffer of the default canvas size.
    let frame0 = sprite.frames()[0].id;
    let composite = pixhaus_core::composite_frame(&host.edit.document, id, frame0).expect("frame 0 composites");
    assert_eq!(
        (composite.width(), composite.height()),
        pixhaus_core::DEFAULT_CANVAS_SIZE,
        "frames are the default canvas size"
    );

    // 5. Undo removes the animated sprite.
    apply_intent(&mut host, Intent::Undo, &ctx);
    assert_eq!(host.edit.document.sprites().len(), 0, "undo removed the animated sprite");
}
