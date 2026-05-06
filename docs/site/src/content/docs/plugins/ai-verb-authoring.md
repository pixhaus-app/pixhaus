---
title: AI verb authoring
description: Write AI verbs that call inference backends, stream progress, and produce pixel previews.
---

import { Aside } from "@astrojs/starlight/components";

AI verbs are the plugin system's most powerful extension point. A verb:

1. Receives a `VerbContext` — a snapshot of the project state.
2. Optionally calls one or more inference backends (image generation, VLM, local model).
3. Returns a list of effects (new layers, modified pixels, frame timing).
4. Streams progress events back to the editor during a long run.
5. Produces a preview the user can accept or reject before anything is committed.

This page covers authoring verbs from Lua. For the Rust/WASM approach (which gives you direct access to the full verb protocol), see [WASM plugins](/plugins/wasm-plugins/) plus the reference implementation at `ai/src/plugin/echo.rs`.

---

## Minimal verb

```lua
app.ai.registerVerb {
  id          = "com.example.my-verb",
  label       = "My Verb",
  description = "One-line description shown in the AI menu.",
  cost        = { credits = 0 },

  run = function(ctx)
    return {
      summary = "Did nothing",
      effects = {},
    }
  end,
}
```

`run()` is the only required field beyond the identity metadata.

---

## VerbContext fields

`ctx` is a read-only snapshot. Do not store references across frames; context snapshots may be garbage-collected once `run()` returns.

| Field | Type | Description |
|---|---|---|
| `ctx.sprite` | Sprite or nil | Active sprite at invocation time. |
| `ctx.activeLayer` | Layer or nil | Layer the user had selected. |
| `ctx.activeFrame` | Frame or nil | Frame the user had selected. |
| `ctx.palette` | Palette | The project palette. |
| `ctx.selection` | Selection or nil | Current selection mask, or nil if nothing selected. |
| `ctx.referenceImages` | array of Image | Layers the user pinned as references. |
| `ctx.styleReference` | StyleRef or nil | Trained style model, if one exists for this project. |

To access pixel data for a specific layer and frame:

```lua
local cel = ctx.activeLayer:cel(ctx.activeFrame)
if cel then
  local img = cel.image   -- Image (read-only snapshot)
  -- img.width, img.height, img:getPixel(x, y)
end
```

---

## VerbEffect types

`run()` returns a table with a `summary` string and an `effects` array. Each effect describes one unit of work the runtime will commit on accept.

### `replace_cel`

Replace the pixels in an existing cel:

```lua
{
  kind   = "replace_cel",
  layer  = ctx.activeLayer,
  frame  = ctx.activeFrame,
  pixels = modified_image,     -- Image object
}
```

### `add_layer`

Add a new layer above the current active layer:

```lua
{
  kind   = "add_layer",
  name   = "AI result",
  pixels = generated_image,    -- Image object
  frame  = ctx.activeFrame,
}
```

### `set_frame_duration`

Change the duration of one or more frames:

```lua
{
  kind     = "set_frame_duration",
  frame    = ctx.activeFrame,
  duration = 120,               -- milliseconds
}
```

### `add_frames`

Insert new frames after the current active frame:

```lua
{
  kind   = "add_frames",
  after  = ctx.activeFrame,
  count  = 4,
  pixels = { img1, img2, img3, img4 },  -- one per new frame
}
```

---

## Calling inference backends

Use `app.ai.backend` to invoke an inference backend. Pixhaus routes the call to whichever backend the user configured (Anthropic, OpenAI, Replicate, Ollama, etc.) for the requested capability.

### Image generation

```lua
run = function(ctx)
  local result = app.ai.backend.generateImage {
    prompt  = "A knight character sprite, pixel art style, 32x32",
    width   = 32,
    height  = 32,
    palette = ctx.palette,   -- snap output to this palette
  }

  if result.error then
    return { error = result.error }
  end

  return {
    summary = "Generated knight sprite",
    effects = {
      {
        kind   = "add_layer",
        name   = "Generated",
        pixels = result.image,
        frame  = ctx.activeFrame,
      }
    },
    actual_cost = result.cost,
  }
end,
```

### Image editing (inpainting)

```lua
run = function(ctx)
  local cel = ctx.activeLayer:cel(ctx.activeFrame)
  if not cel then return { error = "No active cel." } end

  local result = app.ai.backend.editImage {
    source  = cel.image,
    mask    = ctx.selection,    -- edit only within the selection
    prompt  = "Add a scar across the left cheek",
    palette = ctx.palette,
  }

  if result.error then return { error = result.error } end

  return {
    summary = "Applied inpainting",
    effects = {
      { kind = "replace_cel", layer = ctx.activeLayer,
        frame = ctx.activeFrame, pixels = result.image }
    },
    actual_cost = result.cost,
  }
end,
```

### Vision-language analysis (VLM)

```lua
run = function(ctx)
  local cel = ctx.activeLayer:cel(ctx.activeFrame)
  if not cel then return { error = "No active cel." } end

  local response = app.ai.backend.visionQuery {
    image  = cel.image,
    prompt = "List any inconsistencies in the pixel art — off-palette colors, broken outlines, asymmetric details.",
  }

  if response.error then return { error = response.error } end

  -- Return a critique (read-only; no effects).
  return {
    summary  = "Critique complete",
    effects  = {},
    critique = response.text,   -- displayed in the critique panel
    actual_cost = response.cost,
  }
end,
```

---

## Streaming progress

Long-running verbs can stream progress events so the UI shows a live status.

```lua
run = function(ctx, progress)
  progress.started()
  progress.step(0.1, "sending request")

  local result = app.ai.backend.generateImage {
    prompt = "Forest tileset, top-down, 16x16 tiles",
    width  = 256,
    height = 128,
    on_partial = function(partial_image, fraction)
      -- Called as the backend streams partial results.
      progress.partial(partial_image, fraction)
    end,
  }

  progress.step(1.0, "done")

  if result.error then return { error = result.error } end

  return {
    summary = "Generated tileset",
    effects = {
      { kind = "add_layer", name = "Tileset draft",
        pixels = result.image, frame = ctx.activeFrame }
    },
    actual_cost = result.cost,
  }
end,
```

The `progress` object is only available if your registration includes `streaming = true`:

```lua
app.ai.registerVerb {
  id        = "com.example.tileset-gen",
  label     = "Tileset: Generate",
  streaming = true,
  run       = function(ctx, progress) ... end,
}
```

### Progress methods

| Call | Description |
|---|---|
| `progress.started()` | Mark the verb as started. Shows a spinner in the AI panel. |
| `progress.step(fraction, message)` | Update progress bar (0.0–1.0) and status text. |
| `progress.partial(image, fraction)` | Send a partial pixel result for live preview. |
| `progress.log(message)` | Append a line to the verb's log (visible in the AI console). |

---

## Cancellation

A verb that calls a slow backend should honour the cancellation token:

```lua
run = function(ctx, progress, cancel)
  progress.started()

  -- Check before each expensive operation.
  if cancel.is_cancelled() then return { cancelled = true } end

  local result = app.ai.backend.generateImage {
    prompt  = "...",
    cancel  = cancel,     -- pass the token so the backend can abort early
  }

  if result.cancelled then return { cancelled = true } end
  if result.error     then return { error = result.error } end

  return { summary = "Generated", effects = { ... } }
end,
```

Pass `cancellable = true` when registering:

```lua
app.ai.registerVerb {
  id          = "com.example.gen",
  label       = "Generate",
  cancellable = true,
  run         = function(ctx, progress, cancel) ... end,
}
```

The user can cancel via the **X** button in the AI progress panel. The runtime sets the token; your verb is expected to observe it between expensive operations.

---

## Cost declaration

Declare the expected cost upfront so the AI menu can show the user what a verb will cost before they run it:

```lua
app.ai.registerVerb {
  id    = "com.example.generate",
  label = "Generate",

  -- Estimate shown before the verb runs.
  cost  = {
    credits = 5,          -- Pixhaus credit estimate (nil = unknown)
    latency = "15-30s",   -- human-readable time range
  },

  run = function(ctx)
    -- ... actual run ...
    return {
      summary     = "Generated",
      effects     = { ... },
      actual_cost = { credits = 4, latency_ms = 18200 },  -- reported back
    }
  end,
}
```

`actual_cost` in the return value is recorded in the AI cost log visible in **AI > Cost history**.

---

## Input schema

Verbs can declare a JSON schema for their inputs. The runtime uses this to build a settings UI when the user invokes the verb from the command palette.

```lua
app.ai.registerVerb {
  id    = "com.example.generate",
  label = "Generate",

  -- JSON Schema (draft-07) describing the expected inputs.
  input_schema = {
    type = "object",
    properties = {
      prompt  = { type = "string",  description = "What to generate." },
      frames  = { type = "integer", minimum = 1, maximum = 16, default = 4 },
    },
    required = { "prompt" },
  },

  run = function(ctx, progress, cancel, inputs)
    local prompt = inputs.prompt
    local frames = inputs.frames or 4
    -- ...
  end,
}
```

When an `input_schema` is declared, `run()` receives a fourth argument `inputs` — a table matching the schema, populated from the settings UI the user filled in.

---

## Testing a verb

The quickest way to test is to install the plugin and invoke it from the command palette. For automated tests you can also call a verb directly from a test script:

```lua
-- test-my-verb.lua  (run with `lua test-my-verb.lua` or via the Pixhaus script runner)
local verb = app.ai.findVerb("com.example.my-verb")
assert(verb, "verb not registered")

local ctx = app.ai.makeTestContext {
  spriteWidth  = 32,
  spriteHeight = 32,
  paletteSize  = 16,
}

local result = verb:runSync(ctx)
assert(result.effects, "expected effects")
assert(#result.effects > 0, "expected at least one effect")
print("ok")
```

---

## See also

- [Developing plugins](/plugins/developing/) — command and verb basics
- [WASM plugins](/plugins/wasm-plugins/) — Rust verbs with the full protocol
- [Verb protocol spec](/reference/verb-protocol/) — the underlying `pixhaus_ai::plugin` protocol
- `ai/src/plugin/echo.rs` — reference Rust implementation of the full protocol
