//! Repo tooling for Pixhaus, run as `cargo run -q -p pixhaus-xtask -- <command>`.
//!
//! This is a Rust binary instead of shell scripts because the dev machines span
//! macOS, Linux, and Windows: `cargo run` is the one command string every platform
//! shell parses identically, with no interpreter to install per machine and no
//! sh/ps1 mirror pair to keep in sync. (The previous `scripts/post-edit.{sh,ps1}`
//! pair was wired through `powershell`, which does not exist on the unix machines,
//! so the hook silently never ran there.)
//!
//! `post-edit` is the Claude Code `PostToolUse` hook for Edit/Write: it reads the
//! hook JSON from stdin and formats + clippy-checks the crate that owns the touched
//! Rust file, so mistakes surface in the next turn instead of at the Stop gate.
//!
//! Accepted gap: if an edit breaks this crate itself, the outer `cargo run` build
//! fails with cargo's own exit code, so those compile errors reach the user but not
//! the model - converting that to exit 2 would need a per-OS shell wrapper, which
//! this binary exists to avoid. The Stop gate still catches it.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic))]
// This binary's contract is its stderr: Claude Code relays the hook's stderr to
// the model when it exits 2, so diagnostics MUST go there. The workspace print
// lints exist to route library logging through tracing, but a short-lived hook
// process has no subscriber and stderr IS its channel - opted out crate-wide
// rather than per-call.
#![allow(clippy::print_stderr)]

use std::env;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;

/// Exit code that asks Claude Code to feed this hook's stderr back to the model.
/// `PostToolUse` semantics: 0 = silent success, 2 = stderr goes to the model,
/// anything else = non-blocking note shown to the user only. The old shell
/// scripts always exited 0, which dropped the feedback they existed to deliver.
const FEEDBACK: u8 = 2;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("post-edit") => post_edit(args.next()),
        other => {
            eprintln!("xtask: unknown command {other:?}; expected `post-edit`");
            ExitCode::FAILURE
        }
    }
}

fn post_edit(path_arg: Option<String>) -> ExitCode {
    // An explicit argument wins over stdin so the hook can be exercised by hand:
    // `cargo run -q -p pixhaus-xtask -- post-edit crates/core/src/lib.rs`.
    let Some(raw) = path_arg.or_else(file_path_from_stdin) else {
        return ExitCode::SUCCESS;
    };
    let Some(file) = rust_source_path(&raw) else {
        return ExitCode::SUCCESS;
    };
    let Some(file) = absolute_path(Path::new(&file)) else {
        return ExitCode::SUCCESS;
    };
    if !is_within(&file, env::var("CLAUDE_PROJECT_DIR").ok().as_deref()) {
        // The session can sit in this repo while a file in another checkout is
        // edited; feeding a foreign repo's diagnostics back as blocking feedback
        // is worse than staying quiet.
        return ExitCode::SUCCESS;
    }

    let checks_pass = match owning_package_manifest(&file) {
        Some(manifest) => {
            let manifest = manifest.display().to_string();
            // fmt first so clippy reads formatted code; both must pass.
            let fmt = run_cargo(&["fmt", "--manifest-path", &manifest]);
            let clippy = run_cargo(&["clippy", "--manifest-path", &manifest, "--tests", "--", "-D", "warnings"]);
            fmt && clippy
        }
        // No [package] between the file and the workspace root (a workspace-level
        // .rs, or a new crate whose Cargo.toml isn't written yet): format
        // everything, skip clippy - there is no single crate to scope it to.
        None => run_cargo(&["fmt", "--all"]),
    };

    if checks_pass {
        ExitCode::SUCCESS
    } else {
        eprintln!("post-edit: fix the failures above before moving on ({})", file.display());
        ExitCode::from(FEEDBACK)
    }
}

/// The slice of the `PostToolUse` payload this hook reads. serde ignores the many
/// fields not modeled here; `default` tolerates payloads that omit these.
#[derive(Deserialize, Default)]
struct HookPayload {
    #[serde(default)]
    tool_input: ToolInput,
}

#[derive(Deserialize, Default)]
struct ToolInput {
    #[serde(default)]
    file_path: String,
}

fn file_path_from_stdin() -> Option<String> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        // Manual run without a pipe: nothing to read, and blocking on a terminal
        // read would hang the caller.
        return None;
    }
    let mut payload = String::new();
    stdin.read_to_string(&mut payload).ok()?;
    file_path_from_payload(&payload)
}

/// A malformed or unrelated payload is a skip, not an error: the hook must never
/// block editing because of its own plumbing.
fn file_path_from_payload(json: &str) -> Option<String> {
    let payload: HookPayload = serde_json::from_str(json).ok()?;
    (!payload.tool_input.file_path.is_empty()).then_some(payload.tool_input.file_path)
}

/// Normalize and filter the edited path: backslashes become forward slashes so
/// Windows payloads match the same patterns, build output and git internals are
/// skipped, and only Rust sources proceed. Filtering is case-insensitive because
/// Windows and default macOS filesystems are. A source module literally named
/// `target/` is also skipped - accepted: distinguishing it from build output isn't
/// worth the machinery here, and the Stop gate still covers those files.
fn rust_source_path(raw: &str) -> Option<String> {
    // std's Windows prefix parser rejects forward slashes inside verbatim \\?\
    // prefixes, so a blanket backslash swap would corrupt them - strip the
    // verbatim marker first (\\?\UNC\srv\... is the verbatim spelling of \\srv\...).
    let raw = raw
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| raw.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or_else(|| raw.to_owned());
    let path = raw.replace('\\', "/");

    let lower = path.to_ascii_lowercase();
    if lower.starts_with("target/") || lower.contains("/target/") || lower.contains("/.git/") {
        return None;
    }
    Path::new(&path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rs")).then_some(path)
}

/// Hook payloads carry absolute paths; relative ones come from manual runs.
fn absolute_path(file: &Path) -> Option<PathBuf> {
    if file.is_absolute() {
        Some(file.to_path_buf())
    } else {
        env::current_dir().ok().map(|cwd| cwd.join(file))
    }
}

/// Claude Code exports `CLAUDE_PROJECT_DIR` to hooks; when present, only files
/// under it are processed. Manual runs without the variable process everything.
fn is_within(file: &Path, project_dir: Option<&str>) -> bool {
    match project_dir {
        Some(dir) if !dir.is_empty() => file.starts_with(dir.replace('\\', "/")),
        _ => true,
    }
}

/// What a `Cargo.toml` declares, as far as the manifest walk cares.
enum ManifestKind {
    /// Declares `[package]` - the crate to scope fmt/clippy to.
    Package,
    /// Declares `[workspace]` but no `[package]` - a virtual manifest, and the
    /// hard upper boundary of the walk.
    WorkspaceOnly,
    /// Missing, unreadable, or neither section.
    NotAManifest,
}

/// Walk up from the edited file to the nearest `Cargo.toml` that declares a
/// `[package]`, so fmt/clippy scope to the owning crate rather than the whole
/// workspace. The walk stops hard at a virtual `[workspace]` manifest: without
/// that boundary, a stray `[package]` Cargo.toml above the repo (a scratch
/// ~/Cargo.toml is common) would get clippy-checked and its diagnostics fed back
/// as blocking feedback for edits in this repo.
fn owning_package_manifest(file: &Path) -> Option<PathBuf> {
    // ancestors() yields the file itself first; the manifest search starts at
    // its directory.
    file.ancestors()
        .skip(1)
        .find_map(|dir| {
            let manifest = dir.join("Cargo.toml");
            match manifest_kind(&manifest) {
                ManifestKind::Package => Some(Some(manifest)),
                ManifestKind::WorkspaceOnly => Some(None),
                ManifestKind::NotAManifest => None,
            }
        })
        .flatten()
}

fn manifest_kind(manifest: &Path) -> ManifestKind {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return ManifestKind::NotAManifest;
    };
    // The closing bracket keeps dotted sections like [package.metadata.docs] or
    // [workspace.dependencies] from counting as the section header itself.
    let has_section = |header: &str| text.lines().any(|line| line.trim_start().starts_with(header));
    if has_section("[package]") {
        ManifestKind::Package
    } else if has_section("[workspace]") {
        ManifestKind::WorkspaceOnly
    } else {
        ManifestKind::NotAManifest
    }
}

/// Run a cargo subcommand with inherited stdio - clippy and fmt diagnostics land
/// directly on this process's stderr, which is the hook's feedback channel.
/// A missing cargo is a skip (`true`), not a failure: a machine without the
/// toolchain on PATH should not have its edits blocked by the hook.
fn run_cargo(args: &[&str]) -> bool {
    eprintln!("post-edit: cargo {}", args.join(" "));
    // cargo run sets CARGO to the exact binary that launched this hook; reusing
    // it keeps the inner fmt/clippy on the same toolchain regardless of PATH
    // order or rustup's cwd-based selection. PATH lookup covers manual runs of
    // the bare binary.
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    match Command::new(&cargo).args(args).status() {
        Ok(status) => status.success(),
        Err(err) => {
            eprintln!("post-edit: could not launch cargo ({err}); skipping");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use tempfile::TempDir;

    use super::*;

    #[rstest]
    #[case::plain_rust("crates/core/src/lib.rs", Some("crates/core/src/lib.rs"))]
    #[case::windows_backslashes(r"crates\core\src\lib.rs", Some("crates/core/src/lib.rs"))]
    #[case::verbatim_disk(r"\\?\C:\repo\src\lib.rs", Some("C:/repo/src/lib.rs"))]
    #[case::verbatim_unc(r"\\?\UNC\srv\share\src\lib.rs", Some("//srv/share/src/lib.rs"))]
    #[case::uppercase_extension("/repo/src/LIB.RS", Some("/repo/src/LIB.RS"))]
    #[case::build_output("/repo/target/debug/build/probe.rs", None)]
    #[case::windows_build_output(r"C:\repo\target\debug\probe.rs", None)]
    #[case::mixed_case_build_output("/repo/Target/debug/probe.rs", None)]
    #[case::relative_build_output("target/debug/probe.rs", None)]
    #[case::git_internals("/repo/.git/hooks/sample.rs", None)]
    #[case::not_rust("/repo/docs/notes.md", None)]
    #[case::no_extension("/repo/bin/post-edit", None)]
    fn rust_source_path_normalizes_and_filters(#[case] raw: &str, #[case] expect: Option<&str>) {
        assert_eq!(rust_source_path(raw).as_deref(), expect);
    }

    #[rstest]
    #[case::edit_payload(r#"{"tool_input":{"file_path":"/repo/src/lib.rs"},"tool_name":"Edit"}"#, Some("/repo/src/lib.rs"))]
    #[case::missing_file_path(r#"{"tool_input":{}}"#, None)]
    #[case::missing_tool_input(r#"{"tool_name":"Edit"}"#, None)]
    #[case::malformed("not json at all", None)]
    #[case::empty("", None)]
    fn payload_extraction_is_lenient(#[case] json: &str, #[case] expect: Option<&str>) {
        assert_eq!(file_path_from_payload(json).as_deref(), expect);
    }

    #[rstest]
    #[case::no_project_dir("/any/where/lib.rs", None, true)]
    #[case::empty_project_dir("/any/where/lib.rs", Some(""), true)]
    #[case::inside("/repo/crates/core/src/lib.rs", Some("/repo"), true)]
    #[case::outside("/elsewhere/src/lib.rs", Some("/repo"), false)]
    #[case::prefix_but_not_component("/repo-sibling/src/lib.rs", Some("/repo"), false)]
    #[case::windows_project_dir("C:/repo/src/lib.rs", Some(r"C:\repo"), true)]
    fn project_dir_scoping(#[case] file: &str, #[case] project_dir: Option<&str>, #[case] expect: bool) {
        assert_eq!(is_within(Path::new(file), project_dir), expect);
    }

    /// Lay out a fake workspace: a virtual root manifest over one member package.
    fn fake_workspace() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("member/src")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"member\"]\n").unwrap();
        std::fs::write(root.join("member/Cargo.toml"), "[package]\nname = \"member\"\n").unwrap();
        std::fs::write(root.join("member/src/lib.rs"), "").unwrap();
        tmp
    }

    #[test]
    fn walk_up_finds_the_member_manifest_not_the_virtual_root() {
        let tmp = fake_workspace();
        let file = tmp.path().join("member/src/lib.rs");
        assert_eq!(owning_package_manifest(&file), Some(tmp.path().join("member/Cargo.toml")));
    }

    #[test]
    fn nearest_package_wins_over_an_outer_one() {
        let tmp = fake_workspace();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("member/nested/src")).unwrap();
        std::fs::write(root.join("member/nested/Cargo.toml"), "[package]\nname = \"nested\"\n").unwrap();
        let file = root.join("member/nested/src/lib.rs");
        assert_eq!(owning_package_manifest(&file), Some(root.join("member/nested/Cargo.toml")));
    }

    #[test]
    fn the_workspace_root_is_a_hard_boundary_for_the_walk() {
        // A [package] manifest ABOVE the workspace (a scratch ~/Cargo.toml) must
        // never be picked up for a workspace-level file with no owning package.
        let tmp = TempDir::new().unwrap();
        let outer = tmp.path();
        std::fs::write(outer.join("Cargo.toml"), "[package]\nname = \"stray\"\n").unwrap();
        std::fs::create_dir_all(outer.join("repo/src")).unwrap();
        std::fs::write(outer.join("repo/Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        std::fs::write(outer.join("repo/src/probe.rs"), "").unwrap();
        assert_eq!(owning_package_manifest(&outer.join("repo/src/probe.rs")), None);
    }

    #[rstest]
    #[case::package("[package]\nname = \"x\"\n", true, false)]
    #[case::indented_package("  [package]\nname = \"x\"\n", true, false)]
    #[case::workspace_only("[workspace]\nmembers = []\n", false, true)]
    #[case::package_and_workspace("[package]\nname = \"x\"\n[workspace]\n", true, false)]
    // Dotted sections can exist without their bare header in pathological
    // manifests; the bracket-closing match must not treat them as one.
    #[case::package_metadata_only("[package.metadata.docs]\nall-features = true\n", false, false)]
    fn manifest_kind_matches_section_headers(#[case] contents: &str, #[case] is_package: bool, #[case] is_workspace_only: bool) {
        let tmp = TempDir::new().unwrap();
        let manifest = tmp.path().join("Cargo.toml");
        std::fs::write(&manifest, contents).unwrap();
        let kind = manifest_kind(&manifest);
        assert_eq!(matches!(kind, ManifestKind::Package), is_package, "package mismatch for: {contents}");
        assert_eq!(
            matches!(kind, ManifestKind::WorkspaceOnly),
            is_workspace_only,
            "workspace mismatch for: {contents}"
        );
    }

    #[test]
    fn missing_manifest_is_not_a_package() {
        let tmp = TempDir::new().unwrap();
        assert!(matches!(manifest_kind(&tmp.path().join("Cargo.toml")), ManifestKind::NotAManifest));
    }
}
