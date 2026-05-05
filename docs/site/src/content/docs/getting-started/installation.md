---
title: Installation
description: Download and install Pixhaus on Windows, macOS, or Linux.
---

import { Tabs, TabItem, Aside } from "@astrojs/starlight/components";

<Aside type="caution">
Pixhaus is in active development. Pre-release builds are available; stable 1.0 is not yet shipped.
</Aside>

## Download

Grab the latest installer from the [GitHub releases page](https://github.com/pixhaus-app/pixhaus/releases).

<Tabs>
  <TabItem label="Windows">
    Download `pixhaus-x.y.z-windows-x64.msi` and run the installer.

    System requirements:
    - Windows 10 1903 or later (Windows 11 recommended)
    - GPU with DirectX 11 or Vulkan support
    - 4 GB RAM minimum, 8 GB recommended
  </TabItem>
  <TabItem label="macOS">
    Download `pixhaus-x.y.z-macos.dmg`. Drag Pixhaus to Applications.

    System requirements:
    - macOS 11 (Big Sur) or later
    - Intel or Apple Silicon (universal binary)
    - GPU with Metal support
    - 4 GB RAM minimum, 8 GB recommended
  </TabItem>
  <TabItem label="Linux">
    Download the `.deb`, `.rpm`, or AppImage for your distribution.

    System requirements:
    - glibc 2.31 or later
    - Wayland or X11
    - Vulkan-capable GPU (Intel, AMD, NVIDIA)
    - 4 GB RAM minimum
    
    AppImage is the most portable option — no install required, just make it executable and run it.
  </TabItem>
</Tabs>

## Build from source

```bash
# Prerequisites: Rust stable, Node 22+, pnpm 10+

git clone https://github.com/pixhaus-app/pixhaus.git
cd pixhaus
pnpm bootstrap     # installs dependencies, first-time setup
pnpm dev           # opens Pixhaus with hot-module reload
```

See [CONTRIBUTING.md](https://github.com/pixhaus-app/pixhaus/blob/main/CONTRIBUTING.md) for a full development environment guide.

## Verifying the installation

Open Pixhaus. The application shell should appear with the command palette accessible via `Ctrl+K` (Windows/Linux) or `Cmd+K` (macOS). If nothing opens, check the [FAQ](/faq/) for common issues.
