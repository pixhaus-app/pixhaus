# Scripting, Testing, and CI Tooling for Pixhaus

**Date:** May 2026  
**Scope:** Embedded scripting, plugin systems, testing frameworks, benchmarking, fuzzing, and CI/quality tooling for Pixhaus (Rust implementation).  
**Primary Consumers:** S37 (plugin loader), S38 (Lua integration), S52 (visual regression), B8 (CI infrastructure).

---

## Embedded Scripting

### mlua

- **Purpose:** High-level Lua bindings for Rust with async/await, supporting multiple Lua versions and LuaJIT.
- **Crates.io:** https://crates.io/crates/mlua
- **Docs:** https://docs.rs/mlua
- **Repo:** https://github.com/mlua-rs/mlua
- **License:** MIT/Apache-2.0
- **Maintenance (May 2026):** Active, well-maintained.
- **When to use:** Pixhaus plugin scripts, editor configuration, animation timelines, tilemap rules. Standard choice for Lua integration in production Rust editors.
- **Alternatives:** rlua (deprecated successor), rhai, rune, piccolo
- **Notes:** Started as a fork of rlua and has become the ecosystem standard. Supports Lua 5.4, 5.3, 5.2, 5.1, LuaJIT, and Roblox Luau. Async functions via `create_async_function` allow non-blocking Lua calls. Vendored Lua/LuaJIT available via `lua-src` / `luajit-src` crates—easiest path for self-contained builds. API differs slightly from rlua (ToLua renamed to IntoLua), but rlua 0.20 includes compatibility aliases.
- **Pixhaus streams using it:** S38 (Lua scripting), S37 (plugin system anchor).

---

### rlua

- **Purpose:** Historic Lua bindings for Rust (now superseded by mlua).
- **Crates.io:** https://crates.io/crates/rlua
- **Docs:** https://docs.rs/rlua
- **Repo:** https://github.com/mlua-rs/rlua
- **License:** MIT/Apache-2.0
- **Maintenance (May 2026):** Deprecated in favor of mlua.
- **When to use:** Do not use for new projects. Existing rlua code should migrate to mlua (same author, drop-in upgrade path via compatibility layer).
- **Alternatives:** mlua
- **Notes:** rlua is the predecessor to mlua. Both are maintained by the same team in the same GitHub organization. rlua remains on crates.io for backward compatibility but receives minimal updates. All future development is on mlua.
- **Pixhaus streams using it:** None (use mlua instead).

---

### rhai

- **Purpose:** Rust-native embedded scripting language, designed for performance and simplicity without external dependencies.
- **Crates.io:** https://crates.io/crates/rhai
- **Docs:** https://docs.rs/rhai
- **Repo:** https://github.com/rhaiscript/rhai
- **License:** MIT/Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Dynamic wrappers around Rust logic, editor hotloading, rule engines. Rhai is roughly 2x slower than Python for typical workloads but integrates cleanly with Rust APIs. Best used as a thin dynamic control layer over Rust implementations.
- **Alternatives:** rune, Lua (mlua/piccolo), dyon
- **Notes:** No external C dependencies; compiles to a single binary. Syntax is Rust-like but simplified. Excellent for sandboxing hostile scripts due to built-in limits on execution depth and memory. Benchmarks available on https://rhai.rs/book/about/benchmarks.html. Slower than Lua but easier integration with native Rust types.
- **Pixhaus streams using it:** Possible for tilemap/animation rule engines.

---

### rune

- **Purpose:** Rust-syntax embedded language with native async/await, designed for high performance and integration with Rust.
- **Crates.io:** https://crates.io/crates/rune
- **Docs:** https://docs.rs/rune
- **Repo:** https://github.com/rune-rs/rune
- **License:** MIT/Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Async-heavy workloads, applications where Rust syntax familiarity is an asset. Rune performs well on async operations and feels like "Rust without types."
- **Alternatives:** rhai, Lua, dyon
- **Notes:** Inspired by Rhai but with different design trade-offs. Strong async support via native select statements. Global state across VM calls may require workarounds. Performance target is comparable to Lua and Python. Best for team with Rust expertise who want to embed a familiar syntax.
- **Pixhaus streams using it:** Consider for async animation controllers or network-heavy plugins.

---

### piccolo

- **Purpose:** Pure-Rust Lua reimplementation, experimentally stackless, with cycle-detecting garbage collector.
- **Crates.io:** https://crates.io/crates/piccolo
- **Docs:** https://docs.rs/piccolo
- **Repo:** https://github.com/kyren/piccolo
- **License:** MIT
- **Maintenance (May 2026):** Maintained (resumed April 2023 after years of hiatus), but still experimental.
- **When to use:** Lua sandboxing where C-linkage cannot be used. Security-first Lua environments. Research into pure-Rust Lua semantics.
- **Alternatives:** mlua (C-based, faster), rhai, rune
- **Notes:** Written in mostly-safe Rust (not full unsafe blocks). Stackless/trampoline-style VM avoids Rust stack nesting issues. Real incremental cycle-detecting garbage collector. Zero-cost Gc pointers usable from safe Rust. Expect pre-1.0 API breakage. Slower than mlua (C-based) but significantly more portable and embeddable in restricted environments. COMPATIBILITY.md on GitHub details Lua version alignment.
- **Pixhaus streams using it:** Not recommended for production; consider for future Lua-in-WASM work.

---

## WebAssembly Runtimes (Plugin Sandboxing)

### wasmtime

- **Purpose:** Lightweight, standards-compliant WebAssembly runtime by Bytecode Alliance, optimized for security and the Component Model.
- **Crates.io:** https://crates.io/crates/wasmtime
- **Docs:** https://docs.rs/wasmtime
- **Repo:** https://github.com/bytecodealliance/wasmtime
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, industry standard.
- **When to use:** Plugin systems demanding capability-based security, standards compliance, and fine-grained control. Cold start ~3ms, 15MB memory footprint. Preferred for untrusted code or regulated environments.
- **Alternatives:** wasmer, wasm3-rs (lightweight), extism (higher-level abstraction)
- **Notes:** Dominant WASM runtime in production. Component Model provides structured plugin interfaces. Functions only access resources explicitly granted (capability model). LLVM-based compilation. Strong focus on standards (WASI, Component Model). Requires explicit setup for host-plugin communication compared to extism's higher-level patterns. Ideal base layer for custom plugin frameworks.
- **Pixhaus streams using it:** S37 (WASM plugin architecture), optional foundation for extism integration.

---

### wasmer

- **Purpose:** Cross-platform WebAssembly runtime with rich SDKs for multiple languages.
- **Crates.io:** https://crates.io/crates/wasmer
- **Docs:** https://docs.rs/wasmer
- **Repo:** https://github.com/wasmerio/wasmer
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Multi-language ecosystem, cross-platform plugin distribution, developer experience prioritized. Cold start ~2ms, 12MB memory footprint, 13,000 req/s throughput. Better than wasmtime on edge devices.
- **Alternatives:** wasmtime, wasm3-rs, extism
- **Notes:** Emphasis on developer tooling and SDKs (Rust, C/C++, Python, JavaScript). Supports first-to-IR-to-native compilation. Similar performance to wasmtime on benchmarks. Offers language bindings beyond Rust. Less focus on standards purity than wasmtime; more focus on pragmatic integration.
- **Pixhaus streams using it:** Consider if multi-language plugin ecosystem is a goal.

---

### wasm3-rs

- **Purpose:** Rust wrapper for WASM3, the fastest lightweight WebAssembly interpreter for embedded systems.
- **Crates.io:** https://crates.io/crates/wasm3-rs
- **Docs:** https://docs.rs/wasm3-rs (minimal; GitHub is primary)
- **Repo:** https://github.com/wasm3/wasm3-rs
- **License:** MIT
- **Maintenance (May 2026):** Maintained, work-in-progress (may not be entirely sound).
- **When to use:** Size-constrained environments (embedded, mobile via Tauri/WASM hybrid), minimal CPU overhead. Not recommended for high-throughput plugin systems.
- **Alternatives:** wasmtime, wasmer
- **Notes:** Compact interpreter. No cmake required to build. Slower than JIT-based runtimes (wasmtime, wasmer) but significantly lighter. Ideal for microbench code-as-config scenarios, not for heavy compute plugins. Sound guarantees still being established.
- **Pixhaus streams using it:** Not primary; consider for future embedded variants or Tauri WASM bridge.

---

### extism

- **Purpose:** Universal plugin framework built atop WASM runtimes, providing high-level abstractions for host-plugin communication.
- **Crates.io:** https://crates.io/crates/extism
- **Docs:** https://docs.rs/extism
- **Repo:** https://github.com/extism/extism
- **License:** Apache-2.0
- **Maintenance (May 2026):** Active, production-ready.
- **When to use:** Rapid plugin system development, multi-language plugin ecosystem, simplified host-plugin data movement. Handles marshalling, persistent memory, HTTP without WASI, runtime limiters. Primary option for Pixhaus plugin architecture.
- **Alternatives:** wasmtime alone (lower-level), wasmer, custom plugin systems
- **Notes:** Wraps wasmtime, wasmer, and other runtimes as interchangeable backends. Provides PDKs (plugin development kits) for 7+ languages. 15+ official Host SDKs. Zero security assumptions—fully sandboxes plugin execution. Eliminates repetitive WASM boilerplate. Runtime timers and memory limits prevent runaway plugins. Recommended for teams prioritizing rapid plugin iteration over low-level control.
- **Pixhaus streams using it:** S37 (primary plugin system candidate if WASM chosen).

---

## Plugin / Extension Architecture

### libloading

- **Purpose:** Dynamic library loading on native platforms (C ABI).
- **Crates.io:** https://crates.io/crates/libloading
- **Docs:** https://docs.rs/libloading
- **Repo:** https://github.com/nagisa/rust_libloading
- **License:** ISC
- **Maintenance (May 2026):** Maintained.
- **When to use:** Legacy native plugin systems. Do not use for new Pixhaus work; prefer WASM (extism, wasmtime) or language-based scripting (Lua, rhai).
- **Alternatives:** abi_stable, dlopen2, WASM (extism, wasmtime)
- **Notes:** Low-level C ABI wrapping. Requires unsafe code, manual symbol lookups, and careful versioning. Rust ABI is unstable; forces conversion to C ABI. Not recommended for distribution across toolchain versions.
- **Pixhaus streams using it:** None (migrate to WASM or scripting).

---

### abi_stable

- **Purpose:** Rust-friendly plugin ABI built on C ABI, with macros and utilities for safe plugin systems.
- **Crates.io:** https://crates.io/crates/abi_stable
- **Docs:** https://docs.rs/abi_stable
- **Repo:** https://github.com/rodrimati1992/abi_stable_rust
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Maintained.
- **When to use:** Native Rust plugins requiring ABI stability across toolchain versions. Safer than raw libloading; still lower-level than WASM.
- **Alternatives:** WASM (extism, wasmtime), dlopen2
- **Notes:** Provides C ABI definitions, stable std library clone, macros to eliminate unsafe code for many patterns. Significantly safer than raw dynamic linking. Still requires careful version management and distribution strategy. WASM (extism) is simpler for new projects.
- **Pixhaus streams using it:** Not recommended unless native performance and Rust API are hard requirements and WASM cannot be used.

---

### dlopen2

- **Purpose:** Safe, high-level dynamic library loading with automatic symbol mapping.
- **Crates.io:** https://crates.io/crates/dlopen2
- **Docs:** https://docs.rs/dlopen2
- **Repo:** https://github.com/OpenByteDev/dlopen2
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Maintained.
- **When to use:** Native plugin systems requiring structured APIs. Nicer interface than libloading; good thread-safety guarantees.
- **Alternatives:** libloading, abi_stable, WASM
- **Notes:** Automatic symbol loading into user-defined structures. Zero-cost structural wrappers prevent dangling symbols. Preferred over libloading for native plugins. Still subject to Rust ABI instability; less future-proof than WASM.
- **Pixhaus streams using it:** Not primary; consider only if native performance is non-negotiable and WASM cannot be adopted.

---

## Testing Frameworks

### cargo test (built-in)

- **Purpose:** Rust's native test runner and assertion framework.
- **Crates.io:** N/A (stdlib)
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Part of Rust core; actively maintained.
- **When to use:** All unit and integration tests. Fast iteration during development.
- **Alternatives:** cargo-nextest (parallel, faster)
- **Notes:** Default test runner. Sufficient for most projects but single-threaded by default, leading to slower test suites in large workspaces. Pairs well with assertion helpers (pretty_assertions, assert_cmd, assert_fs).
- **Pixhaus streams using it:** S37, S38, S52, B8 (all test suites).

---

### cargo-nextest

- **Purpose:** Next-generation parallel test runner, up to 3x faster than cargo test, production-ready for CI.
- **Crates.io:** https://crates.io/crates/cargo-nextest
- **Docs:** https://nexte.st/
- **Repo:** https://github.com/nextest-rs/nextest
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active; RustRover 2026.1 added native IDE integration.
- **When to use:** Large test suites, CI pipelines, local development in monorepos. Up to 60% faster than cargo test. Built-in retry for flaky tests. Separates build and run phases (useful for distributed CI).
- **Alternatives:** cargo test (slower), custom test harnesses
- **Notes:** Not the default runner, but widely adopted in 2026. RustRover IDE now integrates nextest directly with progress reporting and structured results. Parallelizes test execution across CPU cores. No extra assertions needed—fully compatible with standard test attributes. Recommended for Pixhaus given scale of editor codebase.
- **Pixhaus streams using it:** B8 (CI infrastructure), S37, S38, S52 (optional faster iteration).

---

### rstest

- **Purpose:** Fixture-based testing with parameterized cases and table-driven tests.
- **Crates.io:** https://crates.io/crates/rstest
- **Docs:** https://docs.rs/rstest
- **Repo:** https://github.com/la10736/rstest
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Parameterized tests (multiple inputs/outputs), fixtures (setup/teardown), async test fixtures.
- **Alternatives:** test-case, macro_rules-based parameterization
- **Notes:** Procedural macros for `#[rstest]` (parametrized), `#[fixture]` (setup), and async variants. Fixtures can be injected by other fixtures. Each parameterized case generates an independent test that fails independently. Async tests require runtime specification (tokio, async_std, etc.). Very clean API; standard for fixture-heavy projects.
- **Pixhaus streams using it:** S52 (visual regression fixtures), S38 (Lua integration tests), S37 (plugin loading scenarios).

---

### test-case

- **Purpose:** Attribute macro for parameterized tests with inline case definitions.
- **Crates.io:** https://crates.io/crates/test-case
- **Docs:** https://docs.rs/test-case
- **Repo:** https://github.com/frondeus/test-case
- **License:** MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Simple parameterized tests where rstest fixtures are not needed.
- **Alternatives:** rstest, proptest
- **Notes:** Lighter weight than rstest. `#[test_case]` macro with inline parameters. No fixture support. Good for straightforward table-driven tests.
- **Pixhaus streams using it:** Optional for simple parameterized tests.

---

### proptest

- **Purpose:** Property-based testing framework (Hypothesis-like for Rust), generates random shrinking test inputs.
- **Crates.io:** https://crates.io/crates/proptest
- **Docs:** https://docs.rs/proptest
- **Repo:** https://github.com/proptest-rs/proptest
- **License:** Apache-2.0 / MIT / Dual
- **Maintenance (May 2026):** Active.
- **When to use:** Invariant properties (e.g., "serialization round-trip preserves data"), fuzzing-adjacent testing, complex input generation. Proptest uses explicit Strategy objects, enabling more sophisticated generation and shrinking than QuickCheck.
- **Alternatives:** quickcheck, bolero
- **Notes:** Superior to QuickCheck for custom generation strategies. Different strategies for same type; composable. Slower generation (up to 10x) than QuickCheck due to stateful shrinking, but vastly more expressive. Standard choice for property-based testing in Rust. Minimal vs. maximal test case discovery via shrinking.
- **Pixhaus streams using it:** S52 (image transformation properties), S37 (plugin lifecycle invariants).

---

### quickcheck

- **Purpose:** Property-based testing with simpler, faster generation via typeclass-based Arbitrary.
- **Crates.io:** https://crates.io/crates/quickcheck
- **Docs:** https://docs.rs/quickcheck
- **Repo:** https://github.com/BurntSushi/quickcheck
- **License:** MIT / Unlicense
- **Maintenance (May 2026):** Maintained.
- **When to use:** Simple property tests where proptest's Strategy flexibility is overkill. Faster generation for basic types.
- **Alternatives:** proptest, bolero
- **Notes:** Generates/shrinks via Arbitrary trait only. One generator and shrinker per type; custom generation requires newtypes. Roughly 10x faster generation than proptest but less expressive. Good for quick property proofs, not for elaborate input construction. Still actively used but proptest is more common in new projects.
- **Pixhaus streams using it:** Optional fallback if performance matters for simple properties.

---

### insta

- **Purpose:** Snapshot testing library for comparing output against stored golden files.
- **Crates.io:** https://crates.io/crates/insta
- **Docs:** https://docs.rs/insta
- **Repo:** https://github.com/mitsuhiko/insta
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, standard for Rust snapshot testing.
- **When to use:** Visual rendering (tilemap output, animation frames), serialized data, text output. Supports CSV, JSON, TOML, YAML, RON via Serde. Batteries-included with CLI review/approval.
- **Alternatives:** Custom file-comparison tests (manual)
- **Notes:** Industry standard for snapshot testing. `assert_snapshot!` and `assert_debug_snapshot!` macros. `cargo insta test` reviews snapshots interactively. Snapshots stored in snapshot/ folder. Excellent for regression detection (e.g., image rendering changes). Very mature and widely used.
- **Pixhaus streams using it:** S52 (visual regression snapshots), S38 (Lua output snapshots), S37 (plugin API contract snapshots).

---

### mockall

- **Purpose:** Mock object generation from trait definitions via procedural macros.
- **Crates.io:** https://crates.io/crates/mockall
- **Docs:** https://docs.rs/mockall
- **Repo:** https://github.com/asomers/mockall
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Unit tests with complex dependencies. Automatic or manual mock generation from traits.
- **Alternatives:** Manual trait mocks, mockito (HTTP-specific), wiremock-rs (HTTP-specific)
- **Notes:** Procedural macros generate mock structs from traits. Supports expectations, call ordering, mutable mocks. Industry standard for Rust mocking. Less friction than manual mocks. Integrates well with dependency injection patterns.
- **Pixhaus streams using it:** S37 (mock plugin loaders), S38 (mock Lua environments), S52 (mock image backends).

---

### mockito / httpmock / wiremock-rs

- **Purpose:** HTTP mocking for testing code that makes HTTP requests.
- **Crates.io:** 
  - https://crates.io/crates/mockito
  - https://crates.io/crates/httpmock
  - https://crates.io/crates/wiremock
- **Docs:** 
  - https://docs.rs/mockito
  - https://docs.rs/httpmock
  - https://docs.rs/wiremock
- **License:** MIT (all)
- **Maintenance (May 2026):** All active. wiremock-rs is async-native and recommended.
- **When to use:** Integration tests for network calls (plugin downloads, config updates, API interactions).
- **Alternatives:** None (domain-specific)
- **Notes:** **wiremock-rs** (by Luca Palmieri) is async-first, works with tokio/async_std, supports request matching and response templating, spying (verifying calls), and standalone mode. Recommended over mockito/httpmock for new code. All support request matching and mocking of HTTP responses.
- **Pixhaus streams using it:** S37 (plugin registry HTTP mocks), optional for network-dependent features.

---

### pretty_assertions

- **Purpose:** Colorful diffs for failed assert_eq! comparisons.
- **Crates.io:** https://crates.io/crates/pretty_assertions
- **Docs:** https://docs.rs/pretty_assertions
- **Repo:** https://github.com/rust-pretty-assertions/rust-pretty-assertions
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** All assertion-heavy tests. Drop-in replacement for assert_eq!; no API changes.
- **Alternatives:** insta (snapshots), assert_fs (file assertions)
- **Notes:** Overrides standard assert_eq! macro with colorful, side-by-side diffs. Dramatically improves test failure readability. Cargo feature `std` controls display formatting. Standard practice in modern Rust projects.
- **Pixhaus streams using it:** S37, S38, S52, B8 (all test suites).

---

## Benchmarking

### criterion

- **Purpose:** Statistical benchmarking harness with confidence intervals, linear regression, and comparison across runs.
- **Crates.io:** https://crates.io/crates/criterion
- **Docs:** https://docs.rs/criterion
- **Repo:** https://github.com/bheisler/criterion.rs
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, de facto standard.
- **When to use:** Performance regressions, algorithmic tuning, local development. Works on stable Rust.
- **Alternatives:** iai-callgrind (CI-stable), divan (simpler API)
- **Notes:** Industry standard for Rust benchmarking. Runs benchmarks repeatedly, computes statistics, detects regressions. HTML reports with graphs. Can be noisy in CI environments due to variance. Pairs well with iai-callgrind for CI.
- **Pixhaus streams using it:** S37 (plugin loading perf), S38 (Lua execution), S52 (image rendering), B8 (benchmark suite).

---

### iai-callgrind

- **Purpose:** Instruction-counted benchmarking using Valgrind's Callgrind, deterministic in CI.
- **Crates.io:** https://crates.io/crates/iai-callgrind
- **Docs:** https://docs.rs/iai-callgrind
- **Repo:** https://github.com/iai-callgrind/iai-callgrind
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** CI benchmarking, instruction-level performance tracking. Eliminates noise from system load, CPU frequency scaling, etc.
- **Alternatives:** criterion (for local dev), divan
- **Notes:** Counts instructions (via Valgrind), not wall-clock time. Deterministic and reproducible in any environment (GitHub Actions, local VMs). Pairs perfectly with criterion for complete benchmark coverage: criterion for local iteration, iai-callgrind for CI regression detection. Requires Linux with Valgrind; slower to run but exact.
- **Pixhaus streams using it:** B8 (CI benchmarks), S52 (image processing regressions).

---

### divan

- **Purpose:** Simpler, statistically-comfortable benchmarking with allocation measurement and generic function support.
- **Crates.io:** https://crates.io/crates/divan
- **Docs:** https://docs.rs/divan
- **Repo:** https://github.com/clockworklabs/divan
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, gaining adoption.
- **When to use:** Quick performance iteration, allocation profiling, generic function benches (criterion limitation).
- **Alternatives:** criterion (heavier), iai-callgrind
- **Notes:** Newer entrant with focus on ease of use. API is simpler than criterion. Measures allocations. Supports generic functions (criterion does not). Statistically conservative by design. Growing ecosystem but less mature than criterion.
- **Pixhaus streams using it:** Optional modern alternative for new benchmarks.

---

## Fuzzing

### cargo-fuzz

- **Purpose:** Libfuzzer integration for Rust, finding crash-inducing inputs via instrumented coverage feedback.
- **Crates.io:** https://crates.io/crates/cargo-fuzz
- **Docs:** https://rust-fuzz.github.io/book/cargo-fuzz/
- **Repo:** https://github.com/rust-fuzz/cargo-fuzz
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, part of Rust Fuzzing Authority ecosystem.
- **When to use:** Image parsing, tilemap loaders, file format handling, plugin WASM modules.
- **Alternatives:** bolero, afl.rs
- **Notes:** De facto standard for Rust fuzzing. Requires LLVM sanitizer support, nightly Rust, C++11 compiler. Works on x86-64 and Aarch64 on Unix (not Windows). Inputs must implement Arbitrary trait. `cargo fuzz init` and `cargo fuzz add` set up fuzz targets. Good integration with CI (OSS-Fuzz).
- **Pixhaus streams using it:** S37 (plugin WASM validation), S52 (image decoder fuzz).

---

### arbitrary

- **Purpose:** Trait and derive macro for converting unstructured input bytes into Rust values.
- **Crates.io:** https://crates.io/crates/arbitrary
- **Docs:** https://docs.rs/arbitrary
- **Repo:** https://github.com/rust-fuzz/arbitrary
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, part of Rust Fuzzing Authority.
- **When to use:** Paired with cargo-fuzz (or other fuzzers) to define Arbitrary implementations for custom types.
- **Alternatives:** Manual Arbitrary impl, quickcheck (simpler but less expressive)
- **Notes:** Automatically derives Arbitrary for enums and structs via `#[derive(Arbitrary)]`. Enables structure-aware fuzzing (fuzzer understands nested data). Essential plumbing for effective fuzzing. Low overhead.
- **Pixhaus streams using it:** Paired with cargo-fuzz for all fuzz targets.

---

### bolero

- **Purpose:** Unified property-testing and fuzzing framework supporting multiple backends (libfuzzer, afl, honggfuzz).
- **Crates.io:** https://crates.io/crates/bolero
- **Docs:** https://camshaft.github.io/bolero/
- **Repo:** https://github.com/camshaft/bolero
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Testing code with multiple fuzzing engines, property tests that double as fuzz targets.
- **Alternatives:** cargo-fuzz (single engine), proptest (no fuzzing)
- **Notes:** Front-end unifying cargo-fuzz (libfuzzer), afl.rs, and honggfuzz. Runs same test with different engines. Good for teams exploring fuzzing or needing engine flexibility. Less common than cargo-fuzz + proptest pairing but valuable for comprehensive testing.
- **Pixhaus streams using it:** Alternative to cargo-fuzz if multi-engine exploration is desired.

---

### afl.rs

- **Purpose:** Rust bindings for American Fuzzy Lop (AFL), coverage-guided fuzzer.
- **Crates.io:** https://crates.io/crates/afl
- **Docs:** https://docs.rs/afl
- **Repo:** https://github.com/rust-fuzz/afl.rs
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Maintained, part of Rust Fuzzing Authority.
- **When to use:** Alternative to libfuzzer with different trade-offs, or when bolero supports multiple engines.
- **Alternatives:** cargo-fuzz (libfuzzer), bolero
- **Notes:** Lower barrier to entry than cargo-fuzz (standard stable Rust, no nightly). Slightly less powerful than libfuzzer. Good for teams unable to use nightly or prefer afl's approach.
- **Pixhaus streams using it:** Optional alternative engine.

---

## Visual Regression Testing (Image-Heavy Applications)

### insta (text snapshots)

- **Purpose:** Golden file comparison for structured data and text output.
- **Docs:** https://docs.rs/insta
- **Notes:** See "Testing Frameworks" section. Works for text-based assertions of rendered output.
- **When to use for visual regression:** Snapshots of JSON/TOML tilemap exports, animation frame metadata, shader code generation.
- **Pixhaus streams using it:** S52 (snapshot metadata), S38 (Lua output).

---

### pixelmatch-rs

- **Purpose:** Pixel-perfect image comparison (pure Rust port of pixelmatch.js).
- **Crates.io:** https://crates.io/crates/pixelmatch-rs
- **Docs:** https://docs.rs/pixelmatch-rs
- **Repo:** https://github.com/Brooooooklyn/pixelmatch-rs (Node-bound); check for standalone Rust ports
- **License:** MIT
- **Maintenance (May 2026):** Exists but minimal activity. Consider image-compare or custom solutions.
- **When to use:** Per-pixel diff detection, threshold-based matching, manual visual regression.
- **Alternatives:** image-compare, insta + manual verification, Tauri screenshot testing
- **Notes:** Port of pixelmatch.js. Not heavily maintained as standalone. Useful for scripted regression but lacks interactive UI. Manual review workflows still preferred for Pixhaus visual regression.
- **Pixhaus streams using it:** S52 (if automated pixel-level diffs are needed).

---

### image-compare

- **Purpose:** Image comparison and diff generation in Rust.
- **Crates.io:** https://crates.io/crates/image-compare
- **Docs:** https://docs.rs/image-compare
- **License:** MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Quantitative image diffs, SSIM/PSNR metrics, automated visual testing.
- **Alternatives:** pixelmatch-rs, manual insta snapshots, browser-based screenshot tools
- **Notes:** Supports various comparison metrics (Euclidean, SSIM, PSNR). Generates diff images. Works with image crate. Useful for headless/CI testing of rendered output.
- **Pixhaus streams using it:** S52 (quantitative visual regression).

---

### Tauri screenshot testing (manual)

- **Purpose:** Browser-based visual regression via Tauri's test utilities.
- **Docs:** https://v2.tauri.app/develop/tests/
- **License:** N/A (Tauri framework feature)
- **Maintenance (May 2026):** Active with Tauri 2.x.
- **When to use:** Full UI visual regression (entire editor window), interactive component testing.
- **Alternatives:** Playwright, Cypress (not Rust-native), manual insta snapshots
- **Notes:** Tauri.js provides screenshot comparison APIs. Test harness in Rust can invoke screenshot methods. Best paired with manual review or external services (Percy, Applitools) for intelligent diffing. Rust-side is mostly orchestration; visual intelligence lives in browser/JS.
- **Pixhaus streams using it:** S52 (UI-level visual regression, optional).

---

## Test Fixtures and CLI Testing

### assert_fs

- **Purpose:** Temporary filesystem management and assertions for integration tests.
- **Crates.io:** https://crates.io/crates/assert_fs
- **Docs:** https://docs.rs/assert_fs
- **Repo:** https://github.com/assert-rs/assert_fs
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active.
- **When to use:** File I/O tests, tilemap export validation, configuration file handling.
- **Alternatives:** tempfile (basic temp dirs), std::fs (manual)
- **Notes:** Creates temporary files/dirs that live for test scope. Supports assertions on file existence, content, permissions. Pairs perfectly with assert_cmd. Clean, readable test setup.
- **Pixhaus streams using it:** S37 (plugin file staging), S38 (Lua config files), S52 (output image files).

---

### predicates

- **Purpose:** Composable predicates for flexible assertions on file content, process output, etc.
- **Crates.io:** https://crates.io/crates/predicates
- **Docs:** https://docs.rs/predicates
- **Repo:** https://github.com/assert-rs/predicates-rs
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Pattern matching in test assertions (regex, startswith, contains, custom predicates).
- **Alternatives:** Manual string matching, test-case with custom assertions
- **Notes:** Composable predicate functions. Integrates with assert_fs and assert_cmd. Much more readable than raw regex assertions. Standard practice in CLI testing.
- **Pixhaus streams using it:** S37, S38, S52 (output validation).

---

### assert_cmd

- **Purpose:** Integration testing for binary CLIs, capturing stdout/stderr and exit codes.
- **Crates.io:** https://crates.io/crates/assert_cmd
- **Docs:** https://docs.rs/assert_cmd
- **Repo:** https://github.com/assert-rs/assert_cmd
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, standard for CLI testing.
- **When to use:** Pixhaus CLI tool testing (sprite export, tilemap compile, animation render).
- **Alternatives:** Manual Command spawning, custom test harnesses
- **Notes:** Spawns CLI binaries, captures output, asserts on exit code and stdout/stderr. Pairs with predicates and assert_fs for complete CLI testing. Very readable test cases.
- **Pixhaus streams using it:** B8 (CLI binary tests), S37 (plugin loader CLI).

---

## Code Quality and Linting

### cargo-clippy

- **Purpose:** Rust linter with 600+ lint rules for correctness, performance, and idiomatic code.
- **Docs:** https://doc.rust-lang.org/nightly/cargo/commands/cargo-clippy.html
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Part of Rust core; actively maintained.
- **When to use:** All projects (CI gate). Catch common mistakes, inefficiencies, non-idiomatic patterns before review.
- **Alternatives:** None; cargo-clippy is the Rust standard.
- **Notes:** Run via `cargo clippy` on stable. 600+ configurable lints. CI should enforce with `cargo clippy -- -D warnings` to deny all warnings. Catches serious bugs (unwrap in unreachable code) and style issues (unnecessary clone).
- **Pixhaus streams using it:** B8 (CI gate), all streams.

---

### cargo-fmt

- **Purpose:** Automatic code formatter for Rust, enforcing consistent style.
- **Docs:** https://rust-lang.github.io/rustfmt/
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Part of Rust core; actively maintained.
- **When to use:** Every commit. Enforce via CI (`cargo fmt --check`).
- **Alternatives:** None; cargo-fmt is the standard.
- **Notes:** Configured via rustfmt.toml. Run pre-commit or in CI. Eliminates style debates; focuses team on logic. Can format Cargo.toml with experimental features.
- **Pixhaus streams using it:** B8 (CI gate), all streams.

---

### cargo-deny

- **Purpose:** Dependency linting: license compliance, banned crates, duplicate versions, supply-chain security.
- **Crates.io:** https://crates.io/crates/cargo-deny
- **Docs:** https://embarkstudios.com/open-source/cargo-deny
- **Repo:** https://github.com/EmbarkStudios/cargo-deny
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active.
- **When to use:** CI gate for all projects. Enforces license policy, bans problematic crates, detects supply-chain issues.
- **Alternatives:** cargo-audit (security only), custom scripts
- **Notes:** Four checks: advisories (like cargo-audit), licenses, bans, sources. Configured via deny.toml. Use alongside cargo-audit for comprehensive supply-chain coverage.
- **Pixhaus streams using it:** B8 (CI gate).

---

### cargo-audit

- **Purpose:** Security advisory scanner against RustSec database.
- **Crates.io:** https://crates.io/crates/cargo-audit
- **Docs:** https://docs.rs/cargo-audit
- **Repo:** https://github.com/rustsec/cargo-audit
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active, backed by Rust Secure Code WG.
- **When to use:** CI gate. Catches unmaintained and vulnerable crates.
- **Alternatives:** cargo-deny (broader), custom queries
- **Notes:** Queries RustSec Advisory Database. Excellent for local development and CI. Use alongside cargo-deny (cargo-deny adds license/ban checks; cargo-audit adds unmaintained crate detection).
- **Pixhaus streams using it:** B8 (CI gate).

---

### cargo-machete

- **Purpose:** Fast, imprecise unused dependency detection.
- **Crates.io:** https://crates.io/crates/cargo-machete
- **Docs:** https://github.com/bnjbvr/cargo-machete#readme
- **Repo:** https://github.com/bnjbvr/cargo-machete
- **License:** MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Regular cleanup of Cargo.toml (e.g., monthly). Optional CI check (may have false positives).
- **Alternatives:** cargo-udeps (slower, more accurate), manual review
- **Notes:** Searches src/ for crate names; if not found, likely unused. Fast (imprecise)—misses generated code, build scripts, macros. GitHub Action available. Good for periodic cleanup but not zero-false-positive.
- **Pixhaus streams using it:** Optional for dependency hygiene.

---

### cargo-outdated

- **Purpose:** List outdated dependencies and available updates.
- **Crates.io:** https://crates.io/crates/cargo-outdated
- **Docs:** https://docs.rs/cargo-outdated
- **License:** MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Periodic dependency upgrade campaigns. Identify breaking vs. patch updates.
- **Alternatives:** cargo upgrade (part of cargo-edit)
- **Notes:** Reports semver compatibility of available updates. Helps plan upgrade sprints. Pairs with cargo-deny (check for new advisory after upgrade).
- **Pixhaus streams using it:** B8 (periodic upgrade sprints).

---

### cargo-watch

- **Purpose:** Auto-rerun cargo commands on file changes (dev loop acceleration).
- **Crates.io:** https://crates.io/crates/cargo-watch
- **Docs:** https://docs.rs/cargo-watch
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Local development. Watch tests, clippy, build on file save.
- **Alternatives:** IDE file watchers (RustRover, VS Code)
- **Notes:** `cargo watch -x build -x clippy -x test` on save. Dramatically speeds up iteration. Less essential with modern IDEs but still valuable for terminal-based workflows.
- **Pixhaus streams using it:** Optional dev workflow (S37, S38, S52).

---

### cargo-edit

- **Purpose:** CLI commands for editing Cargo.toml (cargo add, cargo remove, cargo upgrade).
- **Crates.io:** https://crates.io/crates/cargo-edit
- **Docs:** https://docs.rs/cargo-edit
- **Repo:** https://github.com/killercup/cargo-edit
- **License:** MIT / Apache-2.0
- **Maintenance (May 2026):** Maintained; cargo add/remove merged into cargo core (cargo remove in dev).
- **When to use:** `cargo add <crate>`, `cargo rm <crate>`, `cargo upgrade` from CLI instead of manual Cargo.toml edits.
- **Alternatives:** Manual Cargo.toml, IDE support
- **Notes:** `cargo add foo` appends foo to [dependencies]. `cargo upgrade` updates semver within constraints. Faster than manual editing. Some commands (add, rm) moving to cargo core in 2026; still useful for cargo upgrade.
- **Pixhaus streams using it:** Dev workflow (optional convenience).

---

### cargo-llvm-cov / tarpaulin

- **Purpose:** Code coverage measurement.
- **Crates.io:** 
  - https://crates.io/crates/cargo-llvm-cov
  - https://crates.io/crates/cargo-tarpaulin
- **Docs:**
  - https://docs.rs/cargo-llvm-cov
  - https://docs.rs/cargo-tarpaulin
- **License:** Apache-2.0 / MIT (both)
- **Maintenance (May 2026):** Both active. cargo-llvm-cov recommended for new projects.
- **When to use:** Coverage reporting in CI, identifying untested code paths.
- **Comparison:**
  - **cargo-llvm-cov:** LLVM source-based instrumentation, region-level precision, cross-platform (Linux, macOS, Windows), more accurate.
  - **tarpaulin:** ptrace-based (Linux x86_64 only), line-level, no compiler flags needed, slower.
- **Alternatives:** None; must choose one.
- **Notes:** cargo-llvm-cov is the modern choice. tarpaulin remains viable on Linux. Output formats: LCOV, Cobertura XML, JSON. Pair with CI (GitHub Actions, etc.) for trend tracking.
- **Pixhaus streams using it:** B8 (CI coverage gates).

---

### typos

- **Purpose:** Typo detection in code, comments, docs.
- **Crates.io:** https://crates.io/crates/typos
- **Docs:** https://docs.rs/typos-cli
- **Repo:** https://github.com/crate-ci/typos
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active.
- **When to use:** CI pre-commit check. Optional; quality-of-life.
- **Alternatives:** None (orthogonal to other tooling)
- **Notes:** Fast spell-checker for source code. Configured via typos.toml. Catches "teh", "recieve", etc. Low false-positive rate. GitHub Action available.
- **Pixhaus streams using it:** Optional B8 quality gate.

---

### committed

- **Purpose:** Commit message linting (conventional commits, etc.).
- **Crates.io:** https://crates.io/crates/committed
- **Docs:** https://docs.rs/committed
- **Repo:** https://github.com/crate-ci/committed
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Optional CI gate to enforce commit message standards (e.g., "fix: " prefix).
- **Alternatives:** commitlint (Node-based), pre-commit hooks
- **Notes:** Rust-native conventional commits linter. Improves git history readability. Optional but recommended for collaborative projects.
- **Pixhaus streams using it:** Optional B8 quality gate.

---

## Profiling

### pprof

- **Purpose:** In-process CPU profiler with flamegraph output.
- **Crates.io:** https://crates.io/crates/pprof
- **Docs:** https://docs.rs/pprof
- **Repo:** https://github.com/tikv/pprof-rs
- **License:** Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Bottleneck detection, hotspot identification during development and CI profiling.
- **Alternatives:** flamegraph (perf-based), cargo-flamegraph, native OS tools
- **Notes:** Statistical sampling via SIGPROF. Native Rust interface; no external tools needed. Generates flamegraph output. Minimal overhead. Excellent for identifying where CPU time is spent.
- **Pixhaus streams using it:** S37 (plugin loading perf), S52 (image rendering perf).

---

### flamegraph

- **Purpose:** Flamegraph visualization via cargo-flamegraph (perf/DTrace on Linux/macOS).
- **Crates.io:** https://crates.io/crates/flamegraph
- **Docs:** https://docs.rs/flamegraph
- **Repo:** https://github.com/flamegraph-rs/flamegraph
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** CPU profiling on Linux (perf), macOS/BSD (DTrace). Standalone visualization.
- **Alternatives:** pprof (in-process), criterion (with pprof integration)
- **Notes:** `cargo flamegraph --bin <name>` produces flamegraph.svg. Works on Linux/macOS/BSD. Lower overhead than pprof for continuous profiling. Requires perf/DTrace; not Windows.
- **Pixhaus streams using it:** Development profiling (S37, S52).

---

### coz

- **Purpose:** Causal profiler measuring impact of program changes on execution time.
- **Crates.io:** https://crates.io/crates/coz
- **Docs:** https://docs.rs/coz
- **Repo:** https://github.com/plasma-umass/coz (main), Rust bindings
- **License:** Apache-2.0
- **Maintenance (May 2026):** Research project; active but specialized.
- **When to use:** Causal performance analysis, understanding which optimizations matter most.
- **Alternatives:** pprof, flamegraph, criterion
- **Notes:** Measures which code is most responsible for slowness (not just hot code). Advanced use case. Useful for research or extreme optimization; overkill for most projects.
- **Pixhaus streams using it:** Optional research/extreme optimization.

---

### dhat

- **Purpose:** Memory profiling and allocation tracking (Valgrind DHAT tool).
- **Crates.io:** https://crates.io/crates/dhat
- **Docs:** https://docs.rs/dhat
- **Repo:** https://github.com/nnethercote/dhat-rs
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Maintained.
- **When to use:** Memory leak detection, allocation bottlenecks, heap profiling.
- **Alternatives:** Valgrind directly, iai-callgrind (includes DHAT), manual instrumentation
- **Notes:** Rust wrapper for DHAT (part of Valgrind). Slow (full instrumentation) but precise. HTML reports showing allocation sites and memory usage. Valuable for memory-heavy applications (image processing).
- **Pixhaus streams using it:** S52 (image buffer profiling), optional.

---

## Error Reporting in Tests

### color-eyre

- **Purpose:** Colorful, formatted error reports with backtraces and context.
- **Crates.io:** https://crates.io/crates/color-eyre
- **Docs:** https://docs.rs/color-eyre
- **Repo:** https://github.com/eyre-rs/color-eyre
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Integration tests, CLI tools, better panic messages in test failures.
- **Alternatives:** standard panic, eyre (uncolored)
- **Notes:** Custom panic hook for colored, human-readable error reports. Three formats: minimal, short, full. Integrates with tracing for context. Dramatically improves debugging experience.
- **Pixhaus streams using it:** S37, S38, S52 (integration test errors).

---

### backtrace

- **Purpose:** Stack backtrace capture and pretty printing.
- **Crates.io:** https://crates.io/crates/backtrace
- **Docs:** https://docs.rs/backtrace
- **License:** Apache-2.0 / MIT
- **Maintenance (May 2026):** Part of Rust ecosystem; stable.
- **When to use:** Paired with color-eyre or custom error handling.
- **Alternatives:** std::backtrace (Rust 1.65+, unstable)
- **Notes:** Captures backtraces with symbol resolution. Integrates with error handling crates. Already used by panic handlers; explicit usage less common. color-eyre uses backtrace internally.
- **Pixhaus streams using it:** Indirect (via color-eyre).

---

## AI-Friendly Testing Patterns

### Conventions for Agents to Write Good Rust Tests

When Claude Code or other AI tools author tests for Pixhaus, follow these patterns:

1. **Fixture-based structure:** Use rstest fixtures for setup. Agents understand fixtures better than ad-hoc test harnesses. Define `#[fixture]` functions for common test state.

   ```rust
   #[fixture]
   fn sprite_context() -> SpriteContext { /* ... */ }
   
   #[rstest]
   fn test_animation_frame(sprite_context: SpriteContext) {
       // Test body is declarative; agent understands dependencies
   }
   ```

2. **Parameterized table-driven tests:** Use `#[rstest]` with `#[case]` for multiple inputs. Agents excel at generating test matrices.

   ```rust
   #[rstest]
   #[case(0, 0)]
   #[case(256, 1)]
   #[case(-1, -1)]
   fn test_palette_index(#[case] input: i32, #[case] expected: i32) {
       assert_eq!(input.saturating_abs(), expected.abs());
   }
   ```

3. **Property-based tests with proptest:** Agents can generate property strategies and shrinking rules. Explicit Strategy objects are better than QuickCheck's implicit Arbitrary.

   ```rust
   #[test]
   fn prop_serialization_roundtrip(sprite in sprite_strategy()) {
       let serialized = sprite.to_bytes();
       let deserialized = Sprite::from_bytes(&serialized).unwrap();
       assert_eq!(sprite, deserialized);
   }
   ```

4. **Snapshot tests with insta:** Agents easily maintain snapshots. Use for complex output validation (rendered frames, export formats).

   ```rust
   #[test]
   fn test_tilemap_export() {
       let tilemap = TileMap::load("test.tmx").unwrap();
       insta::assert_json_snapshot!(tilemap.to_json());
   }
   ```

5. **CLI testing with assert_cmd and predicates:** Agents can generate binary test cases with clear assertions.

   ```rust
   #[test]
   fn cli_sprite_export() {
       let mut cmd = Command::cargo_bin("pixhaus-cli").unwrap();
       cmd.arg("export").arg("sprite.pxh").arg("output.png");
       cmd.assert().success()
           .stdout(predicate::str::contains("Exported"));
   }
   ```

6. **Mock objects with mockall:** Agents can derive mocks from trait definitions. Explicit `.expect()` chains are more readable than assertion inference.

   ```rust
   #[test]
   fn plugin_loader_calls_init() {
       let mut mock_plugin = MockPlugin::new();
       mock_plugin.expect_init().times(1).returning(|| Ok(()));
       let loader = PluginLoader::with_plugin(mock_plugin);
       loader.load().unwrap();
   }
   ```

7. **Avoid bare integration tests:** Use assert_fs + assert_cmd + predicates instead of manual file I/O and Command spawning. Agents understand these higher-level APIs.

8. **Test naming:** Use descriptive names following `test_<unit>_<scenario>_<expectation>`. Agents generate better tests when naming is explicit.

   ```rust
   #[test]
   fn test_sprite_load_invalid_magic_number_returns_err() { }
   
   // Better than:
   // fn test_sprite() { }
   ```

9. **Async test support:** Specify runtime (tokio, async_std) explicitly in rstest. Agents handle async better with clear annotations.

   ```rust
   #[tokio::test]
   async fn test_plugin_async_init() { }
   
   #[rstest]
   #[tokio::test]
   async fn test_with_fixture(context: AsyncContext) { }
   ```

10. **Error path testing:** Use should_panic, .unwrap_err(), and color_eyre for panic context. Agents can generate exhaustive error case coverage.

    ```rust
    #[test]
    #[should_panic(expected = "invalid")]
    fn test_palette_invalid_range() { }
    
    #[test]
    fn test_load_missing_file() {
        let err = Sprite::load("missing.pxh").unwrap_err();
        assert!(matches!(err, SpriteError::NotFound));
    }
    ```

---

## Key Research Findings

### mlua vs rlua (Definitive 2026 Verdict)

**Use mlua for all new projects.** rlua is deprecated and maintained only for backward compatibility. mlua is the same team's successor with broader Lua version support (5.1–5.4, LuaJIT, Luau), async/await integration, and better API design. Migration is straightforward: rlua 0.20+ includes compatibility aliases.

### wasmtime vs extism for Editor Plugin System

**extism is the higher-level choice for Pixhaus.** It abstracts away WASM runtime details, provides host-plugin marshalling, persistent memory, and runtime limiters out of the box. wasmtime is the low-level foundation extism uses; choose it only if you need fine-grained capability control or extism's assumptions don't fit. For a visual editor with third-party plugins, extism's rapid iteration model is preferable.

### cargo-nextest as Default Test Runner (2026)

**cargo-nextest has not replaced cargo test as the default runner, but adoption is accelerating.** RustRover 2026.1 integrated native nextest support (major tooling validation). For projects with large test suites (Pixhaus qualifies), adopting nextest can yield 60–3x speedup with minimal friction (drop-in replacement, no code changes). Recommended for B8 CI infrastructure; optional for local development.

### insta Snapshot Testing (2026 Standard)

**insta remains the de facto Rust snapshot testing standard in 2026.** No serious competitors. Widely used in production. Recommended for S52 (visual regression of rendered output), S38 (Lua output snapshots), and S37 (plugin API contracts).

### Property-Based Testing (2026 Verdict)

**proptest dominates for new projects.** quickcheck is simpler but proptest's explicit Strategy objects enable far more sophisticated testing (custom shrinking, composable generators). For Pixhaus: use proptest for invariant properties (serialization round-trips, tilemap transforms), quickcheck only if simplicity is critical. bolero is less common but valuable if exploring multiple fuzzing engines.

### Visual Regression for Tauri App

**No perfect Rust-native solution exists.** Options:
- **insta snapshots:** Text-based snapshots of rendered data (metadata, JSON exports). Recommended for asset validation.
- **image-compare / pixelmatch-rs:** Per-pixel diffs, quantitative metrics. Useful for headless rendering tests.
- **Tauri JS API + manual review:** Full UI screenshots. Requires JavaScript bindings; limited Rust-side automation. Best paired with external services (Percy, Applitools) for intelligent diffing.
- **Recommended approach for Pixhaus:** insta for metadata/data snapshots, image-compare for unit-level rendering tests, Tauri JS + manual review for full-UI regression.

### Code Coverage (2026 Standard)

**cargo-llvm-cov is the modern recommendation.** LLVM source-based instrumentation provides region-level precision, cross-platform support (Linux, macOS, Windows), and accuracy superior to tarpaulin's ptrace approach. tarpaulin remains viable on Linux-only projects. For Pixhaus (cross-platform), cargo-llvm-cov is the right choice.

---

## Recommended Dependency Tree for Pixhaus

**Core scripting:** mlua (Lua) + optional rhai/rune for rules.  
**Plugins (primary):** extism + wasmtime (WASM plugins).  
**Plugins (native, if needed):** abi_stable (safer than raw dynamic loading).  
**Testing:** cargo-nextest, rstest, proptest, insta, mockall, assert_cmd, assert_fs, pretty_assertions.  
**Benchmarking:** criterion (local) + iai-callgrind (CI).  
**Fuzzing:** cargo-fuzz + arbitrary.  
**Visual regression:** insta + image-compare.  
**CI/quality:** cargo-clippy, cargo-fmt, cargo-deny, cargo-audit, cargo-llvm-cov, cargo-machete.  
**Error reporting:** color-eyre.  
**Profiling:** pprof (in-process), flamegraph (system-level).

---

**End of document.** Last updated: May 2026.
