//! Progress channel between a verb and its caller.
//!
//! A verb that streams results sends [`VerbProgressEvent`]s through a
//! [`VerbProgress`] handle. The runtime pairs the handle with a
//! `tokio::sync::mpsc::Receiver` exposed on
//! [`super::runtime::VerbInvocation`]; the UI consumes the receiver
//! and renders progress in real time.
//!
//! # Backpressure and the discarded sender
//!
//! The channel is bounded so a slow consumer applies backpressure
//! rather than letting a runaway verb fill memory.
//! [`VerbProgress::discard`] returns a sentinel handle whose `send`
//! and `try_send` succeed by dropping the event. Verbs that don't care
//! about the receiver — or callers that don't want progress — share
//! the same trait surface and can be ignored uniformly.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::context::PixelData;

/// Channel capacity used by the runtime when constructing a
/// `(VerbProgress, mpsc::Receiver)` pair.
///
/// 64 events sit comfortably between "starves the producer" and "hides
/// backpressure". Verbs emitting at higher frequency than the consumer
/// drains will block on `send`, which is the correct behaviour — the
/// alternative is silently dropping progress.
pub const PROGRESS_CHANNEL_CAPACITY: usize = 64;

/// Severity of a verb-side log message.
#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    /// Verbose detail; shown only when the user opts in.
    Trace,
    /// Routine information.
    Debug,
    /// Default level — surfaces in the runtime's progress feed.
    #[default]
    Info,
    /// Non-fatal issue the user should see.
    Warning,
    /// Recoverable error — the verb chose to continue but wants to
    /// flag the situation.
    Error,
}

/// Running cost update emitted while the verb is mid-flight.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CostUpdate {
    /// USD cents accumulated so far.
    pub usd_cents: f32,
    /// Tokens consumed so far, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_input: Option<u32>,
    /// Tokens generated so far, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_output: Option<u32>,
}

/// One progress event a verb emits while running.
///
/// Events are descriptive: the receiver renders them however it likes.
/// New variants are additive within a Pixhaus build — every consumer
/// is compiled against the same enum, so a verb that emits a new
/// variant always reaches a runtime that knows the variant. Across
/// the IPC boundary, the wire form is `#[serde(tag = "kind")]`;
/// unknown tags surface as a deserialisation error rather than being
/// silently dropped, so a UI built against an older protocol is
/// expected to upgrade in lockstep with the runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerbProgressEvent {
    /// Verb has begun. Carries the optional backend identifier so the
    /// UI can show "running on Anthropic Sonnet 4.6" before the call
    /// settles.
    Started {
        /// Backend the verb selected, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        backend: Option<String>,
    },
    /// Coarse step in the verb's pipeline.
    Step {
        /// `0.0`–`1.0` completion fraction. `None` for indeterminate
        /// progress.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fraction: Option<f32>,
        /// One-line description of the current step.
        message: String,
    },
    /// Partial pixel output (a streaming verb that wants to surface
    /// in-progress frames). The host may render these for live
    /// preview.
    PartialPixels {
        /// Index into the eventual `VerbOutput::effects` list this
        /// chunk corresponds to.
        effect_index: u32,
        /// Pixel chunk.
        pixels: PixelData,
    },
    /// Running cost update.
    Cost(CostUpdate),
    /// Free-form log message.
    Log {
        /// Severity.
        level: LogLevel,
        /// Message body.
        message: String,
    },
    /// Estimated time remaining.
    Eta {
        /// Estimated remaining duration.
        remaining: Duration,
    },
}

/// Sender half of the progress channel, handed to the verb.
///
/// `VerbProgress` is `Clone`-friendly so a verb can hand copies to
/// sub-tasks (e.g. parallel image generation across N directions for
/// the Extend verb). Each clone shares the same channel.
#[derive(Clone, Debug)]
pub struct VerbProgress {
    inner: ProgressInner,
}

#[derive(Clone, Debug)]
enum ProgressInner {
    /// Real channel sending to a paired `Receiver`.
    Live(mpsc::Sender<VerbProgressEvent>),
    /// Sink — `send`/`try_send` succeed by dropping the event.
    Discard,
}

impl VerbProgress {
    /// Constructs a `(VerbProgress, Receiver)` pair backed by a
    /// bounded channel. Used by the runtime when starting an
    /// invocation; verb authors and tests typically call
    /// [`VerbProgress::discard`] instead.
    #[must_use]
    pub fn channel() -> (Self, mpsc::Receiver<VerbProgressEvent>) {
        let (tx, rx) = mpsc::channel(PROGRESS_CHANNEL_CAPACITY);
        (
            Self {
                inner: ProgressInner::Live(tx),
            },
            rx,
        )
    }

    /// Returns a sink handle that silently drops everything sent
    /// through it. Useful for tests and for callers that do not want
    /// to consume progress.
    #[must_use]
    pub const fn discard() -> Self {
        Self {
            inner: ProgressInner::Discard,
        }
    }

    /// Returns `true` if the handle is the discard sink. Verbs may
    /// branch on this to skip building expensive progress payloads.
    #[must_use]
    pub const fn is_discarded(&self) -> bool {
        matches!(self.inner, ProgressInner::Discard)
    }

    /// Sends an event, awaiting capacity if the channel is full.
    /// Returns `true` if the event was delivered; `false` when the
    /// receiver was dropped or this is a [`Self::discard`] sink (in
    /// which case the event was dropped on the floor by design).
    pub async fn send(&self, event: VerbProgressEvent) -> bool {
        match &self.inner {
            ProgressInner::Live(tx) => tx.send(event).await.is_ok(),
            ProgressInner::Discard => {
                // Intentional drop: the discard sink succeeds silently.
                let _ = event;
                true
            }
        }
    }

    /// Non-blocking send. Returns `Ok(())` on success; `Err` if the
    /// channel is full or closed. Discard sinks always return `Ok`.
    pub fn try_send(
        &self,
        event: VerbProgressEvent,
    ) -> Result<(), mpsc::error::TrySendError<VerbProgressEvent>> {
        match &self.inner {
            ProgressInner::Live(tx) => tx.try_send(event),
            ProgressInner::Discard => {
                let _ = event;
                Ok(())
            }
        }
    }

    /// Convenience wrapper for the very common
    /// [`VerbProgressEvent::Step`] variant.
    pub async fn step(&self, fraction: Option<f32>, message: impl Into<String>) -> bool {
        self.send(VerbProgressEvent::Step {
            fraction,
            message: message.into(),
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn channel_round_trips_events() {
        let (tx, mut rx) = VerbProgress::channel();
        assert!(!tx.is_discarded());
        tx.send(VerbProgressEvent::Started { backend: None }).await;
        tx.step(Some(0.5), "halfway").await;

        let first = rx.recv().await.unwrap();
        let second = rx.recv().await.unwrap();
        assert!(matches!(first, VerbProgressEvent::Started { .. }));
        assert!(matches!(second, VerbProgressEvent::Step { .. }));
    }

    #[tokio::test]
    async fn discard_succeeds_silently() {
        let tx = VerbProgress::discard();
        assert!(tx.is_discarded());
        let ok = tx.send(VerbProgressEvent::Started { backend: None }).await;
        assert!(ok);
        let try_ok = tx.try_send(VerbProgressEvent::Step {
            fraction: None,
            message: "ignored".into(),
        });
        assert!(try_ok.is_ok());
    }

    #[tokio::test]
    async fn send_returns_false_after_receiver_drop() {
        let (tx, rx) = VerbProgress::channel();
        drop(rx);
        let ok = tx.send(VerbProgressEvent::Started { backend: None }).await;
        assert!(!ok);
    }

    #[test]
    fn event_round_trips_as_json() {
        let e = VerbProgressEvent::Step {
            fraction: Some(0.25),
            message: "warming up".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: VerbProgressEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn log_level_serializes_snake_case() {
        let json = serde_json::to_string(&LogLevel::Warning).unwrap();
        assert_eq!(json, "\"warning\"");
    }
}
