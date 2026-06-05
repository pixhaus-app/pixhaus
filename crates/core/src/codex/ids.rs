//! [`CodexEntryId`]: the stable identifier for a Codex entry (bible 11.2).
//!
//! Like the rest of the model's ids, this is a monotonic `u64` newtype minted from
//! an [`IdCounter`](crate::ids::IdCounter) the [`Codex`](crate::codex::Codex) owns.
//! References point at this id, never at the handle or display name, so renaming an
//! entry never breaks a reference (bible 11.2).

use serde::{Deserialize, Serialize};

/// Stable identifier for a [`CodexEntry`](crate::codex::CodexEntry) within one
/// project's [`Codex`](crate::codex::Codex).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CodexEntryId(pub u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_compare_by_value() {
        assert_eq!(CodexEntryId(3), CodexEntryId(3));
        assert!(CodexEntryId(1) < CodexEntryId(2));
    }
}
