# Storage and persistence

eframe 0.34.2. How app state survives a restart: the `Storage` trait, the `get_value` /
`set_value` / `storage_dir` helpers, what persists where, and the `persistence` feature
that gates all of it.

## Contents
- The `persistence` feature (gates everything)
- The save/restore cycle
- The `Storage` trait
- `get_value` / `set_value` / `storage_dir`
- What persists, and where
- What belongs in Storage (and what doesn't)

## The persistence feature gates everything

Without the `persistence` cargo feature, none of this runs: `App::save` is never called,
`cc.storage` is always `None`, `persist_window` and `persist_egui_memory` do nothing. If
saving "silently doesn't work," check the feature first.

```toml
eframe = { version = "0.34", default-features = false, features = ["wgpu", "persistence"] }
```

## The save/restore cycle

Two halves, both keyed by string:

- **Restore** in your `AppCreator` (`new`), reading `cc.storage` with `eframe::get_value`.
- **Save** in `App::save`, writing with `eframe::set_value`. eframe calls `save` on shutdown
  and every `auto_save_interval` (default 30s).

```rust
impl Pixhaus {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ui_prefs = cc.storage
            .and_then(|s| eframe::get_value::<UiPrefs>(s, "ui_prefs"))
            .unwrap_or_default();
        Self { ui_prefs, /* ... */ }
    }
}

impl eframe::App for Pixhaus {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "ui_prefs", &self.ui_prefs);
    }
}
```

`UiPrefs` must be `serde::Serialize + DeserializeOwned`. `get_value` returns `None` on first
run or on a deserialize failure (e.g. you changed the struct shape) — `unwrap_or_default()`
makes that a clean reset rather than a crash. If you must survive schema changes, version
your prefs struct or store under a new key.

## The Storage trait

```rust
pub trait Storage {
    fn get_string(&self, key: &str) -> Option<String>;
    fn set_string(&mut self, key: &str, value: String);
    fn flush(&mut self);
}
```
The low-level key→string store: browser local-storage on web, a file on desktop. You usually
don't touch it directly — `get_value`/`set_value` wrap it with serialization. Reach for
`get_string`/`set_string` only to store a raw string yourself. `flush` writes pending data
out; eframe flushes for you around `save`, so manual calls are rare.

`Frame` exposes the live store at runtime via `frame.storage()` / `frame.storage_mut()` if
you need to read or write outside the `save` hook.

## get_value / set_value / storage_dir

```rust
pub fn get_value<T: serde::de::DeserializeOwned>(storage: &dyn Storage, key: &str) -> Option<T>;
pub fn set_value<T: serde::Serialize>(storage: &mut dyn Storage, key: &str, value: &T);
pub fn storage_dir(app_id: &str) -> Option<PathBuf>;
```

- `get_value` deserializes a value stored under `key` from RON; `None` if absent or
  malformed.
- `set_value` serializes `value` to RON and stores it under `key`. Takes `value` by
  reference.
- `storage_dir(app_id)` returns the OS directory eframe uses for that app id — useful for
  logging where state lives or placing sibling files. The `app_id` is the first argument you
  passed to `run_native`.

eframe serializes to **RON** (Rusty Object Notation), not JSON. That's an eframe-internal
detail — it doesn't change how you call the helpers, but it's why the on-disk file is `.ron`
and why your types just need serde derives.

## What persists, and where

Three independent things persist, each with its own switch:

| What | Controlled by | Notes |
|---|---|---|
| Your custom data | `App::save` + `get_value`/`set_value` | You choose the keys and the shape. |
| Window position/size | `NativeOptions::persist_window` | Auto-restored next launch. |
| egui memory (panel sizes, collapsed state, scroll, last-focused) | `App::persist_egui_memory` (default `true`) | egui's own widget state. |

Location: the OS config/state dir for `app_id` (e.g. on Windows under the user's
`AppData`), unless you override with `NativeOptions::persistence_path`. Query the resolved
path with `eframe::storage_dir(app_id)`.

## What belongs in Storage (and what doesn't)

Persist **preferences and small UI state** — theme choice, panel layout, recent-files list,
last-used tool. Keep `save` cheap: it runs every 30s while the app is live, so serializing a
large structure there will hitch the UI.

Do **not** put the document (the pixel buffers, layers, the undo stack) in eframe Storage.
That's what the `.pixhaus` project format is for — MessagePack + zstd, written explicitly on
the user's save action through the `io` crate, not auto-serialized to RON every 30 seconds.
The dividing line: eframe Storage is for "how the editor was set up," the project file is for
"what the user made." Mixing them turns autosave into a multi-megabyte stall and couples the
document model to eframe.

## Flagged / verify

- The `get_value`/`set_value` generic bounds (`DeserializeOwned` / `Serialize`) and the
  by-reference `value` argument match eframe's long-standing signatures but were not fully
  rendered in the scraped docs — confirm against docs.rs if the compiler argues about bounds
  or value-vs-reference.
- `storage_dir`'s exact return (`Option<PathBuf>`) and argument name are as documented on the
  crate index; verify if you depend on the precise path resolution.
