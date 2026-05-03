// Integration tests are their own compilation unit; the workspace
// clippy denies (`unwrap_used`, `expect_used`, `disallowed_methods`)
// don't get the test-mode pass that lib.rs sets, so allow them here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]

//! End-to-end round-trip tests for the data model.
//!
//! Exercises the public surface of `pixhaus_core::project` from outside
//! the crate, the way `io`, `app`, and the Tauri command layer will.
//! Inline tests cover individual types; this file verifies the
//! [`Project`] composite via the [`fixtures::sample_project`] helper.
//!
//! Also emits a JSON fixture to `ui/tests/fixtures/sample-project.json`
//! so the TypeScript-side parse test stays in lockstep with the Rust
//! data model — re-running this test refreshes the fixture.

use std::path::PathBuf;

use pixhaus_core::fixtures::sample_project;
use pixhaus_core::project::{FeatureFlags, Project, SchemaVersion};

/// The MessagePack-named encoder serializes structs as maps so
/// `skip_serializing_if` round-trips correctly. The data model expects
/// every consumer (B3 file format, B4 IPC, B6 Unity handoff) to use
/// the same encoding.
fn to_msgpack(p: &Project) -> Vec<u8> {
    rmp_serde::to_vec_named(p).expect("messagepack encode")
}

fn from_msgpack(bytes: &[u8]) -> Project {
    rmp_serde::from_slice(bytes).expect("messagepack decode")
}

#[test]
fn sample_project_round_trips_via_messagepack() {
    let project = sample_project();
    let bytes = to_msgpack(&project);
    let back = from_msgpack(&bytes);
    assert_eq!(project, back);
}

#[test]
fn sample_project_round_trips_via_json() {
    let project = sample_project();
    let json = serde_json::to_string(&project).expect("json encode");
    let back: Project = serde_json::from_str(&json).expect("json decode");
    assert_eq!(project, back);
}

#[test]
fn schema_version_is_first_field_in_json() {
    let project = sample_project();
    let json = serde_json::to_string(&project).expect("json encode");
    // The header parser in B3 reads `schema_version` before doing
    // anything else; serde guarantees field order from the struct
    // definition for `serde_json`, so this assertion locks that in.
    let head: &str = &json[..40];
    assert!(head.starts_with("{\"schema_version\":"), "got: {head}");
}

#[test]
fn fixture_advertises_expected_feature_flags() {
    let p = sample_project();
    assert!(p.feature_flags.contains(FeatureFlags::TILEMAPS));
    assert!(p.feature_flags.contains(FeatureFlags::REFERENCES));
    assert!(p.feature_flags.contains(FeatureFlags::ANIMATIONS));
    assert!(p.feature_flags.contains(FeatureFlags::SLICES));
    assert!(!p.feature_flags.contains(FeatureFlags::VERB_HISTORY));
}

#[test]
fn fixture_uses_current_schema_version() {
    let p = sample_project();
    assert_eq!(p.schema_version, SchemaVersion::current());
}

#[test]
fn unknown_minor_version_still_loads() {
    let mut project = sample_project();
    project.schema_version = SchemaVersion {
        major: SchemaVersion::MAJOR,
        minor: 999,
    };
    let bytes = to_msgpack(&project);
    let back = from_msgpack(&bytes);
    assert_eq!(back.schema_version.minor, 999);
}

#[test]
fn write_json_fixture_for_typescript_parse_test() {
    // Resolve from CARGO_MANIFEST_DIR so the path is stable regardless
    // of where `cargo test` is invoked from. The TS-side parse test
    // imports the file at this exact location.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("ui");
    path.push("tests");
    path.push("fixtures");
    std::fs::create_dir_all(&path).expect("create fixture dir");
    path.push("sample-project.json");

    let json = serde_json::to_string_pretty(&sample_project()).expect("json encode");
    std::fs::write(&path, json + "\n").expect("write fixture");
}
