# Finding where the time goes on the 8192x8192 canvas

The smart move here is exactly what you're doing: measure before you touch anything. A 256x256 sprite is ~256 KB of RGBA; an 8192x8192 canvas is 256 MB. A 1000x size jump that produces lag almost always means some piece of work is scaling with *total canvas area* instead of with the area you actually painted. Your job before changing code is to prove which piece. Here's how to do that on Windows, cheapest tools first.

## Step 1: Frame the hypothesis before you profile

Brush drawing on a frame has roughly four cost centers. Write them down so you know what you're looking for:

1. **CPU pixel mutation** — writing the brush stamp into the layer's `Vec<u8>`.
2. **CPU->GPU upload** — pushing changed pixels to the GPU texture each frame (`queue.write_texture`).
3. **GPU work** — the wgpu render pass that draws the canvas texture into the viewport.
4. **egui/eframe overhead** — layout, tessellation, the paint callback plumbing.

The 1000x scaling tells you the culprit is something that touches every pixel or every byte of the canvas, not just the brush footprint. The two prime suspects are (1) re-stamping or re-scanning the whole buffer, and (2) re-uploading the entire 256 MB texture on every brush move. A full 256 MB upload per frame at 60 fps is ~15 GB/s of PCIe traffic — that alone will tank you. But *don't assume* — measure.

## Step 2: Always run a release build for perf numbers

Debug builds make pixel loops 10-50x slower and the ratio is not uniform, so debug timings lie about *where* the cost is. For any measurement:

```
cargo run --release
```

If you want symbols in the release profile for the profiler in step 5, add to the workspace `Cargo.toml`:

```toml
[profile.release]
debug = true        # line-level symbols, no speed cost
```

or use a dedicated `[profile.profiling]` that inherits release.

## Step 3: Cheap manual timing to localize the cost (do this first)

Before reaching for any profiler, drop coarse timers around the four cost centers in the brush/paint path. This takes ten minutes and usually points straight at the problem.

```rust
let t = std::time::Instant::now();
self.layer.stamp_brush(pos, &brush);          // cost center 1
let t_stamp = t.elapsed();

let t = std::time::Instant::now();
self.upload_dirty_region(&queue);             // cost center 2
let t_upload = t.elapsed();

log::debug!("stamp={:?} upload={:?}", t_stamp, t_upload);
```

Run it on the 256x256 and then the 8192x8192 and compare. The number that explodes by ~1000x between the two is your answer. Concretely:

- If `t_stamp` scales with canvas size: you're iterating the whole buffer per stroke instead of just the brush's bounding box.
- If `t_upload` scales with canvas size: you're re-uploading the full texture instead of just the dirty rectangle.
- If neither CPU number moves but frames are still slow: the cost is on the GPU or in egui repaint — go to steps 4 and 5.

Keep these timers scoped and behind `log::debug!` so they're easy to pull out later. This is throwaway instrumentation, not something to commit.

## Step 4: Confirm whether it's CPU-side or GPU-side / present-blocked

egui repaints, so first make sure you're actually comparing frame times, not idle time. Enable eframe's built-in frame timing or just time your whole `update()`:

```rust
let frame_start = std::time::Instant::now();
// ... all your update work ...
log::debug!("frame {:?}", frame_start.elapsed());
```

Two important things to separate:

- **Is the CPU frame time high, or is the CPU fast but the GPU/present is stalling?** If your `update()` returns in 1 ms but you still see lag, the cost is on the GPU or you're blocked on `present()`/vsync waiting for the GPU to finish a fat render pass or a huge upload. wgpu work is queued and runs asynchronously, so CPU-side timers around `write_texture`/`submit` measure *encoding* time, not *execution* time.
- **Is it lag (latency) or low throughput (fps)?** Watch the frame-time distribution, not just the average. A periodic spike every N frames looks different from a uniformly slow frame and points at different causes (e.g., an allocation/reupload on stroke-end vs. per-frame full work).

For the GPU side on Windows, the most direct tool is below.

## Step 5: Profile properly on Windows

Once the manual timers tell you which side it's on, confirm with a real profiler. On Windows, in rough order of effort:

**CPU sampling — `superluminal` or Windows Performance Analyzer (WPA / `xperf`).**
- Superluminal is the nicest paid option for Rust on Windows; it gives you a flame graph with real symbols.
- Free path: WPA from the Windows ADK, or use the `samply` crate (`cargo install samply`, then `samply record cargo run --release`) which opens a Firefox-profiler flame graph in the browser. Cross-platform and zero-config. This will show you instantly if `stamp_brush` or a `memcpy`-style upload dominates a hot frame.

**In-process frame profiling — `puffin`.**
- Add `puffin` + `puffin_egui` and wrap your paint path in `puffin::profile_scope!("stamp")` / `profile_scope!("upload")`. Since you're already in egui, `puffin_egui` gives you a live flame graph *inside the app* with no external tooling. This is the best fit for your stack: you can drag the brush and watch which scope balloons in real time. Strongly recommended for an egui app.

**GPU profiling — PIX on Windows or RenderDoc.**
- If step 3/4 says the CPU is idle and you're GPU/present-bound, capture a frame with PIX (Microsoft's D3D12 tool; wgpu uses DX12 on Windows by default) or RenderDoc. You'll see how long the canvas render pass takes and how big each `write_texture` copy is. This is where you'd confirm a "uploading 256 MB per frame" theory directly.
- `wgpu` also has timestamp queries (`Features::TIMESTAMP_QUERY`) if you want pass timings logged from inside the app without an external capture tool.

## What I'd actually do, in order

1. `cargo run --release` with `debug = true` in the release profile.
2. Add `puffin` + `puffin_egui` (or just the manual `Instant` timers from step 3) around stamp / upload / submit.
3. Drag the brush on the 256 and the 8192, read off which scope scales with canvas area.
4. If it's CPU, you've localized it. If the CPU scopes are flat but it's still laggy, capture a frame in PIX/RenderDoc to look at the render pass and texture-upload sizes.
5. *Only now* decide what to change — and the fix will almost certainly be "bound the per-stroke work to the dirty region" (both the pixel write and the texture upload), but let the measurement say so rather than guessing.

## Why not just start optimizing

Two reasons. First, the obvious fix (dirty-region uploads) might already be partly in place, and the real cost could be somewhere you didn't expect (e.g., egui re-tessellating, or a full-buffer clone in your undo path on every brush move). Second, without a baseline number you can't prove a change helped — you'll be flying blind on whether 8192 went from "laggy" to "fine." Get the per-frame breakdown first, then each change has a number to beat.
