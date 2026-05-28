# Native Create mode — raw clip review and chroma key

Status: proposed. Extends [`native-ui-animation-studio.md`](./native-ui-animation-studio.md)
(the Clip & loop stage) and references [`native-ui-animation-pipeline.md`](./native-ui-animation-pipeline.md).

The studio generates a clip and lets you mark a loop, but it never lets you just
*watch* the raw video. The frames you see in the Clip stage are decoded straight
from Seedance's mp4, but they sit behind a frame-by-frame scrubber built for
setting in/out points — not for judging the motion at speed. And the chroma key
that decides how the backdrop comes off — the magenta or green Seedance bakes in
— is selectable in the editor's timeline today but missing from the studio, so
you commit to a loop blind to how it will key.

This doc adds two things to the studio's Clip & loop stage: a real raw-clip
player, and key-color selection you can eyedrop and preview against that raw
clip, baked into the loop at Land.

## Why

- **You can't see the raw mp4.** The whole point of generating a clip is to look
  at it before you spend effort picking frames. The current scrubber steps one
  frame at a time and shares its track with the loop markers; there is no
  play-at-speed transport, no time readout, no clean way to watch the motion the
  model actually produced. The first thing the artist does after i2v — eyeball
  the raw result for drift, pivots, and artifacts — has no surface.
- **You can't choose the key in the studio.** Seedance returns a flat
  magenta/green backdrop, and the key color plus tolerance decide how cleanly it
  strips. The editor already exposes this (the timeline background-removal
  panel: a swatch, a border-sample Detect, an eyedrop-the-foreground path), but
  the studio doesn't. You pick a loop, land it, and only then discover the key
  was wrong.

## One override

The pipeline doc says background removal is a re-runnable timeline operation,
"not baked into the landing." This doc reverses that single decision for the
studio path: when a key color is set, **Land bakes the chroma key into
normalize**, so the loop arrives on the timeline already stripped. The timeline
operation still stands — it remains the way to re-run or fix removal later — but
the studio no longer makes you do it as a mandatory second step.

## What it reuses — and what it does not

This is a surface over parts that already exist. Nothing here needs a new decode
path, a new transport, or a new keying primitive.

Reuse from v2 (`pixhaus-worktrees/v2/shell/src/`), do not rebuild:

- **The decode.** `anim::decode_clip` (`anim.rs:62`) already turns the raw clip
  into `VideoFrame`s — mp4/H.264 via the `mp4` + `openh264` crates, gif/apng via
  `image`, no external ffmpeg. `VideoFrame` (`anim.rs:19`) carries tightly-packed
  RGBA plus width, height, and a timestamp. The player needs nothing the decode
  doesn't already hand it.
- **The clip candidate.** `ClipCandidate` (`app.rs:207`) keeps the raw `clip`
  bytes, the `mime`, the decoded `frames`, and a lazy per-frame `thumbs` cache.
  The raw video is already in memory; the player just plays it.
- **The transport.** `anim_clip_playing`, `anim_scrub`, `anim_play_mode`
  (`Clip` / `Loop` / `Picks`), `tick_clip_playback`, `current_play_indices`,
  `toggle_clip_play`, and `set_scrub` (`app.rs`) already cycle frames at the
  clip's fps from the frame loop. The player is a control surface over this
  state, not new playback machinery.
- **Frame rendering.** The studio already draws frames as NEAREST egui textures
  (`studio_clip_frame`, `video_frame_to_texture`) — no wgpu canvas involved — and
  the seekbar's pointer-to-frame math already exists in `studio_scrubber`
  (`studio.rs`). The player draws the same textures and reuses the same math.
- **Keying.** `bg_key_color` and `bg_tolerance` on `ShellApp`, the
  `background_removal_panel` controls (`bg_removal.rs:27`), and the core
  primitives `ChromaKey`, `chroma_key`, `detect_key_color`, and
  `NormalizeOptions` (`core::transforms::normalize`) are the whole keying
  toolkit. The studio reuses the same two fields — one source of truth shared
  with the timeline op — and the same helpers.

Inspiration only, not a dependency: the `egui-video` `Player` the artist
referenced is a clean model for the *controls* — a hover-reveal seekbar, a
play/pause overlay, a time readout, click-the-frame-to-toggle, an animated
playhead. Borrow that layout. Do **not** borrow its backend: it is built on
`ffmpeg_the_third` (which links FFmpeg, LGPL/GPL) and `sdl2`, and neither is in
the project's `cargo deny` allow list (`.cargo/deny.toml` permits only
MIT/Apache/BSD/ISC/MPL-class licenses, `exceptions = []`). Adding them would fail
the license gate. The native `mp4` + `openh264` decode is already the supported
path and stays the only one.

## The raw-clip player

A focused, native, audioless video player for the Clip stage surface, modeled on
`egui-video`'s `render_controls`:

- **Plays the whole clip** at its native resolution and real fps
  (`AnimPlayMode::Clip`), so you watch the raw mp4 exactly as Seedance returned
  it — before any pick, normalize, or key.
- **A seekbar along the bottom of the frame:** a track, a filled progress bar to
  the playhead, and a draggable playhead. Click or drag anywhere on it to scrub.
  The pixel-to-frame mapping is the one `studio_scrubber` already computes.
- **Play/pause** from a button and from clicking the frame itself (the
  `egui-video` gesture).
- **A time readout**, `mm:ss / mm:ss`, derived from the frame index, the fps, and
  the frame count by a small `format_clip_time(frame, fps, total)` helper —
  integer math, no `chrono`.
- **Loop on/off**, defaulting on, since these are short cycles.
- **Controls fade in on hover and while paused** (`Context::animate_bool_with_time`),
  so the raw frame is unobstructed during playback and the chrome appears only
  when you reach for it.

The player and the loop-marker scrubber are one timeline, not two. The seekbar is
the player's track; the loop in/out handles (the existing draggable markers)
overlay that same bar. One affordance reads two ways — "watch the raw clip" and
"mark the loop" — without a second widget. A new `studio_clip_player(ui, i)`
method replaces the current frame-plus-basic-scrubber split, drawing the frame,
the fading controls, the playhead, and the loop handles together; it reuses
`studio_clip_frame` for the image and the existing marker-drag logic for the
handles.

## Key-color selection in the studio

Bring the chroma key into the Clip stage, chosen and judged against the raw clip:

- **Eyedrop from the clip.** Arm the eyedropper, click anywhere on the displayed
  clip frame, and read the RGBA at that texel straight from the `VideoFrame` —
  mapping the click to a texel exactly as the first-frame mask canvas already
  does. That sets `bg_key_color`. This is the studio's analog of the Picker
  tool's `do_pick`, but it reads the `VideoFrame` rather than the wgpu
  `display_frame`, because the studio renders egui textures and never drives the
  canvas.
- **A swatch and a tolerance**, the same `color_edit_button_srgba` over
  `bg_key_color` and a slider over `bg_tolerance` the `background_removal_panel`
  uses — so the studio and the timeline op speak through the same two fields.
- **A Detect button** that samples the frame border via `detect_key_color`, for
  the common flat-backdrop case where the corner pixels are the key.
- **A live keyed preview toggle.** With it on, the player renders each frame
  through `chroma_key(frame, ChromaKey { color: bg_key_color, tolerance:
  bg_tolerance })` before upload, so you watch the clip with the backdrop
  removed and judge the key before you commit. The keyed frames are cached per
  frame and invalidated whenever the color or tolerance changes, so the preview
  stays cheap while you tune.

## Bake the key into Land

When a key is set, the loop lands already stripped:

- `integrate_picked` builds `NormalizeOptions.chroma = Some(ChromaKey { color:
  bg_key_color, tolerance: bg_tolerance })` in place of today's `None`, so
  `normalize_frames` keys each picked frame before it measures the bounding box
  and baseline. Keying first also sharpens the measurement — the backdrop no
  longer pulls the bbox outward — which is why normalize takes the chroma key as
  an input in the first place.
- A **Remove background on Land** toggle gates it. On when a key is chosen; off
  lands the raw frames untouched and leaves stripping to the timeline operation,
  preserving the pipeline doc's behavior as the fallback.
- The chosen key stays in `bg_key_color` / `bg_tolerance` either way, so the
  post-land timeline background-removal op — and its AI fallback when keying
  misses — is seeded with the same key for any re-run.

## Code changes the studio needs

Described here for the implementing PRs, not written in this doc.

- **`studio_clip_player(ui, i)`** — the Clip-stage surface that replaces the
  current frame-and-scrubber pair: the raw frame, the hover-fading controls, the
  seekbar and playhead, and the loop in/out handles on one timeline. Reuses
  `studio_clip_frame` and the `studio_scrubber` pointer math.
- **A keyed-preview cache** — an optional per-frame keyed texture (a
  `keyed_thumbs` parallel to `ClipCandidate::thumbs`, or a studio-side cache
  keyed by clip, frame, color, and tolerance), invalidated on any key change.
- **Studio key state** — a `picking_key: bool` on `StudioState` plus the
  click-to-texel-to-RGBA read from the selected `VideoFrame`, and the controls:
  swatch, tolerance, Detect, eyedrop, keyed-preview toggle, and the "Remove
  background on Land" toggle — all over the existing `bg_key_color` /
  `bg_tolerance` and the `chroma_key` / `detect_key_color` helpers.
- **`integrate_picked`** — thread `NormalizeOptions.chroma` from the studio key
  when "Remove background on Land" is set; otherwise keep `None`.
- **`format_clip_time(frame, fps, total)`** — the `mm:ss / mm:ss` helper, with a
  unit test; reuse core's already-tested `chroma_key`.

## Build order

Each step is independently demoable.

1. **Raw player.** The seekbar, playhead, play/pause, time readout, and loop over
   the existing transport, replacing the basic scrubber, with the loop handles
   folded onto the same timeline. Proves you can watch the raw clip at speed and
   still mark a loop.
2. **Key selection.** The swatch, tolerance, Detect, and the eyedrop-from-frame.
   Proves you can choose the key against the raw clip.
3. **Keyed preview.** The toggle and the per-frame keyed texture. Proves you can
   judge the clip keyed versus raw before committing.
4. **Bake at Land.** `NormalizeOptions.chroma` from the studio key plus the
   "Remove background on Land" toggle. Proves a loop can land already stripped.

## Verification

By hand, from the `v2` worktree: generate a clip, watch the raw mp4 with the
player — play and pause, scrub, read the time, confirm it loops — eyedrop the
magenta backdrop, toggle the keyed preview to judge the strip, then land with
"Remove background on Land" set and confirm the timeline loop is already keyed.
Toggle it off and confirm the loop lands with its backdrop intact and the
timeline op still strips it with the same key.

Automated: unit tests for the new pure helpers — `format_clip_time` across
sub-minute and over-minute durations, and the eyedrop texel-to-RGBA read — plus a
test that `integrate_picked` passes `chroma = Some(..)` when a key and the Land
toggle are set and `None` otherwise (or a pure helper that builds the
`NormalizeOptions` the test can assert against). Reuse core's existing
`chroma_key` and `normalize_frames` tests. `cargo nextest run -p pixhaus-shell`
and `--workspace` stay green; `cargo clippy --workspace -- -D warnings` clean.

## Out of scope and non-goals

- **Audio, subtitles, and volume.** The `egui-video` player has them; generated
  clips are silent, so the studio player has no audio path.
- **ffmpeg and SDL2 backends.** Rejected by the license gate. The native `mp4` +
  `openh264` decode stays the only path, so WebM/VP9/AV1 clips remain
  unsupported (`decode_clip` returns `Unsupported`), unchanged by this doc.
- **Per-frame painting in the player.** Editing a generated frame is normal
  drawing on the timeline after Land, per the pipeline doc's non-goals.
- **Multi-key or per-frame keys.** One key color per clip; a frame that needs
  more than a flat key is a job for the AI fallback on the timeline.
