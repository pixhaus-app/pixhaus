---
name: pixhaus-testing-conventions
description: Use when writing or reviewing tests in Pixhaus — covers unit vs integration layout, rstest fixtures, proptest, insta snapshots, image-compare visual regression, mockall, wiremock, and nextest workflows
---

# Pixhaus testing conventions

How to write tests in this codebase. Every change produces tests; consistent
conventions make them composable across PRs.

## The rule

**Every public function has at least one test.** This is the floor. The
ceiling is "every behavior the function promises has a test that proves
it." Public surface shrinks the API; tests guard the contract.

## Where tests live

- **Unit tests** — inline `#[cfg(test)] mod tests` at the bottom of the
  module they test:

  ```rust
  // core/src/blend.rs
  pub fn blend_normal(src: Rgba, dst: Rgba) -> Rgba { ... }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn opaque_src_replaces_dst() {
          let src = Rgba::new(255, 0, 0, 255);
          let dst = Rgba::new(0, 0, 255, 255);
          assert_eq!(blend_normal(src, dst), src);
      }
  }
  ```

  Inline tests share visibility with the module — they can call private
  functions directly. That makes them ideal for testing implementation
  detail you don't want in the public API.

- **Integration tests** — files under `<crate>/tests/`:

  ```
  core/
  ├── src/
  └── tests/
      ├── pixel_buffer.rs
      └── undo_round_trip.rs
  ```

  Each file is a separate binary that links against the crate as an
  external consumer. Use these to test the *public* surface — they catch
  regressions in API shape that inline tests miss.

- **Doc tests** — examples in rustdoc that compile and run:

  ```rust
  /// Decodes a PNG byte buffer into a [`PixelBuffer`].
  ///
  /// ```
  /// # use pixhaus_io::png::decode_png;
  /// let bytes = std::fs::read("examples/sample.png")?;
  /// let buffer = decode_png(&bytes)?;
  /// assert_eq!(buffer.width(), 64);
  /// # Ok::<(), Box<dyn std::error::Error>>(())
  /// ```
  pub fn decode_png(bytes: &[u8]) -> Result<PixelBuffer> { ... }
  ```

  Doc tests double as documentation that can't lie — if the API changes,
  the example fails. Add one for any public function whose usage is
  non-obvious.

  Lines prefixed with `#` compile but stay hidden from the rendered docs —
  use them for imports, fixture setup, and the `Ok::<(), _>(())` wrapper so
  the visible example shows only what a reader cares about. Hidden lines are
  the lever that keeps an example both runnable and clean: a function that
  reads a file can still have an executing example if the hidden lines write
  a `tempfile` first, so prefer that over switching the example off. The
  fence also takes attributes that change how the block runs:

  - `no_run` — compile but don't execute. The fallback for an example that
    genuinely can't run in CI: it opens a window, needs a GPU surface, hits
    the network, or depends on a file the repo doesn't ship. Don't reach for
    it just because there's a side effect — if you can make the example
    self-contained with a hidden `tempfile` or in-memory fixture, a running
    example proves more than one that only compiles.
  - `should_panic` — the example is expected to panic; the test passes when
    it does.
  - `compile_fail` — the example is expected *not* to compile. Useful for
    showing that a misuse is caught at compile time (a sealed trait, a
    type-state guard).
  - `ignore` — skip entirely. Avoid it; if you only want formatting, mark
    the block `text` so it isn't a test at all.

  Doc tests run under `cargo test --doc`, not `cargo nextest run` — see the
  nextest section for why that split matters here.

## Unit test style

A test reads top-to-bottom: arrange, act, assert. Name the test for the
behavior it pins:

```rust
#[test]
fn out_of_bounds_pixel_returns_none() {
    let buf = PixelBuffer::new(8, 8);
    assert!(buf.pixel(100, 100).is_none());
}
```

Bad name: `test_pixel`. Good name: `out_of_bounds_pixel_returns_none`.

Test the smallest unit that proves the behavior. Don't reach into a
service to test a helper that has its own test; double-coverage adds noise
without value.

### Name tests like sentences

A good test name names three things so the test report reads as a spec
without opening the file:

- **the unit** — the function under test (`blend_normal`, `decode_png`)
- **the expected behavior** — what it does (`returns_none`, `clamps_alpha`)
- **the state that triggers it** — the case (`when_out_of_bounds`, `for_empty_input`)

`out_of_bounds_pixel_returns_none` carries all three. `test_pixel` carries
none — a failure tells you nothing until you read the body. Tests are the
first place someone looks to learn how a function is used, so the name is
documentation, not a label.

### Group a unit's tests in a nested module

When one function grows several tests, nest them in a module named for the
function. The module prefix shows up in the test report and gives editors a
single run button for the whole group:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    mod blend_normal {
        use super::*;

        // reported as `blend_normal::opaque_src_replaces_dst`
        #[test]
        fn opaque_src_replaces_dst() { /* ... */ }

        // reported as `blend_normal::transparent_src_keeps_dst`
        #[test]
        fn transparent_src_keeps_dst() { /* ... */ }
    }
}
```

The prefix replaces a repeated `blend_normal_` on every name. Reach for the
nested module once a unit has three or more tests; a single test reads fine
flat.

### One behavior per test

A test should prove one thing. When it fails, the name alone should tell you
what broke. Packing several behaviors into one body hides which assertion
fired and forces you to fix them one re-run at a time:

```rust
// Bad: a failure could be either line, and the name commits to neither.
#[test]
fn parse_works() {
    assert!(Glyph::parse("abcd").is_ok());
    assert!(Glyph::parse("ABCD").is_err());
}

// Good: each behavior is its own named test.
#[test]
fn lowercase_is_accepted() {
    assert!(Glyph::parse("abcd").is_ok());
}

#[test]
fn uppercase_is_rejected() {
    assert!(Glyph::parse("ABCD").is_err());
}
```

When the only thing that varies is the input, don't copy-paste bodies — use
an `rstest` case table with a descriptive label per case (see below). One
assertion per test is the target; more than one is a smell that you're
testing more than one behavior.

## How to assert

Two macros cover almost everything: `assert!` for a boolean, `assert_eq!`
for equality. Both take a trailing format string that prints only on
failure — use it to say what the actual value was, so a red test explains
itself instead of just saying "assertion failed":

```rust
assert!(value.is_ok(), "expected Ok, got {:?}", value.unwrap_err());
assert_eq!(got, want, "blend diverged by {}", got.r as i16 - want.r as i16);
```

The `is_ok()` / `is_err()` path is the one that bites: the default message
is a bare `false`, which tells you nothing. Always attach the error so the
failure carries the reason.

When you care that a value matches a *shape* but not its contents, pair
`assert!` with `matches!` instead of destructuring by hand:

```rust
assert!(
    matches!(err, DecodeError::BadHeader(_)),
    "expected BadHeader, got {err:?}"
);
```

For large `assert_eq!` comparisons, `pretty_assertions` renders a colored,
line-by-line diff in place of Rust's wall-of-text default. Import it in the
test module so the override is scoped to tests:

```rust
#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq; // shadows std's assert_eq! in this module
    use super::*;
    // ...
}
```

Use `#[should_panic]` only when a panic is the documented contract (an
assertion in a constructor, say). For ordinary failure paths, return a
`Result` and assert on the `Err` — a panic is harder to pin to a cause and
can't carry a typed error. When you do use it, match the message so the test
can't pass on the wrong panic: `#[should_panic(expected = "stride must be")]`.

Mark a test you've deliberately parked with `#[ignore = "reason"]`, never a
bare `#[ignore]`. The reason — ideally a tracking issue — is what keeps a
parked test from rotting into a permanently dead one.

## `rstest` for fixtures and parameterization

`rstest` lets multiple tests share setup and lets one test body run
across a parameter table:

```rust
use rstest::{fixture, rstest};

#[fixture]
fn small_buffer() -> PixelBuffer {
    PixelBuffer::filled(4, 4, Rgba::new(255, 255, 255, 255))
}

#[rstest]
fn fill_changes_every_pixel(mut small_buffer: PixelBuffer) {
    small_buffer.fill(Rgba::new(0, 0, 0, 255));
    for px in small_buffer.pixels() {
        assert_eq!(px, Rgba::new(0, 0, 0, 255));
    }
}

#[rstest]
#[case::opaque_src_wins(Rgba::new(255, 0, 0, 255), Rgba::new(0, 0, 0, 255), Rgba::new(255, 0, 0, 255))]
#[case::transparent_src_keeps_dst(Rgba::new(0, 0, 0, 0), Rgba::new(0, 0, 255, 255), Rgba::new(0, 0, 255, 255))]
#[case::half_alpha_blends_midpoint(Rgba::new(255, 255, 255, 128), Rgba::new(0, 0, 0, 255), Rgba::new(128, 128, 128, 255))]
fn blend_normal(#[case] src: Rgba, #[case] dst: Rgba, #[case] expected: Rgba) {
    assert_eq!(blend_normal_fn(src, dst), expected);
}
```

Label every case with `#[case::name(...)]`. The label becomes the test name
(`blend_normal::opaque_src_wins`), so a failing case reads like a sentence in
the report instead of `blend_normal::case_1`. Without labels, a case table
trades one unreadable name for several.

When the same fixture appears in 3+ tests, lift it. Don't reach for
fixtures for a single test — inline setup is clearer.

## `proptest` for property-based tests

Use when an invariant should hold across an input space too large to
enumerate:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn pixel_round_trips_through_indexed_palette(
        r in 0u8..=255,
        g in 0u8..=255,
        b in 0u8..=255,
    ) {
        let palette = Palette::standard_64();
        let rgba = Rgba::new(r, g, b, 255);
        let index = palette.nearest(rgba);
        let back = palette.color_at(index);
        // Quantization is lossy; the round-trip should land in the same bucket.
        prop_assert_eq!(palette.nearest(back), index);
    }

    #[test]
    fn blend_is_alpha_monotonic(
        a1 in 0u8..=255,
        a2 in 0u8..=255,
    ) {
        let src1 = Rgba::new(255, 255, 255, a1);
        let src2 = Rgba::new(255, 255, 255, a2);
        let dst = Rgba::new(0, 0, 0, 255);
        let r1 = blend_normal_fn(src1, dst);
        let r2 = blend_normal_fn(src2, dst);
        if a1 <= a2 {
            prop_assert!(r1.r <= r2.r);
        }
    }
}
```

When proptest finds a failing case, it shrinks to the minimal counterexample.
Save the regression seed to `proptest-regressions/` so it runs first next
time — the file is committed.

Don't use `proptest` for things `rstest` does better. Properties prove
invariants ("alpha is monotonic"); cases prove specific behaviors ("this
exact triple blends to that exact result").

## `insta` for snapshot tests

Use snapshots when a value's exact shape is hard to write inline but easy
to recognize:

```rust
use insta::assert_yaml_snapshot;

#[test]
fn project_serializes_to_stable_yaml() {
    let project = Project::sample();
    assert_yaml_snapshot!(project);
}
```

The first run writes `tests/snapshots/project__project_serializes_to_stable_yaml.snap`.
Subsequent runs compare against it. When you change the format on purpose:

```bash
cargo insta review
# inspect each pending snapshot, accept or reject
```

Never `mv` a `.snap.new` file to overwrite the baseline without reviewing
it. Snapshot drift defeats the point.

Snapshot what's stable: serialized data, debug formatting of complex
structures, command catalogs. Don't snapshot timestamps, random IDs,
or floating-point values that vary by platform — redact them with
`insta::with_settings` filters or use `assert_eq!` with explicit values.

## Visual regression with `image-compare`

Pixel-level rendering tests:

```rust
use image_compare::Algorithm;
use std::path::Path;

#[test]
fn brush_circle_8px_matches_baseline() {
    let actual = render_brush(Brush::circle(8), 32, 32);
    let baseline = image::open(Path::new("tests/snapshots/brush_circle_8px.png")).unwrap();

    let result = image_compare::rgba_hybrid_compare(&actual.into(), &baseline.into())
        .expect("compare failed");

    assert!(
        result.score >= 0.999,
        "brush render diverged: score = {}",
        result.score
    );
}
```

Baselines live in `<crate>/tests/snapshots/<test-name>.png`, committed.
When intentionally changing the renderer, regenerate:

```bash
PIXHAUS_UPDATE_SNAPSHOTS=1 cargo test -p pixhaus-core
```

Tag the regeneration in the PR description so the reviewer audits the
new images.

Compare scores: 1.0 is identical. `>=0.999` is "imperceptibly different"
(JPEG-style noise won't trip it). Don't lower the threshold to make a
flaky test pass; investigate the source of the variance instead.

## Mocking with `mockall`

The pattern: define a trait for the dependency, mock the trait, inject it.

```rust
use mockall::automock;

#[automock]
pub trait Backend: Send + Sync {
    fn name(&self) -> &str;
    async fn complete(&self, prompt: &str) -> Result<String>;
}

pub struct Verb<B: Backend> {
    backend: B,
}

impl<B: Backend> Verb<B> {
    pub async fn run(&self, prompt: &str) -> Result<String> {
        self.backend.complete(prompt).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verb_passes_prompt_to_backend() {
        let mut backend = MockBackend::new();
        backend
            .expect_complete()
            .with(mockall::predicate::eq("hello"))
            .times(1)
            .returning(|_| Ok("world".to_string()));

        let verb = Verb { backend };
        let out = verb.run("hello").await.unwrap();
        assert_eq!(out, "world");
    }
}
```

`#[automock]` generates `MockBackend` automatically. Use `.expect_xxx()`
to set expectations and `.returning(...)` to define behavior. `times(N)`
enforces call count.

Don't mock types you own and can construct cheaply — use the real thing.
Mock only at the boundaries: external services, slow I/O, time, randomness.

## HTTP mocking with `wiremock-rs`

For AI backend tests that exercise an HTTP client:

```rust
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path, header};

#[tokio::test]
async fn anthropic_backend_sends_api_key_header() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{"type": "text", "text": "hello"}]
        })))
        .mount(&server)
        .await;

    let backend = AnthropicBackend::with_base_url(&server.uri(), "test-key");
    let response = backend.complete("hi").await.unwrap();
    assert_eq!(response, "hello");
}
```

`MockServer` listens on a random local port. The mock matches request
shape; unmatched requests return 404, which the backend should surface
as an error. Use `.expect(1)` if you need to assert exactly one call:

```rust
Mock::given(method("POST"))
    .respond_with(ResponseTemplate::new(200))
    .expect(1)
    .mount(&server)
    .await;
```

Don't hit real APIs in tests. Even rate-limited "best-effort" hits cost
money and create flake. The CI environment never gets API keys.

## `cargo nextest` over `cargo test`

`nextest` runs tests in parallel processes (not just threads), with a
better progress indicator and JUnit-compatible output for CI:

```bash
cargo nextest run --workspace
```

Local feedback loop:

```bash
cargo nextest run -p pixhaus-core --no-fail-fast
```

Watch mode (with `cargo-watch`):

```bash
cargo watch -x 'nextest run --no-fail-fast'
```

Fall back to `cargo test` only when you need doc tests (nextest doesn't
run them) — run `cargo test --doc` separately.

CI uses both:

```yaml
- run: cargo nextest run --workspace
- run: cargo test --doc --workspace
```

## Local workflow

### Fast loop while iterating

```bash
# The post-edit hook formats and lints the affected crate as you save.
# Test only what you're working on:
cargo nextest run -p pixhaus-core test_name_substring
```

### Pre-PR sweep

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --doc --workspace
cargo deny check --config .cargo/deny.toml
```

If any of these are red, the PR isn't ready.

### Background watcher (recommended)

In a side terminal, leave running:

```bash
bacon
```

`bacon` is a Rust-aware watcher. The default jobs run check, clippy,
test, and doc — switch with the keystrokes shown in the bacon UI.

## Reviewer checklist for tests

When reviewing a PR:

- [ ] Every new public function has at least one test
- [ ] Tests are named for the behavior they pin, not for the function
- [ ] Each test proves one behavior; multi-input cases use an `rstest` table
- [ ] `is_ok()` / `is_err()` asserts carry a failure message with the actual value
- [ ] No bare `#[ignore]` — every parked test has `#[ignore = "reason"]`
- [ ] Snapshot files are committed and look reviewed (not auto-accepted)
- [ ] `proptest-regressions/` files for new properties are committed
- [ ] No real API calls — mock at the HTTP boundary with `wiremock`
- [ ] Tests don't depend on wall-clock time, network, or ordering between
      independent tests
- [ ] Visual regression baselines in `tests/snapshots/*.png` are checked in
- [ ] `cargo nextest run --workspace` is green in CI

## Anti-patterns

- **`#[test] fn it_works()`** — placeholder name. Replace before merge.
- **`assert!(x.is_ok())` with no message** — when it fails you get a bare
  `false` and have to re-run to see the error. Print the `Err`.
- **Several behaviors in one test** — `assert!(parse("a").is_ok());
  assert!(parse("A").is_err());` hides which half broke. Split them.
- **Asserting against the implementation** — `assert_eq!(state.cache.len(), 3)`
  ties tests to internal structure. Assert against observable behavior instead.
- **Sleep-based synchronization** — `thread::sleep(100ms)` to wait for an
  async result. Use `.await`, channels, or test-time clocks.
- **Test order dependence** — running test A first changes test B's outcome.
  Tests must pass in any order; nextest randomizes by default.
- **Disabled tests with no link** — `#[ignore]` without a comment pointing
  at a tracking issue is rot. Fix it or delete it.
- **Snapshots reviewed by Cmd-A then Accept** — if you accept changes
  without reading them, snapshots stop catching regressions.
