---
title: Publishing plugins
description: Package, version, and distribute your Pixhaus plugin.
---

import { Aside, FileTree } from "@astrojs/starlight/components";

## Folder layout

A well-organized plugin looks like this:

<FileTree>

- my-plugin/
  - plugin.toml         ← required
  - main.lua            ← entry point (or plugin.wasm for WASM plugins)
  - icon.png            ← 32 × 32 PNG shown in the plugin manager (optional)
  - README.md           ← shown in the plugin manager description view
  - CHANGELOG.md        ← version history
  - LICENSE             ← include a license file; MIT and Apache-2.0 are common choices
  - lib/                ← optional: split large plugins across files
    - util.lua
  - assets/             ← optional: images, palettes, etc.

</FileTree>

Required files are `plugin.toml` and your entry point. Everything else is optional but strongly recommended for discoverability.

---

## Package format

A Pixhaus plugin is distributed as a `.zip` archive. The archive must contain the plugin folder at the root:

```
my-plugin-0.1.0.zip
└── my-plugin/
    ├── plugin.toml
    ├── main.lua
    └── icon.png
```

The folder name inside the zip becomes the installed plugin's ID. Match it to your plugin's `name` slug (lowercase, hyphens, no spaces).

### Building the zip

```sh
# From the directory containing your plugin folder
zip -r my-plugin-0.1.0.zip my-plugin/
```

For WASM plugins, compile first, then package:

```sh
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/my_plugin.wasm my-plugin/plugin.wasm
zip -r my-plugin-0.1.0.zip my-plugin/
```

---

## Installing a plugin

### From a zip file

Users install via **Edit > Plugins > Install from file**. Pixhaus extracts the plugin folder into `~/.pixhaus/plugins/` and loads it immediately.

### Manual install

Copy the plugin folder directly:

```sh
cp -r my-plugin ~/.pixhaus/plugins/
```

Changes take effect immediately (hot-reload) or on next editor launch.

---

## Distributing on GitHub

The simplest distribution method requires no registry:

1. Create a GitHub repository named `pixhaus-<plugin-name>` (the `pixhaus-` prefix makes the plugin easy to find).
2. Put the plugin folder at the repository root.
3. Add a `README.md` with a description, screenshots, and install instructions.
4. Create a GitHub Release for each version:
   - Tag: `v0.1.0`
   - Attach: `my-plugin-0.1.0.zip`
5. Link to the plugin in the [community plugins discussion](https://github.com/pixhaus-app/pixhaus/discussions).

Users can install from the release zip or clone the repo directly.

### Suggested README structure

```markdown
# My Plugin — short tagline

Screenshot or demo GIF here.

## Install

Download [my-plugin-0.1.0.zip](https://github.com/you/pixhaus-my-plugin/releases/latest)
and install via **Edit > Plugins > Install from file**.

## What it does

...

## Permissions requested

- `commands` — registers N command palette entries
- `verbs` — registers the "X" AI verb

## Changelog

### 0.1.0
- Initial release
```

---

## Versioning

Follow [Semantic Versioning](https://semver.org/):

| Change | Version bump |
|---|---|
| New features, new commands | MINOR (`0.1.0` → `0.2.0`) |
| Bug fixes only | PATCH (`0.1.0` → `0.1.1`) |
| Removed features or API breaks | MAJOR (`0.1.0` → `1.0.0`) |

Keep `version` in `plugin.toml` in sync with your release tags.

---

## Compatibility

Include a `pixhaus` field in `plugin.toml` to declare which editor versions your plugin supports:

```toml
[plugin]
name    = "My Plugin"
version = "0.1.0"
pixhaus = ">=0.5.0"      # semver constraint on the editor version
# ...
```

The editor checks this on install and shows a warning if the constraint is not satisfied. Omitting the field means "any version."

---

## Signing (future)

A future release will add Ed25519 plugin signing so users can verify publisher identity. Prepare for this now by generating a keypair:

```sh
openssl genpkey -algorithm ed25519 -out my-plugin-private.pem
openssl pkey -in my-plugin-private.pem -pubout -out my-plugin-public.pem
```

Keep the private key secret. When the signing spec ships, you will add your public key to `plugin.toml` and sign the archive with the private key. Unsigned plugins will show a "Unverified publisher" badge in the plugin manager.

---

## Plugin registry (planned)

A centralized plugin registry at `plugins.pixhaus.app` is planned. When it launches, you will be able to submit your plugin for listing. Signed plugins will get a verified badge. The install flow will allow one-click install from the registry within the editor.

Until the registry exists, GitHub + the community discussions are the discovery path.
