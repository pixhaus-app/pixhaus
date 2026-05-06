//! End-to-end integration tests for the audio-timing verb (S34).
//!
//! Exercises the full verb lifecycle through the public API — no
//! `pub(crate)` access — to confirm the protocol contract holds for the
//! host and any downstream consumer (UI, scripting, CLI).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::missing_panics_doc
)]

use pixhaus_ai::plugin::{
    AUDIO_TIMING_VERB_ID, AudioFormat, AudioTimingInputs, AudioTimingMode, AudioTimingVerb,
    VerbContext, VerbEffect, VerbId, VerbInputs, VerbProgressEvent, VerbRuntime,
};
use pixhaus_core::project::{ProjectMetadata, SpriteId};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn metadata() -> ProjectMetadata {
    ProjectMetadata {
        name: "audio-timing-integration".into(),
        description: None,
        author: None,
        created_at: 0,
        updated_at: 0,
        editor_version: env!("CARGO_PKG_VERSION").into(),
    }
}

fn ctx_with_sprite() -> VerbContext {
    let mut ctx = VerbContext::empty(metadata());
    ctx.active_sprite = Some(SpriteId::new(42));
    ctx
}

/// Builds a minimal mono 16-bit PCM WAV with a single loud onset at
/// the start followed by silence. Large enough to trigger detection.
fn onset_wav(sample_rate: u32, total_samples: u32) -> Vec<u8> {
    let channels: u16 = 1;
    let bits: u16 = 16;
    let bytes_per_sample = u32::from(bits / 8);
    let data_size = total_samples * u32::from(channels) * bytes_per_sample;
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity((file_size + 8) as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align = channels * bits / 8;
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits.to_le_bytes());

    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    for i in 0..total_samples {
        let s: i16 = if i < 20 { i16::MAX } else { 0 };
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

fn beat_inputs(wav: Vec<u8>) -> VerbInputs {
    VerbInputs::from_struct(&AudioTimingInputs {
        audio_bytes: wav,
        format: AudioFormat::Wav,
        mode: AudioTimingMode::Beat,
        fps: 12.0,
        start_frame: 0,
        sensitivity: 0.1,
        tag_name: Some("Beats".into()),
        layer_name: None,
    })
    .expect("inputs serialise")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_beat_lifecycle_invoke_preview_commit() {
    let runtime = VerbRuntime::new();
    runtime.register(AudioTimingVerb::new()).expect("register");

    let descs = runtime.list();
    assert_eq!(descs.len(), 1);
    assert_eq!(descs[0].id.as_str(), AUDIO_TIMING_VERB_ID);

    let wav = onset_wav(44100, 44100);
    let mut inv = runtime
        .invoke(
            &VerbId::new(AUDIO_TIMING_VERB_ID),
            ctx_with_sprite(),
            beat_inputs(wav),
        )
        .expect("invoke");

    let mut events = Vec::new();
    while let Some(ev) = inv.next_progress().await {
        events.push(ev);
    }
    assert!(
        matches!(events.first(), Some(VerbProgressEvent::Started { .. })),
        "first event should be Started"
    );

    let preview = inv.finish().await.expect("preview");
    assert_eq!(preview.verb.as_str(), AUDIO_TIMING_VERB_ID);
    assert!(
        !preview.output.effects.is_empty(),
        "expected at least one effect"
    );
    assert!(matches!(
        preview.output.effects.first(),
        Some(VerbEffect::AddFrames { .. })
    ));

    // Commit returns a VerbCommit wrapping the effects.
    let commit = runtime.commit(preview.clone());
    assert_eq!(commit.preview, preview.id);
    assert!(!commit.effects.is_empty());
}

#[tokio::test]
async fn beat_mode_adds_frame_tag() {
    let runtime = VerbRuntime::new();
    runtime.register(AudioTimingVerb::new()).unwrap();

    let wav = onset_wav(44100, 44100);
    let inv = runtime
        .invoke(
            &VerbId::new(AUDIO_TIMING_VERB_ID),
            ctx_with_sprite(),
            beat_inputs(wav),
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();

    let has_tag = preview
        .output
        .effects
        .iter()
        .any(|e| matches!(e, VerbEffect::AddTag { tag, .. } if tag.name == "Beats"));
    assert!(has_tag, "expected AddTag with name 'Beats'");
}

#[tokio::test]
async fn lipsync_mode_adds_mouth_layer_and_cels() {
    let runtime = VerbRuntime::new();
    runtime.register(AudioTimingVerb::new()).unwrap();

    let wav = onset_wav(44100, 44100);
    let inputs = VerbInputs::from_struct(&AudioTimingInputs {
        audio_bytes: wav,
        format: AudioFormat::Wav,
        mode: AudioTimingMode::LipSync,
        fps: 12.0,
        start_frame: 0,
        sensitivity: 0.1,
        tag_name: Some("Lip sync".into()),
        layer_name: Some("Mouth".into()),
    })
    .unwrap();

    let inv = runtime
        .invoke(
            &VerbId::new(AUDIO_TIMING_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();

    let mouth = preview
        .output
        .effects
        .iter()
        .find(|e| matches!(e, VerbEffect::AddLayer { layer, .. } if layer.name == "Mouth"));
    assert!(mouth.is_some(), "expected AddLayer for 'Mouth'");

    if let Some(VerbEffect::AddLayer {
        cels,
        pixel_buffers,
        ..
    }) = mouth
    {
        assert!(!cels.is_empty(), "mouth layer should have cels");
        assert_eq!(cels.len(), pixel_buffers.len(), "cel/buffer count mismatch");

        // Each cel should carry mouth_state text.
        for cel in cels {
            let state = cel.user_data.text.as_deref().unwrap_or("");
            assert!(
                state == "open" || state == "closed",
                "unexpected mouth_state '{state}'"
            );
        }
    }
}

#[tokio::test]
async fn discard_round_trips() {
    let runtime = VerbRuntime::new();
    runtime.register(AudioTimingVerb::new()).unwrap();

    let wav = onset_wav(44100, 44100);
    let inv = runtime
        .invoke(
            &VerbId::new(AUDIO_TIMING_VERB_ID),
            ctx_with_sprite(),
            beat_inputs(wav),
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();
    let discard = runtime.discard(preview.clone(), Some("user rejected".into()));
    assert_eq!(discard.preview, preview.id);
    assert_eq!(discard.reason.as_deref(), Some("user rejected"));
}

#[tokio::test]
async fn missing_active_sprite_returns_context_error() {
    use pixhaus_ai::plugin::VerbError;

    let runtime = VerbRuntime::new();
    runtime.register(AudioTimingVerb::new()).unwrap();

    let wav = onset_wav(44100, 100);
    let inputs = VerbInputs::from_struct(&AudioTimingInputs {
        audio_bytes: wav,
        format: AudioFormat::Wav,
        mode: AudioTimingMode::Beat,
        fps: 12.0,
        start_frame: 0,
        sensitivity: 0.5,
        tag_name: None,
        layer_name: None,
    })
    .unwrap();

    let inv = runtime
        .invoke(
            &VerbId::new(AUDIO_TIMING_VERB_ID),
            VerbContext::empty(metadata()),
            inputs,
        )
        .unwrap();
    let res = inv.finish().await;
    assert!(
        matches!(res, Err(VerbError::MissingContext(_))),
        "expected MissingContext, got {res:?}"
    );
}

#[tokio::test]
async fn non_wav_input_returns_schema_error() {
    use pixhaus_ai::plugin::VerbError;

    let runtime = VerbRuntime::new();
    runtime.register(AudioTimingVerb::new()).unwrap();

    // Pass OGG magic bytes with AudioFormat::Unknown so the verb sniffs it.
    let inputs = VerbInputs::from_struct(&AudioTimingInputs {
        audio_bytes: b"OggS\x00\x00\x00\x00".to_vec(),
        format: AudioFormat::Unknown,
        mode: AudioTimingMode::Beat,
        fps: 12.0,
        start_frame: 0,
        sensitivity: 0.5,
        tag_name: None,
        layer_name: None,
    })
    .unwrap();

    let inv = runtime
        .invoke(
            &VerbId::new(AUDIO_TIMING_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let res = inv.finish().await;
    assert!(
        matches!(res, Err(VerbError::Schema(_))),
        "expected Schema error for Ogg input, got {res:?}"
    );
}

#[tokio::test]
#[allow(clippy::float_cmp)]
async fn descriptor_is_discoverable_after_registration() {
    let runtime = VerbRuntime::new();
    runtime.register(AudioTimingVerb::new()).unwrap();

    let desc = runtime
        .descriptor(&VerbId::new(AUDIO_TIMING_VERB_ID))
        .expect("descriptor should be present after registration");

    assert!(desc.required_capabilities.is_empty());
    assert!(desc.cancellable);
    assert!(desc.streaming);
    assert_eq!(desc.cost_estimate.typical_usd_cents, 0.0);
}
