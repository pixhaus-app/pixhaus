---
name: pixhaus-directories
description: >
  Use when Pixhaus needs a platform-correct location on disk for its own files
  with the `directories` crate — where to put preferences/keybindings, the
  thumbnail/tile/AI-result cache, palettes/brushes/Lua scripts/plugins, logs, or
  the default folder an open/save/export dialog should start in. Trigger this for
  ANY "where do config/cache/data files go", "find the user's config dir", "app
  data directory", "AppData / Library / XDG / .config / .cache path", "default
  export folder", "ProjectDirs / BaseDirs / UserDirs", or "this path is wrong on
  macOS/Windows" request, and whenever you see `directories::ProjectDirs`,
  `BaseDirs`, `UserDirs`, `::from(`, `cache_dir()`, `config_dir()`, `data_dir()`,
  or `dirs::config_dir`. Two traps make this worth stopping for: `directories`
  only COMPUTES paths — it never creates the folders, so a first-run write fails
  with "No such file or directory" unless you `create_dir_all` first — and on
  macOS `config_dir()` and `data_dir()` are the SAME folder, so same-named files
  collide. Reach for this skill rather than hand-rolling `$HOME`/`%APPDATA%`
  joins, which break on at least one platform.
---

# directories for Pixhaus

`directories` answers one question: given Pixhaus needs to stash a file, where on
*this* OS does it belong? It computes platform-correct, standards-compliant paths
— XDG Base/User dirs on Linux, the Known Folder API on Windows, the macOS
directory guidelines — so the shell never hardcodes `$HOME/.config` or
`%APPDATA%` and never gets it wrong on a platform you didn't test on.

It is "a tiny library with a minimal API (3 structs, 4 factory functions,
getters)". The whole skill is choosing the right struct and the right getter, then
remembering the two things the crate does *not* do for you (create the folder,
keep macOS config/data apart).

## Version and license

| Crate | Version | License | cargo deny |
|---|---|---|---|
| `directories` | 6.0 | `MIT OR Apache-2.0` | passes the MIT lock |

Permissive metadata, so it clears the workspace MIT lock. Pulls in `dirs-sys`
(also MIT/Apache) for the OS calls; nothing to configure.

```toml
directories = "6"
```

Don't confuse it with the lower-level `dirs` crate (bare `dirs::config_dir()`
functions with no app namespacing). Pixhaus wants the namespacing `ProjectDirs`
gives — one folder per app, derived correctly per platform — so reach for
`directories`, not `dirs`.

## The three structs — pick by who owns the file

```
A file Pixhaus owns (its config, its cache, its data)?
  └─ ProjectDirs::from("", "Pixhaus", "Pixhaus")   ← the default, ~everything

A user-facing standard folder (Documents, Pictures, Downloads)?
  └─ UserDirs                                       ← default dir for save/export dialogs

A base location NOT scoped to any app (rare here)?
  └─ BaseDirs                                        ← only if ProjectDirs doesn't fit
```

Almost all Pixhaus storage is app-owned, so `ProjectDirs` is the workhorse. Reach
for `UserDirs` only to seed a file dialog at the user's Pictures/Documents folder.
`BaseDirs` is the un-namespaced base (`~/.config`, not `~/.config/pixhaus`) — you
rarely want it once `ProjectDirs` exists.

## ProjectDirs — the main path

`from(qualifier, organization, application)` returns `Option<ProjectDirs>` — `None`
only when no home directory can be found (a degenerate environment). Build it once
and reuse the owned `PathBuf`s; do not call `from` per access (it re-reads env vars
and OS folders and allocates every time).

```rust
use directories::ProjectDirs;

// qualifier is a reverse-domain-ish prefix that only affects the macOS bundle name;
// "" is fine for an open-source app with no domain. Define this ONE triple in the
// shell crate so every path agrees.
let dirs = ProjectDirs::from("", "Pixhaus", "Pixhaus")
    .ok_or(StorageError::NoHomeDir)?;   // map to a thiserror variant; never unwrap

let cache = dirs.cache_dir();   // &Path — see the platform table below
```

### Getters (example: qualifier `"com"`, organization `"Foo Corp"`, application `"Bar App"`)

| Method | Return | Linux | macOS | Windows |
|---|---|---|---|---|
| `project_path()` | `&Path` | `barapp` | `com.Foo-Corp.Bar-App` | `Foo Corp\Bar App` |
| `cache_dir()` | `&Path` | `~/.cache/barapp` | `~/Library/Caches/com.Foo-Corp.Bar-App` | `…\AppData\Local\Foo Corp\Bar App\cache` |
| `config_dir()` | `&Path` | `~/.config/barapp` | `~/Library/Application Support/com.Foo-Corp.Bar-App` | `…\AppData\Roaming\Foo Corp\Bar App\config` |
| `config_local_dir()` | `&Path` | `~/.config/barapp` | `~/Library/Application Support/com.Foo-Corp.Bar-App` | `…\AppData\Local\Foo Corp\Bar App\config` |
| `data_dir()` | `&Path` | `~/.local/share/barapp` | `~/Library/Application Support/com.Foo-Corp.Bar-App` | `…\AppData\Roaming\Foo Corp\Bar App\data` |
| `data_local_dir()` | `&Path` | `~/.local/share/barapp` | `~/Library/Application Support/com.Foo-Corp.Bar-App` | `…\AppData\Local\Foo Corp\Bar App\data` |
| `preference_dir()` | `&Path` | `~/.config/barapp` | `~/Library/Preferences/com.Foo-Corp.Bar-App` | `…\AppData\Roaming\Foo Corp\Bar App\config` |
| `runtime_dir()` | `Option<&Path>` | `/run/user/1001/barapp` | `None` | `None` |
| `state_dir()` | `Option<&Path>` | `~/.local/state/barapp` | `None` | `None` |

How the name is built: Linux uses the **application** only, lowercased with spaces
stripped (`barapp`). macOS joins all three with dots, spaces → hyphens, case kept
(`com.Foo-Corp.Bar-App`) — so the qualifier is the *only* reason to set one.
Windows uses **organization\application** verbatim, spaces and case kept.

### Which getter for which Pixhaus file

- **`config_dir()`** — preferences, keybindings, recent-files list, window layout.
  Small, portable, the kind of thing a user would want to follow them. Roams on
  Windows.
- **`cache_dir()`** — anything regenerable: thumbnails, the tile cache, decoded
  sprite caches, AI model/result caches. Always *local* on Windows (never roams),
  which is exactly right — you don't sync a multi-gig cache across machines.
- **`data_dir()` / `data_local_dir()`** — user-created persistent assets that
  aren't the `.pixhaus` project itself: palettes, brushes, custom templates, Lua
  scripts, installed plugins. Use `data_local_dir()` for anything large.
- Note `.pixhaus` *project files* are not app-data — those live wherever the user
  saves them, reached via a `UserDirs` dialog, not under `ProjectDirs`.

`from_path(PathBuf)` exists but is "strongly discouraged" — it uses the path
verbatim and breaks OS conventions on at least two platforms. Don't.

## UserDirs — the user's own folders (file-dialog defaults)

`UserDirs::new() -> Option<UserDirs>`. `home_dir()` returns `&Path`; every other
getter returns `Option<&Path>` because the folder may be unconfigured or unsupported.

| Method | Linux | macOS | Windows |
|---|---|---|---|
| `home_dir()` `&Path` | `~` | `~` | `C:\Users\Alice` |
| `picture_dir()` | `XDG_PICTURES_DIR` | `~/Pictures` | `…\Pictures` |
| `document_dir()` | `XDG_DOCUMENTS_DIR` | `~/Documents` | `…\Documents` |
| `download_dir()` | `XDG_DOWNLOAD_DIR` | `~/Downloads` | `…\Downloads` |
| `desktop_dir()` | `XDG_DESKTOP_DIR` | `~/Desktop` | `…\Desktop` |
| `video_dir()` | `XDG_VIDEOS_DIR` | `~/Movies` | `…\Videos` |
| `audio_dir()` | `XDG_MUSIC_DIR` | `~/Music` | `…\Music` |
| `font_dir()` | `~/.local/share/fonts` | `~/Library/Fonts` | `None` |

Use `picture_dir()` (falling back to `document_dir()`, then `home_dir()`) as the
starting folder when you open an export-PNG or open-project dialog — it puts the
picker where a pixel-artist expects, instead of the process CWD. Treat every
`Option` as "may be absent" and fall back, don't unwrap.

## BaseDirs — un-namespaced bases (you usually want ProjectDirs instead)

`BaseDirs::new() -> Option<BaseDirs>`, then `home_dir()`/`cache_dir()`/`config_dir()`/
`data_dir()`/`config_local_dir()`/`data_local_dir()`/`preference_dir()` (all `&Path`)
and `executable_dir()`/`runtime_dir()`/`state_dir()` (all `Option<&Path>`, `None`
off Linux). These are the *parent* locations (`~/.config`), without the per-app
subfolder. Reach for `BaseDirs` only when you genuinely need a location not tied
to the Pixhaus app namespace — otherwise `ProjectDirs` is the safer choice because
it keeps every app's files apart.

## Gotcha 1: it computes paths, it does not create folders

The single most common bug. Every getter returns a path that **may not exist on
disk** — on first run, none of them do. Writing straight to `dirs.config_dir()`
fails with "No such file or directory". Always create the directory first:

```rust
use std::fs;

let config_dir = dirs.config_dir();
fs::create_dir_all(config_dir)?;            // idempotent; safe to call every time
fs::write(config_dir.join("prefs.msgpack"), &bytes)?;
```

`create_dir_all` is cheap and a no-op when the folder already exists, so call it
right before the write rather than trying to track whether you've created it.

## Gotcha 2: on macOS, config_dir and data_dir are the same folder

Look at the table: on macOS `config_dir()`, `data_dir()`, `data_local_dir()`, and
`config_local_dir()` all resolve to `~/Library/Application Support/<bundle>`. Only
Linux and Windows keep config and data apart. So a file written as
`config_dir().join("state")` and another as `data_dir().join("state")` are the
**same file on macOS** and silently clobber each other.

Don't rely on the config/data split for namespacing. Either give files distinct
names, or put each category in its own subfolder you control:

```rust
let prefs = dirs.config_dir().join("prefs.msgpack");
let palettes = dirs.data_dir().join("palettes");   // distinct leaf name survives the macOS merge
fs::create_dir_all(&palettes)?;
```

## Gotcha 3: preference_dir on macOS is Apple's, not yours

`preference_dir()` on macOS is `~/Library/Preferences`, which the system reserves
for `.plist` files it manages through `NSUserDefaults`. Don't drop arbitrary
Pixhaus files there. Use `config_dir()` for your own config; on Linux and Windows
`preference_dir()` equals `config_dir()` anyway, so just use `config_dir()`
everywhere and forget `preference_dir()` exists.

## Gotcha 4: the Option-returning getters are None off Linux

`runtime_dir()`, `state_dir()`, and `BaseDirs::executable_dir()` return `None` on
macOS and Windows; `UserDirs::font_dir()` is `None` on Windows; `template_dir()`
is `None` on macOS. Match the `Option` and fall back — never `unwrap()` a path
getter, or Pixhaus panics on the platform you didn't run.

## Interplay with eframe's own storage

eframe already persists egui state and your serializable `App` fields to its own
per-app location (see `pixhaus-eframe`'s Storage / `get_value`/`set_value`). Don't
reinvent that with `directories` — let eframe own window layout and small UI
state. Use `directories` for what eframe Storage does *not* cover: the regenerable
caches (`cache_dir`), bulky user assets like brushes/palettes/scripts (`data_dir`),
logs, and file-dialog default folders (`UserDirs`).

## Errors and the no-unwrap rule

`new()`/`from()` return `Option`, not `Result` — a `None` means no home dir. In a
library crate, convert it into a `thiserror` variant (`#[error("no home directory")]`)
with `.ok_or(...)`; `anyhow` stays in the binary. The follow-on `fs::create_dir_all`
/ `fs::write` calls return `io::Result` — map those with `#[from] std::io::Error`.
Never `unwrap()` a directories getter or an `fs` call outside tests; a missing
home dir or an unwritable disk is a user-facing error to report, not a panic. See
`pixhaus-rust-conventions`.

## Testing paths

The getters read live env vars (`XDG_*`, `HOME`, `APPDATA`) and OS APIs, so their
output varies by machine — don't snapshot the absolute paths they return. Test
*your* logic instead: that you join the right leaf, that you `create_dir_all`
before writing, that a `None` home is handled. Drive those tests against a
`tempfile::tempdir()` you control rather than the real `ProjectDirs` output, and
inject the base path so the test never touches the developer's real config dir.
See `pixhaus-testing-conventions`.

## Decision shortcut

```
Need a path on disk in Pixhaus?
├─ A file Pixhaus owns?
│    ├─ regenerable (thumbnails, tiles, AI results)? → ProjectDirs::cache_dir()  (local, never roams)
│    ├─ small portable settings (prefs, keybinds)?    → ProjectDirs::config_dir()
│    └─ user assets (palettes, brushes, scripts)?     → ProjectDirs::data_dir() / data_local_dir()
├─ Default folder for an open/save/export dialog?     → UserDirs::picture_dir() → document_dir() → home_dir()
├─ The .pixhaus project file itself?                  → not here — wherever the user picks (UserDirs dialog)
└─ Small UI/window state egui already persists?        → eframe Storage, not directories
ALWAYS: fs::create_dir_all(dir)? before the first write — directories never makes the folder.
NEVER:  assume config_dir != data_dir on macOS (same folder), or unwrap a getter (None off Linux).
```
