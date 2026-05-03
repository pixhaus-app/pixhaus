# Rust vs Electron — the answer

## The short answer

Yes, Rust is the better choice for Pixhaus. The fact that you've never written it doesn't change that. Building a sprite editor on Electron is the kind of decision you regret 18 months in, when the binary is 150MB, startup is three seconds, and pushing pixel buffers through V8 has slowed everything to a crawl. Building it on Tauri (Rust core + web UI) hits Aseprite-class performance with a 10MB binary and gives you a reason to learn one of the most useful systems languages of the decade. The learning curve is real but bounded, and AI agents in 2026 are strong enough at Rust that you won't be alone in the deep end.

## Why a sprite editor specifically punishes Electron

Sprite editors are unusually demanding for what they look like. The reasons:

The work is dominated by pixel-buffer manipulation. Every brush stroke, every layer composite, every blend mode operation, every undo/redo allocation, every onion-skin render is a loop over pixel arrays. In JavaScript these loops live inside V8 with all the boxing, type-coercion, and garbage-collection overhead that implies. In Rust they're cache-friendly tight loops with SIMD where it helps. The performance gap on this specific workload is 5-50x depending on the operation. Aseprite is written in C++ and feels instantaneous. Pixelorama is written on Godot's GDScript and feels sluggish. The language matters here.

Memory pressure is high and predictable. A 256x256 sprite with 50 frames and 10 layers is 30MB of pixel data uncompressed; the undo stack pushes that toward 300MB. Electron eats 100-150MB before you load the first sprite. On a 16GB Mac that's tolerable; on a 8GB Linux laptop running Discord and Slack, it's not. Tauri sits at 30-40MB resting and the savings stack with whatever you load on top.

Binary size matters for an open-source indie tool. Aseprite ships ~5MB. Pixelorama ships ~80MB. Electron apps ship 100-150MB minimum. People download and try indie tools the way they don't download and try enterprise tools — the friction of a fat installer is real. Tauri Pixhaus could ship at 8-15MB.

Startup time matters. Aseprite cold-starts in under a second on any machine made this decade. Electron apps take 2-4 seconds to show their splash. The artist's mental model of "open the tool, fix the thing" wants the first number, not the second.

## What Electron does well that we'd be giving up

The Electron side of the trade isn't nothing.

You'd be writing UI in TypeScript / React / Vue / Svelte the way the entire frontend ecosystem does. AI agents produce excellent code in this stack. Iteration is fast — hot reload is instant, the dev tools are mature, and the talent pool to recruit from is the largest in software.

You'd skip the Rust learning curve. The borrow checker fights, the lifetime annotations, the `Arc<Mutex<>>` pattern matching when you need shared mutable state — none of it is in your way.

You'd ship faster on day one. An Electron prototype is up and running in an hour. A Tauri prototype takes a day. A Rust-heavy Tauri prototype takes three days.

These are real costs. They are not the dominant costs.

## What Tauri specifically gets right

Tauri is the version of "Rust + web UI" that actually works for product builders. The architecture is:

- **Rust core process** owns the heavy work: image manipulation, file I/O, color math, blending, palette ops, AI inference orchestration.
- **WebView UI** owns the visual layer: canvas, panels, command palette, theming, animations. You write it in whatever frontend stack you like (the Pixhaus call would be TypeScript + a lightweight framework — Solid, Svelte, or vanilla TS with a small reactive core).
- **IPC bridge** between them — Tauri commands. The UI calls into Rust by name; Rust exposes async functions; the protocol is JSON-RPC-ish under the hood.

The split maps naturally to what a sprite editor needs. The Rust side does what Rust is good at (memory layout, performance, file formats, concurrency). The web side does what web is good at (interactive UI, theming, command palettes, animations). Neither is forced into a domain it doesn't fit.

The web view is system-provided — Tauri uses your OS's native webview (WebView2 on Windows, WebKit on macOS, WebKitGTK on Linux). That's why the binary is 5-10MB instead of 100MB: there's no bundled Chromium. The cost is that you can't assume bleeding-edge Chrome features; the benefit is you ship a tool that doesn't ship a browser.

## On the Rust learning curve specifically

Rust has a reputation for being hard. The reputation is half-deserved, half-mythologized.

The borrow checker is the actual hard part for the first month. You'll fight it. You'll learn to fight it less. After roughly 80-120 hours of writing Rust, the patterns become natural and you start writing code that compiles on the first try because you've internalized the rules. Below that threshold it's frustrating; above it it's pleasant.

Most of the rest of Rust is a normal modern language: pattern matching, traits (interfaces), generics, async/await, an excellent standard library, an even better third-party ecosystem (`crates.io`). Once you're past the borrow checker, you're past the wall.

A sprite editor is actually a great Rust learning project. The reasons:

The data structures are bounded. You're dealing with rectangular pixel buffers, palettes, layers, frames. You don't have to also learn networking + databases + serialization across the wire + auth. Domain complexity is contained, so you can focus on language complexity.

Performance feedback is immediate and visible. Make a change, run a brush stroke benchmark, see the number. This is the kind of language-as-instrument feedback that makes learning Rust enjoyable.

Memory ownership maps cleanly onto image manipulation. A pixel buffer is the canonical "single owner with shared read access" pattern. Brush strokes are mutations that need exclusive access. Undo stacks are immutable snapshots. The borrow checker is teaching you exactly what you'd want to know about how this code should be structured.

The concurrency story matches your needs. AI inference, file I/O, and rendering all want to run off the main thread. Rust's `tokio` async + `rayon` data parallelism + `Arc` for shared state is the right toolkit for this. Learning these patterns once means you'll reach for them in every future project.

## On AI agents and Rust quality

In May 2026, Claude Opus 4.6 and Sonnet 4.6 produce production-quality Rust. Same for the GPT-5 class of OpenAI models. The era when "agents are better at TypeScript" was a meaningful argument for tech-stack choice closed in 2024-2025. Modern frontier models write idiomatic Rust, navigate the borrow checker correctly, and reach for the right ecosystem crates without prompting.

Where agents still struggle is the same places humans struggle: complex unsafe code, intricate lifetime gymnastics in generic library code, and tricky async cancellation. None of those are core to a sprite editor. The image-manipulation, file-I/O, and command-orchestration code that makes up 80% of the Rust surface here is firmly in the agent comfort zone.

The actual implication for your build: you can dispatch agents to write Rust modules, review the output, and learn by reading what they produce. The codebase becomes a teaching resource. This is genuinely a better way to learn Rust than any tutorial — you read working code in the domain you care about, you understand why each pattern was chosen, and you internalize the idiom by exposure.

## What changes if you choose Electron anyway

Honest framing: if you'd rather ship faster and never learn Rust, Electron is a defensible choice. Pixelorama runs on Godot which is its own kind of "non-native UI runtime" and survives. Many beloved tools are Electron (VS Code, Slack, Discord, Linear). It's not a death sentence.

What changes:

You lose the performance ceiling. Pixhaus on Electron will never feel as fast as Aseprite. It will feel like Pixelorama or Krita-on-a-bad-day. You'll spend engineering time fighting V8 instead of building features.

You take a binary-size hit that hurts indie distribution. Plan on 100MB+ installers. Plan on people trying it once, finding it slow, and going back to Aseprite.

You ship faster on day one and ship slower on day 365. The compounding cost of every brush operation being 10x slower than it could be is real. By the time you discover this, the codebase is too large to port.

You don't learn Rust. That's a personal cost, not a technical one.

## The recommendation, in one paragraph

Pick Tauri with Rust core and TypeScript UI. Accept that the first two weeks will be frustrating while you learn enough Rust to be productive. Use AI agents to accelerate the curve — dispatch agents on Rust modules, read their output, ask them why they made specific choices. Treat the Rust core as the teaching codebase you've always wanted. The performance ceiling matches what a sprite editor actually demands, the binary stays indie-friendly, and you finish the project with a real new skill rather than a more crowded electron `node_modules`.

If after two weeks of trying Rust you genuinely hate it and your iteration speed has cratered, you can fall back to Electron without throwing away most of the work — the data structures and file format specs are language-agnostic, and the UI is web-stack either way. The downside risk is bounded. The upside isn't.
