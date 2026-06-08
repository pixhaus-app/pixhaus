## platform

This group is exemplary, rubric-aware code with no real violations. Both `directories` gotchas are handled head-on with recorded rationale — `create_dir_all` before any first-run write, and distinct-named `log`/`autosave` leaves under `data_local_dir()` to survive the macOS config==data merge and Windows roaming. Error handling is textbook thiserror, there is no unwrap/expect/panic outside `#[cfg(test)]`, disk-touching functions carry `#[tracing::instrument]` with no subscriber install, and the crate correctly localizes nothing. The only confirmed finding is a thin test-coverage gap on one public delegating function.

### Strengths

- Both `directories` gotchas are handled with recorded rationale: `dirs.rs:99-103` derives `log`/`autosave` as distinct-named leaves under `data_local_dir()` to beat the macOS config==data merge and Windows roaming, and `log_dir_in` -> `create` calls `std::fs::create_dir_all` (`dirs.rs:128-141`) so a first-run write never fails with "No such file or directory".
- Error type is textbook thiserror: `DirsError::Create` (`dirs.rs:47-56`) drops `#[from]` because it carries `what`/`path` beyond the source, uses `#[source]` on the io field, and is built via `.map_err` at the single call site (`dirs.rs:136-140`) — the pattern `pixhaus-thiserror` prescribes for multi-field variants.
- No unwrap/expect/panic in non-test code: `app_dirs` uses `.ok_or(DirsError::NoHome)?` and `create` uses `.map_err(...)?`; all `panic!()` calls sit inside `#[cfg(test)]` (`dirs.rs:143-208`), and the crate root sets `#![cfg_attr(test, allow(clippy::unwrap_used, ...))]` (`lib.rs:10`).
- Tracing is correct for a library crate: `#[tracing::instrument(level="debug")]` on the two disk-touching fns `log_dir` (`dirs.rs:116`) and `create` (`dirs.rs:134`), no subscriber installed, no `println!`/`eprintln!`; `app_dirs` is left uninstrumented because it only computes paths.
- The `log_dir_in` seam is a clean testability decision: splitting leaf-creation from resolution lets the test point `log` at a `tempfile::tempdir()` (`dirs.rs:161-184`) and never touch the developer's real data dir, with the why recorded at `dirs.rs:121-127` — matching the `pixhaus-directories` "inject the base path" guidance.
- Tests assert the right thing per the directories skill: they check leaf joins and parent relationships (`dirs.rs:155-158`) rather than snapshotting machine-varying absolute paths, and `create_reports_what_and_path_on_failure` (`dirs.rs:186-207`) drives the Err path and asserts the carried `what`/`path`.
- The `NoHome` branch is deliberately left untested with a thorough comment (`dirs.rs:32-38`) explaining that forcing `ProjectDirs::from` to return `None` would require unsafe `env::set_var` (racy under nextest edition 2024) or defeating the Windows Known Folder API — honest, not over-claimed.
- Doc comments are complete and decision-recording: the LOCKED `PROJECT_DIRS` triple (`dirs.rs:17-23`), the `# Errors` sections on both public fns, and the module-level note that app owns the subscriber while platform only resolves the path (`dirs.rs:9-10`, mirrored in `crates/platform/CLAUDE.md`).

### Findings

| ID | File:Lines | Severity | Category | Issue -> Fix |
|----|------------|----------|----------|--------------|
| U7-1 | crates/platform/src/dirs.rs:116-119 | low | tests | `pixhaus-testing-conventions` floor "every public function has at least one test" is unmet: public `log_dir()` has zero tests of its own — its collaborators `app_dirs()` and `log_dir_in()` are each tested, but the wiring plus the `#[instrument]` on the public surface is never executed. -> Add a smoke test calling `log_dir()` and asserting the path `is_dir()` and ends in `logs`; since it writes into the real data dir, either guard it behind `#[ignore = "touches real data dir"]` or accept the inject-via-`log_dir_in` seam (`dirs.rs:121-127`) as the deliberate trade-off, making this informational. |

### Checked and cleared (false positives)

- U7-2 (AppDirs.config/.data/.cache/.autosave computed but unconsumed): rejected — the finding states it is "NOT a defect" and "No change needed now," its source is a CLAUDE.md scaffold-stage status note rather than a rule, and the unused fields match the documented scaffold stage and the bible's five-bucket decision the struct holds. No rule broken, no action proposed.
