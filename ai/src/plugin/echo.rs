//! Reference verb: [`EchoVerb`].
//!
//! Echo takes a [`PixelData`] payload and returns it unchanged as a
//! new layer on the active sprite. It exercises every part of the
//! protocol — descriptor, validation, context lookup, progress
//! events, cancellation, effect production, cost reporting — without
//! depending on any inference backend. Verb-author documentation in
//! `docs/verb-protocol.md` walks through it line by line.

use std::time::Duration;

use async_trait::async_trait;
use pixhaus_core::project::{Cel, FrameIndex, Layer, LayerId, PixelBufferId, Size};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::context::{PixelData, VerbContext};
use super::descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId};
use super::error::{Result, VerbError};
use super::inputs::VerbInputs;
use super::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use super::progress::{VerbProgress, VerbProgressEvent};
use super::verb::Verb;

/// Stable identifier for the built-in echo verb.
pub const ECHO_VERB_ID: &str = "pixhaus.builtin.echo";

/// Effect-local placeholder ID used for the new layer and its single
/// pixel buffer. The host rewrites both at commit time.
const ECHO_PLACEHOLDER: u32 = 0;

/// Inputs for [`EchoVerb`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EchoInputs {
    /// Pixels to echo back as a new layer.
    pub pixels: PixelData,
    /// Display name for the resulting layer. Defaults to `"Echo"`.
    #[serde(default)]
    pub layer_name: Option<String>,
}

/// Reference plugin: returns its input pixels as a new layer.
///
/// Useful as:
/// - the smallest possible end-to-end exercise of the protocol,
/// - a fixture in tests that need a verb-shaped placeholder,
/// - a worked example for plugin authors.
#[derive(Debug)]
pub struct EchoVerb {
    descriptor: VerbDescriptor,
}

impl EchoVerb {
    /// Constructs a fresh echo verb.
    #[must_use]
    // The `serde_json::json!` macro expands to internal helpers that
    // call `Result::unwrap` on infallible builders. The workspace
    // disallows `unwrap` everywhere, so we exempt the constructor
    // rather than open-code the schema as a `Value::Object`.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pixels": {
                    "type": "object",
                    "properties": {
                        "width": {"type": "integer", "minimum": 1},
                        "height": {"type": "integer", "minimum": 1},
                        "bytes_per_pixel": {"type": "integer"},
                        "stride": {"type": "integer"},
                        "bytes": {"type": "array", "items": {"type": "integer"}}
                    },
                    "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                },
                "layer_name": {"type": ["string", "null"]}
            },
            "required": ["pixels"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(ECHO_VERB_ID),
                display_name: "Echo".into(),
                description: "Returns the supplied pixels unchanged as a new layer".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::empty(),
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddLayer],
                cost_estimate: CostEstimate::free(),
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verb-protocol.md#echo".into()),
            },
        }
    }
}

impl Default for EchoVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for EchoVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: EchoInputs = inputs.deserialize()?;
        if !parsed.pixels.is_well_formed() {
            return Err(VerbError::Schema(
                "echo: pixel buffer dimensions and byte count are inconsistent".into(),
            ));
        }
        Ok(())
    }

    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = std::time::Instant::now();
        // Consume the JSON value: the payload carries pixel bytes that
        // would otherwise be cloned by the borrowed deserialiser.
        let inputs: EchoInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;
        let active_frame = ctx.active_frame.unwrap_or(FrameIndex::new(0));

        progress
            .send(VerbProgressEvent::Started { backend: None })
            .await;
        progress.step(Some(0.5), "preparing layer").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let layer_name = inputs.layer_name.clone().unwrap_or_else(|| "Echo".into());
        let layer = Layer::raster(LayerId::new(ECHO_PLACEHOLDER), &layer_name);
        let buffer_id = PixelBufferId::new(ECHO_PLACEHOLDER);
        let cel = Cel::raster(
            layer.id,
            active_frame,
            buffer_id,
            Size::new(inputs.pixels.width, inputs.pixels.height),
        );

        let pixel_buffer = NewPixelBuffer {
            placeholder: buffer_id,
            pixels: inputs.pixels.clone(),
        };

        progress.step(Some(1.0), "echoed input").await;

        let elapsed = started.elapsed();
        let summary = format!(
            "Add layer \"{layer_name}\" ({}x{})",
            inputs.pixels.width, inputs.pixels.height
        );

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::AddLayer {
                sprite: sprite_id,
                layer,
                cels: vec![cel],
                pixel_buffers: vec![pixel_buffer],
            }],
            thumbnail: Some(inputs.pixels),
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "echo-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn ctx_with_sprite() -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_frame = Some(FrameIndex::new(0));
        ctx
    }

    #[test]
    fn descriptor_advertises_no_capabilities() {
        let verb = EchoVerb::new();
        assert!(verb.descriptor().required_capabilities.is_empty());
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }

    #[test]
    fn validate_rejects_malformed_pixels() {
        let verb = EchoVerb::new();
        let inputs = VerbInputs::from_struct(&EchoInputs {
            pixels: PixelData {
                width: 4,
                height: 4,
                bytes_per_pixel: 4,
                stride: 16,
                // wrong: only 8 bytes for 4×4 RGBA
                bytes: vec![0; 8],
            },
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[tokio::test]
    async fn echo_round_trips_through_runtime() {
        let runtime = VerbRuntime::new();
        runtime.register(EchoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&EchoInputs {
            pixels: PixelData::rgba8(
                2,
                2,
                vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            ),
            layer_name: Some("Echo".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(ECHO_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.verb.as_str(), ECHO_VERB_ID);
        assert_eq!(preview.output.effects.len(), 1);
        match &preview.output.effects[0] {
            VerbEffect::AddLayer {
                layer,
                cels,
                pixel_buffers,
                ..
            } => {
                assert_eq!(layer.name, "Echo");
                assert_eq!(cels.len(), 1);
                assert_eq!(pixel_buffers.len(), 1);
                assert_eq!(pixel_buffers[0].pixels.bytes.len(), 16);
            }
            other => panic!("unexpected effect kind: {other:?}"),
        }
    }

    #[tokio::test]
    async fn echo_requires_active_sprite() {
        let runtime = VerbRuntime::new();
        runtime.register(EchoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&EchoInputs {
            pixels: PixelData::rgba8(1, 1, vec![1, 2, 3, 4]),
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(ECHO_VERB_ID),
                VerbContext::empty(metadata()),
                inputs,
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active sprite"))
        ));
    }
}
