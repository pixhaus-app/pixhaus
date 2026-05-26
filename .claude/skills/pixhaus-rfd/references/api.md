# rfd 0.17 API reference

Exhaustive signatures and the platform support matrix. The `SKILL.md` covers the
patterns that matter for Pixhaus; this file is the lookup table for "what is the
exact signature" and "does this method do anything on platform X".

All signatures are for rfd 0.17.2. Builder setters take `self` and return `Self`,
so they chain. The terminal methods (`pick_*`, `save_file`, `show`) consume the
builder.

## Table of contents

- [FileDialog (sync)](#filedialog-sync)
- [AsyncFileDialog](#asyncfiledialog)
- [FileHandle](#filehandle)
- [MessageDialog (sync) and AsyncMessageDialog](#messagedialog-sync-and-asyncmessagedialog)
- [Enums: MessageButtons, MessageLevel, MessageDialogResult](#enums)
- [Platform support matrix](#platform-support-matrix)
- [save_file extension behavior](#save_file-extension-behavior)

## FileDialog (sync)

Returns paths directly. **Blocks the calling thread until the user dismisses the
dialog.** Never call on the egui UI thread — see `SKILL.md`.

```rust
pub fn new() -> Self

// Builders (chainable)
pub fn add_filter(self, name: impl Into<String>, extensions: &[impl ToString]) -> Self
pub fn set_directory<P: AsRef<Path>>(self, path: P) -> Self
pub fn set_file_name(self, file_name: impl Into<String>) -> Self
pub fn set_title(self, title: impl Into<String>) -> Self
pub fn set_can_create_directories(self, can: bool) -> Self
pub fn set_parent<W: HasWindowHandle + HasDisplayHandle + ?Sized>(self, parent: &W) -> Self

// Terminals (block, then return)
pub fn pick_file(self) -> Option<PathBuf>
pub fn pick_files(self) -> Option<Vec<PathBuf>>
pub fn pick_folder(self) -> Option<PathBuf>
pub fn pick_folders(self) -> Option<Vec<PathBuf>>
pub fn save_file(self) -> Option<PathBuf>
```

`None` means the user cancelled. Implements `Clone`, `Debug`, `Default`.

## AsyncFileDialog

Same builders as `FileDialog`; the terminals return futures instead of blocking.
This is the API Pixhaus uses inside the egui shell. The returned futures are
`Send`, so they run on Tokio worker threads; rfd marshals the actual native UI
work onto the platform's required thread internally.

```rust
pub fn new() -> Self

// Builders — identical signatures to FileDialog
pub fn add_filter(self, name: impl Into<String>, extensions: &[impl ToString]) -> Self
pub fn set_directory<P: AsRef<Path>>(self, path: P) -> Self
pub fn set_file_name(self, file_name: impl Into<String>) -> Self
pub fn set_title(self, title: impl Into<String>) -> Self
pub fn set_can_create_directories(self, can: bool) -> Self
pub fn set_parent<W: HasWindowHandle + HasDisplayHandle + ?Sized>(self, parent: &W) -> Self

// Terminals — return futures resolving to FileHandle(s)
pub fn pick_file(self)    -> impl Future<Output = Option<FileHandle>>
pub fn pick_files(self)   -> impl Future<Output = Option<Vec<FileHandle>>>
pub fn pick_folder(self)  -> impl Future<Output = Option<FileHandle>>
pub fn pick_folders(self) -> impl Future<Output = Option<Vec<FileHandle>>>
pub fn save_file(self)    -> impl Future<Output = Option<FileHandle>>
```

Implements `Clone`, `Debug`, `Default`, `Send`, `Sync`, `Unpin`.

## FileHandle

What the async terminals resolve to. A thin wrapper over a path on desktop and a
JS `File` on WASM, so the same code compiles on both. On desktop, get the path
out and proceed as normal.

```rust
pub fn wrap(path_buf: PathBuf) -> Self      // construct one yourself
pub fn file_name(&self) -> String           // just the file name, no directory
pub fn path(&self) -> &Path                  // NOT available on WASM32
pub async fn read(&self) -> Vec<u8>          // reads the whole file; off-thread on desktop
pub async fn write(&self, data: &[u8]) -> std::io::Result<()>
pub fn inner(&self) -> &Path                 // behind the `file-handle-inner` feature
```

Conversions:

```rust
impl From<PathBuf> for FileHandle
impl From<FileHandle> for PathBuf
impl From<&FileHandle> for PathBuf
```

So on desktop, both of these work:

```rust
let path: PathBuf = handle.into();
let path: PathBuf = (&handle).into();
let path: &Path   = handle.path();   // desktop only
```

Implements `Clone`, `Debug`.

Prefer `handle.path()` (or the `PathBuf` conversion) on desktop and load the file
yourself through the `io` crate, which already owns format logic. Reserve
`FileHandle::read()` for code paths that must also compile for WASM — Pixhaus is
desktop-only, so you rarely need it.

## MessageDialog (sync) and AsyncMessageDialog

Identical builders. `MessageDialog::show` blocks and returns; `AsyncMessageDialog::show`
returns a future. Same UI-thread blocking rule as the file dialogs.

```rust
pub fn new() -> Self
pub fn set_level(self, level: MessageLevel) -> Self
pub fn set_title(self, text: impl Into<String>) -> Self
pub fn set_description(self, text: impl Into<String>) -> Self
pub fn set_buttons(self, buttons: MessageButtons) -> Self
pub fn set_parent<W: HasWindowHandle + HasDisplayHandle + ?Sized>(self, parent: &W) -> Self

// MessageDialog:
pub fn show(self) -> MessageDialogResult
// AsyncMessageDialog:
pub fn show(self) -> impl Future<Output = MessageDialogResult>
```

`AsyncMessageDialog` is `Send` but `!Sync`. `MessageDialog` implements `Clone`,
`Debug`, `Default`.

## Enums

```rust
pub enum MessageLevel { Info, Warning, Error }   // Default: Info

pub enum MessageButtons {
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
    // Custom button labels. On Windows these only render with the
    // `common-controls-v6` feature; without it they fall back to the
    // standard labels.
    OkCustom(String),
    OkCancelCustom(String, String),
    YesNoCancelCustom(String, String, String),
}

pub enum MessageDialogResult {
    Yes,
    No,
    Ok,
    Cancel,
    Custom(String),   // the label of the custom button that was pressed
}
```

`MessageDialogResult` implements `Clone`, `Debug`, `PartialEq`, `Eq`, and
`Display`. It is not `Copy` (the `Custom` variant owns a `String`). Match on it:

```rust
match dialog.show() {
    MessageDialogResult::Yes => save_and_close(),
    MessageDialogResult::No  => close_without_saving(),
    _ /* Cancel */           => {} // stay open
}
```

A closed window-manager dialog (user hit the X / Esc) resolves to `Cancel`, so
always have a `Cancel`/`_` arm that does the safe thing.

## Platform support matrix

Which builder methods actually do something, per platform. "—" means the method
is accepted (so cross-platform code compiles) but has no effect there.

| Method | Windows | macOS | Linux (xdg-portal) | Linux (gtk3) | WASM |
|---|---|---|---|---|---|
| `add_filter` (named) | yes | name merged | yes | yes | merged |
| `set_directory` | yes | yes | yes | yes | — |
| `set_file_name` | yes | yes | yes | yes | yes (save) |
| `set_title` | yes | yes | yes | yes | yes |
| `set_parent` | yes | yes | yes | — | — |
| `set_can_create_directories` | — | yes | — | — | — |
| `pick_folder` / `pick_folders` | yes | yes | yes | yes | not present |
| `FileHandle::path()` | yes | yes | yes | yes | not present |

Notes:
- On macOS, filter *names* aren't shown; macOS uses the extension list only.
- `pick_folder`/`pick_folders` and `FileHandle::path()` do not exist on WASM
  (compile error if referenced there). Pixhaus is desktop-only, so this is moot
  unless someone adds a web target.

## save_file extension behavior

`save_file` returns the path the user chose; whether an extension is auto-appended
from the active filter differs by platform:

- **Windows**: appends the active filter's first extension if the user typed a
  bare name.
- **macOS**: enforces the filter extension.
- **GTK / XDG portal**: behavior varies; the returned path may lack an extension.

Do not trust the returned path to carry the extension you expect. After
`save_file`, normalize it yourself in the `io` crate (append the canonical
`.pixhaus`/`.png` if missing) before writing. That keeps behavior identical across
the three desktop platforms Pixhaus ships on.
