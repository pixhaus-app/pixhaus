//! [`CodexHandle`]: the validated, lowercase handle that powers `@`-mentions.
//!
//! A handle is the stable, human-readable token an artist types after `@` (bible
//! 11.3). It is not a display name and never localizes: it is lowercase ASCII, made
//! of `a-z`, `0-9`, and `_`, must start with a letter, and never holds a namespace
//! prefix (the [`EntryType`](crate::codex::EntryType) carries that). Validation lives
//! here so a malformed handle can never enter the model.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated lowercase handle used for `@`-mentions, e.g. `bit`, `mossy_stone`,
/// `fungal_catacombs`.
///
/// Construct one through [`CodexHandle::new`], which rejects anything that is not a
/// lowercase ASCII identifier. The inner string is private so a `CodexHandle` is
/// always valid by construction.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CodexHandle(String);

/// Why a string could not become a [`CodexHandle`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HandleError {
    /// The handle was empty.
    #[error("handle is empty")]
    Empty,
    /// The handle did not start with a lowercase ASCII letter.
    #[error("handle must start with a lowercase letter, got {0:?}")]
    BadFirstChar(char),
    /// The handle held a character outside `a-z`, `0-9`, `_`.
    #[error("handle has an invalid character {0:?}; allowed: a-z, 0-9, underscore")]
    InvalidChar(char),
}

impl CodexHandle {
    /// Validates `raw` and returns a handle, or a [`HandleError`] describing why it
    /// was rejected. The input must already be lowercase; an uppercase letter is an
    /// `InvalidChar` so the caller lowercases deliberately rather than silently.
    ///
    /// # Errors
    /// Returns [`HandleError`] when `raw` is empty, starts with a non-letter, or holds
    /// a character outside the allowed set.
    pub fn new(raw: impl Into<String>) -> Result<Self, HandleError> {
        let raw = raw.into();
        let mut chars = raw.chars();
        let first = chars.next().ok_or(HandleError::Empty)?;
        if !first.is_ascii_lowercase() {
            return Err(HandleError::BadFirstChar(first));
        }
        if let Some(bad) = raw.chars().find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')) {
            return Err(HandleError::InvalidChar(bad));
        }
        Ok(Self(raw))
    }

    /// The handle as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_handles() {
        for ok in ["bit", "mossy_stone", "fungal_catacombs", "pixel_art_32", "a"] {
            assert!(CodexHandle::new(ok).is_ok(), "expected {ok} to be valid");
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(CodexHandle::new(""), Err(HandleError::Empty));
    }

    #[test]
    fn rejects_bad_first_char() {
        assert_eq!(CodexHandle::new("9lives"), Err(HandleError::BadFirstChar('9')));
        assert_eq!(CodexHandle::new("_lead"), Err(HandleError::BadFirstChar('_')));
    }

    #[test]
    fn rejects_invalid_chars() {
        // An uppercase letter after a valid first char is an invalid char.
        assert_eq!(CodexHandle::new("biT"), Err(HandleError::InvalidChar('T')));
        assert_eq!(CodexHandle::new("mossy-stone"), Err(HandleError::InvalidChar('-')));
        assert_eq!(CodexHandle::new("with space"), Err(HandleError::InvalidChar(' ')));
    }

    #[test]
    fn rejects_uppercase_first_char_as_bad_first() {
        assert_eq!(CodexHandle::new("Bit"), Err(HandleError::BadFirstChar('B')));
    }

    #[test]
    fn as_str_round_trips() {
        let h = CodexHandle::new("bit").unwrap();
        assert_eq!(h.as_str(), "bit");
    }
}
