# Pixhaus Rust Runtime, State, UI, Concurrency, and Library Companion

## Purpose

This document is a companion to the Pixhaus Architecture Bible and the Pixhaus Save File Format Architecture document.

Its purpose is to define the recommended Rust runtime and application infrastructure stack for Pixhaus: how state should be managed, how egui should be used safely, how UI state should be separated from project state, how localization and logging should work, and how Pixhaus should use concurrency and parallelism without turning the application into a threading mess.

This document intentionally excludes `egui_mobius`. Pixhaus should use a custom architecture built around its own project model, command system, workspace registry, job manager, asset system, and provider registry.

Pixhaus is a native Rust application using egui. It targets Windows, macOS, and Linux. It should support modern workstation hardware, including multi-core CPUs and GPU-backed rendering/compute where appropriate.

---

## Core Position

Pixhaus should not look for a generic state-management framework to become its architecture.

Pixhaus is not a dashboard. It is a native creative application with a large amount of domain-specific state:

- projects
- assets
- sprites
- layers
- frames
- cels
- brushes
- selections
- timelines
- animation clips
- palettes
- AI recipes
- generated assets
- undo/redo history
- workspace layouts
- provider configuration
- background jobs
- caches

The correct approach is:

> Use Rust crates for infrastructure, but own the Pixhaus app state architecture.

Use libraries for:

- UI rendering
- docking
- localization
- logging/tracing
- async I/O
- CPU parallelism
- channels
- app directories
- serialization
- compression
- hashing
- image I/O
- native file dialogs
- profiling

But Pixhaus should own:

- project state
- UI state boundaries
- editing context
- workspace registry
- module registry
- command system
- undo/redo
- job manager
- asset cache
- AI provider registry
- save/load lifecycle
- dirty state tracking
- project format semantics

---

## Immediate-Mode UI Implication

Pixhaus uses egui, which is an immediate-mode GUI library. This means the UI is redrawn from the current state rather than being represented as a long-lived retained widget tree.

This is excellent for developer velocity, custom tooling, panels, debug views, docked workspaces, and native utility interfaces.

However, it also means Pixhaus must avoid letting egui widget code become the application state model.

The discipline should be:

```text
egui draws current state
egui collects user intent
commands mutate durable project state
jobs produce async/background results
render caches display project-derived imagery
```

Do not let UI widgets own durable creative data.

---

## State Buckets

Pixhaus state should be explicitly separated into five categories.

### 1. Project State

Project state is durable creative data. It belongs to the project and is saved to disk.

Examples:

- sprites
- canvases
- documents
- layers
- frames
- cels
- animation clips
- frame durations
- palettes
- color ramps
- references
- imported assets
- generated assets
- AI recipes
- prompt templates
- output structures
- visual styles
- export presets that belong to the project
- project metadata

Project state must be serializable and versioned.

Project state should never depend on egui types.

### 2. Session State

Session state belongs to the currently running app session.

Examples:

- open project
- active project path
- active sprite
- active asset
- active document
- active frame
- active layer
- active workspace
- active tool
- current provider
- running job list
- selected generated result
- recent commands
- dirty project status
- autosave state

Some session state may be persisted across launches, but it is not core project content.

### 3. UI State

UI state belongs to the interface.

Examples:

- panel collapsed/expanded state
- tab selection
- scroll position
- hovered timeline cell
- selected list row in a panel
- filter text
- search field text
- modal stack
- drag/drop state
- command palette open state
- workspace layout
- dock tree
- window positions

UI state may be saved as user preferences or workspace layout files, but it should not pollute project data.

### 4. Tool Interaction State

Tool interaction state is transient and usually exists only during direct manipulation.

Examples:

- current brush stroke
- lasso points
- transform preview
- selection drag start
- frame scrub operation
- color picker hover sample
- canvas pan gesture
- shape preview rectangle
- tile stamp preview
- AI brush masked region

Tool interaction state should be small, explicit, and cleared when the interaction ends or is cancelled.

### 5. Derived and Cache State

Derived state can be recomputed.

Examples:

- composited frame textures
- timeline thumbnails
- asset thumbnails
- generated result previews
- palette usage analysis
- dirty region maps
- coverage analysis
- compiled prompt previews
- texture handles
- GPU buffers
- image decode cache
- save/load indexes

Caches are never the source of truth.

They should be keyed by stable IDs, content hashes, revision counters, dirty regions, or asset versions.

---

## Golden State Rule

The main Pixhaus app/session owns the authoritative project state.

Background workers must not mutate the live project directly.

Instead:

```text
worker receives immutable input, snapshot, asset handle, or job request
worker performs expensive work
worker returns result
app applies result through a command
command records undo/redo and marks caches dirty
```

This protects:

- undo/redo correctness
- save consistency
- project dirty tracking
- cancellation
- AI result review
- crash recovery
- predictable debugging

---

## Recommended Runtime Stack

### UI

Recommended:

- `egui`
- `eframe`
- `egui-wgpu`
- `egui_dock`
- `egui_extras`
- `egui-i18n`

Avoid:

- `egui_mobius`

Rationale:

Pixhaus should keep a custom state architecture and avoid adopting a generic reactive egui framework. Docking, widgets, localization, and rendering helpers are useful. A global app-state framework is not necessary and may fight Pixhaus' domain-specific command/job/project model.

### Rendering

Recommended:

- `wgpu`
- `egui-wgpu`
- `eframe` with the wgpu renderer path

Pixhaus should prefer wgpu for long-term rendering flexibility because it maps well to:

- DirectX 12 on Windows
- Metal on macOS
- Vulkan on Linux and Windows
- OpenGL fallback paths where supported

The GPU renderer should be treated as a view/cache layer, not the source of truth.

### Logging and Diagnostics

Recommended:

- `tracing`
- `tracing-subscriber`
- `tracing-appender`
- `tracing-log`

Pixhaus should use structured tracing from the beginning.

Trace important flows:

- app startup
- GPU backend detection
- project load
- project save
- autosave
- asset load
- frame composite
- texture upload
- thumbnail generation
- import/export
- command execution
- undo/redo
- AI job submission
- AI provider response
- local model worker startup
- save format migration
- plugin/module registration

Logs should support:

- local debugging
- user diagnostic bundles
- performance analysis
- project corruption reports
- AI provider troubleshooting
- GPU/backend issues

### Localization

Recommended:

- `egui-i18n`, wrapped behind a Pixhaus localization service

Do not call localization directly from core project logic.

Use stable localization keys and namespaces:

```text
core.*
workspace.draw.*
workspace.animate.*
workspace.generate.*
panel.layers.*
panel.timeline.*
tool.pencil.*
command.undo.*
provider.openai.*
export.png.*
```

Localization should support:

- runtime language switching
- fallback language
- missing-key detection in dev builds
- module-owned string namespaces
- future recipe/style packs with localized display metadata

### Async Runtime

Recommended:

- `tokio`

Use Tokio for I/O-bound and async jobs:

- remote AI provider calls
- local model worker IPC
- downloads
- update checks
- network requests
- background file operations where async is useful
- provider authentication flows
- telemetry/diagnostic upload if ever added

Do not turn the whole app into a Tokio app conceptually. The UI thread remains the UI thread. Tokio is one executor in the runtime toolbox.

### CPU Parallelism

Recommended:

- `rayon`

Use Rayon for CPU-bound parallel workloads:

- thumbnail batches
- frame compositing batches
- palette analysis
- color reduction
- coverage analysis
- batch validation
- export preparation
- image filters
- spritesheet packing search
- import processing

Rayon is especially useful when the work can be expressed as parallel iteration over independent assets, frames, layers, tiles, or pixels.

### Channels and Job Communication

Recommended:

- `flume` or `crossbeam-channel`

Suggested default:

- `flume` for ergonomic multi-producer/multi-consumer messaging

Use channels for:

- worker-to-app job result delivery
- progress updates
- cancellation notifications
- local model worker communication
- background importer/exporter results
- thumbnail job results

Do not communicate by sharing a giant locked app state object.

Prefer:

```text
message passing for background results
main app applies results through commands
```

### Locks and Shared Data

Useful, but use carefully:

- `parking_lot`
- `arc-swap`
- `dashmap`

Guidance:

- Use `parking_lot` for small shared caches or synchronization primitives.
- Use `arc-swap` for replacing shared immutable configuration/capability snapshots.
- Use `dashmap` only for carefully scoped concurrent maps such as caches.

Avoid:

```text
Arc<Mutex<AppState>> as the core architecture
```

That will become painful.

### App Directories

Recommended:

- `directories-next` or `dirs-next`

Pixhaus needs platform-standard paths for:

- app config
- user settings
- workspace layouts
- logs
- crash/diagnostic bundles
- recent projects
- autosaves
- thumbnail cache
- model cache
- provider credentials/config references
- temporary export cache
- plugin/module data, if needed

Use project-specific app directories rather than hardcoded paths.

### Serialization and Persistence

Recommended:

- `serde`
- `serde_json`
- `toml` where appropriate
- `rmp-serde`, `postcard`, or a custom binary format for compact metadata
- `zstd`
- `blake3`
- `memmap2`
- `tempfile`
- `walkdir`
- `notify`

Suggested usage:

- JSON/TOML for human-editable app settings where useful
- binary metadata for project internals where performance matters
- zstd for compression of large data blocks
- blake3 for content hashing and asset identity
- memmap2 for large asset blobs or indexes where appropriate
- tempfile + atomic rename for save safety
- notify for external project folder changes if needed

### Image and Asset Processing

Recommended:

- `image`
- `png`
- `gif`
- possibly `resvg` later for SVG import
- possibly `ravif` or other format crates later if needed

Pixhaus should own its internal sprite/canvas representation and treat imported/exported image formats as adapters.

### Native Platform Integration

Recommended:

- `rfd` for file dialogs
- `arboard` for clipboard
- `open` for opening files/URLs in OS-default apps
- `notify` for filesystem watching

Keep platform logic behind Pixhaus platform services.

### Error Handling

Recommended:

- `thiserror`
- `anyhow`
- optionally `miette` or `color-eyre`

Guidance:

- Use `thiserror` for library/domain error enums.
- Use `anyhow` at app boundaries where rich internal context matters.
- Use `miette` if you want rich user-facing diagnostic reports.

Pixhaus errors should become actionable user messages when possible.

Bad:

```text
IO error
```

Better:

```text
Could not load sprite asset.
The project manifest references an asset file that is missing from disk.
Suggested action: restore from autosave, relink the asset, or remove the missing reference.
```

### Caching

Recommended:

- `moka`
- `lru`
- `blake3`
- optionally `dashmap` for concurrent caches

Cache types:

- texture cache
- thumbnail cache
- frame composite cache
- palette analysis cache
- AI result preview cache
- prompt compile cache
- coverage cache
- asset metadata cache
- decoded image cache

Caches must be invalidated by:

- content hash
- asset revision
- project revision
- dirty region
- command result
- file timestamp plus hash validation where appropriate

### Profiling and Performance

Recommended:

- `tracing`
- `puffin`
- `criterion`
- `divan`

Use:

- tracing for production diagnostics and structured spans
- puffin for in-app/frame profiling if useful
- criterion/divan for benchmarks of image operations, project load/save, compositing, palette operations, etc.

---

## Concurrency Philosophy

Pixhaus should absolutely care about concurrency and parallelization.

It is a native creative app that may handle:

- large projects
- many assets
- many sprites
- many animation frames
- large canvases
- multiple art styles
- heavy exports
- AI generation
- local model inference
- GPU rendering
- background thumbnails
- autosave

Modern workstations have many CPU cores and often powerful GPUs. Pixhaus should use them.

However:

> Concurrency should be organized through jobs and services, not scattered through UI code.

---

## Execution Lanes

Pixhaus should conceptually have several execution lanes.

### UI Lane

Responsibilities:

- egui frame execution
- input collection
- lightweight state updates
- command submission
- displaying job progress
- applying completed job results through commands

The UI lane must stay responsive.

### Render/GPU Lane

Responsibilities:

- texture uploads
- canvas rendering
- preview rendering
- wgpu work submission
- GPU resource management
- render cache updates

The render layer should be isolated from the project truth.

### CPU Worker Pool

Responsibilities:

- image operations
- thumbnail generation
- palette analysis
- color reduction
- compression
- exports
- validation
- coverage analysis
- batch processing

This is where Rayon is most useful.

### Async I/O Runtime

Responsibilities:

- remote AI APIs
- downloads
- local worker IPC
- network I/O
- async file operations when appropriate

This is where Tokio is most useful.

### AI/Model Workers

Responsibilities:

- local model inference
- CUDA/Metal/Vulkan/CPU model backends
- memory-heavy generation
- provider-specific execution

Recommendation:

Keep local AI/model workers out-of-process at first.

Reasons:

- CUDA dependency isolation
- crash isolation
- model memory isolation
- easier language/runtime flexibility
- easier provider updates
- avoids making Pixhaus fail to launch if AI backend fails

---

## Job Manager

The Job Manager is the central abstraction for expensive work.

Everything expensive should become a job:

- load project
- save project
- autosave project
- import asset
- export sprite sheet
- export GIF/video
- generate thumbnail
- composite frame batch
- analyze palette
- reduce palette
- analyze animation coverage
- generate sprite
- generate animation
- generate in-betweens
- make tile seamless
- validate export
- download model
- warm local provider

Every job should have:

- ID
- kind
- input
- context
- status
- progress
- priority
- cancellation token
- resource requirements
- logs
- result
- error
- creation time
- completion time

Job statuses:

```text
queued
running
blocked
complete
failed
cancelled
```

Resource requirements:

```text
CPU only
GPU optional
GPU preferred
GPU required
network required
provider required
local model required
large memory required
```

The UI should not care how the job runs. It only displays status and lets the user cancel/retry/use results.

---

## Job Result Rule

A job result must not directly mutate project state.

Instead:

```text
job completes
result enters result store
UI presents result
user chooses action
command applies result to project
undo stack records mutation
```

Examples:

### AI Generation

```text
GenerateSpriteJob
  -> GeneratedAsset
  -> user selects result
  -> ApplyGeneratedAssetCommand
```

### Import

```text
ImportAsepriteJob
  -> ImportedAssetBundle
  -> user confirms import
  -> AddImportedAssetsCommand
```

### Palette Reduction

```text
ReducePaletteJob
  -> PaletteReductionPreview
  -> user accepts
  -> ApplyPaletteReductionCommand
```

This keeps artist control central.

---

## Parallelization Priorities

### High Priority

Parallelize these early:

1. Thumbnail generation
2. Timeline preview generation
3. Frame compositing batches
4. Project loading indexes
5. Export preparation
6. Large image decoding/encoding
7. Palette analysis
8. Coverage analysis
9. AI result post-processing
10. Save compression/hashing

### Medium Priority

Parallelize later:

1. batch filters
2. batch transforms
3. spritesheet packing search
4. tile seam QA
5. multi-frame validation
6. generated asset ranking/scoring

### Low Priority / Do Not Rush

Avoid overengineering these early:

1. basic pencil strokes
2. small eraser operations
3. simple selections
4. immediate UI state changes
5. small palette edits

Interactive drawing latency matters more than throughput. Keep brush operations direct and predictable, then schedule expensive derived updates after the command.

---

## Project State and Background Workers

Background workers should receive one of these:

- immutable snapshot
- asset handle
- asset path
- content hash
- serialized job input
- copied pixel region
- read-only project index

They should return one of these:

- generated asset
- preview image
- metadata result
- validation report
- export artifact path
- import bundle
- error report

They should not receive:

- mutable app state
- mutable project reference
- egui context
- live undo stack
- live command bus

---

## Recommended App State Architecture

Pixhaus should own custom high-level state containers.

### ProjectStore

Owns loaded project data and lazy-loaded asset access.

Responsibilities:

- project manifest access
- asset lookup
- sprite/document lookup
- lazy asset loading
- dirty tracking
- revision tracking
- save/load coordination
- asset dependency lookup

### AppSession

Owns current running session state.

Responsibilities:

- active project
- active workspace
- active document
- active sprite
- active frame
- active layer
- selected asset
- active tool
- current editing context
- job manager access
- command bus access

### UiState

Owns interface-only state.

Responsibilities:

- dock layouts
- panel state
- selection in UI lists
- scroll positions
- modal stack
- drag/drop state
- filters/search
- command palette

### EditingContext

Represents where editing actions apply.

Responsibilities:

- active sprite/canvas
- active layer
- active frame
- active cel
- active palette
- active selection
- active tool settings
- onion skin settings when applicable

Draw, Animate, and Tiles workspaces all use the same EditingContext.

### CommandBus

Owns mutation flow.

Responsibilities:

- execute command
- validate command
- record undo/redo
- group transactions
- mark dirty regions/assets
- emit app events

### JobManager

Owns expensive/background work.

Responsibilities:

- queue jobs
- dispatch jobs
- track progress
- cancel jobs
- receive results
- expose status to UI
- preserve job logs/errors

### WorkspaceRegistry

Owns available workspace definitions.

Responsibilities:

- register workspace modules
- expose workspace metadata
- define default layouts
- expose workspace actions
- control workspace activation

### ModuleRegistry

Owns internal module/capability registration.

Responsibilities:

- register tools
- register commands
- register panels
- register asset types
- register importers/exporters
- register AI providers
- register recipe packs
- register themes
- register localization namespaces

### AssetCache

Owns derived/cached data.

Responsibilities:

- thumbnails
- texture handles
- previews
- decoded assets
- prompt previews
- coverage results
- cache invalidation

---

## Internal Modules, Not External Plugins

Pixhaus should support an internal module architecture, not native dynamic plugins.

Modules are compiled into Pixhaus and register capabilities at startup.

Examples:

- CoreModule
- SpriteEditingModule
- AnimationModule
- PixelArtModule
- GenerationModule
- TilesModule
- ExportModule
- ProvidersModule
- LocalModelModule
- FutureParticlesModule
- FutureSpriteUiModule

Modules may register:

- workspaces
- panels
- tools
- commands
- jobs
- asset types
- providers
- importers
- exporters
- localization namespaces
- shortcuts
- menu entries
- settings pages

This gives Pixhaus extensibility without external ABI, packaging, security, or versioning complexity.

---

## UI Layout and Docking

Pixhaus should use a workspace layout abstraction.

`egui_dock` can be used to implement dockable panels, but it should not define the product architecture.

Each workspace should declare a default layout:

### Draw Workspace

- central canvas
- left tool shelf
- top tool options
- right palette/layers/assets inspector
- compact timeline/frame strip
- status bar

### Animate Workspace

- central canvas
- same drawing tools
- dominant timeline
- onion skin controls
- animation clip inspector
- playback controls
- frame/layer tracks
- optional AI in-between assistant

### Generate Workspace

- recipe browser
- prompt composer
- structure/style selector
- result grid
- generation queue
- asset browser
- coverage panel

### Tiles Workspace

- canvas/tile editor
- tile palette
- autotile preview
- seam QA
- variant browser
- material generation panel

### Export Workspace

- export preview
- spritesheet/GIF/video settings
- engine presets
- validation checklist
- output path/settings

Docking should support power users, but every workspace should have an excellent default layout before customization.

---

## Localization Architecture

Pixhaus should use a localization service rather than direct string handling scattered across UI.

Localization requirements:

- stable string keys
- module namespaces
- runtime language switching
- fallback language
- interpolation
- pluralization if supported
- missing-key diagnostics
- dev-mode key display

Recommended string namespace examples:

```text
app.menu.file
app.menu.edit
workspace.draw.title
workspace.animate.title
workspace.generate.title
panel.layers.title
tool.pencil.label
tool.pencil.tooltip
command.undo.draw_pixels
job.generate_sprite.running
error.project.asset_missing
```

Project files should store stable IDs and metadata, not localized strings as the only source of truth.

Display names may be localized at render time.

---

## Logging and Diagnostic Policy

Pixhaus should produce structured logs and spans.

### Always Trace

- startup
- shutdown
- renderer initialization
- GPU backend choice
- module registration
- workspace activation
- project open/close/save
- autosave
- import/export jobs
- AI provider jobs
- local model worker state
- command execution failures
- save migration
- corrupt asset detection

### Performance Spans

Track durations for:

- frame render
- canvas composite
- thumbnail batch
- texture upload
- project load index
- lazy asset load
- export encode
- compression
- AI request
- provider response
- model warmup

### Diagnostic Bundle

Pixhaus should eventually be able to create a diagnostic bundle containing:

- recent logs
- app version
- OS/platform
- renderer backend
- GPU adapter info
- enabled modules
- provider configuration summary without secrets
- project manifest summary
- recent job failures
- recent panic/crash info if available

Never include API keys or private project assets without explicit user consent.

---

## App Directories

Use platform-standard app directories.

Recommended separation:

### Config Directory

- user preferences
- language setting
- keyboard shortcuts
- workspace layouts
- theme preference
- provider configuration references

### Data Directory

- user-created global presets
- global recipe packs
- palette libraries
- brush libraries
- internal module data

### Cache Directory

- thumbnails
- generated previews
- decoded asset cache
- local model cache
- temporary export cache
- remote provider response cache if allowed

### Logs Directory

- rolling log files
- diagnostic traces
- crash reports

### Autosave Directory

- project recovery snapshots
- unsaved project autosaves
- crash recovery metadata

---

## Settings Architecture

Separate settings into categories.

### App Settings

- theme
- language
- recent files
- UI scale
- default workspace
- startup behavior

### Workspace Settings

- dock layouts
- visible panels
- preferred timeline height
- default onion skin settings
- default canvas background

### Tool Settings

- brush size
- brush smoothing
- pixel-perfect behavior
- pressure settings if tablet support is added
- selection behavior

### Provider Settings

- enabled providers
- provider priority
- API key references
- local model paths
- remote endpoints
- safety/usage limits

### Project Settings

- project color profile assumptions
- export defaults
- asset naming rules
- project recipe libraries
- project-specific provider overrides if allowed

---

## GPU and Concurrency Relationship

Pixhaus should separate rendering GPU and compute/AI GPU.

### Rendering GPU

Use wgpu to render:

- canvas
- zoom/pan
- layers
- transparency checkerboard
- onion skins
- grid overlays
- selection overlays
- timeline thumbnails
- previews

### Compute/AI GPU

Use separate provider/backend abstractions for:

- CUDA local models
- Metal/Core ML local models
- Vulkan compute providers
- CPU fallback
- remote AI APIs
- out-of-process model workers

The main app should not know tensor details or provider internals.

It should submit jobs and receive assets/results.

---

## Modern Hardware Usage

Pixhaus should use modern workstation hardware carefully.

### CPU

Use multiple cores for:

- thumbnails
- exports
- batch validation
- palette operations
- compression
- import processing
- coverage analysis

### GPU

Use GPU for:

- rendering
- canvas previews
- compositing where beneficial
- shader previews
- future VFX previews
- possible compute jobs
- local AI where provider supports it

### Memory

Pixhaus must avoid eager loading of entire multigigabyte projects.

Use:

- lazy loading
- asset indexes
- memory-mapped large blobs where appropriate
- thumbnail-first browsing
- cache limits
- eviction policies
- background prefetching

### Storage

Use:

- content hashes
- compressed chunks
- atomic saves
- autosaves
- project folder/bundle architecture
- incremental asset writes where possible

---

## Recommended Baseline Dependency Set

This is the initial recommended crate stack.

### UI and Windowing

```text
egui
eframe
egui-wgpu
egui_dock
egui_extras
egui-i18n
```

### Rendering and GPU

```text
wgpu
```

### Async and Parallel Work

```text
tokio
rayon
flume or crossbeam-channel
parking_lot
arc-swap
```

### Logging and Diagnostics

```text
tracing
tracing-subscriber
tracing-appender
tracing-log
```

### State, IDs, and Data Structures

```text
slotmap or generational-arena
uuid
serde
indexmap
petgraph, optionally for asset dependency graphs
```

### Persistence

```text
serde_json
toml
rmp-serde or postcard
zstd
blake3
memmap2
tempfile
walkdir
notify
```

### Images and Assets

```text
image
png
gif
resvg, optionally later
```

### Platform Integration

```text
directories-next or dirs-next
rfd
arboard
open
```

### Error Handling

```text
thiserror
anyhow
miette or color-eyre optionally
```

### Caching and Profiling

```text
moka or lru
puffin
criterion or divan
```

---

## Recommended Defaults

### Default UI Runtime

Use:

```text
eframe + egui + wgpu renderer
```

### Default Docking Strategy

Use:

```text
egui_dock behind Pixhaus workspace layout abstraction
```

### Default Localization Strategy

Use:

```text
egui-i18n behind Pixhaus localization service
```

### Default Logging Strategy

Use:

```text
tracing with file appender and structured spans
```

### Default CPU Parallel Strategy

Use:

```text
rayon for CPU-heavy batch work
```

### Default Async Strategy

Use:

```text
tokio for network, provider, IPC, and async background jobs
```

### Default Job Communication

Use:

```text
flume or crossbeam-channel for result/progress messaging
```

### Default State Management Strategy

Use:

```text
custom Pixhaus ProjectStore, AppSession, UiState, CommandBus, JobManager, and registries
```

Do not use a generic egui app-state framework.

---

## What To Avoid

Avoid:

- `egui_mobius`
- global `Arc<Mutex<AppState>>` as architecture
- UI widgets owning durable creative state
- background workers mutating live project state
- AI providers writing directly into sprites
- loading full projects eagerly
- blocking UI on save/load/export/generation
- hardcoding provider-specific UI into Generate workspace
- using GPU textures as source of truth
- making Tokio responsible for everything
- parallelizing simple brush strokes too early
- adopting native dynamic plugins
- allowing caches to become project truth

---

## Acceptance Criteria for the Runtime Architecture

The runtime/state/concurrency architecture is healthy when:

- The UI remains responsive during project load/save/export.
- AI generation never blocks drawing.
- Generated results are previewed before being applied.
- Applying generated results is undoable.
- Background workers cannot corrupt the live project state.
- Large projects can open from metadata/indexes before all assets load.
- Thumbnail generation happens in the background.
- Timeline previews do not freeze the app.
- Localization can change without rewriting panels.
- Logs explain what happened when a provider/export/save fails.
- Dock layouts can be reset or customized.
- Workspaces share tools and panels without duplication.
- Draw and Animate use the same editing core.
- Pixel Art mode is a dedicated tooling/mode layer, not the whole app identity.
- Manual drawing works without AI providers configured.
- Pixhaus starts even if local AI backends are unavailable.

---

## Final Architecture Principle

Pixhaus should be built around this principle:

> The app has one authoritative creative model, one command-based mutation path, one job-based background execution path, and many workspace-specific views over the same capabilities.

The crates listed in this document should support that architecture, not replace it.

