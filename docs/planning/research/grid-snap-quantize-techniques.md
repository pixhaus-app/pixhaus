# Grid snapping and palette quantization for AI pixel art

## Why this exists

AI image models do not produce true pixel art. They produce something that *looks* like pixel art on first glance and falls apart on the second: pixel sizes drift across the canvas, the implied grid resolution wobbles, and colors land all over RGB space instead of a strict palette. Painting cleanup on top of this output by hand is what AI was supposed to remove. Post-processing it programmatically is the realistic path.

Sprite Fusion Pixel Snapper (https://github.com/Hugo-Dz/spritefusion-pixel-snapper, MIT, by Hugo Duprez) is the cleanest open-source attempt at that post-processing step we have found. It is a single Rust crate that takes a messy raster, quantizes its colors, detects the underlying grid, and resamples back to a crisp, palette-locked, grid-aligned image. A local checkout sits at `/Users/luismorales/project/pixhaus-app/spritefusion-pixel-snapper`.

This document is a port spec, not a port. It walks the seven techniques the source uses, gives pseudocode for each, names every tunable constant the original exposes, and points at the Pixhaus crate and module each piece should land in. Source-file line ranges are cited so an engineer porting any one technique can cross-check against the original without re-reading the whole crate.

The work this informs is concentrated in three Pixhaus streams: S02 (color and palette ops), S27 (verb: Cleanup), and S35 (verb: Tileset-from-description). S12 (animated export with per-frame palette) gets the quantizer for free.

## Pipeline at a glance

```
raw RGBA  ->  k-means++ quantize              (Technique 1)
          ->  Sobel-style gradient profiles   (Technique 2)
          ->  median-of-peak-spacings step    (Technique 3)
          ->  two-axis step reconciliation    (Technique 4)
          ->  elastic walker cuts             (Technique 5)
          ->  two-pass cross-axis stabilize   (Technique 6)
          ->  majority-vote downsample        (Technique 7)
          ->  crisp, grid-aligned, palettized PNG
```

Quantization runs first because all the downstream gradient and peak work is much cleaner on a quantized image than on raw AI output. Grid detection runs in two axes independently. The two axes then get reconciled because the two-axis aspect ratio of pixel cells in real pixel art is bounded — cells are square-ish. Resampling is mode-per-cell, not bilinear, because dithering is signal, not noise.

## Constants table

Every magic number the original exposes. The original groups them into a `Config` struct; mirror that shape so the Pixhaus version is tunable without touching call sites.

| Constant                  | Default | Source line  | Role                                                  |
| ------------------------- | ------- | ------------ | ----------------------------------------------------- |
| k (palette size)          | 16      | main.rs:42   | Number of colors after quantization                   |
| kmeans_seed               | 42      | main.rs:43   | Deterministic RNG seed for kmeans++ init              |
| max_kmeans_iterations     | 15      | main.rs:46   | Hard cap on kmeans refinement                         |
| kmeans_convergence_eps    | 0.01    | main.rs:386  | Squared centroid movement threshold for early exit    |
| peak_threshold_multiplier | 0.2     | main.rs:49   | Profile-relative gate for peak survivors              |
| peak_distance_filter      | 4       | main.rs:50   | Minimum pixel separation between accepted peaks       |
| max_step_ratio            | 1.8     | main.rs:54   | Allowed x/y step skew before collapse                 |
| fallback_target_segments  | 64      | main.rs:530  | Cells-per-side fallback when no peaks survive         |
| search_window_ratio       | 0.35    | main.rs:49   | Walker search radius as fraction of step              |
| min_search_window         | 2.0     | main.rs:50   | Absolute floor on walker search radius (pixels)       |
| walker_strength_threshold | 0.5     | main.rs:51   | Peak must exceed mean times this value to snap        |
| min_required_cuts         | 4       | main.rs:52   | Stabilizer's "is this axis valid" floor               |

A note on the original defaults: they are tuned for AI-generated pixel art roughly in the 256x256 to 1024x1024 range with implied grid cells of 4 to 32 pixels. They are not universal. Anything outside that envelope (32x32 sprite, 4096x4096 mural) will want different values. Expose the Config struct on every public entry point so the Cleanup verb can override per-call.

## Technique 1: K-means++ color quantization

Original: `quantize_image()`, main.rs:276-414.

Quantization runs first for two reasons. First, the gradient detection downstream is noticeably cleaner on a quantized image — a strong color boundary on a 16-color image is unambiguous, whereas the same boundary on a 24-bit-color AI image bleeds through six near-identical shades. Second, the user wants a palette-locked output anyway; doing it once up front is cheaper than doing it after resampling.

Approach: strip fully transparent pixels (they are noise for clustering), pick k initial centroids with kmeans++ weighted sampling (first centroid uniformly random with a deterministic seed, each subsequent centroid drawn with probability proportional to its squared distance to the nearest existing centroid), iterate Lloyd's algorithm up to `max_kmeans_iterations`, early-exit when the maximum centroid movement falls below `kmeans_convergence_eps`. Replace each opaque pixel with its nearest centroid by squared Euclidean RGB distance. Leave transparent pixels alone.

Pseudocode:

```
quantize(image, k, seed):
    opaque = [p for p in image if p.a > 0]
    centroids = kmeans_pp_init(opaque, k, seed)
    for _ in 0..max_kmeans_iterations:
        assignments = [argmin_i dist2(p, centroids[i]) for p in opaque]
        new_centroids = mean_per_cluster(opaque, assignments, k)
        if max(dist2(new, old) for new, old in zip(new_centroids, centroids)) < kmeans_convergence_eps:
            break
        centroids = new_centroids
    return image.map(p:
        if p.a == 0 then p
        else with_rgb(p, centroids[argmin_i dist2(p, centroids[i])]))

kmeans_pp_init(points, k, seed):
    rng = ChaCha8.from_seed(seed)
    centroids = [points[rng.gen_range(0..len(points))]]
    while len(centroids) < k:
        weights = [min_i dist2(p, centroids[i]) for p in points]
        centroids.push(points[weighted_sample(weights, rng)])
    return centroids
```

Pixhaus landing: `core/src/color/quantize.rs`. Define a `Quantizer` trait with `fn quantize(&self, image: &PixelBuffer<Rgba8>, k: usize) -> PixelBuffer<Rgba8>`. Provide a `KMeansQuantizer` impl. The trait shape leaves room for Median-Cut, Wu's quantizer, or octree later without changing call sites. Wire the quantizer into S02 (palette ops use it on the "extract palette from selection" command), S12 (animated GIF/WebP per-frame palette pass), and S27 (Cleanup verb's snap-to-palette sub-step).

Pixhaus deltas from the original:
- Route through whatever RNG `core/` standardizes on. If `core/` has no opinion yet, `rand_chacha::ChaCha8Rng` with a seeded constructor is the right choice for determinism across platforms.
- No `unwrap()` on `argmin`/`mean_per_cluster` edge cases. Empty clusters return `Result::Err(QuantizeError::EmptyCluster)`; the caller decides whether to re-seed or fall back to k-1 colors.
- Tests: rstest with a fixture of synthetic palettes (3-color triangle, 8-color ramp, 256-color noise). Proptest for idempotence — `quantize(quantize(x, k), k) == quantize(x, k)` for any quantized output. Insta snapshot on the canonical 3-color synthetic. image-compare against a checked-in fixture for a real pixel-art sample.

## Technique 2: Gradient profiling for grid detection

Original: `compute_profiles()`, main.rs:416-456.

Now that the image has discrete color regions, the boundaries between regions carry the grid signal. The original collapses the 2D edge problem into two 1D problems by projecting gradient magnitude onto each axis. This is cheap, cache-friendly, and holds up well on noisy AI input.

Approach: convert the quantized RGBA image to a luma scalar field via BT.601 weights (`0.299 R + 0.587 G + 0.114 B`), treating fully transparent pixels as zero. Apply a vertical `[-1, 0, 1]` kernel and sum the absolute differences down each column — that is `col_proj[x]`, a "how much vertical edge sits in column x" score. Apply the same kernel horizontally and sum across each row to get `row_proj[y]`. No normalization; absolute values throughout.

Pseudocode:

```
profiles(image):
    let h, w = image.height, image.width
    luma = image.map(p:
        if p.a == 0 then 0.0
        else 0.299 * p.r + 0.587 * p.g + 0.114 * p.b)

    col_proj = [0.0; w]
    for x in 0..w:
        for y in 1..h-1:
            col_proj[x] += abs(luma[x, y+1] - luma[x, y-1])

    row_proj = [0.0; h]
    for y in 0..h:
        for x in 1..w-1:
            row_proj[y] += abs(luma[x+1, y] - luma[x-1, y])

    return (col_proj, row_proj)
```

Pixhaus landing: `core/src/grid/profile.rs`. Pure function over an immutable `PixelBuffer<Rgba8>`, returns `(Vec<f32>, Vec<f32>)`. Zero state, zero I/O. Easy SIMD target if profile generation ever shows up in a perf trace; pixel-art canvases are small enough that the scalar version is almost certainly fine.

Pixhaus deltas:
- Use the `PixelBuffer` indexing API; never raw `Vec<u8>` slicing.
- `f32` for the profile vectors. The original uses `f64`; we do not need that precision.
- Treat alpha as a binary mask the way the original does — transparent is zero luma. Soft alpha bleeding through this stage would corrupt the gradient signal.

## Technique 3: Median-of-peak-spacings step estimate

Original: `estimate_step_size()`, main.rs:458-500.

A 1D profile of gradient magnitude has peaks where the grid boundaries fall. The median of the spacings between consecutive peaks gives a stable estimate of the grid cell size. Median, not mean, because one missed or doubled peak will skew a mean but not a median.

Approach: find local maxima of the profile above a threshold of `peak_threshold_multiplier * max(profile)`. Filter out peaks that sit within `peak_distance_filter` pixels of another peak — these are usually sub-pixel artifacts of the gradient kernel itself. Compute consecutive spacings of surviving peaks; return the median. Return `None` if fewer than two peaks survive, since you cannot compute a spacing from one peak.

Pseudocode:

```
estimate_step(profile):
    if profile is empty: return None
    let max_val = max(profile)
    if max_val == 0.0: return None
    let threshold = max_val * peak_threshold_multiplier

    raw_peaks = [i for i in 1..len(profile)-1
                 if profile[i] > threshold
                 and profile[i] > profile[i-1]
                 and profile[i] > profile[i+1]]

    filtered = drop_close_neighbors(raw_peaks, peak_distance_filter)
    if len(filtered) < 2: return None

    spacings = [filtered[i+1] - filtered[i] for i in 0..len(filtered)-1]
    return Some(median(spacings))
```

Pixhaus landing: `core/src/grid/step.rs`. Returns `Option<f32>` per axis. Callers handle `None` via Technique 4's reconciliation step.

Failure mode worth documenting: AI outputs with very smooth shading have flat profiles, no clear peaks, and `estimate_step` returns either `None` or garbage. The reconciliation step covers `None`. Garbage is the cross-axis stabilizer's problem. Neither layer surfaces the failure to the user — see the open questions below for whether we should change that.

## Technique 4: Two-axis step reconciliation

Original: `resolve_step_sizes()`, main.rs:502-535.

A real pixel-art image has roughly square cells. If the x-axis estimator says cells are 8 pixels wide and the y-axis estimator says 80 pixels tall, one of them is wrong. The cheap heuristic the original uses: if both axes return estimates, compare them. If their ratio exceeds `max_step_ratio` (1.8), collapse both to the smaller — the smaller estimate is the conservative bet, since you can downscale further later but you cannot recover detail that has been averaged away. Otherwise, average the two. If only one axis returns an estimate, use it for both. If neither does, fall back to `min(width, height) / fallback_target_segments`.

Pseudocode:

```
resolve(x_step, y_step, override, width, height):
    if override is Some(o): return (o, o)
    match (x_step, y_step):
        (Some(x), Some(y)):
            let big = max(x, y)
            let small = min(x, y)
            if big / small > max_step_ratio:
                return (small, small)
            else:
                let avg = (x + y) / 2.0
                return (avg, avg)
        (Some(x), None): return (x, x)
        (None, Some(y)): return (y, y)
        (None, None):
            let fallback = min(width, height) / fallback_target_segments
            return (fallback, fallback)
```

This encodes a prior — pixel cells are square-ish — and that prior is correct for the use case. Live with it. If a future caller wants non-square cells (rare but possible for some isometric work), expose an opt-out on the Config struct.

Pixhaus landing: same file as Technique 3, `core/src/grid/step.rs`.

## Technique 5: Elastic walker for cut placement

Original: `walk()`, main.rs:610-656.

Now that we know the approximate cell size, we need actual cut positions. A naive approach would be to lay down cuts every `step_size` pixels starting from zero. That fails when the grid is offset (the AI did not start the first cell at x=0) or when the cell size drifts slightly across the canvas (it does, often). The walker handles both: it lays cuts down at the target stride, but at each target it searches a small window for the strongest local gradient and snaps to that peak if it is strong enough. Elastic but bounded — the walker cannot drift more than the search window per step, so it cannot run off into noise.

Approach: compute a search window as `max(step_size * search_window_ratio, min_search_window)`. Starting at position 0, place cuts at increments of `step_size`. At each target, find the argmax of the profile within `target +/- window`. If that maximum exceeds `mean(profile) * walker_strength_threshold`, place the cut at the peak; otherwise place it at the uniform target position.

Pseudocode:

```
walk(profile, step_size):
    let window = max(step_size * search_window_ratio, min_search_window)
    let mean_val = mean(profile)
    let strength_threshold = mean_val * walker_strength_threshold

    cuts = []
    let mut target = 0.0
    while target <= len(profile) as f32:
        let lo = max(0, (target - window) as usize)
        let hi = min(len(profile), (target + window) as usize)
        let (local_max_idx, local_max_val) = argmax(profile[lo..hi])

        if local_max_val > strength_threshold:
            cuts.push(lo + local_max_idx)
        else:
            cuts.push(target as usize)

        target += step_size
    return cuts
```

The walker is the part of the algorithm that gives the output its "snappy" feel — without it, you get either a perfectly uniform grid that ignores the source (bad if the source actually has structure) or a peak-greedy grid that drifts unboundedly (catastrophic on noisy inputs). The fixed-stride + bounded search compromise is the right one.

Pixhaus landing: `core/src/grid/walk.rs`.

## Technique 6: Two-pass cross-axis stabilization

Original: `stabilize_both_axes()` + `stabilize_cuts()`, main.rs:537-702.

This is the cleverest piece in the source crate, and the one most worth porting carefully. The problem: a single axis can fail in isolation. The profile might have spurious peaks, the walker might lock onto a noise pattern, the step estimate might be off. The only outside sanity check available is the perpendicular axis. The two-pass stabilizer uses exactly that.

Pass 1 stabilizes each axis on its own: count cuts (must be at least `min_required_cuts`), check the ratio of largest to smallest spacing along the axis (must not exceed `max_step_ratio`). If either check fails, regenerate the axis with `snap_uniform_cuts` — uniform stride, search-window snapping at each step, no median-based estimate. This recovers from local walker failures.

Pass 2 compares the two now-stabilized axes against each other. Take the median cell size on each axis. If their ratio exceeds `max_step_ratio`, one of the axes is producing skewed cells even after per-axis stabilization. Force both axes to uniform snapping at the smaller of the two cell sizes. The smaller wins for the same reason as Technique 4 — you can always downscale more later, but you cannot recover detail you have averaged away.

Pseudocode:

```
stabilize_both(col_profile, row_profile, step_x, step_y):
    let mut cuts_x = stabilize(walk(col_profile, step_x), col_profile, step_x)
    let mut cuts_y = stabilize(walk(row_profile, step_y), row_profile, step_y)

    let cell_x = median_diff(&cuts_x)
    let cell_y = median_diff(&cuts_y)

    let big = max(cell_x, cell_y)
    let small = min(cell_x, cell_y)
    if big / small > max_step_ratio:
        let shared = small
        cuts_x = snap_uniform(col_profile, shared)
        cuts_y = snap_uniform(row_profile, shared)

    return (cuts_x, cuts_y)

stabilize(cuts, profile, step):
    if len(cuts) >= min_required_cuts and step_ratio(&cuts) <= max_step_ratio:
        return cuts
    return snap_uniform(profile, step)

snap_uniform(profile, target_step):
    let num_cells = round(len(profile) / target_step)
    let actual_step = len(profile) / num_cells
    let window = max(actual_step * search_window_ratio, min_search_window)
    cuts = []
    for i in 0..=num_cells:
        let target = i * actual_step
        let lo = max(0, target - window)
        let hi = min(len(profile), target + window)
        let (idx, val) = argmax(profile[lo..hi])
        if val > mean(profile) * walker_strength_threshold:
            cuts.push(lo + idx)
        else:
            cuts.push(target)
    enforce_monotonic_increasing(&mut cuts)
    return cuts
```

Pixhaus landing: `core/src/grid/stabilize.rs`. Pulls in `walk()`, `snap_uniform()`, and the step-ratio sanity checks. The function the Cleanup verb actually calls is `stabilize_both`.

## Technique 7: Majority-vote downsampling

Original: `resample()`, main.rs:817-870.

The output image has dimensions `(len(col_cuts) - 1, len(row_cuts) - 1)` — one pixel per detected cell. The cell colors come from majority voting on the source pixels that fell inside each cell. Bilinear or any other averaging scheme would destroy dithering, which is a deliberate signal in pixel art, not noise. Majority vote preserves it: the dominant color of a dithered cell is the dominant color, and that is what ends up in the output.

Approach: for each output cell, walk the source pixels inside the corresponding source rectangle, count color frequencies in a small map, and pick the mode. Lexicographic RGBA order is the tiebreak — arbitrary but deterministic. Empty cells (no source pixels inside the rectangle, which only happens for degenerate cut sets) default to transparent.

Pseudocode:

```
resample(image, col_cuts, row_cuts):
    let w_out = len(col_cuts) - 1
    let h_out = len(row_cuts) - 1
    let mut out = PixelBuffer::transparent(w_out, h_out)

    for cy in 0..h_out:
        for cx in 0..w_out:
            let x0, x1 = col_cuts[cx], col_cuts[cx + 1]
            let y0, y1 = row_cuts[cy], row_cuts[cy + 1]

            let mut counts: Map<Rgba, usize> = empty
            for y in y0..y1:
                for x in x0..x1:
                    counts[image[x, y]] += 1

            out[cx, cy] = counts
                .iter()
                .max_by((color, count): (count, lex_order(color)))
                .map(|(color, _)| *color)
                .unwrap_or(Rgba::TRANSPARENT)

    return out
```

Pixhaus landing: `core/src/scale/majority_vote.rs`. Add a sibling `nearest.rs` while we are at it — nearest-neighbor is the natural counterpart for upscaling, and both belong in `core/src/scale/`.

## Pixhaus crate and module map

```
core/src/
  color/
    quantize.rs        <- Technique 1 (S02, S12, S27)
  grid/
    mod.rs             <- re-export
    profile.rs         <- Technique 2 (new submodule)
    step.rs            <- Techniques 3 + 4
    walk.rs            <- Technique 5
    stabilize.rs       <- Technique 6
  scale/
    majority_vote.rs   <- Technique 7

ai/src/verbs/
  cleanup.rs                       <- S27, composes the full pipeline
  tileset_from_description.rs      <- S35, calls grid::* post-generation

licenses/
  spritefusion-pixel-snapper-LICENSE.txt   <- verbatim MIT text from upstream
```

The Cleanup verb is the natural consumer of the full pipeline as a single user-facing operation. The Tileset-from-description verb only needs `grid::*` and `scale::majority_vote` — it does its own palette work earlier in the chain.

## What to leave behind

Five things in the original that should not survive a port into Pixhaus:

1. **Single-file 871-line layout**. Split per the module map above. Each technique gets its own file. Each file gets its own tests next to it.
2. **Zero tests**. The original ships with no unit or integration tests. Anything we port comes with rstest cases for the happy path, proptest for the quantizer's idempotence and the walker's bounded-drift property, insta snapshots for small synthetic inputs, and image-compare snapshots for the full pipeline against a checked-in fixture set of real AI pixel-art outputs. Per `.claude/skills/pixhaus-testing-conventions`.
3. **WASM target**. The original supports a `wasm-bindgen` build for browser use. Pixhaus runs Tauri + native Rust; the WASM path is dead weight for us.
4. **`image` 0.24 as a new dep**. Reuse whatever `core/` already pulls in for raster I/O. Do not introduce a parallel image stack.
5. **Direct `rand`/`rand_chacha`/`rand_distr` deps**. Route through whatever RNG abstraction `core/` already exposes. If `core/` has none, pick one consistent RNG crate at port time and use it everywhere — splitting the dependency across multiple `rand` minor versions has bitten this team before.

## License and attribution mechanics

The upstream project is MIT-licensed. That permits use, modification, sublicensing, and integration into MIT/Apache/proprietary projects, provided the copyright notice and license text are preserved. Concretely, at port time:

1. Add `licenses/spritefusion-pixel-snapper-LICENSE.txt` containing the verbatim MIT text from the upstream repo (the `LICENSE` file at https://github.com/Hugo-Dz/spritefusion-pixel-snapper).
2. At the top of each ported file, add a short header:
   ```
   // Portions of this file are derived from Sprite Fusion Pixel Snapper
   // (https://github.com/Hugo-Dz/spritefusion-pixel-snapper),
   // Copyright (c) Hugo Duprez, MIT-licensed.
   // See licenses/spritefusion-pixel-snapper-LICENSE.txt.
   ```
3. If Pixhaus grows a root `NOTICE` file, add an entry there pointing at the same upstream.
4. Reference the upstream repo URL in the PR description for the port.

These obligations are unambiguous for verbatim ports. For clean-room reimplementations driven from this document the strict legal requirement is weaker — we are reading a spec, not copying code — but the attribution stays. It costs us nothing, it is the right thing to do, and it preserves the trail so future maintainers know where to look when an upstream fix lands.

## Open questions

Four things this doc does not decide. Each maps to a planning surface that should pick them up.

1. **Cleanup verb composition.** Does S27 expose the pipeline as a single `cleanup` verb, or as three independent sub-verbs (`quantize`, `snap-to-grid`, `downsample`) that the user can compose? The verb protocol spec (B5) is currently silent on composition primitives. A single verb is simpler to invoke from the UI; sub-verbs are more useful for power-user scripting. The Cleanup stream brief should pick one.
2. **S35 reuse.** Tileset-from-description generates tilesets that drift in cell size across the output. Re-snapping them with `grid::*` after generation would tighten output quality at low cost. Worth a one-paragraph note in the S35 brief.
3. **Smooth-gradient failure mode.** Technique 3 returns `None` or garbage on flat profiles; Technique 4 silently invents a fallback step size. This is the right behavior for batch processing but the wrong behavior for an interactive verb where the user deserves to know "I could not detect a grid in this image." Pick: adaptive percentile-based threshold, or surface `CleanupError::GridNotDetected` and let the UI prompt for a manual `--pixel-size`.
4. **GPU offload.** Pixhaus uses `wgpu` for compositing. The profile vectors are tiny (single-pass over a 2D buffer, axis-separable), so CPU is almost certainly fine through 4096x4096. Quantization on GPU is a different story — k-means on GPU is a known win for large images. Defer until we see a perf trace that demands it.

## References

- Upstream source: https://github.com/Hugo-Dz/spritefusion-pixel-snapper
- Local checkout for cross-referencing: `/Users/luismorales/project/pixhaus-app/spritefusion-pixel-snapper`
- `docs/planning/work/streams.md` — S02 (color/palette ops), S12 (animated export), S27 (Cleanup verb), S35 (Tileset-from-description)
- `docs/planning/work/bedrock.md` — B2 (data model, `PixelBuffer`), B5 (verb protocol)
- `.claude/skills/pixhaus-rust-conventions` — error handling, no unwrap, async rules
- `.claude/skills/pixhaus-testing-conventions` — rstest, proptest, insta, image-compare
