---
name: pixhaus-arboard
description: >
  Use when moving image or text data across the OS clipboard in Pixhaus — copying a
  pixel selection out so it pastes into another app, pasting an image in, or any
  Clipboard / ImageData / get_image / set_image / get_text / set_text work. Trigger
  this for ANY "copy the selection to the clipboard", "paste an image", "Ctrl+C /
  Ctrl+V for pixels", "put this on the system clipboard", "read an image from the
  clipboard", "ImageData", "Clipboard::new", or "clipboard ownership / it disappears
  on Linux" task, even when the user doesn't say "arboard". arboard is the
  cross-platform OS-clipboard crate; in Pixhaus its job is the IMAGE clipboard —
  egui/eframe already wires plain-text copy/paste to the OS, so reach for this skill
  to bridge RGBA8 pixel buffers to and from the clipboard, and to avoid the Linux
  ownership and cross-platform round-trip traps that older examples get wrong.
---

# arboard for Pixhaus

arboard reads and writes the operating system's clipboard — text, HTML, and most
importantly for Pixhaus, **images**. It is the bridge on one seam: the boundary where a
Pixhaus pixel selection becomes data the rest of the desktop can paste (into Photoshop, a
chat window, another Pixhaus instance), and where an image sitting on the clipboard becomes
pixels Pixhaus can paste in. Its image type is RGBA8 — the same shape as a Pixhaus pixel
buffer — so the conversion is a thin, near-zero-copy hop.

This skill is the floor for clipboard work. The one Pixhaus-specific rule that overrides
what you remember about clipboards, the lifetime model that prevents the recurring "it
vanished" bug, the everyday API, and how it maps onto the shell. When you need an exact
signature or the full type surface, open `references/api.md`; for per-OS behavior and the
Pixhaus integration pattern, open `references/platform-and-pixhaus.md`. Both are derived
from docs.rs 3.6.1.

## The one rule that is different in Pixhaus

**arboard is for images, not text. egui/eframe already owns the text clipboard.** When the
native shell is running, egui wires the OS clipboard to text widgets and to copy/cut/paste
events for you — a `TextEdit` already does Ctrl+C/V, and you read paste with the
`egui::Event::Paste(String)` event and write with `ui.ctx().copy_text(string)`. Routing
plain text (a hex color, a layer name, a palette dump) through `arboard::Clipboard::set_text`
instead means two clipboard stacks fighting over the same OS resource, and it walks straight
into arboard's Linux ownership trap (below) that egui's integration already handles.

So: **use egui for text, reach for arboard when the payload is an image** (or HTML, or a
file list) — the formats egui does not handle. If you find yourself typing
`clipboard.set_text(...)` inside the egui app, stop and use `ctx.copy_text` instead. See the
`pixhaus-egui` skill for the event side.

## Versions

arboard is past 1.0 and moves independently of the egui/wgpu stack.

| Crate | Version |
|---|---|
| `arboard` | 3.6 |

```toml
# In shell/ (the eframe binary owns the Clipboard). Not in render/ or core/.
arboard = { version = "3.6", features = ["wayland-data-control"] }
```

- The default `image-data` feature is what makes `get_image`/`set_image` exist — leave it on.
  It pulls the platform image deps (`image`, `objc2-core-graphics` on macOS, `windows-sys`
  on Windows). Turning it off reduces arboard to text only, which defeats the purpose here.
- `wayland-data-control` is **off by default** and worth turning on: Pixhaus targets Linux,
  and without it arboard talks only X11 (via XWayland) on a Wayland session. With it, arboard
  uses the native Wayland protocol and falls back to X11 when that is unavailable.
- Confirm the license is clean — arboard is `MIT OR Apache-2.0`, which `cargo deny` accepts.
  Its Wayland backend (`wl-clipboard-rs`) is also permissive; check `cargo deny check` after
  adding the feature, per the repo's license rule.

## The mental model: one long-lived owner, two directions

A `Clipboard` is a live handle to an OS resource, not a stateless helper. Constructing one
does real work — on Linux it opens an X11/Wayland connection and runs a background thread to
*serve* whatever you put there. That shapes the one rule that prevents most bugs:

**Create the `Clipboard` once and keep it alive for the app's lifetime.** Store it in the
eframe `App` struct, not in a function that returns. Every method takes `&mut self`, so it
lives naturally as an owned field on the single-threaded egui app — exactly the "every piece
of mutable state has a single owner" rule from the repo conventions. Do **not**
`Clipboard::new()` per Ctrl+C: it is wasteful, and on Linux a `Clipboard` that drops takes
the served contents with it (see `references/platform-and-pixhaus.md` on ownership).

Every operation is one of two directions, and both can fail:

```
   WRITE: your pixels ──set_image()──▶ OS clipboard      (other apps can now paste)
   READ:  OS clipboard ──get_image()──▶ ImageData<'static>   (Pixhaus pastes it in)
```

`ImageData<'a>` is the image payload in both directions:

```rust
pub struct ImageData<'a> {
    pub width: usize,
    pub height: usize,
    pub bytes: Cow<'a, [u8]>,   // RGBA8, row-major, tightly packed: width*height*4 bytes
}
```

Four channels per pixel (R, G, B, A), top-left first, left-to-right then top-to-bottom — the
same layout as a Pixhaus `Vec<u8>` pixel buffer with stride `width*4`. Because `bytes` is a
`Cow`, `set_image` can **borrow** your buffer (`Cow::Borrowed(&pixels)`) with no copy;
`get_image` always returns owned `ImageData<'static>`.

## The everyday API

One constructor, two methods per direction. Everything returns `Result<_, arboard::Error>`.

```rust
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;

// Once, at startup — store the handle in your App struct.
let mut clipboard = Clipboard::new()?;   // Result<Clipboard, Error>

// WRITE a selection out. Borrow the pixel buffer; no copy on the Pixhaus side.
clipboard.set_image(ImageData {
    width,
    height,
    bytes: Cow::Borrowed(&rgba8_pixels),
})?;

// READ an image in. Owned, 'static — safe to keep.
let img = clipboard.get_image()?;        // ImageData<'static>
// img.width, img.height, &img.bytes  -> blit into a new layer / selection

// Text exists too, but in the shell prefer egui (see the one rule above):
let s = clipboard.get_text()?;           // Result<String, Error>
clipboard.set_text("ff8800")?;           // Into<Cow<str>>
clipboard.clear()?;                      // wipe the default clipboard
```

There is also a fluent builder form — `clipboard.get().image()`, `clipboard.set().image(img)`,
`clipboard.clear_with()` — which exists so the Linux selection extension traits can attach
(`get().clipboard(LinuxClipboardKind::Primary).text()`). The plain methods above are the
`Clipboard` (cut/copy/paste) selection; reach for the builder only when you need Primary
selection or Linux `wait` semantics. Full signatures in `references/api.md`.

## Pixhaus applications

- **Copy a pixel selection out.** Take the selected region's RGBA8 bytes, wrap them in
  `ImageData` borrowing the buffer, `set_image`. Other apps re-encode it on paste (PNG on
  Linux, a DIB on Windows, an NSImage on macOS) — arboard handles that. The selection-to-bytes
  step is plain pixel work; keep it in `core/`, and let `shell/` own the `Clipboard` and the
  Ctrl+C wiring.
- **Paste an image in.** `get_image` on Ctrl+V, then create a new layer or floating selection
  from `img.width`/`img.height`/`img.bytes`. Treat `ContentNotAvailable` (clipboard empty or
  holds non-image data, e.g. text) as a normal "nothing to paste" outcome, not an error to
  surface loudly.
- **Round-trips are lossy across platforms — do not build on byte equality.** `set_image`
  then `get_image` is not guaranteed to return identical bytes: the OS re-encodes, and Windows
  DIB / macOS paths can premultiply or drop alpha. Dimensions survive; exact bytes may not.
  This kills the obvious snapshot test — see `pixhaus-testing-conventions` and the caveats in
  `references/platform-and-pixhaus.md`.
- **It is a synchronous, possibly-blocking call on the UI thread.** Clipboard ops can stall
  briefly (a Windows global-lock contention, a Linux round-trip). For typical selections that
  is invisible. A full 8K copy means encoding ~256 MB and can cost a frame — if that shows up,
  build the `ImageData` on the UI thread but consider the cost; do not naively move the
  `Clipboard` to a background task, as it is not guaranteed `Send` across platforms. The 8K
  budget rule from `project_8k_perf_constraint` applies here too.
- **Not the save path.** The clipboard is volatile, OS-mediated, and re-encoded. It is not a
  serialization format. The `.pixhaus` file is MessagePack + zstd (`pixhaus-rmp-serde`); keep
  arboard on the live-clipboard seam, serde on the I/O seam.

## Rules that prevent the recurring bugs

- **One `Clipboard`, owned by the app, for the whole session.** Re-creating it per operation
  is the top mistake and, on Linux, drops your served contents. Store it once.
- **Never `unwrap()` a clipboard call.** Every method returns `Result<_, arboard::Error>` and
  the clipboard genuinely fails at runtime — empty, occupied by another app, wrong format. Map
  it with `thiserror` in a `core/io` helper, surface a toast in the shell, and move on. The
  no-unwrap rule (`pixhaus-rust-conventions`) is not optional here.
- **`ContentNotAvailable` is expected, not exceptional.** Pasting when the clipboard is empty
  or holds text returns this. Handle it as "nothing to paste," not a popup.
- **`ClipboardOccupied` is transient.** Especially on Windows, another process can hold the
  global clipboard lock for a few milliseconds. A short bounded retry is reasonable before
  giving up; do not loop forever.
- **Don't call `set().wait()` in the GUI.** The Linux `wait` extension blocks until another
  app takes the clipboard — it is for set-and-exit CLI tools. Pixhaus keeps the `Clipboard`
  alive, which is what serves the data; `wait()` would freeze the UI thread. Details in
  `references/platform-and-pixhaus.md`.
- **Use egui, not arboard, for text in the shell.** Restated because it is the easiest wrong
  turn: `ctx.copy_text` / `Event::Paste`, not `clipboard.set_text` / `get_text`.

## References

Open the file for what you're doing; each is version-pinned to arboard 3.6.1.

| File | Covers |
|---|---|
| `references/api.md` | Every type and method with exact signatures — `Clipboard`, the `Get`/`Set`/`Clear` builders, `ImageData`, the full `Error` enum, `LinuxClipboardKind`, and the `*ExtLinux` traits |
| `references/platform-and-pixhaus.md` | Per-OS behavior (X11/Wayland ownership, macOS, Windows DIB), `wait`/selection semantics, image round-trip and premultiplied-alpha caveats, and the end-to-end Pixhaus copy/paste pattern with error mapping |

A standing caution: the references record the 3.6.1 API faithfully, but if a deep signature
or a platform behavior is load-bearing for what you're building, confirm it with
`cargo doc -p arboard --open` once the crate is vendored, and test the actual round-trip on
the target OS — clipboard behavior is the kind of thing that only shows its edges at runtime.
