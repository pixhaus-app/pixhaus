---
name: pixhaus-rfd
description: >
  Use when Pixhaus needs a native OS file or folder dialog, or a native
  message/confirmation box — opening a `.pixhaus` project or a PNG to import,
  choosing a save path for export, picking a folder, or showing an "unsaved
  changes / are you sure?" prompt or an error alert. Trigger this for ANY "open
  file dialog", "save as", "file picker", "choose a folder", "browse for a file",
  "import an image", "export to…", "confirmation dialog", "yes/no prompt", or
  "show an error popup" task, even when the user never says "rfd". rfd ("Rusty
  File Dialogs") is the crate Pixhaus uses for all of these. The make-or-break
  rule: the synchronous `FileDialog`/`MessageDialog` APIs BLOCK the calling
  thread and will freeze the whole egui window if called from the UI loop —
  reach for this skill to get the `AsyncFileDialog` + Tokio + channel pattern
  right, and to pick the correct Cargo features (default `xdg-portal` vs opt-in
  `gtk3` on Linux), rather than guessing from memory.
---

# rfd for Pixhaus

rfd ("Rusty File Dialogs") opens the operating system's own file open/save
dialogs and message boxes — the real Windows/macOS/Linux dialogs, not something
egui draws. Pixhaus uses it at every boundary where the user touches the
filesystem: opening a `.pixhaus` project, importing a PNG, exporting a sprite
sheet, and the "you have unsaved changes" confirmation before close.

It is a small crate with one sharp edge. The edge is threading, and it is the
reason this skill exists.

## The one rule that matters: never block the egui UI thread

rfd has two parallel APIs:

- **Synchronous** — `FileDialog`, `MessageDialog`. The terminal call
  (`pick_file`, `show`, …) **blocks the calling thread until the user clicks a
  button**, then returns the result.
- **Asynchronous** — `AsyncFileDialog`, `AsyncMessageDialog`. The terminal call
  returns a future that resolves when the user clicks.

The egui update loop runs on one thread and owns the document (see CLAUDE.md and
[[pixhaus-egui]]). If you call a *synchronous* rfd dialog from inside `ui`, that
thread parks until the user finishes — the window stops painting, stops handling
input, and looks hung for as long as the dialog is open. That is the single most
common way to misuse this crate.

So inside the Pixhaus shell: **always `AsyncFileDialog`/`AsyncMessageDialog`,
spawned on Tokio, with the result delivered back over a channel the update loop
drains each frame.** This is the same shape as every other background task in
Pixhaus (see [[pixhaus-tokio]] and the async rules in
[[pixhaus-rust-conventions]]) — a file dialog is just another task that returns a
value.

rfd handles the platform's "native UI must run on a particular thread" rule for
you inside the async implementation (macOS in particular requires the dialog on
the main thread), which is why spawning the future on a Tokio worker is safe and
correct rather than a hack.

## The canonical open pattern

Store a receiver in the app state, fire the dialog on a click, drain the channel
each frame. A `tokio::sync::oneshot` is the right channel for a one-shot result.

```rust
use std::path::PathBuf;
use tokio::sync::oneshot;

struct PixhausApp {
    // None when no dialog is in flight.
    pending_open: Option<oneshot::Receiver<Option<PathBuf>>>,
    // ...rest of app state...
}

impl PixhausApp {
    // Called from a button handler inside `ui`. Returns immediately.
    fn start_open(&mut self, ctx: &egui::Context) {
        let (tx, rx) = oneshot::channel();
        self.pending_open = Some(rx);
        let ctx = ctx.clone(); // cheap: egui::Context is an Arc handle

        tokio::spawn(async move {
            let picked = rfd::AsyncFileDialog::new()
                .set_title("Open project")
                .add_filter("Pixhaus project", &["pixhaus"])
                .add_filter("PNG image", &["png"])
                .pick_file()
                .await;

            // FileHandle -> PathBuf on desktop. None == user cancelled.
            let path = picked.map(|handle| handle.path().to_path_buf());

            // Ignore send errors: the receiver is gone only if the app is closing.
            let _ = tx.send(path);
            ctx.request_repaint(); // wake the loop so it drains the channel now
        });
    }

    // Called once near the top of `ui`, every frame.
    fn poll_open(&mut self) {
        let Some(rx) = self.pending_open.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(Some(path)) => {
                self.pending_open = None;
                self.load_project(&path); // your io-crate load
            }
            Ok(None) => {
                self.pending_open = None; // cancelled — nothing to do
            }
            Err(oneshot::error::TryRecvError::Empty) => {} // still open, try next frame
            Err(oneshot::error::TryRecvError::Closed) => {
                self.pending_open = None; // task died without sending
            }
        }
    }
}
```

Why these choices:

- **`ctx.request_repaint()` after sending.** egui only repaints on input or on
  request. Without it, the result sits in the channel until the next mouse
  move — the file looks like it took seconds to open. This is the bug you will
  hit if you forget one line.
- **`oneshot`, not `mpsc`.** A pick yields exactly one result. Reach for an
  `mpsc` only if a single in-flight dialog can produce a stream of values, which
  file dialogs don't.
- **Guard against double-firing.** While `pending_open` is `Some`, disable or
  ignore the button so the user can't stack three dialogs.

Save and export are the same shape with `save_file()`; folder import uses
`pick_folder()`. For multi-file import (`pick_files`) the channel carries a
`Vec<PathBuf>`.

## Message and confirmation dialogs

Same rule, same pattern. The classic case is confirming close with unsaved
changes:

```rust
let answer = rfd::AsyncMessageDialog::new()
    .set_level(rfd::MessageLevel::Warning)
    .set_title("Unsaved changes")
    .set_description("Save changes to this project before closing?")
    .set_buttons(rfd::MessageButtons::YesNoCancel)
    .show()
    .await;

match answer {
    rfd::MessageDialogResult::Yes    => { /* save, then close */ }
    rfd::MessageDialogResult::No     => { /* close without saving */ }
    _ /* Cancel or dismissed */      => { /* abort the close */ }
}
```

Always give `Cancel`/`_` the safe behavior: a user who hits Esc or the window's X
gets `MessageDialogResult::Cancel`, and for a close prompt "safe" means *don't
lose their work*.

For a plain error alert, `set_buttons(MessageButtons::Ok)` and ignore the result.

Note the egui-native alternative: for a lightweight in-app confirmation you can
also draw an `egui::Window` modal yourself, which keeps everything on one thread
and matches the app's theme. Use rfd's message dialog when you specifically want
the *OS-native* look (error reporting, OS-standard button ordering); use an egui
window when you want it themed and inline. Both are valid — pick per case.

## Cargo features — get these right once

rfd is MIT licensed, so it passes the workspace MIT lock and `cargo deny`.

In rfd 0.17 the default feature set is `xdg-portal` (plus its Wayland/`pollster`
support deps) — **not** `gtk3`. For Pixhaus that default is exactly what you
want:

```toml
# Desktop-only Pixhaus. The default xdg-portal backend needs no GTK C libraries.
rfd = "0.17"
```

- **`xdg-portal` (default).** On Linux/BSD, talks to the desktop's XDG Desktop
  Portal over D-Bus — the modern, sandbox-friendly path, and it pulls in **no
  GTK system libraries**. Internally it drives its async D-Bus calls with
  `pollster`; there is no longer an `async-std`/`tokio` runtime feature to pick
  (older rfd had one — ignore stale examples that set `features = ["tokio"]`).
- **`gtk3` (opt-in).** Switches the Linux backend to GTK3. This requires the
  GTK3 C libraries and dev headers on every build and CI machine. Only add it if
  a specific dialog behavior forces it — it is a heavier dependency, not the
  default for a reason.
- **`common-controls-v6`.** Windows only. Needed for *custom button labels*
  (`OkCustom`, etc.) to actually render on Windows; without it they fall back to
  standard labels. Add it only if Pixhaus uses custom-labeled message buttons.
- **`file-handle-inner`.** Exposes `FileHandle::inner()`. Niche; skip it unless
  you have a concrete reason.

Pixhaus targets desktop only (Windows, macOS, Linux), so the WASM-specific parts
of rfd never apply here.

## Builders, in brief

Every dialog is a builder: `new()`, chain setters that return `Self`, then one
terminal call. The setters are shared between the sync and async variants.

```rust
rfd::AsyncFileDialog::new()
    .set_title("Export sprite sheet")
    .set_directory(&last_export_dir)   // remember where they were last
    .set_file_name("spritesheet.png")  // pre-fill the name
    .add_filter("PNG image", &["png"])
    .save_file()
    .await;
```

`add_filter(name, &["png", "PNG"])` adds one entry to the type dropdown; call it
once per file type. Filter *names* show on Windows and Linux; macOS uses only the
extensions.

For exhaustive method signatures, the full platform support matrix (which setters
are no-ops where), and the `save_file` extension-handling rules, read
[references/api.md](references/api.md).

## Rules that prevent the recurring bugs

- **In the shell, async only.** Synchronous `FileDialog`/`MessageDialog` are for
  contexts where blocking is fine — tests, a standalone CLI helper, a worker
  thread that has nothing to do but wait. Never on the egui update thread.
- **Always `request_repaint()` after sending the result back.** Forgetting this
  is the "the dialog result lags" bug.
- **Don't trust the saved path's extension.** `save_file` appends extensions
  inconsistently across platforms. Normalize the returned `PathBuf` in the `io`
  crate (append `.pixhaus`/`.png` if absent) before writing. Details in
  [references/api.md](references/api.md#save_file-extension-behavior).
- **`None` is cancellation, not an error.** A cancelled dialog returns
  `Option::None`, not an `Err`. Don't surface an error or log a warning when the
  user simply backed out.
- **Handle the file off the UI thread.** Once you have the `PathBuf`, decoding a
  large PNG or a `.pixhaus` archive is CPU work — keep it off the update loop
  (`spawn_blocking` or the same Tokio task), then send the loaded document back
  over the channel. The dialog and the load are two stages of one background
  flow.
- **No `unwrap`/`expect` outside tests.** Propagate with `?` / `thiserror` per
  [[pixhaus-rust-conventions]]. The dialog returning `None` is an expected
  branch, not an unwrap site.

## Decision shortcut

```
Need a native file/folder/message dialog?
├─ Are you on the egui update thread (anywhere reachable from `ui`)?
│    └─ yes → AsyncFileDialog/AsyncMessageDialog, tokio::spawn, oneshot channel,
│             request_repaint. NEVER the sync API here.
├─ In a test / CLI helper / dedicated worker where blocking is fine?
│    └─ yes → sync FileDialog/MessageDialog is simpler; use it.
├─ Want a themed, inline confirmation rather than the OS look?
│    └─ yes → draw an egui::Window modal instead (see pixhaus-egui), no rfd.
└─ Linux dependency question? → keep the default xdg-portal (no GTK libs).
                                 Add gtk3 only if a dialog behavior demands it.
```
