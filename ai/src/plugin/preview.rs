//! Preview, commit, and discard records.
//!
//! The protocol formalises the "preview-then-commit" lifecycle as
//! plain values:
//!
//! 1. The runtime invokes a verb and the verb returns a
//!    [`super::output::VerbOutput`].
//! 2. The runtime wraps the output in a [`VerbPreview`] tagged with a
//!    fresh [`PreviewId`] and timing metadata.
//! 3. The host shows the preview to the user.
//! 4. On accept, the host calls
//!    [`super::runtime::VerbRuntime::commit`] which returns a
//!    [`VerbCommit`] — the description of effects applied — for the
//!    undo system to consume (S05).
//! 5. On reject, the host calls
//!    [`super::runtime::VerbRuntime::discard`] which returns a
//!    [`VerbDiscard`] for telemetry / analytics.
//!
//! The runtime keeps no per-preview mutable state. Callers carry the
//! `VerbPreview` value between the invocation and the commit/discard;
//! the runtime's role is to mint identifiers and stamp timestamps.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::descriptor::VerbId;
use super::output::{VerbEffect, VerbOutput};

/// Monotonic identifier for a preview within one editor session.
///
/// Minted by the runtime; not stable across sessions. Wire format is a
/// plain `u64` so the IPC layer can round-trip it without bespoke
/// serde.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PreviewId(pub u64);

impl PreviewId {
    /// Constructs a `PreviewId` from a raw value. Prefer
    /// [`PreviewIdMinter`] for issuing new IDs.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the wrapped value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Atomic counter that mints monotonically increasing [`PreviewId`]s.
///
/// The runtime owns one. IDs start at `1`; `0` is reserved as a
/// sentinel meaning "no preview".
#[derive(Debug)]
pub struct PreviewIdMinter {
    next: AtomicU64,
}

impl PreviewIdMinter {
    /// Constructs a fresh minter starting at `1`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    /// Issues the next ID. Skips `0` on wrap so the sentinel invariant
    /// holds for the lifetime of the process. Realistic editor sessions
    /// will never wrap — the type leaves room for a hundred million
    /// previews per millisecond for several centuries — but a CAS loop
    /// is cheap and removes a footgun.
    pub fn issue(&self) -> PreviewId {
        // `Relaxed` is fine: we need a unique value per call, not an
        // ordering with respect to other shared state.
        loop {
            let current = self.next.fetch_add(1, Ordering::Relaxed);
            if current != 0 {
                return PreviewId(current);
            }
            // The post-increment value was 0 (wrapped). Skip it and try
            // again; the next fetch_add will return 1.
        }
    }
}

impl Default for PreviewIdMinter {
    /// Same as [`Self::new`] — starts at `1`. The manual impl exists
    /// because `AtomicU64::default()` would start at `0`, which is
    /// reserved as the "no preview" sentinel.
    fn default() -> Self {
        Self::new()
    }
}

/// A finished verb invocation, ready for the user to accept or reject.
///
/// `VerbPreview` is a value, not a handle. The runtime returns it from
/// [`super::runtime::VerbInvocation::finish`] and the caller keeps
/// hold until they decide what to do. The big payload — pixel buffers
/// inside `output.effects` and the optional `thumbnail` — is plain
/// `Vec<u8>`, so cloning a preview deep-copies those bytes.
/// `VerbPreview` is `Clone` for serde and IPC plumbing; in the normal
/// flow callers should *move* the preview into
/// [`super::runtime::VerbRuntime::commit`] or
/// [`super::runtime::VerbRuntime::discard`] rather than clone it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerbPreview {
    /// Unique ID for this preview within the session.
    pub id: PreviewId,
    /// Verb that produced this preview.
    pub verb: VerbId,
    /// What the verb produced.
    pub output: VerbOutput,
    /// Wall-clock time the runtime spent waiting for the verb.
    pub elapsed: Duration,
    /// Real-world timestamp of when the verb finished, in UTC seconds
    /// since epoch. Stored as a plain integer to avoid pulling in
    /// `chrono` at the protocol level.
    pub finished_at: i64,
}

impl VerbPreview {
    /// Constructs a preview from runtime-minted metadata.
    #[must_use]
    pub fn new(id: PreviewId, verb: VerbId, output: VerbOutput, elapsed: Duration) -> Self {
        Self {
            id,
            verb,
            output,
            elapsed,
            finished_at: now_unix_seconds(),
        }
    }
}

/// Returns the current UTC time as seconds since the Unix epoch.
/// Pre-epoch system clocks (any `duration_since` failure) clamp to `0`
/// — stamping with epoch is preferable to stamping with `i64::MAX`,
/// which would put the timestamp 292 billion years in the future.
/// Far-future clocks that overflow `i64::try_from` clamp to `i64::MAX`.
/// The protocol uses `i64` because seconds-since-epoch round-trips
/// through `serde_json` as a plain number.
pub(crate) fn now_unix_seconds() -> i64 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
pub(crate) fn current_unix_seconds() -> i64 {
    now_unix_seconds()
}

/// Record produced when a preview is committed.
///
/// `VerbCommit` is what the future undo system (S05) turns into a
/// command. The runtime's contribution is to stamp the commit time
/// and route the effects without further interpretation; the
/// command pattern decides how each `VerbEffect` translates into a
/// reversible operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerbCommit {
    /// Preview the commit came from.
    pub preview: PreviewId,
    /// Verb that produced the original preview.
    pub verb: VerbId,
    /// The effects to apply.
    pub effects: Vec<VerbEffect>,
    /// Real-world timestamp of commit (UTC seconds since epoch).
    pub committed_at: i64,
    /// Wall-clock duration the verb took to produce the preview.
    pub elapsed: Duration,
}

/// Record produced when a preview is discarded.
///
/// Carries an optional reason so analytics (and the IPC layer) can
/// distinguish "user rejected" from "preview expired".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerbDiscard {
    /// Preview the discard came from.
    pub preview: PreviewId,
    /// Verb that produced the original preview.
    pub verb: VerbId,
    /// Optional reason for the discard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Real-world timestamp of discard (UTC seconds since epoch).
    pub discarded_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::output::ActualCost;

    fn now_seconds() -> i64 {
        super::current_unix_seconds()
    }

    #[test]
    fn minter_returns_increasing_ids() {
        let minter = PreviewIdMinter::new();
        let a = minter.issue();
        let b = minter.issue();
        let c = minter.issue();
        assert_eq!(a.get(), 1);
        assert_eq!(b.get(), 2);
        assert_eq!(c.get(), 3);
    }

    #[test]
    fn preview_round_trips() {
        let p = VerbPreview {
            id: PreviewId::new(42),
            verb: VerbId::new("pixhaus.builtin.echo"),
            output: VerbOutput {
                summary: "echoed".into(),
                effects: vec![],
                thumbnail: None,
                actual_cost: ActualCost::free(Duration::from_micros(5)),
                notes: vec![],
            },
            elapsed: Duration::from_micros(5),
            finished_at: 1_700_000_000,
        };
        let bytes = rmp_serde::to_vec_named(&p).unwrap();
        let back: VerbPreview = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn preview_new_stamps_finished_at() {
        let before = now_seconds();
        let p = VerbPreview::new(
            PreviewId::new(1),
            VerbId::new("x"),
            VerbOutput {
                summary: String::new(),
                effects: vec![],
                thumbnail: None,
                actual_cost: ActualCost::default(),
                notes: vec![],
            },
            Duration::ZERO,
        );
        let after = now_seconds();
        assert!(p.finished_at >= before && p.finished_at <= after);
    }
}
