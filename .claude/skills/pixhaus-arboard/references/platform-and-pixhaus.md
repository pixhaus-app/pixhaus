# arboard 3.6.1 — platform behavior and the Pixhaus pattern

The clipboard is an OS resource, and each OS models it differently. arboard papers over the
differences, but a few leak through in ways that matter for a long-lived desktop app. This
file covers the per-platform behavior, the Linux ownership model (the source of the classic
"my clipboard vanished" bug), the image round-trip caveats, and the end-to-end Pixhaus
copy/paste pattern with error handling.

## The Linux ownership model — why `wait()` exists

On X11 and Wayland the clipboard is **not** a shared OS buffer. The contents are *served* by
the process that set them: when you `set_text`/`set_image`, your process advertises that it
owns the selection, and when another app pastes, that app asks *your* process for the data.

arboard handles this by running a **background thread** inside the live `Clipboard` that
answers those requests. Two consequences:

1. **The data lives only as long as something serves it.** While your `Clipboard` is alive,
   it serves. If your `Clipboard` (and its thread) drops — or the process exits — the served
   data is gone, *unless* another app or a clipboard manager grabbed a copy first. This is why
   re-creating the `Clipboard` per operation is a bug on Linux: the old one drops and takes the
   contents with it.

2. **Set-and-exit needs `wait()`.** A CLI tool that sets the clipboard and immediately exits
   would lose the data before anyone could paste. `SetExtLinux::wait()` blocks the `set` call
   until another application takes ownership of the clipboard, keeping the serving thread alive
   until then; `wait_until(deadline)` bounds that block.

**Pixhaus is a long-lived GUI, so it is on the right side of this by default.** The
`Clipboard` lives in the `App` struct for the whole session and serves contents the whole
time — exactly what you want. Therefore:

- **Do not call `set().wait()` / `wait_until()` in Pixhaus.** They block the calling thread
  until another app takes the clipboard, which on the egui UI thread means a frozen window.
  They solve a problem (process about to exit) that Pixhaus does not have.
- On app exit, whatever is on the clipboard behaves like any other app's: a running clipboard
  manager (most Linux desktops have one) will have copied it; without one, it is lost on exit.
  That is standard Linux behavior, not an arboard quirk — don't try to "fix" it with `wait`.

macOS and Windows use a real shared OS clipboard, so none of this applies there — set data
persists after your process exits regardless.

## Per-platform `set_image` encoding

`set_image` converts your RGBA8 bytes into the format each OS clipboard expects:

| Platform | On-clipboard image format |
|---|---|
| Linux (X11/Wayland) | PNG |
| Windows | `CF_DIB` and `CF_BITMAP` |
| macOS | `NSImage` (TIFF-backed) |

`get_image` does the reverse, decoding whatever is there back to RGBA8. You never handle these
formats directly — but they explain the round-trip caveats below.

## Image round-trip is lossy — do not assume byte equality

`set_image(x)` then `get_image()` is **not** guaranteed to return `x`'s exact bytes:

- **Re-encoding.** The image is encoded to the platform format on set and decoded on get.
  PNG is lossless for pixels, but the pipeline can still reorder or normalize.
- **Premultiplied alpha.** Windows DIB and macOS image paths can carry premultiplied alpha or
  drop the alpha channel depending on the source app. An image copied *from another app* may
  arrive with alpha already premultiplied against white/black, or fully opaque.
- **Color management.** macOS may attach/convert color profiles.

What you can rely on: **`width` and `height` survive**, and the pixels are RGBA8 row-major on
the way back. What you cannot rely on: exact byte identity, or that alpha is straight
(non-premultiplied). For Pixhaus this means:

- When pasting an image *in*, treat alpha defensively — if pixels look wrong (haloed,
  darkened edges), suspect premultiplied alpha and un-premultiply.
- **Do not write a snapshot/`image-compare` test that asserts a clipboard round-trip is
  byte-identical** — it will be flaky across platforms and CI. Test that dimensions are
  preserved and that an obviously-distinct image survives recognizably, not that bytes match.
  See `pixhaus-testing-conventions`.

## Threading: `Clipboard` is not freely `Send`

`ImageData` is `Send + Sync` and can move between threads. The `Clipboard` handle is **not**
something to scatter across threads — its platform internals are tied to where it was created,
and concurrent access from another thread surfaces as `ClipboardOccupied`. This fits Pixhaus
cleanly: the egui update loop runs on one thread and owns the document directly (per the repo
async conventions), so the `Clipboard` is just another single-owner field on the `App`. Build
the `ImageData` on the UI thread and call `set_image` there.

If a future 8K copy makes the synchronous encode cost a visible frame hitch, the move is to
prepare the *bytes* off-thread and do only the `set_image` call on the UI thread — not to send
the `Clipboard` to a worker. In practice, reach for that only with measured evidence (the 8K
budget rule, `project_8k_perf_constraint`).

## The Pixhaus pattern, end to end

Where it lives, per the repo layout: `shell/` owns the `Clipboard` and the keyboard wiring;
the selection↔`ImageData` conversion is plain pixel work that belongs in `core/` (or a small
`io`-side helper). `render/` never touches arboard.

### Hold the handle in the App

```rust
// shell/ — the eframe App struct.
pub struct PixhausApp {
    clipboard: arboard::Clipboard,
    // ... document, panels, etc.
}

impl PixhausApp {
    fn new(/* ... */) -> anyhow::Result<Self> {
        Ok(Self {
            clipboard: arboard::Clipboard::new()?,  // once; anyhow only in the binary
            // ...
        })
    }
}
```

### A typed error in the library layer

Per `pixhaus-thiserror` / `pixhaus-rust-conventions`, wrap `arboard::Error` where the
conversion logic lives so the shell gets a domain error, not a third-party one:

```rust
// core/ or io/
#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("clipboard holds no image to paste")]
    NothingToPaste,
    #[error("clipboard is busy; try again")]
    Busy,
    #[error("clipboard operation failed: {0}")]
    Backend(#[from] arboard::Error),
}
```

### Copy a selection out

```rust
use std::borrow::Cow;

// `selection_rgba8` is a tightly packed RGBA8 buffer, len == w*h*4.
fn copy_selection(
    clipboard: &mut arboard::Clipboard,
    selection_rgba8: &[u8],
    w: usize,
    h: usize,
) -> Result<(), ClipboardError> {
    clipboard.set_image(arboard::ImageData {
        width: w,
        height: h,
        bytes: Cow::Borrowed(selection_rgba8),  // no copy
    })?;
    Ok(())
}
```

### Paste an image in, mapping the "empty" case

```rust
fn paste_image(clipboard: &mut arboard::Clipboard) -> Result<arboard::ImageData<'static>, ClipboardError> {
    match clipboard.get_image() {
        Ok(img) => Ok(img),                                      // img.width/height/bytes -> new layer
        Err(arboard::Error::ContentNotAvailable) => Err(ClipboardError::NothingToPaste),
        Err(arboard::Error::ClipboardOccupied) => Err(ClipboardError::Busy),
        Err(e) => Err(ClipboardError::Backend(e)),
    }
}
```

In the shell, `NothingToPaste` is a no-op (maybe a quiet status line), `Busy` is worth a short
retry or a toast, and the rest becomes a toast. Nothing here should `unwrap` or `panic`.

### Handling `ClipboardOccupied` with a bounded retry

Windows in particular hands out a brief global lock; another app holding it for a few
milliseconds yields `ClipboardOccupied`. A small, bounded retry is reasonable — but do it
without sleeping the UI thread for long:

```rust
fn set_image_retrying(
    clipboard: &mut arboard::Clipboard,
    img: arboard::ImageData<'_>,
) -> Result<(), arboard::Error> {
    let mut attempt = 0;
    loop {
        // ImageData here borrows; clone per attempt only if the borrow can't be reused.
        match clipboard.set_image(img.clone()) {
            Err(arboard::Error::ClipboardOccupied) if attempt < 3 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => return other,
        }
    }
}
```

Keep the retry count and sleep tiny; a clipboard that stays occupied is the other app's
problem, not something to spin on.

## Quick checklist

- One `Clipboard`, created at startup, owned by the `App`, reused for the session.
- arboard for images (and HTML / file lists); egui (`ctx.copy_text`, `Event::Paste`) for text.
- Never `unwrap`; map `ContentNotAvailable` to "nothing to paste"; bounded retry on
  `ClipboardOccupied`.
- No `wait()` / `wait_until()` — those are for set-and-exit CLIs, and they block.
- Enable the `wayland-data-control` feature for native Wayland; keep default `image-data` on.
- Don't test round-trips for byte equality; dimensions survive, bytes and alpha may not.
