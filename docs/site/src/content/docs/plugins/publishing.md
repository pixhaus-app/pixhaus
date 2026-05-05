---
title: Publishing plugins
description: Package and distribute your Pixhaus plugin.
---

import { Aside } from "@astrojs/starlight/components";

<Aside>
A plugin registry is planned but not yet live. For now, publish plugins on GitHub and share the install link.
</Aside>

## Package format

A Pixhaus plugin is a `.zip` archive containing the plugin folder:

```
my-plugin-0.1.0.zip
└── my-plugin/
    ├── plugin.toml
    ├── main.lua        (or plugin.wasm)
    └── icon.png        (optional)
```

## Installing from a zip

Users can install a plugin zip via `Edit > Plugins > Install from file`. The plugin folder is extracted to `~/.pixhaus/plugins/`.

## Distributing on GitHub

The simplest distribution method:

1. Create a GitHub repository for your plugin.
2. Include the plugin folder at the repo root.
3. Tag releases with the version number (`v0.1.0`).
4. Users install by cloning or downloading the release zip.

Add your plugin to the [community plugins list](https://github.com/pixhaus-app/pixhaus/discussions) so others can find it.

## Signing (future)

A future release will add plugin signing to verify publisher identity. Unsigned plugins will show a warning on install. Prepare for this by generating an Ed25519 keypair and including your public key in `plugin.toml` when the signing spec is finalized.
