# SpriteMaster — research phase

The goal: build an AI-native 2D sprite editor, generator, and animation tool for game artists.

Before designing anything, this research phase maps the existing landscape — every meaningful tool currently used to create, edit, and animate 2D sprites. The point is to understand what artists already have, where the workflows hurt, and where AI can actually move the needle versus where it would be a regression.

## How this folder is organized

```
SpriteMaster/                    # workspace folder — public name is Pixhaus
├── README.md                    # this file
├── index.md                     # master tool list with quick comparison
│
│   # research phase — landscape map of existing tools
├── pixel-art-editors/           # tools built specifically for pixel-perfect work
├── general-purpose/             # painting tools artists co-opt for sprite work
├── skeletal-animation/          # bone-based 2D rigging tools
├── frame-by-frame/              # traditional cel and tween animation
├── engine-integrated/           # sprite tools baked into game engines
├── tilemap-level/               # tilemap and level editors (asset pipeline neighbors)
├── ai-native/                   # current generation of AI sprite tools
├── synthesis/                   # cross-tool patterns, gaps, opportunities
│
│   # build phase — what we're shipping
├── product/                     # locked scope (scope.md), AI capability map, naming
├── architecture/                # Tauri + Rust + TS lock-in, why Rust over Electron
├── ecosystem/                   # Rust/JS library reference + AI-driven Rust best practices
│   ├── 01-foundations.md
│   ├── 02-graphics-and-formats.md
│   ├── 03-ai-ml.md
│   ├── 04-scripting-and-testing.md
│   ├── 05-frontend-and-av.md
│   └── 06-rust-best-practices-2026.md
└── work/                        # parallel work organization for agent dispatch
    ├── README.md                # how to read this section
    ├── bedrock.md               # 8 contracts that must exist before fan-out
    └── streams.md               # 52 parallel work streams with agent briefs
```

Each category folder contains a `README.md` with overview and one `<tool>.md` per tool.

## Coverage

The list intentionally spans the full price and ideology spectrum — from $20 indie staples to $5K/year studio software, from open-source forks to web-based playgrounds, from hand-crafted pixel-perfect editors to diffusion-based generators. A tool that occupies a strong niche teaches you something even if the niche is small.

## What each tool file documents

Same template across every tool:

1. Quick facts — price, license, platforms, company, last meaningful update
2. Origin and purpose — why it exists, who it was built for
3. Drawing and painting tools — brushes, fills, transforms, selection
4. Pixel-specific features — pixel-perfect mode, dithering, palette constraints
5. Color and palette workflow — how palettes are managed and shared
6. Layer system — what layers exist, what operations are layer-aware
7. Animation features — timeline, onion skin, tweening, rigging
8. Export and import — formats, sprite sheets, atlases, metadata
9. Scripting and extensibility — automation surface, plugin ecosystem
10. Engine integration — Unity, Godot, Unreal, GameMaker pipelines
11. Workflow strengths — what it does better than anyone else
12. Workflow gaps — what it can't do or does badly
13. Notable uses — shipped games or projects
14. Community and ecosystem — assets, tutorials, third-party additions

## What this research feeds

Once the tool landscape is mapped, the synthesis folder pulls out the recurring patterns: which features every serious tool has, which tradeoffs the field has settled on, which workflows are still painful across the board, and where AI-native interaction could displace existing UX rather than just bolt onto it.
