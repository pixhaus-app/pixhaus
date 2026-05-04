# AI verb plugin protocol

Bedrock spec **B5**. Defines the contract every Pixhaus AI verb implements
and the runtime that coordinates them. This is the highest-leverage spec
in the project: it gates all 14 built-in verb streams (S23–S36), the
verb runtime stream (S21), the backend adapter stream (S22), and the
plugin loader (S37).

The protocol lives in the `pixhaus-ai` crate under
[`pixhaus_ai::plugin`]. The reference implementation is the
[`EchoVerb`][echo] in `ai/src/plugin/echo.rs`.

[echo]: ../ai/src/plugin/echo.rs

## Goals

1. **One trait, every verb.** Built-in or plugin, local or backend-driven,
   verbs implement the same trait so the runtime treats them uniformly.
2. **Preview before commit.** Every verb produces a *preview* the user
   accepts or rejects. The runtime never mutates the project directly.
3. **Streaming where useful.** Verbs that produce progressive output
   stream events through a bounded channel; the UI renders in real time.
4. **Cancellable.** Long-running verbs honour a `CancellationToken`.
5. **Stateless runtime.** The runtime keeps a registry and an ID minter
   only — no per-preview mutable state. Previews are values the caller
   carries between invocation and commit.
6. **Effects, not edits.** Verbs return a list of [`VerbEffect`]s the
   undo system (S05) materialises into commands. The protocol is
   independent of the undo system; the two layers compose.

## Surface area

```
pixhaus_ai::plugin
├── verb::Verb                — the trait every verb implements
├── descriptor::VerbDescriptor — static metadata
│   ├── VerbId                 — stable identifier
│   ├── BackendCapabilities    — required inference capabilities
│   ├── CostEstimate           — pre-invocation cost / latency
│   └── EffectKind             — coarse classification
├── context::VerbContext       — read-only project snapshot
│   ├── PixelData              — inline pixel bytes + layout
│   ├── ReferenceImage         — visible reference layers
│   └── StyleReference         — palettes, sheets, trained models
├── inputs::VerbInputs         — JSON-shaped input payload
├── output::VerbOutput         — what the verb produced
│   ├── VerbEffect             — one unit of work to commit
│   ├── NewPixelBuffer         — pixel bytes the host registers
│   ├── ActualCost             — real cost reported back
│   └── CritiqueFinding        — read-only outputs (Critique verb)
├── progress::VerbProgress     — sender half of the events channel
│   ├── VerbProgressEvent      — Started / Step / PartialPixels / Cost / Log / Eta
│   └── LogLevel
├── preview::VerbPreview       — runtime-stamped preview
│   ├── PreviewId
│   ├── VerbCommit             — applied preview
│   └── VerbDiscard            — rejected preview
├── runtime::VerbRuntime       — registry + dispatcher
└── runtime::VerbInvocation    — per-call handle
```

## Lifecycle

```
host                                runtime                              verb
 │                                     │                                   │
 │ register(verb)                      │                                   │
 │────────────────────────────────────>│                                   │
 │                                     │                                   │
 │ invoke(id, ctx, inputs)             │                                   │
 │────────────────────────────────────>│ validate(inputs)                  │
 │                                     │──────────────────────────────────>│
 │                                     │                              Ok / Err
 │                                     │ spawn worker                      │
 │                                     │──────────────────────────────────>│
 │ <───── VerbInvocation ─────         │                                   │
 │                                     │                       progress.send(Started)
 │ next_progress() ─loop─              │ <─── VerbProgressEvent ─────      │
 │                                     │                       progress.step(...)
 │                                     │                                   │
 │ cancel()  (optional)                │                                   │
 │────────────────────────────────────>│ cancel.cancel()                   │
 │                                     │                       observes is_cancelled()
 │                                     │                                   │
 │ finish()                            │                       returns VerbOutput
 │────────────────────────────────────>│ <─────────────────────────────────┤
 │ <───── VerbPreview ─────────        │                                   │
 │                                     │                                   │
 │ commit(preview)  -or-               │                                   │
 │ discard(preview, reason)            │                                   │
 │────────────────────────────────────>│                                   │
 │ <───── VerbCommit / VerbDiscard ─   │                                   │
```

The runtime is stateless about previews: `VerbPreview` is a value the
caller holds between `finish` and `commit` / `discard`. Every preview-
tracking system that lived in the runtime had to handle "what if the
preview is dropped?" / "what if the dialog closes?". Handing the value
back avoids the whole class of bugs.

## The `Verb` trait

```rust
#[async_trait]
pub trait Verb: Send + Sync + 'static {
    fn descriptor(&self) -> &VerbDescriptor;

    fn validate(&self, _inputs: &VerbInputs) -> Result<(), VerbError> { Ok(()) }

    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput, VerbError>;
}
```

`async_trait` is used because the runtime stores `Arc<dyn Verb>`; native
async fn in trait can't be erased through `dyn`. The boxing cost is one
allocation per invocation, dominated by any useful verb's actual work.

### `descriptor`

Returns the [`VerbDescriptor`] that describes the verb statically. The
descriptor is cached on registration and read for every UI lookup, so
keep `descriptor()` cheap (return a cached field, don't recompute).

### `validate`

Checks input shape *before* invocation. Default is a no-op. Override
when:

- the schema's "required" set isn't representable in JSON Schema
- a value range needs cross-field validation (e.g. `frame_a < frame_b`)
- a referenced ID must exist (verbs read this from `ctx`, not `inputs`)

Validation runs synchronously. Anything that needs `ctx` belongs in
`invoke`.

### `invoke`

Runs the verb. The runtime spawns it on tokio:

- `ctx` is a read-only snapshot of the project. Mutate via effects, not
  via the snapshot.
- `inputs` is the verb's typed payload. Use `inputs.deserialize::<MyInputs>()`.
- `progress` is the sender half of a bounded `mpsc` channel. `send`
  applies backpressure when the channel is full; `try_send` returns
  immediately. `progress.is_discarded()` lets the verb skip building
  expensive payloads when nobody is listening.
- `cancel` is fired by the runtime on `VerbInvocation::cancel()`. Verbs
  that declare `cancellable: true` must observe it between expensive
  operations:

  ```rust
  if cancel.is_cancelled() { return Err(VerbError::Cancelled); }
  ```

  or in a `select!`:

  ```rust
  tokio::select! {
      biased;
      _ = cancel.cancelled() => return Err(VerbError::Cancelled),
      result = backend.run(payload) => result?,
  }
  ```

The return value is a [`VerbOutput`] describing the effects to commit on
accept.

## The descriptor

A verb's static metadata. Stable for the lifetime of the verb instance:

| Field | Purpose |
|---|---|
| `id` | Stable identifier (`pixhaus.builtin.echo`, `com.example.foo`). |
| `display_name` | Command-palette label. |
| `description` | One-sentence tooltip. |
| `version` | Semantic version of the implementation. |
| `required_capabilities` | Bitfield of [`BackendCapabilities`]. |
| `input_schema` | JSON Schema; the UI builds forms from it. |
| `output_schema` | Optional JSON Schema for non-standard `effects` shapes. |
| `output_kinds` | List of [`EffectKind`]s the verb may produce. |
| `cost_estimate` | Pre-invocation cost / latency expectations. |
| `streaming` | `true` if `invoke` emits progress events. |
| `cancellable` | `true` if the verb honours its `CancellationToken`. |
| `documentation_url` | Optional link to user-facing docs. |

### Backend capabilities

```rust
BackendCapabilities::TEXT_GENERATION
                  | BackendCapabilities::VISION_LANGUAGE
                  | BackendCapabilities::IMAGE_GENERATION
                  | BackendCapabilities::IMAGE_EDIT
                  | BackendCapabilities::IMAGE_INPAINT
                  | BackendCapabilities::FRAME_INTERPOLATION
                  | BackendCapabilities::POSE_ESTIMATION
                  | BackendCapabilities::SEGMENTATION
                  | BackendCapabilities::AUDIO_ANALYSIS
                  | BackendCapabilities::STYLE_TRAINING
                  | BackendCapabilities::TOOL_USE
                  | BackendCapabilities::EMBEDDINGS
                  | BackendCapabilities::VIEW_SYNTHESIS
```

Adding a capability is additive (new bit position). Repurposing or
removing one is a breaking change for every plugin that declared it.

The runtime checks the user's configured backend(s) against the verb's
required set before invocation; a missing capability surfaces as
[`VerbError::UnsupportedCapability`] rather than a vague backend error
later.

### Cost estimate vs. actual cost

`CostEstimate` is the verb author's best-effort guess. After invocation,
the verb returns an [`ActualCost`] inside the `VerbOutput` with the real
spend (USD cents, tokens, elapsed). The UI shows the estimate up front
("This may take 5–15s and cost \$0.02") and the actual cost after the
fact ("Took 7.2s, \$0.018"). Verbs that ran on the local CPU return
`ActualCost::free(elapsed)`.

## Context

The runtime hands every verb a [`VerbContext`]:

```rust
pub struct VerbContext {
    pub project: ProjectMetadata,
    pub sprite: Option<Sprite>,
    pub active_sprite: Option<SpriteId>,
    pub active_layer: Option<LayerId>,
    pub active_frame: Option<FrameIndex>,
    pub active_palette: Option<Palette>,
    pub selection: Option<Rect>,
    pub references: Vec<ReferenceImage>,
    pub style_refs: Vec<StyleReference>,
}
```

The full `Sprite` is included so verbs can read frames, layers,
palettes, and tilesets without round-tripping. Cloning a sprite is bytes
of structured data, not pixel buffers — those are referenced by
`PixelBufferId` and resolved by the host before context construction.

Helpers:

```rust
ctx.require_sprite()?;          // Sprite or VerbError::MissingContext
ctx.require_sprite_id()?;       // SpriteId or VerbError::MissingContext
ctx.require_active_layer()?;    // LayerId or VerbError::MissingContext
ctx.require_active_frame()?;    // FrameIndex or VerbError::MissingContext
```

## Inputs

`VerbInputs` is a thin wrapper around `serde_json::Value`. Verb authors
build a `#[derive(Serialize, Deserialize)]` struct and round-trip:

```rust
#[derive(Serialize, Deserialize)]
struct InbetweenInputs {
    frame_a: FrameIndex,
    frame_b: FrameIndex,
    n_intermediate: u8,
}

let inputs = VerbInputs::from_struct(&InbetweenInputs { ... })?;
// inside invoke:
let parsed: InbetweenInputs = inputs.deserialize()?;
```

The descriptor's `input_schema` describes the wire format; the UI uses
it to render a form. The runtime does not validate against the schema —
the verb's `validate` does.

## Outputs and effects

A verb returns a [`VerbOutput`]:

```rust
pub struct VerbOutput {
    pub summary: String,            // shown in the "Apply preview?" dialog
    pub effects: Vec<VerbEffect>,   // applied on commit, in order
    pub thumbnail: Option<PixelData>,
    pub actual_cost: ActualCost,
    pub notes: Vec<String>,         // warnings to surface alongside
}
```

`VerbEffect` is an `enum` of operations:

| Variant | Use |
|---|---|
| `AddLayer { sprite, layer, cels, pixel_buffers }` | New layer. Used by Echo, Extend, Variant, Sketch finishing. |
| `AddCels { sprite, cels, pixel_buffers }` | Cels on existing layers. Used by Inbetween, Continue. |
| `ReplaceCels { sprite, cels, pixel_buffers }` | Overwrite existing cels. Used by Cleanup. |
| `AddFrames { sprite, after, frames, cels, pixel_buffers }` | Append frames. Used by Continue, Motion-from-video. |
| `AddTag { sprite, tag }` | New frame tag. Used by Motion-from-video, Audio-driven timing. |
| `AddSlice { sprite, slice }` | Named region. |
| `AddPalette { sprite, palette }` | New palette. Used by Project style learning. |
| `AddTileset { sprite, tileset, pixel_buffers }` | New tileset (with optional inline atlas). Used by Tile, Tileset-from-description. |
| `Critique { findings }` | Read-only findings. Used by Critique. |
| `Custom { name, payload }` | Verb-specific. Used by Auto-mesh-deformation, anything novel. |

### Placeholder IDs

Verbs cannot mint real `LayerId` / `FrameIndex` / `PixelBufferId`
values: those come from the live editor state. Effects use *placeholder*
IDs that the host rewrites at commit:

- **Layers.** `AddLayer` carries one new layer; the host assigns a real
  `LayerId` and rewrites every cel in the effect.
- **Pixel buffers.** Effects that create buffers carry a parallel
  `Vec<NewPixelBuffer>`. Cels reference each buffer by its
  `placeholder` ID; the host registers the bytes and rewrites the
  reference.
- **Frames.** `AddFrames` places frames at indices relative to `after`;
  the host renumbers absolute indices on commit.

By convention, the first placeholder is `LayerId::new(0)` /
`PixelBufferId::new(0)`, increasing within the effect. The Echo verb
shows the simplest case (one layer + one buffer + one cel).

## Progress

Verbs send [`VerbProgressEvent`]s through a [`VerbProgress`] sender:

```rust
progress.send(VerbProgressEvent::Started { backend: Some("anthropic.claude".into()) }).await;
progress.step(Some(0.25), "encoding palette").await;
progress.step(Some(0.5),  "calling backend").await;
progress.send(VerbProgressEvent::PartialPixels { effect_index: 0, pixels: ... }).await;
progress.step(Some(1.0),  "snap to palette").await;
```

Channel capacity is `64` events; `send` applies backpressure when full,
which is correct — the alternative is silently dropping progress.

`VerbProgress::discard()` returns a sink handle whose `send`/`try_send`
succeed by dropping the event. Tests and callers that don't want
progress use this; verbs share the same trait surface uniformly.

## Cancellation

```rust
async fn invoke(
    &self,
    ctx: VerbContext,
    inputs: VerbInputs,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<VerbOutput, VerbError> {
    if cancel.is_cancelled() { return Err(VerbError::Cancelled); }
    let res = backend_call(&inputs).await?;
    if cancel.is_cancelled() { return Err(VerbError::Cancelled); }
    let snapped = snap_to_palette(res, ctx.active_palette.as_ref())?;
    Ok(snapped)
}
```

For `select!` over backend futures and the cancel token, use `biased;`
so the cancel branch wins ties — otherwise the runtime may resolve in
arrival order and lose cancels under load:

```rust
tokio::select! {
    biased;
    _ = cancel.cancelled() => Err(VerbError::Cancelled),
    res = backend.run(payload) => Ok(snap(res?)),
}
```

The runtime additionally guards against verbs that *return Ok* despite
the token having fired: if `cancel.is_cancelled()` after a successful
return, the runtime overrides the result with `Err(VerbError::Cancelled)`
to prevent a partial preview from sneaking onto the undo stack.

## Errors

`VerbError` is a closed enum; every variant is actionable on the UI:

| Variant | When |
|---|---|
| `NotFound(VerbId)` | No verb registered with that ID. |
| `AlreadyRegistered(VerbId)` | Duplicate register. |
| `Schema(String)` | Inputs failed validation. |
| `MissingContext(&'static str)` | `require_sprite` / `require_active_layer` etc. |
| `UnsupportedCapability { verb, capability }` | Configured backend lacks a needed capability. |
| `Cancelled` | Token fired before a preview was produced. |
| `NotCancellable(VerbId)` | Caller cancelled a verb that declared `cancellable: false`. |
| `Aborted(String)` | Worker panicked or was aborted by the executor. |
| `Backend(String)` | Catch-all for backend-side failures. |
| `Payload(serde_json::Error)` | (De)serialisation issue. |

## Worked example: the echo verb

The simplest end-to-end verb. Takes pixels in, returns them as a new
layer on the active sprite. No backend. Demonstrates: descriptor, input
deserialisation, context lookup, progress, cancellation, effect
production with placeholder IDs.

```rust
use pixhaus_ai::plugin::{
    BackendCapabilities, CostEstimate, EchoInputs, EchoVerb, EffectKind,
    PixelData, VerbContext, VerbInputs, VerbRuntime, ECHO_VERB_ID, VerbId,
};
use pixhaus_core::project::{FrameIndex, ProjectMetadata, SpriteId};

let runtime = VerbRuntime::new();
runtime.register(EchoVerb::new())?;

let mut ctx = VerbContext::empty(ProjectMetadata { /* … */ });
ctx.active_sprite = Some(SpriteId::new(1));
ctx.active_frame  = Some(FrameIndex::new(0));

let inputs = VerbInputs::from_struct(&EchoInputs {
    pixels: PixelData::rgba8(2, 2, vec![/* 16 RGBA bytes */]),
    layer_name: Some("Echo".into()),
})?;

let mut inv = runtime.invoke(&VerbId::new(ECHO_VERB_ID), ctx, inputs)?;

// Drain progress as the verb runs.
while let Some(event) = inv.next_progress().await { /* render */ }

let preview = inv.finish().await?;
let commit  = runtime.commit(preview);
// commit.effects is now the AddLayer effect ready for the undo stack.
```

The verb itself is ~100 lines: see `ai/src/plugin/echo.rs`.

## Mapping to the 14 built-in verbs

The protocol must be expressive enough to support every verb in
`docs/planning/work/streams.md` (S23–S36). The mapping:

| Verb | Stream | Required capabilities | Effect kinds |
|---|---|---|---|
| Inbetween | S23 | `IMAGE_GENERATION` + `FRAME_INTERPOLATION` | `AddCels` |
| Continue | S24 | `IMAGE_GENERATION` | `AddFrames` + `AddCels` |
| Extend | S25 | `IMAGE_GENERATION` + `VIEW_SYNTHESIS` | `AddLayer` |
| Variant | S26 | `IMAGE_EDIT` | `AddLayer` |
| Cleanup | S27 | (optional `VISION_LANGUAGE`) | `ReplaceCels` |
| Tile | S28 | `IMAGE_GENERATION` | `AddTileset` |
| Critique | S29 | `VISION_LANGUAGE` | `Critique` |
| Project style learning | S30 | `STYLE_TRAINING` | `Custom { name: "pixhaus.style.lora" }` |
| Conversational editing | S31 | `VISION_LANGUAGE` + `TOOL_USE` | many — verb plans a sequence |
| Motion-from-video | S32 | `POSE_ESTIMATION` + `VISION_LANGUAGE` | `AddTag` + `AddCels` |
| Auto-mesh-deformation | S33 | `SEGMENTATION` + `VIEW_SYNTHESIS` | `Custom { name: "pixhaus.deform.rig" }` |
| Audio-driven timing | S34 | `AUDIO_ANALYSIS` (+ optional `VISION_LANGUAGE` for lip-sync) | `AddTag` + `AddCels` |
| Tileset-from-description | S35 | `IMAGE_GENERATION` | `AddTileset` |
| Sketch finishing | S36 | `IMAGE_EDIT` | `AddLayer` |

`Custom` is the escape hatch for the two stream goals (style learning,
auto-mesh-deformation) whose outputs aren't natural sprite edits. The
host routes `Custom` effects to verb-specific handlers via the `name`
namespace.

## Threading

`Verb::invoke` runs on whichever runtime called it (tokio under Tauri).
The protocol does **not** blanket-wrap invocations in `spawn_blocking`
because the common case — backend-driven verbs — is I/O-bound. CPU-bound
verbs self-schedule:

```rust
async fn invoke(&self, ctx, inputs, progress, cancel) -> Result<VerbOutput> {
    let parsed: ClassicalCleanupInputs = inputs.deserialize()?;
    let frame_in = ctx.require_active_frame()?;

    let snapped = tokio::task::spawn_blocking(move || classical::snap_to_palette(...))
        .await
        .map_err(|e| VerbError::Aborted(e.to_string()))?;

    Ok(/* output */)
}
```

The convention: if the verb touches every pixel in a buffer, it
`spawn_blocking`s. Backend HTTP I/O stays on the reactor.

## Schema evolution

The protocol's own version is the workspace's `schema_version` (B2,
`pixhaus_core::project::SchemaVersion`). New fields on `VerbDescriptor`,
`VerbContext`, `VerbOutput` are additive (bump minor). Removing or
repurposing a field is a break (bump major). Plugin authors pin the
`pixhaus-ai` minor version in their `Cargo.toml`; majors require a
recompile.

## What this protocol is **not**

- **Not the verb runtime.** This crate ships the contract; S21 ships
  the runtime that bridges verbs to backends, manages API keys, sets
  fallback chains, and wires up cost tracking to the UI.
- **Not the backend layer.** S22 ships the Anthropic, OpenAI, Replicate,
  Ollama, ComfyUI, and Stability adapters that satisfy the capabilities
  declared here.
- **Not the plugin loader.** S37 ships the dynamic loader that
  instantiates `Box<dyn Verb>` from Lua / WASM plugin packages.
- **Not the undo system.** S05 ships the command pattern that turns
  `VerbCommit` into reversible operations.
- **Not the IPC catalog.** B4 / S13 expose `VerbDescriptor` and
  `VerbInvocation` to the UI through Tauri commands.

Each of those streams builds on B5 without modifying the protocol.

## References

- Bedrock spec: `docs/planning/work/bedrock.md#b5-ai-verb-plugin-protocol`
- Streams that consume this: S21, S22, S23–S36, S37
- Reference plugin: `ai/src/plugin/echo.rs`
- Integration test: `ai/tests/echo_lifecycle.rs`
- Workspace conventions: `.claude/skills/pixhaus-rust-conventions`
