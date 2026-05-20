//! Headless end-to-end driver for the reference-sheet anchor flow.
//!
//! Runs the same chain the GUI does, without a window:
//!   1. create a project and a Custom sprite entity (one state),
//!   2. obtain an anchor image — a real `OpenAI` `gpt-image` generation when a
//!      key is available, otherwise a synthetic placeholder so the rest of the
//!      chain still runs and proves out,
//!   3. attach the image as a draft reference-sheet variant,
//!   4. approve it as the canonical anchor (the real `core` approval flow,
//!      including palette extraction),
//!   5. export the canonical anchor PNG to a temp file.
//!
//! Run it:
//! ```text
//! # full live run (reads the key from the OS keychain or OPENAI_API_KEY):
//! cargo run -p pixhaus-ai --example anchor_flow
//! # force the synthetic (offline) path even if a key is present:
//! PIXHAUS_ANCHOR_SYNTHETIC=1 cargo run -p pixhaus-ai --example anchor_flow
//! ```
//! On macOS the keychain prompts for access the first time — click Allow.

// Examples are a separate crate, so lib.rs's `#![cfg_attr(test, allow(...))]`
// doesn't reach them; lift the same exemptions plus stdout/stderr printing.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::missing_panics_doc
)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pixhaus_ai::backends::openai::OpenAiBackend;
use pixhaus_ai::backends::{
    ImageGenRequest, ImageQuality, InferenceBackend, InferenceRequest, InferenceResponse,
};
use pixhaus_ai::plugin::progress::VerbProgress;
use pixhaus_core::color::extraction::ExtractionOptions;
use pixhaus_core::project::{
    AiMetadata, AssetInfo, Entity, EntityContent, EntityDefaults, EntityId, EntityKind,
    NamedSprite, Project, ReferenceImage, ReferenceSheet, SheetVariant, SheetVariantId, Size,
    Sprite, SpriteId, StateId, UserData, approve_sheet_variant,
};
use tokio_util::sync::CancellationToken;

const PROMPT: &str = "a single pixel-art knight character sprite, front view, \
     plain flat background, crisp pixels, 32x32 sprite scaled up";

#[tokio::main]
async fn main() {
    let force_synthetic = std::env::var("PIXHAUS_ANCHOR_SYNTHETIC").is_ok();

    // Step 1: obtain the anchor image bytes.
    let backend = if force_synthetic {
        None
    } else {
        obtain_backend()
    };
    let (bytes, source) = if let Some(backend) = backend {
        println!("[1/4] generating anchor via OpenAI gpt-image (this costs a few cents)...");
        match generate(&backend).await {
            Ok(bytes) => (bytes, "openai gpt-image"),
            Err(err) => {
                eprintln!("OpenAI generation failed: {err}");
                std::process::exit(1);
            }
        }
    } else {
        println!(
            "[1/4] no OpenAI key (or synthetic forced) — using a synthetic placeholder so the\n      \
             create-sprite -> reference-sheet -> approve-as-anchor chain still runs offline."
        );
        (synthetic_png(), "synthetic placeholder")
    };
    println!("      anchor source: {source}, {} bytes", bytes.len());

    // Step 2: create a project with a Custom sprite entity that carries the
    // image as a single draft reference-sheet variant.
    let entity_id = EntityId::new(1);
    let variant_id = SheetVariantId::new(1);
    let project = build_project_with_draft(entity_id, variant_id, bytes);
    println!(
        "[2/4] created sprite entity {} with one draft reference-sheet variant",
        entity_id.get()
    );

    // Step 3: approve the draft as the canonical anchor (real core flow).
    let mut project = project;
    let approval = approve_sheet_variant(
        &mut project,
        entity_id,
        variant_id,
        ExtractionOptions::default(),
    )
    .expect("approval should succeed");
    println!(
        "[3/4] approved variant {} as the canonical anchor; extracted {} palette swatches",
        approval.canonical_id.get(),
        approval.palette_size
    );

    // Step 4: confirm the anchor is set and export it.
    let EntityContent::Sprites {
        reference_sheet: Some(sheet),
        ..
    } = &project.library.entities[0].content
    else {
        panic!("entity lost its reference sheet");
    };
    let canonical = sheet
        .canonical
        .as_ref()
        .expect("a canonical anchor must exist after approval");
    assert_eq!(canonical.id, variant_id, "wrong variant became canonical");

    let out = std::env::temp_dir().join("pixhaus-anchor-demo.png");
    std::fs::write(&out, &canonical.image.bytes).expect("failed to write anchor PNG");
    println!(
        "[4/4] ANCHOR READY ({source}): {}x{}, wrote {} bytes to {}",
        canonical.width,
        canonical.height,
        canonical.image.bytes.len(),
        out.display()
    );
    println!("\nThe full chain ran end to end. Open the PNG above to see the anchor.");
}

/// Builds a backend from `OPENAI_API_KEY` if set, otherwise from the OS
/// keychain (the source the app uses). `None` when no key is available.
fn obtain_backend() -> Option<OpenAiBackend> {
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(OpenAiBackend::new(trimmed));
        }
    }
    OpenAiBackend::from_keychain().ok()
}

/// Runs a real `OpenAI` image generation and returns the first image's bytes.
async fn generate(backend: &OpenAiBackend) -> Result<Vec<u8>, String> {
    let request = ImageGenRequest {
        model: None, // backend default (gpt-image-2)
        prompt: PROMPT.into(),
        negative_prompt: None,
        width: 1024,
        height: 1024,
        steps: None,
        seed: None,
        num_images: 1,
        quality: Some(ImageQuality::Low),
        style_image: None,
        reference_images: Vec::new(),
    };
    let response = tokio::time::timeout(
        Duration::from_secs(180),
        backend.invoke(
            InferenceRequest::ImageGeneration(request),
            VerbProgress::discard(),
            CancellationToken::new(),
        ),
    )
    .await
    .map_err(|_| "request timed out after 180s".to_owned())?
    .map_err(|err| err.to_string())?;

    let InferenceResponse::Image(image) = response else {
        return Err("expected an image response".to_owned());
    };
    image
        .images
        .into_iter()
        .next()
        .filter(|b| !b.is_empty())
        .ok_or_else(|| "OpenAI returned no image bytes".to_owned())
}

/// A tiny offline stand-in for a generated sheet: a 64x64 RGBA PNG with a
/// simple pattern, so the downstream chain runs without a network call.
fn synthetic_png() -> Vec<u8> {
    let mut img = image::RgbaImage::new(64, 64);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let on = ((x / 8) + (y / 8)) % 2 == 0;
        *pixel = if on {
            image::Rgba([60, 120, 220, 255])
        } else {
            image::Rgba([20, 24, 36, 255])
        };
    }
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("encode synthetic png");
    bytes
}

fn build_project_with_draft(
    entity_id: EntityId,
    variant_id: SheetVariantId,
    bytes: Vec<u8>,
) -> Project {
    let mut project = Project::new("anchor-demo");
    let sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(64, 64));
    let variant = SheetVariant::from_image(
        variant_id,
        now_secs(),
        ReferenceImage {
            bytes,
            mime: "image/png".into(),
        },
    );
    project.library.entities.push(Entity {
        id: entity_id,
        kind: EntityKind::Custom("Character".into()),
        name: "Hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![NamedSprite {
                id: StateId::new(1),
                state_name: "idle".into(),
                sprite,
                engine_tags: Vec::new(),
            }],
            reference_sheet: Some(Box::new(ReferenceSheet {
                canonical: None,
                variants: vec![variant],
                prompts: Vec::new(),
                info: AssetInfo::default(),
            })),
        },
        ai: AiMetadata::default(),
        user_data: UserData::default(),
        created_at: now_secs(),
        updated_at: now_secs(),
    });
    project
}

fn now_secs() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
    .unwrap_or(0)
}
