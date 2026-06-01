# arboard 3.6.1 — full API reference

Every public type and method, with exact signatures from docs.rs 3.6.1. Crate is
`MIT OR Apache-2.0`. The public surface is small: one struct you construct (`Clipboard`),
three builder structs (`Get`, `Set`, `Clear`), one payload struct (`ImageData`), two enums
(`Error`, `LinuxClipboardKind`), and three Linux-only extension traits.

## Table of contents

- [`Clipboard`](#clipboard) — the handle; direct methods and builder entry points
- [`ImageData`](#imagedata) — the image payload, both directions
- [`Get` builder](#get-builder) — read text / image / html / file_list
- [`Set` builder](#set-builder) — write text / html / image / file_list
- [`Clear` builder](#clear-builder)
- [`Error`](#error) — every variant
- [`LinuxClipboardKind`](#linuxclipboardkind) — selection on Linux
- [Linux extension traits](#linux-extension-traits) — `GetExtLinux`, `SetExtLinux`, `ClearExtLinux`

---

## `Clipboard`

> "The OS-independent structure for accessing the clipboard."

The live handle to the OS clipboard. Construct one and keep it; every method takes
`&mut self`. There are two ways to use it: **direct methods** (the common path — they act on
the platform default clipboard, i.e. the cut/copy/paste selection) and **builder entry
points** (`get`, `set`, `clear_with`) that return a builder so Linux extension traits can
attach a non-default selection or `wait` behavior.

### Constructor

```rust
pub fn new() -> Result<Self, Error>
```
"Creates an instance of the clipboard." Errors if clipboards are not supported on the
current platform/environment.

### Direct text methods

```rust
pub fn get_text(&mut self) -> Result<String, Error>
```
"Fetches UTF-8 text from the clipboard and returns it." Errors if the clipboard is empty or
its contents are not UTF-8 text.

```rust
pub fn set_text<'a, T: Into<Cow<'a, str>>>(&mut self, text: T) -> Result<(), Error>
```
"Places the text onto the clipboard. Any valid UTF-8 string is accepted." Accepts `&str`,
`String`, or `Cow<str>`.

### Direct HTML method

```rust
pub fn set_html<'a, T: Into<Cow<'a, str>>>(
    &mut self,
    html: T,
    alt_text: Option<T>,
) -> Result<(), Error>
```
"Places the HTML as well as a plain-text alternative onto the clipboard." The `alt_text` is
what plain-text-only consumers see. (Reading HTML back is on the `Get` builder — `get().html()`.)

### Direct image methods

```rust
pub fn get_image(&mut self) -> Result<ImageData<'static>, Error>
```
"Fetches image data from the clipboard, and returns the decoded pixels." Errors if the
clipboard is empty, does not hold an image, or holds an unsupported image format. The
returned `ImageData` is owned (`'static`).

```rust
pub fn set_image(&mut self, image: ImageData<'_>) -> Result<(), Error>
```
"Places an image to the clipboard." The on-clipboard encoding is platform-specific (NSImage
on macOS, PNG on Linux, CF_DIB / CF_BITMAP on Windows) — arboard does the conversion.

### Clear

```rust
pub fn clear(&mut self) -> Result<(), Error>
```
"Clears any contents that may be present from the platform's default clipboard, regardless
of the format."

### Builder entry points

```rust
pub fn get(&mut self) -> Get<'_>          // "Begins a 'get' operation to retrieve data."
pub fn set(&mut self) -> Set<'_>          // "Begins a 'set' operation to set the contents."
pub fn clear_with(&mut self) -> Clear<'_> // "Begins a 'clear' option to remove data."
```

`clipboard.get_text()` is exactly `clipboard.get().text()`; the direct methods are
shorthands for the builder default. Use the builder when you need a Linux extension method.

---

## `ImageData`

> "Container for pixel data of an image."

```rust
pub struct ImageData<'a> {
    pub width: usize,
    pub height: usize,
    pub bytes: Cow<'a, [u8]>,
}
```

Pixel format, quoting the docs verbatim:

> "Each element in `bytes` stores the value of a channel of a single pixel. This struct
> stores four channels (red, green, blue, alpha) so a `3*3` image is going to be stored on
> `3*3*4 = 36` bytes of data."
>
> "The pixels are in row-major order meaning that the second pixel in `bytes` (starting at
> the fifth byte) corresponds to the pixel that's sitting to the right side of the top-left
> pixel (x=1, y=0)"

So: **RGBA8, 4 bytes per pixel, row-major, tightly packed**, length `width * height * 4`.
This matches a Pixhaus `Vec<u8>` pixel buffer with stride `width * 4`. The `Cow` lets a write
borrow your buffer with no copy; a read returns owned data.

### Methods

```rust
pub fn into_owned_bytes(self) -> Cow<'static, [u8]>
```
Converts `bytes` to guaranteed-owned data — moves if already owned, clones if borrowed.

```rust
pub fn to_owned_img(&self) -> ImageData<'static>
```
Returns an owned `ImageData` (clones borrowed bytes). Use this when you need to store an image
that currently borrows a buffer you're about to drop.

### Trait impls

`Clone`, `Debug`. Auto: `Send`, `Sync`, `Unpin`, `RefUnwindSafe`, `UnwindSafe` (an `ImageData`
can be moved to another thread freely — unlike `Clipboard`).

---

## `Get` builder

> "A builder for retrieving values from the clipboard."

Returned by `Clipboard::get()`. Terminal methods consume the builder:

```rust
pub fn text(self) -> Result<String, Error>
// "Completes the 'get' operation by fetching UTF-8 text from the clipboard."

pub fn image(self) -> Result<ImageData<'static>, Error>
// "Completes the 'get' operation by fetching image data ... returning the decoded pixels."

pub fn html(self) -> Result<String, Error>
// "Completes the 'get' operation by fetching HTML from the clipboard."

pub fn file_list(self) -> Result<Vec<PathBuf>, Error>
// "Completes the 'get' operation by fetching a list of file paths from the clipboard."
```

`GetExtLinux::clipboard(self, selection)` (below) is the non-terminal that picks the
selection before a terminal call: `clipboard.get().clipboard(LinuxClipboardKind::Primary).text()`.

---

## `Set` builder

> "A builder for writing values to the clipboard."

Returned by `Clipboard::set()`. Terminal methods:

```rust
pub fn text<'a, T: Into<Cow<'a, str>>>(self, text: T) -> Result<(), Error>
// "Completes the 'set' operation by placing text onto the clipboard."

pub fn html<'a, T: Into<Cow<'a, str>>>(self, html: T, alt_text: Option<T>) -> Result<(), Error>
// "Completes the 'set' operation by placing HTML as well as a plain-text alternative ..."

pub fn image(self, image: ImageData<'_>) -> Result<(), Error>
// "Completes the 'set' operation by placing an image onto the clipboard."

pub fn file_list(self, file_list: &[impl AsRef<Path>]) -> Result<(), Error>
// "Completes the 'set' operation by placing a list of file paths onto the clipboard."
```

`SetExtLinux` adds non-terminal `clipboard`, `wait`, and `wait_until` (below).

---

## `Clear` builder

> "A builder for clearing clipboard data."

Returned by `Clipboard::clear_with()`. Terminal method:

```rust
pub fn clear(self) -> Result<(), Error>
```

`ClearExtLinux::clipboard(self, selection)` picks which selection to clear.

---

## `Error`

> "Exceptions that may occur during clipboard operations."

`#[non_exhaustive]` — always include a `_ =>` arm when matching. Implements `Debug`,
`Display`, `std::error::Error`, and is `Send + Sync`.

| Variant | Meaning (docs verbatim, abridged) |
|---|---|
| `ContentNotAvailable` | "The clipboard contents were not available in the requested format. This could either be due to the clipboard being empty or the clipboard contents having an incompatible format to the requested one (eg when calling `get_image` on text)" |
| `ClipboardNotSupported` | "The selected clipboard is not supported by the current configuration (system and/or environment)." — e.g. `Primary` on an old Wayland compositor, `Secondary` on Wayland. |
| `ClipboardOccupied` | "The native clipboard is not accessible due to being held by another party." Can come from another process or another thread of the same program. Transient — a bounded retry is reasonable. |
| `ConversionFailure` | "The image or the text that was about the be transferred to/from the clipboard could not be converted to the appropriate format." |
| `Unknown { description: String }` | "Any error that doesn't fit the other error types." The `description` is "meant to be used by the developer to debug the issue" — not for end-user display. |

Treat `ContentNotAvailable` as a normal "nothing to paste" outcome. Map the rest through
`thiserror` in a library helper and surface a user-facing message in the shell (see
`pixhaus-thiserror` and `pixhaus-rust-conventions`).

---

## `LinuxClipboardKind`

> Clipboard selection (Linux-specific). Pick it on a builder via the `*ExtLinux::clipboard`
> methods. Ignored on macOS/Windows code paths.

| Variant | Meaning (docs verbatim) |
|---|---|
| `Clipboard` | "Typically used selection for explicit cut/copy/paste actions (ie. windows/macos like clipboard behavior)" — the default. |
| `Primary` | "Typically used for mouse selections and/or currently selected text. Accessible via middle mouse click." On Wayland needs compositor support (data-control v2+); errors with `ClipboardNotSupported` if unavailable. |
| `Secondary` | "The secondary clipboard is rarely used but theoretically available on X11." Unavailable on Wayland — errors there. |

For Pixhaus's copy/paste, the default `Clipboard` selection (i.e. the plain `set_image`/
`get_image` methods) is what you want. `Primary` is only relevant if you deliberately support
middle-click paste on Linux.

---

## Linux extension traits

These add methods to the builders and are **only in scope behind `#[cfg(target_os = "linux")]`-style
code** — they live in `arboard` but the methods exist for the Linux backend. Import the trait
to call them.

### `GetExtLinux`

```rust
fn clipboard(self, selection: LinuxClipboardKind) -> Self
// "Sets the clipboard the operation will retrieve data from."
```

### `SetExtLinux`

```rust
fn clipboard(self, selection: LinuxClipboardKind) -> Self
// "Sets the clipboard the operation will store its data to."

fn wait(self) -> Self
// "Whether to wait for the clipboard's contents to be replaced after setting it."

fn wait_until(self, deadline: std::time::Instant) -> Self
// Like wait(), but returns no later than the given deadline.
```

`wait` / `wait_until` are the Linux ownership mechanism for **set-and-exit** programs — see
`platform-and-pixhaus.md`. A long-lived GUI like Pixhaus must **not** call them (they block).

### `ClearExtLinux`

```rust
fn clipboard(self, selection: LinuxClipboardKind) -> Self
// Picks which selection clear() targets.
```
