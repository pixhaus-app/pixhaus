//! Verb input payload.
//!
//! Inputs are typed by the verb, not the runtime. The runtime ships a
//! `VerbInputs` (a thin wrapper around [`serde_json::Value`]) and the
//! verb's `validate` / `invoke` deserialise it into whatever struct the
//! verb wants. JSON Schema (carried on
//! [`super::descriptor::VerbDescriptor::input_schema`]) is the contract
//! the UI consults when generating the form; the runtime does not
//! enforce it directly.

use serde::{Deserialize, Serialize};

use super::error::{Result, VerbError};

/// Untyped input payload handed to a verb at invocation.
///
/// Conceptually a `serde_json::Value` — the wrapper exists so verbs
/// can `let inputs: MyInputs = inputs.deserialize()?` ergonomically and
/// so the IPC layer has a nominal type to round-trip rather than a
/// bare `Value`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerbInputs(serde_json::Value);

impl VerbInputs {
    /// Constructs `VerbInputs` from any JSON-shaped value.
    #[must_use]
    pub fn new(value: serde_json::Value) -> Self {
        Self(value)
    }

    /// Constructs `VerbInputs` by serialising a concrete struct. Verb
    /// authors typically build their inputs as a `#[derive(Serialize)]`
    /// type and call this on the way in.
    pub fn from_struct<T: Serialize>(value: &T) -> Result<Self> {
        Ok(Self(serde_json::to_value(value)?))
    }

    /// Empty payload — useful for verbs that take no inputs.
    #[must_use]
    pub fn empty() -> Self {
        Self(serde_json::Value::Object(serde_json::Map::new()))
    }

    /// Borrowed access to the underlying JSON value.
    #[must_use]
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    /// Consumes the wrapper, returning the inner JSON value.
    #[must_use]
    pub fn into_value(self) -> serde_json::Value {
        self.0
    }

    /// Deserialises the payload into a verb-specific type. Clones the
    /// inner JSON value, so for large payloads (anything carrying
    /// `PixelData` or other byte buffers) prefer
    /// [`Self::deserialize_owned`] from inside `invoke`. Wraps the
    /// underlying serde error as
    /// [`super::error::VerbError::Schema`] so callers can route
    /// validation failures through a single error variant.
    pub fn deserialize<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        serde_json::from_value(self.0.clone()).map_err(|e| VerbError::Schema(e.to_string()))
    }

    /// Consuming counterpart to [`Self::deserialize`]. The verb's
    /// [`super::verb::Verb::invoke`] receives `VerbInputs` by value,
    /// so the typical hot path can hand the JSON straight to serde
    /// without the deep clone — meaningful when the payload carries a
    /// `PixelData` chunk in the megabyte range.
    pub fn deserialize_owned<T: for<'de> Deserialize<'de>>(self) -> Result<T> {
        serde_json::from_value(self.0).map_err(|e| VerbError::Schema(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        count: u32,
    }

    #[test]
    fn empty_inputs_are_an_empty_object() {
        let i = VerbInputs::empty();
        assert!(i.as_value().is_object());
    }

    #[test]
    fn from_struct_round_trips() {
        let s = Sample {
            name: "echo".into(),
            count: 3,
        };
        let i = VerbInputs::from_struct(&s).unwrap();
        let back: Sample = i.deserialize().unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn malformed_input_surfaces_as_schema_error() {
        let i = VerbInputs::new(serde_json::json!({"name": 7}));
        let err = i.deserialize::<Sample>().unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn into_value_consumes_wrapper() {
        let i = VerbInputs::new(serde_json::json!({"x": 1}));
        let v = i.into_value();
        assert_eq!(v, serde_json::json!({"x": 1}));
    }

    #[test]
    fn deserialize_owned_matches_borrowed() {
        let s = Sample {
            name: "echo".into(),
            count: 7,
        };
        let i = VerbInputs::from_struct(&s).unwrap();
        let owned: Sample = i.deserialize_owned().unwrap();
        assert_eq!(owned, s);
    }

    #[test]
    fn deserialize_owned_surfaces_schema_error() {
        let i = VerbInputs::new(serde_json::json!({"name": 7}));
        let err = i.deserialize_owned::<Sample>().unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }
}
