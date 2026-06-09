//! Test-only builders for Codex entries with fields `core` gates behind
//! `pub(crate)` mutators.
//!
//! The compiler, resolver, and validation all read entry fields — prompt fragments,
//! aliases, character palette refs — that `core` only lets a command write, and no
//! public command sets them this pass. Rather than reach into `core` internals (a
//! layering break) or skip the cases, these helpers round-trip a command-built
//! [`Codex`] through JSON and patch the extra fields into the serialized form, then
//! deserialize it back. The round-trip uses only public `serde` derives, so it stays
//! within the layer.

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]

use serde_json::{Value, json};

use pixhaus_core::{Codex, CodexEntryId, InclusionPriority};

/// Serializes `codex`, patches `entry`'s entry object via `patch`, and deserializes
/// the result back into a [`Codex`]. The entry must already exist.
fn patch_entry(codex: &Codex, entry: CodexEntryId, patch: impl FnOnce(&mut Value)) -> Codex {
    let mut value = serde_json::to_value(codex).expect("codex serializes");
    // Entries serialize as a map keyed by the id's stringified inner value.
    let key = entry.0.to_string();
    let entry_value = value
        .get_mut("entries")
        .and_then(Value::as_object_mut)
        .and_then(|m| m.get_mut(&key))
        .expect("entry exists in the serialized codex");
    patch(entry_value);
    serde_json::from_value(value).expect("patched codex deserializes")
}

/// Returns a copy of `codex` with `text` added as a prompt fragment of `priority` on
/// `entry`.
pub fn with_prompt_fragment(codex: &Codex, entry: CodexEntryId, text: &str, priority: InclusionPriority) -> Codex {
    patch_entry(codex, entry, |e| {
        let priority_tag = match priority {
            InclusionPriority::Critical => "Critical",
            InclusionPriority::Important => "Important",
            InclusionPriority::Normal => "Normal",
            InclusionPriority::Optional => "Optional",
            InclusionPriority::NeverInPrompt => "NeverInPrompt",
        };
        let fragments = e
            .get_mut("prompt_fragments")
            .and_then(Value::as_array_mut)
            .expect("prompt_fragments is an array");
        fragments.push(json!({ "text": text, "priority": priority_tag }));
    })
}

/// Returns a copy of `codex` with `text` added as a negative fragment on `entry`.
pub fn with_negative_fragment(codex: &Codex, entry: CodexEntryId, text: &str) -> Codex {
    patch_entry(codex, entry, |e| {
        let negatives = e
            .get_mut("negative_fragments")
            .and_then(Value::as_array_mut)
            .expect("negative_fragments is an array");
        negatives.push(json!(text));
    })
}

/// Returns a copy of `codex` with `alias` added to `entry`'s aliases.
pub fn with_alias(codex: &Codex, entry: CodexEntryId, alias: &str) -> Codex {
    patch_entry(codex, entry, |e| {
        let aliases = e.get_mut("aliases").and_then(Value::as_array_mut).expect("aliases is an array");
        aliases.push(json!(alias));
    })
}

/// Returns a copy of `codex` with `entry`'s character `palette_ref` set to `handle`.
/// The entry must be a character.
pub fn with_palette_ref(codex: &Codex, entry: CodexEntryId, handle: &str) -> Codex {
    patch_entry(codex, entry, |e| {
        let details = e.get_mut("details").expect("entry has details");
        let character = details.get_mut("Character").expect("entry is a character");
        character
            .as_object_mut()
            .expect("character body is an object")
            .insert("palette_ref".to_owned(), json!(handle));
    })
}
