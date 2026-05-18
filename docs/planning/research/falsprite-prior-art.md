# FalSprite prior art — patterns worth borrowing

## Attribution

This note examines an external project for ideas worth adopting in Pixhaus.

- **Upstream:** https://github.com/lovisdotio/falsprite
- **Author:** lovisdotio
- **License:** MIT, declared in upstream `README.md`. No `LICENSE` file is
  present in the upstream repo at the observed commit, so any future code
  or prompt lift carries the MIT text verbatim plus the copyright line
  `Copyright (c) 2026 lovisdotio` in its attribution header.
- **Commit observed:** `10107b38c01df342f5b5eb9b3fd5bded32dcbfa6`
  (2026-02-26).

MIT and Pixhaus's MIT are compatible. Adoption requires preserving the
copyright notice and license text in any file that ports code or prompt
content. This PR adopts nothing yet — it sets up the catalog. The first
code or content port introduces a project-wide `NOTICES.md` (or
equivalent) via the planning-doc revision path that `.conclaude.yaml`
expects for new top-level paths, and each ported file carries a header
comment that points at that file.

## Why this note exists

FalSprite solves a narrow problem — "generate a grid-shaped sprite-sheet
animation from a text prompt" — that overlaps two Pixhaus surfaces: the
AI verb runtime (B5 / S21 / S23–S36) and the animation export
pipeline (S11). It does so with a small, readable codebase (~1.4k LOC,
vanilla JS frontend + Node serverless backend) that lets each pattern
stand on its own without framework noise.

Pixhaus is well past FalSprite in infrastructure: the verb protocol is
real, the backend adapter trait is real, the `FalBackend` adapter is
already wired with queue submission, SSE streaming, cancellation, and
LoRA training. FalSprite's value is therefore not the wiring — it's
**creative content** (two specific system prompts that direct an LLM to
produce useful grid-shape animation choreography), a **verb shape** we
don't have today (animated sprite-sheet-from-prompt with grid + action
selection), and a small set of **UI patterns** for frame playback and
GIF export that map onto S19 (timeline) and S11 (GIF export).

The rest of this doc inventories what's worth borrowing, what's not, and
which Pixhaus streams each item lands in.

## What FalSprite is

A single-purpose web app:

- **Frontend:** vanilla JS (no framework, no bundler) at `public/app.js`
  + `public/styles.css`. State lives in one `state` object; DOM
  references are cached once.
- **Backend:** Node HTTP server (`server.mjs`) for local dev plus
  Vercel-style serverless functions (`api/generate.mjs`, `api/upload.mjs`,
  `api/fal/media.mjs`). Both paths share `lib/fal.mjs`.
- **AI pipeline:** three chained fal.ai calls.
  1. LLM rewrite via OpenRouter (GPT-4o-mini) — turns a short user
     description into CHARACTER + CHOREOGRAPHY directions.
  2. Image generation via `fal-ai/nano-banana-2`
     (or `fal-ai/nano-banana-pro/edit` when a reference image is
     supplied) — outputs one PNG containing a `g×g` grid of equally-sized
     cells.
  3. Background removal via `fal-ai/bria/background/remove` — outputs
     the transparent variant.
- **Frontend playback:** the generated PNG is loaded, frames extracted
  via row-major grid math, displayed via `requestAnimationFrame` with
  configurable FPS. GIF export uses `gif.js` with two web workers.
- **Deployment:** Vercel, with 300s function timeout and permissive
  CORS. Single fal API key powers all three services.

There are no automated tests, no CI, no plugin surface, and no
persistence beyond `localStorage` for the user's API key. The codebase
runs about 1.4k LOC. The README declares MIT but ships no `LICENSE`
file. The implementation is direct and readable; that's the appeal.

## What Pixhaus already has — don't relitigate

Several patterns the upstream demonstrates are already implemented in
Pixhaus. Skip these; they're listed so the catalog doesn't double back
on them.

- **`InferenceBackend` trait + fallback chain.** Defined in
  `ai/src/plugin/backend.rs`. Backends advertise capabilities, the
  runtime ANDs them against the verb's required set, fallback chain is
  priority-ordered.
- **fal.ai adapter with queue submit + SSE streaming + cancellation.**
  `ai/src/backends/fal.rs` already does what `lib/fal.mjs`'s
  `runQueuedModel` does — and more. It prefers SSE
  (`.../endpoint/stream`) when available, falls back to queue submit
  + status stream + result fetch when SSE returns 404, and cancels via
  `tokio_util::sync::CancellationToken` plumbed through `select!`. The
  upstream's 1.8-second polling loop is the conceptual ancestor of
  what's in `call_queue_image_endpoint`, but Pixhaus already moved past
  fixed-interval polling to event-driven streams.
- **Reference image conditioning.** `ai/src/backends/fal.rs`
  (`build_fal_generation_body`, `build_fal_edit_body`) already accepts
  reference images and converts them to data URIs. The "graceful
  degradation" pattern (fall back to text-only when upload fails) is
  *not* explicitly modeled at the request type, but the `Result` flow
  and progress events make it expressible.
- **Verb protocol with preview-then-commit, streaming, cancellation,
  cost estimates.** `ai/src/plugin/verb.rs` + `runtime.rs` +
  `descriptor.rs` cover this end-to-end.
- **Generate-reference-sheet verb with composition templates.**
  `ai/src/verbs/reference_sheet/` ships Character / Item / Tileset /
  Custom layouts. This handles *static* multi-view sheets — a different
  job than FalSprite's animated grid.

## High-value patterns worth borrowing

Five patterns rank high. Each gets a sketch of the Rust or TS shape it
takes in Pixhaus and a stream pointer. Pseudocode is illustrative; real
ports happen in the streams it maps to and follow the conventions in
that stream's existing code.

### 1. CHARACTER × CHOREOGRAPHY system-prompt split

**What FalSprite does.** Two distinct system prompts run in sequence
against an LLM. The first (`buildRewriteSystemPrompt` in
`lib/fal.mjs:234-255`) instructs the model to return exactly two
sections — CHARACTER (visual description) and CHOREOGRAPHY
(`g`-beat motion loop with weight-shift and leg-alternation rules) —
and forbids numbers, grid-language, and any mention of the underlying
generation. The second
(`buildSpritePrompt` in `lib/fal.mjs:198-232`) wraps the rewritten text
in strict technical requirements (FORMAT, FORBIDDEN, CONSISTENCY,
ANIMATION FLOW, MOTION QUALITY) that the image model sees alongside the
character/choreography description.

This split — *concept-shaping prompt for the LLM* + *constraint-laden
prompt for the image model* — is the reason FalSprite's outputs feel
coherent across cells without an animation-aware backbone model. It is
the most directly useful artifact in the project, and it is
creative content under copyright. Adopt verbatim with attribution.

**Pixhaus mapping.** A new verb stream (proposed below as **S-NEW.1
"Animated sprite sheet from prompt"**) under `ai/src/verbs/`. The
prompts live as `include_str!` text assets in the verb's directory so
they are MIT-attributed verbatim:

```rust
// ai/src/verbs/animated_sprite_sheet/prompts.rs
//
// CHARACTER and CHOREOGRAPHY system prompts adapted from FalSprite
// (https://github.com/lovisdotio/falsprite), MIT-licensed.
// Copyright (c) 2026 lovisdotio. See NOTICES for full license text.

pub const REWRITE_SYSTEM_PROMPT_TEMPLATE: &str =
    include_str!("prompts/rewrite_system.txt");

pub const SPRITE_CONSTRAINT_TEMPLATE: &str =
    include_str!("prompts/sprite_constraints.txt");

/// Substitutes the grid-size word ("two", "three", ... "six") into a
/// prompt template. FalSprite uses string interpolation; we keep the
/// same word-substitution scheme so the prompts read identically.
pub fn fill_grid_word(template: &str, grid_size: u8) -> String {
    let word = match grid_size {
        2 => "two", 3 => "three", 4 => "four",
        5 => "five", 6 => "six", _ => "four",
    };
    template.replace("{grid_word}", word)
}
```

The two `.txt` files carry the same per-file MIT attribution header in
a leading comment block (or, since `.txt` has no comment syntax, a
sibling `LICENSE.txt` in the directory naming the upstream).

**Variations to try once it lands.** The fixed leg-alternation rule for
locomotion (`lib/fal.mjs:225-227`) and the no-numbers rule
(`lib/fal.mjs:247`) are FalSprite-specific design calls. Some Pixhaus
art styles (chibi, isometric, sidescroll) may want different
constraints. Make the prompt a template, not a constant, and let
project style learning (S30) tune it over time.

### 2. Strict technical-requirements scaffold for grid-shaped output

**What FalSprite does.** `buildSpritePrompt` (`lib/fal.mjs:198-232`)
prefaces the user-rewritten character/choreography description with a
hierarchical block of constraints: FORMAT (exact grid dimensions),
FORBIDDEN (no text, no UI), CONSISTENCY (same character, same camera,
same scale), ANIMATION FLOW (left-to-right, top-to-bottom reading
order, last cell loops to first), MOTION QUALITY (weight shift,
follow-through, no repeated poses). This is the contract the image
model is held to.

The same scaffold structure is useful for other grid-shaped Pixhaus
verbs: tilesets, turnarounds, multi-direction views, expression sets.
Each verb specializes the constraints to its domain but keeps the
structure.

**Pixhaus mapping.** A shared helper in `ai/src/plugin/` that builds a
labelled-section prompt scaffold, plus per-verb specializations:

```rust
// ai/src/plugin/prompt_scaffold.rs

pub struct PromptScaffold {
    pub sections: Vec<(&'static str, String)>,
}

impl PromptScaffold {
    /// Renders to the upstream's all-caps-heading prose form. Backends
    /// that prefer structured input (JSON, message arrays) can choose
    /// instead to send `sections` directly.
    pub fn render(&self, payload: &str) -> String {
        let mut out = String::new();
        for (heading, body) in &self.sections {
            out.push_str(heading);
            out.push_str(":\n");
            out.push_str(body);
            out.push_str("\n\n");
        }
        out.push_str(payload);
        out
    }
}

// In ai/src/verbs/animated_sprite_sheet/mod.rs:
fn animated_sheet_scaffold(grid_size: u8) -> PromptScaffold {
    // Section bodies adapted verbatim from FalSprite buildSpritePrompt
    // (lib/fal.mjs:198-232). See attribution header.
    PromptScaffold {
        sections: vec![
            ("STRICT TECHNICAL REQUIREMENTS", format::format_for_grid(grid_size)),
            ("FORBIDDEN", FORBIDDEN_TEXT.into()),
            ("CONSISTENCY", CONSISTENCY_TEXT.into()),
            ("ANIMATION FLOW", animation_flow_text(grid_size)),
            ("MOTION QUALITY", MOTION_QUALITY_TEXT.into()),
            ("CHARACTER AND ANIMATION DIRECTION", String::new()),
        ],
    }
}
```

The same scaffold (different section bodies) extends naturally to
S35 (Tileset-from-description) and existing reference-sheet templates.

### 3. Row-major frame grid math + `requestAnimationFrame` playback with FPS gating

**What FalSprite does.** `public/app.js:469-527` extracts and animates
frames from a grid-packed PNG. The math is trivial but worth lifting
verbatim because the *invariants* matter: cells are read row-major
(`col = id % g; row = floor(id / g)`), cell pixel size is
`floor(image.width / g)` (so non-divisible widths floor-truncate
rather than round), the canvas explicitly sets
`imageSmoothingEnabled = false` for pixel-perfect upscaling, and the
RAF tick gates redraws on `timestamp - lastTick >= 1000 / fps` so the
frame rate is decoupled from the monitor refresh.

These are exactly the invariants Pixhaus's timeline preview needs.

**Pixhaus mapping.** A pure TS module in `ui/src/timeline/` (for the
timeline panel) and reused inside `ui/src/canvas/` (for the viewport's
own playback overlay). Solid primitives handle the lifecycle:

```ts
// ui/src/timeline/frame-grid.ts
//
// Adapted from FalSprite public/app.js:469-527 (MIT, lovisdotio).

export type GridCoord = { col: number; row: number };

export function frameCoord(frameId: number, gridSize: number): GridCoord {
  return { col: frameId % gridSize, row: Math.floor(frameId / gridSize) };
}

export function cellSize(image: { width: number; height: number }, gridSize: number) {
  return {
    w: Math.floor(image.width / gridSize),
    h: Math.floor(image.height / gridSize),
  };
}

// ui/src/timeline/use-animation-loop.ts

import { createSignal, onCleanup } from "solid-js";

export function useAnimationLoop(
  fps: () => number,
  playing: () => boolean,
  onTick: (frameIndex: number) => void,
  frameCount: () => number,
) {
  const [index, setIndex] = createSignal(0);
  let last = 0;
  let rafId = 0;

  const tick = (now: number) => {
    if (playing()) {
      const interval = 1000 / Math.max(1, fps());
      if (now - last >= interval) {
        last = now;
        const next = (index() + 1) % Math.max(1, frameCount());
        setIndex(next);
        onTick(next);
      }
    }
    rafId = requestAnimationFrame(tick);
  };

  rafId = requestAnimationFrame(tick);
  onCleanup(() => cancelAnimationFrame(rafId));
  return index;
}
```

The actual draw call sets `ctx.imageSmoothingEnabled = false` once when
the canvas mounts. This is non-negotiable for pixel art and is the kind
of detail easy to forget.

### 4. Worker-pool GIF export with per-frame canvas composition

**What FalSprite does.** `public/app.js:576-634` composes a GIF in the
browser. Each animation frame is drawn onto a single reusable canvas
(`fillStyle = backdrop; drawImage(...)`) and fed to `gif.js` configured
with `workers: 2`, `quality: 8`. The `gif.js` worker offloads encoding
off the main thread; the result is an `image/gif` `Blob` that's served
as an Object URL. For server-side batch jobs, `batch-generate.mjs`
swaps `gif.js` for `gifenc` (pure Node, no DOM dependency).

The pattern — encode off-thread, compose each frame as a canvas blit,
use the same code for preview and download — is the right shape for
Pixhaus's animated export.

**Pixhaus mapping.** Two-sided:

- **Browser GIF export** lives in `ui/src/canvas/export/` and uses a
  Web Worker. The TS interface is straightforward:

```ts
// ui/src/canvas/export/gif-export.worker.ts
//
// Worker-pool composition adapted from FalSprite
// public/app.js:576-634 (MIT, lovisdotio).

import GIF from "gif.js"; // or a maintained fork

type Frame = { pixels: ImageData; durationMs: number };

self.addEventListener("message", (event) => {
  const { frames, sizePx } = event.data as { frames: Frame[]; sizePx: number };
  const gif = new GIF({ workers: 2, quality: 8, width: sizePx, height: sizePx });
  for (const frame of frames) {
    gif.addFrame(frame.pixels, { copy: true, delay: frame.durationMs });
  }
  gif.on("finished", (blob: Blob) => self.postMessage({ blob }, [blob as unknown as ArrayBuffer]));
  gif.render();
});
```

- **Native (Rust) GIF writer** in `io/src/animated/` for S11. The `image`
  crate's `GifEncoder` is the obvious choice, but the *frame composition
  loop* is what gets borrowed — render each frame to an RGBA buffer
  (re-using a scratch buffer), apply palette quantization or dither
  according to user choice, push to the encoder. Pseudocode:

```rust
// io/src/animated/gif.rs (sketch)
//
// Frame composition pattern adapted from FalSprite
// batch-generate.mjs (server-side gifenc usage), MIT, lovisdotio.

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame};
use std::time::Duration;

pub fn encode_gif(
    frames: impl IntoIterator<Item = (image::RgbaImage, Duration)>,
    repeat: Repeat,
    writer: impl std::io::Write,
) -> io::Result<()> {
    let mut encoder = GifEncoder::new(writer);
    encoder.set_repeat(repeat)?;
    for (pixels, duration) in frames {
        let delay = Delay::from_saturating_duration(duration);
        encoder.encode_frame(Frame::from_parts(pixels, 0, 0, delay))?;
    }
    Ok(())
}
```

S11's brief calls out Floyd–Steinberg and ordered Bayer 8×8 dithering;
those go into the per-frame composition step alongside palette
quantization. FalSprite skips Pixhaus's palette concerns entirely — its
output is RGBA — so this is a Pixhaus-specific addition.

### 5. Per-row action selection with grid-row constraint

**What FalSprite does.** The user picks `g` (grid size, 2–6) and an
ordered list of "actions" — `idle`, `walk`, `run`, `attack`, `cast`,
`jump`, `dance`, `death`, `dodge`, or a custom string. The UI
constrains `len(actions) <= g` (one action per row), and the action
list feeds into the choreography prompt as a sequence of motion beats.
This product framing is the difference between "generate a sprite
sheet" and "generate a sheet with the moves I actually want."

**Pixhaus mapping.** The verb's input schema (`VerbInputs`) takes a
`grid_size: u8` and a `Vec<Action>` (or `Vec<String>` for free-form
strings). The chip-multi-select UI lives in `ui/src/verbs/` (a verb
invocation surface S21/S22's UI hooks should expose) with a constraint:

```ts
// ui/src/verbs/animated-sprite-sheet/AnimatedSpriteSheetForm.tsx (sketch)

import { createSignal, For, Show } from "solid-js";

const PRESETS = ["idle", "walk", "run", "attack", "cast", "jump", "dance"];

export function AnimatedSpriteSheetForm(props: { onSubmit: (i: Inputs) => void }) {
  const [grid, setGrid] = createSignal<2 | 3 | 4 | 5 | 6>(4);
  const [actions, setActions] = createSignal<string[]>([]);

  const toggle = (a: string) => {
    const picked = actions();
    if (picked.includes(a)) {
      setActions(picked.filter((x) => x !== a));
    } else if (picked.length < grid()) {
      setActions([...picked, a]);
    }
    // else: silently refuse — surface the constraint visually instead
    // of throwing, mirroring FalSprite's UX.
  };

  return (
    <form onSubmit={(e) => { e.preventDefault(); props.onSubmit({ grid: grid(), actions: actions() }); }}>
      {/* grid picker, prompt textarea, chip grid for actions */}
    </form>
  );
}

type Inputs = { grid: 2 | 3 | 4 | 5 | 6; actions: string[] };
```

The "silently refuse extra picks" behaviour is worth keeping —
FalSprite flashes the accent colour briefly when the user exceeds the
limit, which is friendlier than disabling or throwing.

## Medium-value patterns

These are worth noting but don't need a verbatim port.

- **Non-fatal warnings array propagated server → UI.** FalSprite's
  `api/generate.mjs:39` pushes strings into a `warnings` array when a
  non-critical stage fails (rewrite skipped, BG removal skipped) and
  returns them alongside the successful result. Pixhaus's
  `VerbProgressEvent::Log { level: LogLevel::Warn, message }` already
  carries this shape; the equivalent at verb-completion time is a
  `Vec<String>` on the `VerbOutput` or on a custom payload field.
  Confirm the existing channel covers this before adding anything.

- **History strip with LRU eviction + lazy GIF thumbnails.** Per
  `public/app.js:637-696`, keeps the last 16 generations. Thumbnails
  are 240×240 animated GIFs (10 fps) computed lazily; if GIF generation
  fails, falls back to a static PNG. Maps to a future verb-history
  panel in `ui/src/verbs/` (no existing stream yet); the LRU + fallback
  pattern is reusable.

- **Constraint-aware UI control.** The action chip's "max picks = grid
  rows" plus the visual flash on excess picks. Maps to any verb form in
  `ui/src/verbs/`.

- **Single-file shared API helpers.** `lib/fal.mjs` is consumed by both
  the local dev server (`server.mjs`) and the serverless handlers
  (`api/*.mjs`), so neither path drifts. Pixhaus doesn't have this
  duplication problem (single Tauri app, no dual-host serverless), but
  the principle — adapters live in one place, callers don't reimplement
  — is already followed in `ai/src/backends/`.

## Anti-patterns to avoid

- **Duplicated entry points.** FalSprite has near-identical
  `handleGenerate` logic in both `server.mjs` and `api/generate.mjs`.
  Pixhaus avoids this by virtue of being a single binary, but if/when
  CLI tooling lands (e.g. `pixhaus batch-generate`), the verb runtime
  should be the single dispatcher, not a parallel CLI path.

- **Depth-first JSON scan to extract LLM output.** `extractRewrittenPrompt`
  in `lib/fal.mjs:126-156` walks the response payload looking for any
  string field that looks promising. It works but rots the moment a
  provider shifts the schema. Pixhaus already uses `serde` for typed
  responses (`FalQueueSubmitResponse`, `FalTrainingResponse`); keep
  doing that.

- **Unbounded `fetch` in worker code.** Several places use `fetch(...)`
  with no abort signal. In Pixhaus, every backend call goes through
  `tokio::select!` with `cancel.cancelled()` and a reqwest client built
  with a `Duration::from_secs(180)` timeout (see
  `ai/src/backends/fal.rs:62-68`). Keep that contract.

- **localStorage for API keys.** FalSprite stores the fal API key in
  the browser's localStorage. Pixhaus uses the OS keychain via
  `keyring` (`ai/src/backends/keys.rs`); don't backslide.

- **Zero automated tests.** FalSprite ships none. Pixhaus's testing
  conventions (rstest, proptest, insta, image-compare, mockall —
  see `.claude/skills/pixhaus-testing-conventions/`) apply to every
  ported pattern.

## Mapping to Pixhaus streams

| Pattern | Pixhaus surface | Stream(s) | Bedrock touchpoint |
| --- | --- | --- | --- |
| CHARACTER × CHOREOGRAPHY prompts | `ai/src/verbs/animated_sprite_sheet/` (new) | new verb (proposed S-NEW.1) | B5 (protocol — verb implements `Verb` trait) |
| Technical-requirements scaffold | `ai/src/plugin/prompt_scaffold.rs` (new) + per-verb specializations | new shared helper; consumed by S25, S26, S35, S-NEW.1 | B5 |
| Frame grid math + RAF playback | `ui/src/timeline/`, `ui/src/canvas/` | S14, S19 | — |
| GIF export with worker pool | `ui/src/canvas/export/`, `io/src/animated/` | S11 + UI surface in S13 | B6 (Unity handoff already covers spritesheet+JSON; GIF is an alternate export path) |
| Per-row action selection UI | `ui/src/verbs/animated-sprite-sheet/` (new) | S-NEW.1 (verb-invocation UI surface) | B5 |
| Non-fatal warnings | `VerbProgressEvent::Log` (existing) + verb output | S21 (already exists) | B5 |

## Follow-up tasks

These are suggested, not enqueued. They go into `work/queue.md` only
after the verb-protocol-tied items have a clear interface to point at.

1. **Propose a new verb stream: "Animated sprite sheet from prompt"**
   — Adds an entry to `docs/planning/work/streams.md` between S35 and
   S36. The verb produces a single PNG output containing a `g×g` grid
   of frames plus a transparent variant. UI: grid size dropdown, action
   chip multi-select with the grid-row constraint, optional reference
   image upload. Lifts FalSprite's two prompts as `.txt` assets with
   attribution. Not blocked.

2. **Add `ai/src/plugin/prompt_scaffold.rs`** — the labelled-section
   prompt builder. Use it from the new verb in (1), and migrate
   `iterate_reference_sheet` / `reference_sheet` to use it where their
   current prompt construction could be tightened. Not blocked.

3. **Build the Solid frame-grid playback primitive in `ui/src/timeline/`**
   — pure TS module + `useAnimationLoop` Solid primitive. Used by S19
   timeline preview and the new verb's preview surface. Not blocked.

4. **Add GIF export to `io/src/animated/`** — Rust-side encoder for
   batch / CLI / programmatic use, with palette quantization and
   Floyd–Steinberg + Bayer 8×8 dithering. UI side gets a Web Worker
   wrapper in `ui/src/canvas/export/`. Maps onto S11. Not blocked.

5. **Introduce a project-wide attribution surface** — when (1) or any
   other code-or-content lift lands, add `NOTICES.md` (or
   `docs/THIRD-PARTY.md`, or a per-crate `THIRD_PARTY.md` — pick during
   the PR) via the planning-doc revision path that
   `.conclaude.yaml` requires for new top-level paths. Until then,
   attribution lives in this research note. Blocked by: the first real
   port deciding the canonical location.

## Attribution policy for future code or content lifts

When a future PR ports code or prompt content from FalSprite:

- The lifted file carries a header comment naming the upstream, the
  upstream URL, the upstream's MIT license, and the
  `Copyright (c) 2026 lovisdotio` line. For `.txt` assets, the same
  text goes in a sibling `LICENSE.txt` in the asset directory.
- The attribution surface introduced in follow-up task (5) above gains
  a "FalSprite" entry with the affected Pixhaus paths appended.
- If the port is structural rather than verbatim (e.g. the polling
  loop was *inspired by* FalSprite but rewritten against tokio +
  reqwest), the header reads "Adapted from" rather than "Copied from"
  and the rewrite is explicit about which invariants were preserved.

MIT does not require a NOTICES file at the repo root, only that the
copyright notice and license text travel with the code. Inline headers
plus a per-project attribution registry satisfy that. The exact
location of the registry is a planning-doc decision the first lift PR
makes — this PR does not lock it in.
