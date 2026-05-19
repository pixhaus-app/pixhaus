//! Aseprite export downgrades the eight S55 OpenToonz-derived blend
//! modes to `Normal` because the Aseprite format has no slot for them.
//! This test exercises that round-trip and confirms the layer
//! deserialises back as `Normal`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]

use pixhaus_core::project::BlendMode;
use pixhaus_io::aseprite::{
    AsepriteDocument, DocumentFrame,
    chunk::{Chunk, LayerChunk, LayerKindCode},
    decode, encode,
};

fn layer_with_blend(name: &str, blend: BlendMode) -> LayerChunk {
    LayerChunk {
        flags: 0x0001 | 0x0002, // visible | editable
        kind: LayerKindCode::Normal,
        child_level: 0,
        blend,
        unknown_blend_code: None,
        opacity: 255,
        name: name.into(),
        tileset_index: 0,
        uuid: None,
    }
}

fn document_with_layer(blend: BlendMode) -> AsepriteDocument {
    let mut doc = AsepriteDocument::empty(8, 8);
    let mut frame = DocumentFrame::new(100);
    frame
        .chunks
        .push(Chunk::Layer(layer_with_blend("bg", blend)));
    doc.frames.push(frame);
    doc
}

fn first_layer_blend(doc: &AsepriteDocument) -> BlendMode {
    let chunk = doc.frames[0]
        .chunks
        .iter()
        .find(|c| matches!(c, Chunk::Layer(_)))
        .expect("encoded document must carry the layer chunk back");
    match chunk {
        Chunk::Layer(layer) => layer.blend,
        _ => unreachable!("matches! guarded above"),
    }
}

#[test]
fn linear_light_downgrades_to_normal_on_export() {
    let doc = document_with_layer(BlendMode::LinearLight);
    let bytes = encode(&doc).expect("encode succeeds");
    let round = decode(&bytes).expect("decode succeeds");
    assert_eq!(first_layer_blend(&round), BlendMode::Normal);
}

#[test]
fn every_new_blend_mode_downgrades_to_normal() {
    // Per-mode round-trip: every one of the eight S55 modes loses its
    // identity through the Aseprite codec and comes back as Normal.
    for mode in [
        BlendMode::LinearBurn,
        BlendMode::DarkerColor,
        BlendMode::LinearDodge,
        BlendMode::LighterColor,
        BlendMode::VividLight,
        BlendMode::LinearLight,
        BlendMode::PinLight,
        BlendMode::HardMix,
    ] {
        let doc = document_with_layer(mode);
        let bytes = encode(&doc).expect("encode succeeds");
        let round = decode(&bytes).expect("decode succeeds");
        assert_eq!(
            first_layer_blend(&round),
            BlendMode::Normal,
            "downgrade failed for {mode:?}",
        );
    }
}

#[test]
fn native_modes_round_trip_unchanged() {
    // Control: Aseprite-native modes survive the round-trip. This guards
    // against accidentally downgrading a supported mode along with the
    // unsupported ones.
    for mode in [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::ColorBurn,
        BlendMode::Luminosity,
    ] {
        let doc = document_with_layer(mode);
        let bytes = encode(&doc).expect("encode succeeds");
        let round = decode(&bytes).expect("decode succeeds");
        assert_eq!(first_layer_blend(&round), mode, "{mode:?}");
    }
}
