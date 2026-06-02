# Pixhaus Architecture Bible

**Version:** 0.1  
**Scope:** Native Rust + egui application architecture for a cross-platform sprite creation, sprite animation, and AI-native creative production tool.  
**Platforms:** Windows, macOS, Linux.  
**Primary UI stack:** Rust + egui/eframe.  
**Primary rendering direction:** wgpu-backed native rendering.  
**Product principle:** Manual-first, AI-assisted, extensible, artist-respecting.

---

## 1. Executive Summary

Pixhaus should be built as a **modular native creative platform**, not as a single monolithic editor screen.

The app is focused on **sprite creation and sprite animation across multiple art styles**, not only pixel art. Pixel art is important enough to receive a dedicated mode/tooling layer, but Pixhaus should also support hand-painted sprites, clean HD sprites, retro-inspired art, painterly game assets, UI sprites, tilesets, effects, and AI-assisted production workflows.

The architectural goal is:

> Pixhaus is a native creative host with a shared sprite editing core, task-focused workspaces, a modular capability registry, a command-based mutation model, a job-based background execution model, and AI/provider extensibility that does not get in the artist’s way.

The core idea is to separate:

- **What the project is** from how it is displayed.
- **What the user can do** from where that action appears in the UI.
- **Manual editing** from workspace layout.
- **AI generation** from direct canvas mutation.
- **Rendering GPU** from AI/compute GPU.
- **Internal modules** from external dynamic plugins.

Pixhaus should be built one workspace at a time, but not by giving each workspace its own private model. Instead, every workspace sits on top of shared systems:

- Project model
- Sprite editing model
- Animation model
- Tool system
- Command system
- Undo/redo system
- Renderer
- Job system
- Asset library
- AI recipe/provider system
- Import/export pipeline
- Workspace/panel/action registry

The first major engineering milestone should not be “all workspaces exist.” It should be:

> The shared sprite editing core is solid enough that both Draw and Animate can use it.

---

## 2. Product Architecture Principles

### 2.1 Manual-first

Pixhaus must be credible as a manual creative tool even if AI is unavailable.

A seasoned artist should be able to:

- Draw manually.
- Animate manually.
- Use layers.
- Use palettes.
- Use onion skinning.
- Edit frame-by-frame.
- Export production-ready assets.
- Ignore AI entirely.

AI is a multiplier, not a replacement for the editor.

### 2.2 AI-assisted, not AI-intrusive

AI should appear as:

- Contextual actions.
- Generation workflows.
- Assistant tools.
- Background suggestions.
- Reusable recipes.
- Batch production helpers.

AI should not dominate the default manual editing flow.

The rule:

> AI proposes. The artist decides.

Every AI result should be previewable, reversible, editable, and traceable.

### 2.3 Workspaces are task-focused layouts

A workspace is not the owner of a feature. A workspace is a task-focused arrangement of shared capabilities.

For example:

- Draw focuses on current-frame editing.
- Animate focuses on frames over time.
- Generate focuses on AI-assisted creation.
- Tiles focuses on tile and terrain workflows.
- Export focuses on production output.

The Pencil tool does not belong to Draw. It belongs to the shared Tool System. Animate can also use it. Tiles can also use it.

### 2.4 Draw and Animate are siblings over the same editing core

Draw and Animate go hand in hand.

To animate manually, the user must draw inside the animation workspace while seeing onion skinning, frame timing, playback, and clip structure.

Therefore:

- Draw is **editing in space**.
- Animate is **editing in space over time**.

Both use the same sprite/layer/frame/cel model, the same tools, the same commands, and the same canvas renderer.

### 2.5 Pixel art is a dedicated mode, not the only identity

Pixhaus should support multiple sprite art styles:

- Pixel art
- Clean HD sprites
- Hand-painted sprites
- Vector-inspired raster sprites
- Retro-inspired but not strict pixel art
- Painterly game assets
- UI sprites
- Tilesets
- Particles and sprite VFX
- Character animation sprites

Pixel art should have special tooling because it has unique rules:

- Indexed palettes
- Palette ramps
- Dithering
- Grid snapping
- Nearest-neighbor zoom
- Tile boundaries
- Pixel-perfect brushes
- Color-count discipline
- Onion skin optimized for low-resolution frames
- Palette-preserving AI operations

But Pixhaus should not describe itself as only a pixel art editor.

### 2.6 Internal modules, not external native dynamic plugins

Pixhaus should be extensible through internal modules.

The architecture should support plugin-like registration of capabilities, but those modules are compiled into the app.

Do not build a Stage 5 native dynamic plugin system. It is unnecessary for the current product and would add complexity around ABI stability, cross-platform packaging, versioning, and security.

The desired model is:

> Internal modules register workspaces, panels, tools, commands, asset types, AI providers, importers, exporters, recipes, themes, and shortcuts.

This gives most of the benefits of plugins without the problems of external binary plugins.

### 2.7 Jobs, not direct side effects

Anything long-running, expensive, asynchronous, or external should be a job.

Examples:

- AI generation
- Local model inference
- Importing large files
- Exporting spritesheets
- Rendering thumbnails
- Palette reduction
- Upscaling
- In-between generation
- Autotile validation
- Background cleanup

Jobs produce results. Results are then applied through undoable commands.

AI generation must never directly mutate the canvas.

The lanes that run jobs and the worker input/output contract are sections 31 and 13.6.

### 2.8 Commands own mutation

Major changes to project data should happen through commands.

This enables:

- Undo/redo
- Transaction grouping
- History names
- Macro possibilities
- Safer agent-driven development
- Better testing
- Consistent mutation boundaries

Tools, panels, workspaces, and AI results should request mutations through commands rather than modifying core state directly.

### 2.9 GPU rendering and GPU compute are separate concerns

Pixhaus needs GPU support in two different ways:

1. **Rendering:** UI, canvas, overlays, previews, zooming, compositing.
2. **Compute / AI:** local models, image operations, possible CUDA/Metal/Vulkan workloads.

These should not share a single abstraction.

The rendering layer should use a cross-platform graphics strategy. The compute layer should use capability-driven backends. They run in separate execution lanes (section 31.2).

### 2.10 Project data is the source of truth

GPU textures, previews, thumbnails, and generated result caches are views or derived data.

The authoritative creative data lives in the project model:

- Documents
- Sprites
- Layers
- Frames
- Cels
- Palettes
- Animation clips
- Assets
- Recipes
- Metadata

The renderer may cache textures, but the project model is the source of truth.

---

## 3. Platform and Technology Direction

### 3.1 UI framework

Pixhaus uses Rust and egui/eframe for native UI.

The app should treat egui as the presentation and interaction layer, not the product architecture.

Good egui responsibilities:

- Drawing UI panels
- Capturing user intent
- Showing current state
- Displaying overlays and widgets
- Routing input events
- Showing command palette, menus, inspectors, and panels

Bad egui responsibilities:

- Owning durable project state
- Mutating project data directly from random widgets
- Owning AI provider logic
- Owning file format logic
- Owning long-running jobs
- Encoding workspace-specific business logic inside one giant update loop

### 3.2 Rendering backend

The preferred rendering direction is **egui/eframe with wgpu**.

wgpu is suitable because it is a cross-platform Rust graphics API running natively on Vulkan, Metal, DirectX 12, and OpenGL, which aligns with the Windows/macOS/Linux target matrix.

Primary platform expectations:

- Windows: DirectX 12 through wgpu, Vulkan where appropriate.
- macOS: Metal through wgpu.
- Linux: Vulkan through wgpu, with fallbacks where feasible.

Pixhaus should use GPU acceleration for rendering where it improves the experience, but should preserve graceful fallback behavior.

### 3.3 GPU compute and local models

Local AI and GPU compute should be handled separately from app rendering.

Potential compute backends:

- CPU worker pool
- wgpu compute
- CUDA
- Metal/Core ML bridge
- Vulkan compute
- External local model service
- Remote AI provider

Pixhaus should not assume one compute backend exists everywhere.

The app should detect available capabilities and choose the best provider/backend per job.

### 3.4 Cross-platform goals

Pixhaus should launch and remain useful on all supported platforms even when optional acceleration is unavailable.

Minimum acceptable behavior:

- Manual editing works without local AI.
- App launches without CUDA.
- App launches without downloaded model files.
- Remote AI providers can be disabled.
- Local model providers can fail without crashing the app.
- Rendering should fall back where possible.

### 3.5 Platform matrix

| Area | Windows | macOS | Linux |
|---|---|---|---|
| UI | egui/eframe | egui/eframe | egui/eframe |
| Rendering | wgpu via D3D12/Vulkan | wgpu via Metal | wgpu via Vulkan/OpenGL fallback |
| Local AI | CUDA optional, CPU fallback | Metal/Core ML optional, CPU fallback | CUDA optional, Vulkan/CPU optional |
| File dialogs | Native wrapper | Native wrapper | Native wrapper |
| Packaging | Installer/MSIX later | DMG/pkg later | AppImage/deb/rpm later |
| Manual editor | Required | Required | Required |
| AI providers | Optional | Optional | Optional |

---

## 4. Layered Architecture

Pixhaus should be organized conceptually into the following layers.

### 4.1 Host App Layer

Owns:

- App lifecycle
- Window lifecycle
- Native menu integration
- Startup/shutdown
- Settings loading
- Project loading/unloading
- Module registration
- Workspace selection
- Global event loop
- Global command palette
- Top-level error handling
- Crash-safe state recovery hooks

The Host App should not own detailed sprite-editing logic.

### 4.2 Workspace Runtime Layer

Owns:

- Workspace registry
- Active workspace
- Workspace default layouts
- Workspace-specific panel composition
- Workspace-specific context actions
- Workspace-specific shortcuts
- Workspace-specific tool emphasis
- Workspace-level status items

A workspace is a composition of panels, tools, actions, and services.

### 4.3 Creative Core Layer

Owns the domain model:

- Projects
- Documents
- Sprites
- Layers
- Frames
- Cels
- Palettes
- Selections
- Animation clips
- Tilesets
- Assets
- Recipes
- Metadata

The Creative Core should not depend on egui.

### 4.4 Service Layer

Owns shared behavior:

- Command execution
- Undo/redo
- Transactions
- Job scheduling
- Asset indexing
- Thumbnail generation
- Prompt compilation
- Provider dispatch
- Import/export orchestration
- Validation
- Caching
- Autosave

### 4.5 Rendering Layer

Owns:

- Canvas rendering
- Texture cache
- Composite previews
- Grid overlays
- Selection overlays
- Tool previews
- Onion skin overlays
- Timeline thumbnails
- Asset thumbnails
- Tile previews
- Export previews

The renderer should read project state and UI state, then draw. It should not be the source of truth.

### 4.6 Platform Layer

Owns:

- Native dialogs
- Clipboard
- File associations
- Recent files
- OS settings paths
- Window management
- Native drag/drop
- GPU capability detection
- External process launching
- Local model worker supervision

### 4.7 Internal Module Layer

Owns modular registration of capabilities.

Example modules:

- Core Module
- Sprite Editing Module
- Animation Module
- Generation Module
- Tiles Module
- Export Module
- Pixel Art Module
- Palette Module
- Asset Library Module
- OpenAI Provider Module
- Local Model Provider Module
- Future Particle VFX Module
- Future Sprite UI Module

Modules are compiled into the app. They register capabilities with the host.

---

## 5. Core Concepts and Vocabulary

### 5.1 Capability

A capability is something Pixhaus can do.

Examples:

- Draw pixels
- Paint raster strokes
- Manage layers
- Animate frames
- Use onion skinning
- Generate sprites
- Export spritesheets
- Reduce to palette
- Create particles
- Make seamless tiles

Capabilities are registered by internal modules.

### 5.2 Workspace

A workspace is a task-focused layout of capabilities.

Examples:

- Draw
- Animate
- Generate
- Tiles
- Export
- Pixel Art
- Future Particle VFX
- Future Sprite UI

A workspace does not own all the features it displays.

### 5.3 Panel

A panel is a reusable UI surface.

Examples:

- Canvas Panel
- Tool Shelf
- Palette Panel
- Layers Panel
- Timeline Panel
- Onion Skin Panel
- Prompt Composer
- Asset Browser
- Export Inspector

Panels can appear in multiple workspaces.

### 5.4 Tool

A tool is a direct interaction mode.

Examples:

- Brush
- Pencil
- Eraser
- Fill
- Selection
- Move
- Shape
- Color Picker
- AI Brush
- Tile Stamp
- Particle Brush

Tools operate through the active editing context and create commands.

### 5.5 Command

A command is an undoable mutation to project state.

Examples:

- Draw stroke
- Erase stroke
- Add layer
- Delete frame
- Duplicate frame
- Apply generated asset
- Create animation clip
- Change palette color
- Reduce to palette

Commands should be testable without egui.

### 5.6 Job

A job is asynchronous or expensive work.

Examples:

- Generate sprite
- Generate animation
- Import Aseprite file
- Export spritesheet
- Build thumbnails
- Run local model
- Palette analysis
- Autotile validation

Jobs produce results that are then applied through commands.

### 5.7 Asset

An asset is a reusable creative object in the project.

Examples:

- Sprite
- Animation clip
- Tileset
- Palette
- Reference image
- Prompt recipe
- Generated result
- Particle system
- UI sprite component

### 5.8 Document

A document is an editable unit inside a project.

Examples:

- Sprite document
- Tileset document
- Particle document
- UI sprite document
- Composition/recipe document

Documents may have specialized editors/workspaces.

### 5.9 Editing Context

The editing context describes what the user is currently editing.

It includes:

- Active project
- Active document
- Active sprite
- Active frame
- Active layer
- Active cel
- Active palette
- Active selection
- Active tool
- Active brush settings
- Active art mode

Draw, Animate, Tiles, and Pixel Art modes all use the editing context.

---

## 6. Workspace Architecture

### 6.1 Workspaces are layouts over capabilities

Workspaces should be built as compositions of registered panels, tools, and actions.

A workspace contributes:

- Name
- Icon
- Purpose
- Default layout
- Visible panels
- Primary tools
- Contextual actions
- Menu entries
- Shortcut profile
- Command palette actions
- Optional status bar items
- Optional job views
- Optional validation panels

### 6.2 Initial workspaces

Pixhaus should start with these primary workspaces:

1. Draw
2. Animate
3. Generate
4. Tiles
5. Export

Additional future workspaces may include:

- Pixel Art
- Palette Lab
- Sprite UI
- Particle VFX
- Rigging
- Asset Browser
- Composition Library
- Batch Processing

### 6.3 Draw Workspace

Purpose:

> Edit sprites and individual frames with manual creative tools.

Primary focus:

- Canvas
- Brushes/tools
- Palette/color
- Layers
- Current frame
- Sprite list
- Quick export

It should include a compact timeline or frame strip so frame-based workflows are not hidden.

The Draw workspace is where manual creation feels fastest.

### 6.4 Animate Workspace

Purpose:

> Edit sprites over time with drawing tools, onion skinning, playback, frame timing, and animation clips.

Primary focus:

- Canvas
- Drawing tools
- Large timeline
- Onion skin
- Playback
- Clip ranges
- Frame timing
- Animation tags
- AI in-between actions

The Animate workspace must still allow drawing. It should feel like Draw plus time.

### 6.5 Generate Workspace

Purpose:

> Create, refine, and apply AI-assisted sprite assets using recipes, structures, styles, references, and generation results.

Primary focus:

- Prompt composer
- Recipe/template browser
- Structure selector
- Style selector
- AI provider selector
- Generation result grid
- Asset browser
- Coverage panel
- Apply-to-project actions

Generate is the AI-forward workspace. It should be useful for non-artists while still respecting artists.

### 6.6 Tiles Workspace

Purpose:

> Create tilesets, terrain pieces, seamless tiles, autotile sets, and tile variants.

Primary focus:

- Tile canvas
- Tile preview
- Seam checking
- Tile rules
- Terrain variation
- Tile stamping
- Autotile generation
- Tile export presets

Tiles may use pixel art tooling heavily but should not be limited to pixel art.

### 6.7 Export Workspace

Purpose:

> Validate and export production-ready assets for games and engines.

Primary focus:

- Spritesheet preview
- Animation export
- GIF/video preview
- Engine presets
- Packing
- Metadata
- Validation checklist
- Batch export

Export is where production discipline lives.

### 6.8 Pixel Art Mode / Workspace

Pixel art should be treated as a specialized art mode and possibly a dedicated workspace.

It should provide:

- Indexed palette mode
- Palette locking
- Color-count warnings
- Nearest-neighbor scaling
- Pixel-perfect pencil
- Dithering tools
- Ramp generation
- Palette harmony
- Tile grid helpers
- 8px/16px major grid options
- Pixel-art AI constraints
- Palette-preserving generation
- Pixel cleanup tools

This can appear as:

- A workspace
- An art mode inside Draw/Animate/Tiles
- A project setting
- A sprite setting

The best long-term architecture is to support all of these: a sprite can have an art mode, and workspaces can emphasize that mode.

---

## 7. Internal Module System

### 7.1 Why modules exist

Modules allow Pixhaus to grow without becoming a monolith.

They are internal and compiled into the app, but they behave like plugins architecturally.

This enables:

- Clear boundaries
- Feature flags
- Agent-friendly development
- Easier testing
- Future extensibility
- Better ownership of capabilities

### 7.2 What modules can register

Modules may register:

- Workspaces
- Panels
- Tools
- Commands
- Actions
- Menu items
- Keyboard shortcuts
- Asset types
- Document types
- Importers
- Exporters
- AI providers
- AI recipes
- Brush types
- Render overlays
- Validators
- Background jobs
- Themes
- Settings pages

### 7.3 Recommended initial modules

#### Core Module

Registers:

- Project lifecycle
- Settings
- Command system
- Job system
- Event bus
- Asset registry
- Workspace registry

#### Sprite Editing Module

Registers:

- Sprite document type
- Canvas panel
- Tool shelf
- Brush tools
- Layer panel
- Palette panel
- Sprite panel
- Draw workspace
- Core sprite editing commands

#### Animation Module

Registers:

- Animation clips
- Timeline panel
- Onion skin panel
- Playback controls
- Animate workspace
- Animation commands
- Animation export hooks

#### Generation Module

Registers:

- Generate workspace
- Prompt composer
- Recipe library
- Structures/styles/templates
- Generation results panel
- Coverage panel
- AI job types
- Generated asset type

#### Pixel Art Module

Registers:

- Indexed palette mode
- Pixel-perfect tools
- Palette reduction
- Dithering
- Pixel grid overlays
- Palette validation
- Pixel-art generation constraints
- Optional Pixel Art workspace/layout

#### Tiles Module

Registers:

- Tiles workspace
- Tile document type
- Tile preview panel
- Autotile rules
- Seam validation
- Tile stamping tools
- Tileset export targets

#### Export Module

Registers:

- Export workspace
- Export validators
- Export presets
- Spritesheet exporter
- PNG exporter
- GIF/video exporter
- Engine metadata exporters

#### Provider Modules

Registers AI providers:

- Remote provider
- Local model provider
- Mock provider
- Future specialized providers

### 7.4 Module boundaries

Modules should not mutate project data directly except through registered commands.

Modules should not create hidden global state unless it belongs to a service registry.

Modules should not assume they are the only contributor to a workspace.

Modules should be able to be disabled in development/test builds.

---

## 8. Registry Architecture

Pixhaus should use registries to discover available capabilities.

### 8.1 Workspace Registry

Stores all workspace definitions.

Used by:

- Top workspace tabs
- Command palette
- Layout manager
- Preferences

### 8.2 Panel Registry

Stores all reusable panels.

Used by:

- Workspaces
- Docking/layout system
- User-customizable layouts

### 8.3 Tool Registry

Stores tools available for the active document/art mode.

Used by:

- Tool shelf
- Shortcut system
- Command palette
- Contextual tool switching

### 8.4 Command Registry

Stores command metadata.

Used by:

- Undo/redo
- Menus
- Command palette
- Automation/macros later
- Testing

### 8.5 Action Registry

Stores contextual actions that may or may not mutate state.

Examples:

- Generate variations
- Clean selected area
- Export selected clip
- Create palette from current sprite

Actions may launch jobs or dispatch commands.

### 8.6 Asset Type Registry

Stores known asset types.

Examples:

- Sprite
- Palette
- Tileset
- Animation clip
- Generated result
- Particle system
- UI sprite

Used by:

- Asset browser
- Project file loader
- Import/export
- Workspaces

### 8.7 Provider Registry

Stores AI and compute providers.

Used by:

- Generate workspace
- AI contextual actions
- Job system
- Provider settings

### 8.8 Importer/Exporter Registry

Stores supported file formats and export targets.

Used by:

- File menu
- Drag/drop
- Export workspace
- Batch processing

### 8.9 Validator Registry

Stores project and export validators.

Examples:

- Transparent pixel validation
- Frame size validation
- Palette color count validation
- Missing animation validation
- Tile seam validation
- Export metadata validation

---

## 9. Project and Document Model

### 9.1 Project

A project is the top-level container.

It owns:

- Project metadata
- Documents
- Assets
- Palettes
- Recipes
- Export presets
- Provider metadata
- Workspace settings
- Plugin/module extension data

### 9.2 Documents

Documents are editable units.

Initial document types:

- Sprite document
- Tileset document
- Recipe library document, if needed

Future document types:

- Particle document
- UI sprite document
- Rigging document
- Scene preview document

### 9.3 Sprite document

A sprite document should support:

- Multiple frames
- Multiple layers
- Sparse cels
- Animation clips
- Frame timing
- Palettes
- Art mode metadata
- Per-frame/layer/cel metadata
- Generated-source metadata

### 9.4 Layer model

Layers should support:

- Name
- Visibility
- Lock state
- Opacity
- Blend mode
- Grouping later
- Type
- Per-frame cels

Potential layer types:

- Raster layer
- Reference layer
- Guide layer
- Shadow/preview layer
- Generated draft layer
- Future vector/shape layer
- Future effect layer

### 9.5 Frame model

Frames should support:

- Frame index/order
- Duration
- Tags
- Notes
- Generated metadata
- Manual-edit metadata
- Clip membership

### 9.6 Cel model

A cel is the content for a specific layer/frame intersection.

Important properties:

- It can be absent.
- It can be linked to another cel.
- It can have generated metadata.
- It can have manual edit markers.
- It should support undoable pixel changes.

### 9.7 Palette model

Palettes should support:

- Named palettes
- Color entries
- Ramps
- Locked colors
- Tags
- Project-level palette
- Sprite-level palette
- Pixel-art indexed palettes
- AI palette constraints

### 9.8 Art mode metadata

Sprites/documents should be able to declare an art mode.

Examples:

- Pixel art
- HD raster
- Hand-painted
- Retro-inspired
- UI sprite
- Tileset
- VFX

Art mode should influence:

- Tool defaults
- Grid behavior
- Rendering filters
- AI constraints
- Export validation
- Palette behavior
- Workspace recommendations

---

## 10. Pixel Art vs General Raster Architecture

### 10.1 Why this matters

Pixel art and general sprite art have different needs.

Pixel art needs:

- Exact pixels
- Indexed palettes
- Nearest-neighbor scaling
- Strict grid behavior
- Low color counts
- Palette-aware generation

General raster sprite art may need:

- Larger canvases
- Soft brushes
- Pressure later
- Alpha blending
- Non-indexed color
- Painterly effects
- Higher resolution exports

Pixhaus should support both.

### 10.2 Internal surface types

The project model should allow multiple surface representations.

Recommended conceptual surface types:

- RGBA raster surface
- Indexed palette surface
- Mask/selection surface
- Preview/composite surface
- Generated draft surface

The surface abstraction should allow pixel art tools to enforce indexed constraints without forcing the whole app to be pixel-only.

### 10.3 Art-mode-specific tooling

Pixel Art mode should expose:

- Pencil instead of soft brush
- Hard eraser
- Palette index picker
- Dithering brush
- Ramp tools
- Color replacement
- Tile grid
- Pixel-perfect transforms

General Raster mode may expose:

- Brush
- Soft eraser
- Opacity/flow
- Smudge/blur later
- Shape tools
- Alpha-aware fill
- Higher-res canvas tools

Both modes can still use layers, frames, animation, and export.

### 10.4 Rendering correctness

Pixel Art mode requires:

- No texture filtering
- Exact grid alignment
- Major/minor grid controls
- Crisp zoom
- Pixel-perfect pointer mapping

General Raster mode may allow:

- Smooth zoom previews
- Anti-aliased brush previews
- Larger texture handling

The renderer should know the active art mode and render accordingly.

---

## 11. Tool System

### 11.1 Tool ownership

Tools belong to the shared Tool System, not to a specific workspace.

Workspaces choose which tools to show and emphasize.

### 11.2 Tool responsibilities

A tool should:

- Interpret pointer/keyboard input.
- Read the active editing context.
- Produce preview overlays.
- Create commands or transactions.
- Respect art mode constraints.
- Respect selection masks.
- Respect active layer/frame/cel.

A tool should not:

- Save files.
- Call AI providers directly.
- Mutate arbitrary project state directly.
- Own durable project data.
- Know too much about workspace layout.

### 11.3 Initial tools

Core manual tools:

- Brush/Pencil
- Eraser
- Fill
- Line
- Rectangle
- Ellipse
- Selection
- Move
- Color Picker
- Pan/Zoom

Pixel Art tools:

- Pixel-perfect pencil
- Dither brush
- Palette replace
- Ramp tool
- Tile stamp

AI-assisted tools:

- AI Brush
- Cleanup brush
- Variation brush
- Material/style transfer brush
- In-between assistant action

Animation tools:

- Frame move/duplicate
- Onion skin controls
- Motion arc overlay
- Pose marker tools later

Tiles tools:

- Tile stamp
- Seam fix
- Terrain brush
- Autotile preview selector

### 11.4 Tool settings

Tool settings should be shared where appropriate.

Examples:

- Brush size
- Opacity
- Hardness
- Pixel-perfect toggle
- Palette lock
- Dither mode
- Symmetry
- Stabilization later
- Selection mode

Settings should be stored according to scope:

- Global user preference
- Workspace preference
- Tool-specific setting
- Project-specific setting
- Document/art-mode-specific setting

---

## 12. Command System

### 12.1 Purpose

The command system is the mutation boundary for project state.

It enables:

- Undo/redo
- Transaction groups
- Reliable save state
- Testing
- History display
- AI result application
- Agent-safe development

### 12.2 Command rules

Commands should:

- Have a user-facing name.
- Be undoable when they mutate project state.
- Be testable outside egui.
- Be composable into transactions.
- Capture enough previous state to undo safely.
- Avoid hidden side effects.

### 12.3 Transactions

Some operations should be grouped.

Examples:

- Brush stroke containing many pixel changes
- Apply generated animation frames
- Import sprite with layers/frames/palette
- Delete a layer and associated cels
- Create a full animation clip

The user should see one undo step for one conceptual action.

### 12.4 Pixel mutation commands

Pixel edits should avoid storing full image copies for every stroke where possible.

The conceptual model should be patch-based:

- A patch records changed positions.
- A patch records before and after values.
- A stroke is one undoable command.

For very large edits, the system may choose snapshots or tiled patches.

### 12.5 AI application commands

AI results should be applied through commands.

Examples:

- Apply result as new sprite
- Apply result as new layer
- Apply result to selected region
- Apply generated frames to clip
- Apply palette suggestion
- Apply cleanup patch

This preserves artist control and undoability.

---

## 13. Job System

### 13.1 Purpose

The job system handles background work.

Jobs should be cancellable where possible and should report progress.

### 13.2 Job categories

AI jobs:

- Text-to-sprite
- Image-to-sprite
- Sprite variations
- In-between generation
- Animation completion
- Palette suggestion
- Cleanup
- Upscale/downscale

Render jobs:

- Thumbnail generation
- Composite preview generation
- Export preview rendering

IO jobs:

- Import
- Export
- Project save
- Project load
- Bundle packing/unpacking

Analysis jobs:

- Coverage analysis
- Tile seam validation
- Palette validation
- Animation jitter detection

### 13.3 Job lifecycle

A job should have:

- ID
- Type
- Input
- Context
- Status
- Progress
- Priority
- Logs
- Result
- Error
- Cancellation state
- Backend/provider metadata
- Creation time
- Completion time

Status is one of:

```text
queued
running
blocked
complete
failed
cancelled
```

Typical lifecycle:

1. UI submits job.
2. Job is queued.
3. Backend/provider picks it up.
4. Progress updates are emitted.
5. Result is stored as an asset or transient result.
6. User applies result through a command.

### 13.4 Job resource requirements

Jobs should declare requirements:

- CPU only
- GPU optional
- GPU preferred
- GPU required
- Remote provider allowed
- Local-only
- Network required
- Provider required
- Local model required
- Large memory required
- Memory estimate
- VRAM estimate
- Can be cancelled
- Can run in parallel

This enables future scheduling decisions.

### 13.5 Mock jobs

The system should include mock providers and mock jobs early.

This allows agents and developers to build UI workflows without relying on paid APIs, downloaded models, or GPUs.

### 13.6 Background worker contract

A worker is handed immutable input and returns a result. It never gets a handle that lets it mutate the live project, so it cannot corrupt project state or undo (section 31.3).

A worker receives one of: an immutable snapshot, an asset handle, an asset path, a content hash, a serialized job input, a copied pixel region, or a read-only project index.

A worker returns one of: a generated asset, a preview image, a metadata result, a validation report, an export artifact path, an import bundle, or an error report.

A worker never receives: mutable app state, a mutable project reference, the egui context, the live undo stack, or the live command bus.

---

## 14. AI Architecture

### 14.1 AI is a service, not a workspace dependency

The Generate workspace is AI-forward, but AI capabilities should also appear contextually in Draw, Animate, Tiles, and Export.

Examples:

- Draw: clean selection, generate variation, fill region.
- Animate: generate in-betweens, fix jitter, extend clip.
- Tiles: make seamless, generate variants, validate edges.
- Export: validate transparency, optimize palette, check missing animations.

### 14.2 Provider abstraction

AI providers should be registered by provider modules.

Provider examples:

- Remote image generation provider
- Local CUDA model provider
- Local Metal model provider
- Local Vulkan/CPU provider
- Mock provider
- Specialized in-between model
- Specialized cleanup model

The app should ask for capabilities, not specific providers.

Example capability questions:

- Who can generate a sprite from text?
- Who can edit a selected region?
- Who can generate in-between frames?
- Who can preserve palette constraints?
- Who can run offline?
- Who supports transparent output?

### 14.3 Provider settings

Provider settings should live in provider-specific settings panels, not hardcoded into the Generate workspace.

The Generate workspace may show selected provider and capability summary, but provider-specific complexity should be modular.

### 14.4 Recipe system

Pixhaus should support reusable AI recipes.

Recipe concepts:

- Templates: what is being made.
- Structures: expected output format.
- Styles: visual treatment.
- Variables: user-configurable slots.
- Constraints: palette, size, transparency, animation count, etc.

This system is core to Pixhaus’ AI-native identity.

### 14.5 Composition Library

The Composition Library should manage:

- Templates
- Structures
- Styles
- Variables
- Recipe packs
- Built-in recipes
- User recipes
- Previews
- Test generations
- Coverage mappings

It should feel like a creative library, not a settings table.

### 14.6 Generation context

AI requests should include context.

Possible contexts:

- No context/new asset
- Current sprite
- Current frame
- Current layer
- Current selection
- Animation clip
- Palette
- Reference image
- Tileset
- Existing generated result

Context is what makes AI feel integrated rather than bolted on.

### 14.7 AI metadata

Generated results should preserve metadata:

- Prompt
- Negative prompt, if any
- Recipe/template/style/structure
- Variables
- Provider
- Model
- Seed
- Date/time
- Source context
- Palette constraints
- Size
- Result settings
- Manual edits after generation

This enables reproducibility, history, and professional workflows.

### 14.8 Local models

Local models should ideally run out-of-process at first.

Benefits:

- Crash isolation
- Easier GPU dependency management
- Easier CUDA/Metal differences
- Optional installation
- Easier future language/runtime flexibility
- Pixhaus can launch without model dependencies

The main app communicates with local model workers through a job/provider protocol.

---

## 15. Animation Architecture

### 15.1 Animation belongs to sprites

Animation should not be a disconnected document type.

A sprite naturally has:

- Frames
- Layers
- Cels
- Clips
- Timing
- Tags
- Onion skin settings
- Playback settings

Draw focuses on one frame. Animate focuses on many frames.

### 15.2 Animation clips

Animation clips define named frame ranges.

Examples:

- idle
- walk
- run
- jump
- fall
- attack
- hurt
- death
- custom

Clip metadata should include:

- Name
- Frame range
- FPS or timing mode
- Loop mode
- Export name
- Tags
- Source recipe, if AI-generated
- Notes

### 15.3 Timeline model

The timeline should represent:

- Frame order
- Frame duration
- Active frame
- Active clip
- Layers/cels per frame
- Linked cels
- Generated/manual edit markers
- Playback range
- Loop region

### 15.4 Onion skin

Onion skin should be part of the animation capability but rendered through the shared canvas renderer.

Settings:

- Show previous frames
- Show next frames
- Number of frames
- Opacity falloff
- Color tint
- Clip-only mode
- Layer-specific onion skin
- Ignore hidden layers

### 15.5 Animation AI

AI animation support should appear as contextual actions:

- Generate in-betweens
- Create idle animation
- Create walk cycle
- Extend clip
- Fix jitter
- Smooth timing
- Generate missing coverage
- Create variations of clip

AI-generated frames should be marked and editable.

### 15.6 Manual animation flow

The manual animation flow should be first-class:

1. Draw first frame.
2. Switch to Animate.
3. Duplicate frame.
4. Enable onion skin.
5. Draw next pose.
6. Adjust timing.
7. Preview loop.
8. Create named clip.
9. Export.

This flow must be excellent without AI.

---

## 16. Rendering Architecture

### 16.1 Renderer responsibilities

The renderer should handle:

- Sprite compositing
- Layer visibility/opacity
- Frame rendering
- Canvas scaling
- Grid overlays
- Selection overlays
- Tool overlays
- Onion skin
- Tile previews
- Generated result previews
- Timeline thumbnails
- Export previews

### 16.2 Texture cache

A texture cache should avoid unnecessary uploads and recompositions.

Cache keys may conceptually include:

- Sprite/frame composite
- Cel/layer texture
- Asset thumbnail
- Generated result
- Tileset preview
- Export preview

Caches should be invalidated by commands and project changes.

### 16.3 Canvas rendering pipeline

Conceptual pipeline:

1. Resolve active document/sprite/frame.
2. Composite visible layers.
3. Apply art-mode rendering rules.
4. Update texture cache if dirty.
5. Draw canvas texture.
6. Draw checkerboard/grid if enabled.
7. Draw onion skin overlays if enabled.
8. Draw selection/tool overlays.
9. Draw HUD/status overlays.

### 16.4 Art-mode rendering rules

Pixel Art:

- Nearest-neighbor scaling
- Crisp grid
- Pixel-perfect pointer mapping
- Optional major grid
- No filtering

General Raster:

- Smooth zoom may be allowed
- Larger canvas previews
- Alpha compositing emphasis
- Brush previews may be softer

### 16.5 Renderer is not the data source

The renderer may cache visual data, but project data remains authoritative.

Never rely on GPU texture contents as the only copy of the artwork.

The renderer runs in its own execution lane, isolated from project truth (section 31.2).

---

## 17. Asset System

### 17.1 Asset library

The asset library should manage reusable project assets:

- Sprites
- Animations
- Palettes
- Tilesets
- Generated results
- References
- Recipes
- Future particles
- Future UI components

### 17.2 Generated assets

Generated results should enter the project as generated assets or transient job results first.

They should not automatically overwrite existing artwork.

User actions:

- Insert as new sprite
- Insert as new layer
- Apply to selected region
- Apply to current frame
- Apply to animation clip
- Save to asset library
- Create variation
- Discard

### 17.3 Asset metadata

Assets should store:

- Name
- Type
- Created date
- Modified date
- Source
- Tags
- Thumbnail
- Art mode
- Related recipe/provider metadata, if applicable

### 17.4 Future asset types

The architecture should allow future asset types:

- Particle system
- UI sprite component
- Nine-slice sprite
- Rig/bone setup
- Reference board
- Material/style pack
- Prompt recipe pack
- Tileset ruleset

Unknown future asset data should be preserved if possible.

---

## 18. Project File Architecture

### 18.1 Goals

The project format should be:

- Extensible
- Versioned
- Durable
- Debuggable
- Recoverable
- Friendly to autosave
- Able to preserve unknown extension data

### 18.2 Development format

During development, a folder-based project format is recommended.

Conceptually:

- Project manifest
- Documents folder
- Assets folder
- Palettes folder
- Recipes folder
- Generated results folder
- Exports folder, optional
- Thumbnails/cache folder, optional
- Extension data folder

This makes debugging easier and reduces risk.

### 18.3 Packaged format

Later, Pixhaus can support a single-file bundle.

A `.pixhaus` file can be a packaged project archive.

The internal structure should mirror the folder format.

### 18.4 Versioning

The project format should include:

- Project format version
- App version that created it
- Migration history, if needed
- Module/extension data versions

### 18.5 Unknown data preservation

If a future module adds data the current app does not understand, the loader should avoid destroying it when possible.

This is important for long-term extensibility.

### 18.6 App directories

Pixhaus puts its own files in platform-standard locations, resolved through the platform crate, never hardcoded paths. The split:

- **Config** — preferences, language, shortcuts, workspace layouts, theme, provider config references.
- **Data** — user-created global presets, recipe packs, palette and brush libraries, module data.
- **Cache** — thumbnails, generated previews, decoded-asset cache, local model cache, temporary export cache.
- **Logs** — the rolling daily log, diagnostic traces, crash reports.
- **Autosave** — recovery snapshots, unsaved-project autosaves, crash-recovery metadata.

Two traps. `directories` only computes paths — create the directory before the first write or it fails with "no such file or directory." On macOS the config and data directories are the same folder, so logs and autosaves are distinct leaves under local data, not config. This is code today: `pixhaus_platform::app_dirs()` and `log_dir()` (`crates/platform/src/dirs.rs`); see the `pixhaus-directories` skill.

---

## 19. Import and Export Architecture

### 19.1 Importers

Importers should be registered capabilities.

Initial importers:

- PNG
- Image sequence
- Spritesheet
- Palette files, if supported

Future importers:

- Aseprite
- GIF
- Tiled
- LDtk
- TexturePacker
- Photoshop-like layered files, if needed

### 19.2 Exporters

Exporters should be registered capabilities.

Initial exporters:

- PNG
- Spritesheet
- GIF or animation preview
- JSON metadata

Future exporters:

- Unity sprite atlas metadata
- Godot resource metadata
- Unreal-compatible metadata
- Aseprite export
- Tiled/LDtk-compatible tilesets
- Web/game-ready atlas formats

### 19.3 Export workspace

The Export workspace should not just be a save dialog.

It should provide:

- Preview
- Validation
- Presets
- Batch export
- Naming rules
- Engine metadata
- Packing options
- Animation clip selection
- Scale options
- Pixel art constraints, where relevant

### 19.4 Validators

Export validators should warn about:

- Mismatched frame sizes
- Missing frames
- Empty frames
- Transparent garbage pixels
- Unsupported palette mode
- Too many colors for pixel-art target
- Missing animation coverage
- Non-looping clip marked as loop
- Tile seams
- Naming conflicts

---

## 20. Layout and Docking Architecture

### 20.1 Default layouts first

Pixhaus should provide excellent default layouts before pursuing fully customizable docking.

Each workspace should have a strong default layout.

### 20.2 Layout abstraction

Even before custom docking, workspaces should declare layout composition conceptually:

- Top bar
- Tool options
- Left rail
- Center surface
- Right inspector
- Bottom tray/timeline
- Status bar

### 20.3 Docking later

A docking system may be introduced later for advanced users.

Docking should be layered on top of registered panels.

Panels must remain reusable and not assume fixed placement.

### 20.4 Bottom tray model

The bottom area should be treated as a production tray.

Depending on workspace, it may show:

- Timeline
- Frames
- Assets
- AI results
- Tile variants
- Export logs

This gives Pixhaus a serious production feel.

---

## 21. Event Architecture

### 21.1 Why events exist

An event system prevents unrelated panels from directly coupling to each other.

Example:

- Timeline selects frame.
- Canvas should update.
- Onion skin should update.
- Inspector should update.
- Status bar should update.

This should happen through state changes/events, not direct panel-to-panel calls.

### 21.2 Event categories

Potential events:

- Project opened
- Project saved
- Document selected
- Sprite selected
- Frame selected
- Layer selected
- Tool selected
- Workspace changed
- Command executed
- Undo/redo occurred
- Job submitted
- Job completed
- Generated result selected
- Provider changed
- Art mode changed

### 21.3 Events vs commands

Commands mutate project state.

Events communicate that something happened or that UI/session state changed.

Not every event is undoable. Every project mutation should still go through commands.

---

## 22. State Architecture

Pixhaus state separates into five buckets, each with a different lifetime and owner. Keep them apart: durable project state is saved, session and UI state are not project content, tool interaction state is cleared when an interaction ends, and derived state is recomputable. Mixing them is how a UI scroll position ends up in a save file and how a half-finished brush stroke ends up in undo. The app owns these containers directly; it does not adopt a generic egui state framework (section 22.7).

### 22.1 Durable project state

Saved in the project:

- Documents
- Sprites
- Layers
- Frames
- Cels
- Palettes
- Animation clips
- Recipes
- Assets
- Export presets
- Art mode metadata
- Project metadata

### 22.2 Session state

Belongs to the running app session:

- Open project
- Active document
- Active workspace
- Active tool
- Active selection
- Active job list
- Undo/redo stack
- Dirty state
- Recent commands

Some of this may be restored after restart, but it is not the same as project data.

### 22.3 UI state

Belongs to presentation:

- Panel collapsed states
- Scroll positions
- Hovered items
- Selected tabs
- Temporary text fields
- Drag state
- Modal state
- Current dock layout
- Zoom/pan

UI state should not pollute the creative model.

### 22.4 Settings architecture

Settings separate by scope, not by panel. Five categories:

- **App settings** — theme, accent, language, UI scale, default workspace, startup behavior, recent files. The shell's `Prefs` is the seed for these.
- **Workspace settings** — dock layouts, visible panels, timeline height, default onion-skin and canvas-background defaults.
- **Tool settings** — brush size, smoothing, pixel-perfect behavior, selection behavior. Stored at the scope they belong to; section 11.4 keeps the finer document/art-mode scope.
- **Provider settings** — enabled providers, priority, key references, local model paths, endpoints, usage limits. These live behind provider modules (section 14.3), not hardcoded into the Generate workspace.
- **Project settings** — default art mode, palette behavior, export presets, project recipe packs, validation rules. These travel with the project.

App, workspace, tool, and provider settings are user/session scope; project settings are durable project state (section 22.1).

### 22.5 Tool interaction state

Tool interaction state is transient — it usually exists only during a direct manipulation and is cleared when the interaction ends or is cancelled.

Examples:

- current brush stroke
- lasso points
- transform preview
- selection drag start
- frame scrub operation
- color-picker hover sample
- canvas pan gesture
- shape preview rectangle
- tile-stamp preview
- AI-brush masked region

Keep it small and explicit. It never leaks into undo or project state — a stroke becomes one undoable command when it commits, not before.

### 22.6 Derived and cache state

Derived state can be recomputed, so it is never the source of truth.

Examples:

- composited frame textures
- timeline and asset thumbnails
- generated-result previews
- palette-usage analysis
- dirty-region maps
- coverage analysis
- compiled prompt previews
- texture handles and GPU buffers
- decoded-image cache

Key it by stable id, content hash, revision counter, dirty region, or asset version, and invalidate it on the matching change (sections 2.10, 16.5, 23.2). A cache that becomes load-bearing is a bug.

### 22.7 App-state containers (target shape, current homes)

The app owns its state containers; it does not adopt a generic egui state framework. The target container set, and where each lives today:

| Container | Owns | Current home |
|---|---|---|
| ProjectStore | loaded project, lazy asset access, dirty/revision tracking, save/load coordination | reserved for `core`; `SessionState` already reserves `active_document`/`undo_stack` |
| AppSession | active project/workspace/document/sprite/frame/layer, active tool, editing context, job and command access | `SessionState` inside `ShellState` inside `Host` (`crates/ui/src/state/`) |
| UiState | dock layout, panel state, list selection, scroll, modal stack, drag, filters, command palette | `crates/ui/src/state/ui_state.rs::UiState`, owned by `Host` |
| EditingContext | where editing applies: active sprite/layer/frame/cel/palette/selection/tool settings | section 5.9 made concrete; not yet a distinct type |
| CommandBus | execute/validate command, record undo, group transactions, mark dirty, emit events | today the deferred-intent path (`IntentSink` + `apply_intent`); real bus lands with `services` |
| JobManager | queue/dispatch/cancel jobs, track progress, deliver results, expose status | today mocked by `SessionState.jobs` + the background channel (section 13) |
| WorkspaceRegistry, ModuleRegistry | workspace and capability registration | already sections 7–8; today `Registries` on `Host` |
| AssetCache | thumbnails, texture handles, previews, decoded assets, invalidation | the section 22.6 bucket; lands in `services`/`render` (section 16.2) |

The names are the target. Today the shell owns a subset under `Host`/`ShellState`/`SessionState`/`UiState`; the missing containers are reserved seams in `core` and `services`, not missing decisions. Fill the seam; do not invent a parallel container.

---

## 23. Performance Architecture

### 23.1 Performance goals

Pixhaus should feel responsive while editing.

Core expectations:

- Smooth pan/zoom.
- Instant brush feedback.
- Low-latency frame switching.
- Non-blocking AI and export jobs.
- Efficient thumbnail generation.
- Responsive timeline even with many frames.

### 23.2 Data locality

Pixel operations should be designed with locality in mind.

Potential future strategies:

- Tiled surfaces for large sprites
- Dirty regions
- Patch-based undo
- Lazy composite updates
- Texture cache invalidation
- Background thumbnail rendering

### 23.3 Large document handling

Although sprites are often small, Pixhaus should not assume everything is tiny.

It may eventually handle:

- HD sprites
- Large character sheets
- Many animation frames
- Multiple layers
- Tilesets
- Reference images
- Generated result batches

### 23.4 UI responsiveness

The UI thread must not block on:

- AI provider calls
- Local model inference
- File exports
- Heavy imports
- Thumbnail generation
- Large palette analysis

Use jobs/workers.

### 23.5 Parallelization priorities

Interactive drawing latency beats throughput. Keep brush ops direct and predictable, then schedule expensive derived updates after the command lands.

Parallelize early:

1. thumbnail generation
2. timeline preview generation
3. frame compositing batches
4. project-load indexes
5. export preparation
6. large image decode/encode
7. palette analysis
8. coverage analysis
9. AI result post-processing
10. save compression and hashing

Parallelize later: batch filters and transforms, spritesheet packing search, tile seam QA, multi-frame validation, generated-asset ranking.

Do not rush: pencil strokes, small erases, simple selections, immediate UI state changes, small palette edits. These are latency-sensitive, not throughput-bound — parallelizing them early adds complexity for no felt gain.

### 23.6 Modern hardware usage

- **CPU.** Use multiple cores for thumbnails, exports, batch validation, palette ops, compression, import processing, and coverage (section 31.2, the CPU worker pool).
- **GPU.** Use it for rendering, canvas previews, compositing where it pays, and local AI where a provider supports it (section 31.2 keeps render and compute separate).
- **Memory.** Do not eager-load a multi-gigabyte project. Lazy-load, index assets, browse thumbnails first, bound caches with eviction, and memory-map large blobs where it helps (`memmap2` is a candidate, section 33).
- **Storage.** Content hashes, compressed chunks, atomic saves, autosaves, and incremental asset writes (section 18).

---

## 24. Error Handling and Recovery

### 24.1 Error philosophy

Pixhaus should be robust and transparent.

Errors should be:

- User-understandable where surfaced.
- Developer-diagnostic where logged.
- Recoverable when possible.
- Non-fatal unless truly unrecoverable.

A surfaced error should be actionable. "IO error" tells the artist nothing. "Could not load sprite asset. The project manifest references an asset file that is missing from disk. Restore from autosave, relink the asset, or remove the missing reference." tells them what happened and what to do next.

### 24.2 Project safety

Important safety features:

- Autosave
- Recovery files
- Save transactions
- Avoid corrupting project on failed write
- Preserve unknown data
- Backup before migration

### 24.3 Job failures

A failed job should not crash the app.

Examples:

- AI provider unavailable
- CUDA unavailable
- Model missing
- Export path invalid
- Import file unsupported

The UI should show actionable errors.

### 24.4 Provider failures

Provider failures should be isolated.

If a local model worker crashes, Pixhaus should continue running.

### 24.5 Diagnostic bundle

Pixhaus should be able to assemble a diagnostic bundle when something fails: recent logs, app version, OS and platform, renderer backend, GPU adapter info, enabled modules, a provider-configuration summary without secrets, a project-manifest summary, and recent job and crash failures. Never include API keys or private project assets without explicit user consent. The bundle is a support and debugging artifact, not telemetry — it is assembled on request, not sent by default.

---

## 25. Testing Strategy

### 25.1 Test outside egui

Most domain logic should be testable without UI.

Test heavily:

- Project load/save
- Commands
- Undo/redo
- Pixel patches
- Palette operations
- Frame operations
- Animation clips
- Prompt compilation
- Coverage detection
- Export validation
- Import/export round trips

### 25.2 Workspace acceptance tests

Each workspace should have clear acceptance criteria.

Draw workspace is ready when:

- A user can create a sprite.
- Draw/erase/fill/select works.
- Palette and layer workflows work.
- Undo/redo works.
- Save/reload works.
- PNG export works.

Animate workspace is ready when:

- A user can create frames.
- Draw on any frame.
- Use onion skin.
- Play animation.
- Define clips.
- Export animation.
- Undo/redo frame operations.

Generate workspace is ready when:

- Recipes compile.
- Jobs run through mock provider.
- Results appear in result tray.
- Results can be applied through commands.
- Metadata is preserved.
- Provider failures are handled.

### 25.3 Agent development contracts

When using agents heavily, give them constrained contracts:

- Which module they can modify.
- Which capability they are implementing.
- Which commands they may add.
- Which registries they may register into.
- Which tests must pass.
- What boundaries they must not cross.

This reduces architectural drift.

### 25.4 Runtime architecture acceptance criteria

The runtime, state, and concurrency architecture is healthy when:

- The UI stays responsive during project load, save, and export.
- AI generation never blocks drawing.
- Generated results are previewed before they are applied, and applying them is undoable.
- Background workers cannot corrupt the live project state.
- Large projects open from metadata and indexes before all assets load.
- Thumbnail and timeline-preview generation never freezes the app.
- Localization can change without rewriting panels.
- Logs explain what happened when a provider, export, or save fails.
- Dock layouts can be reset or customized.
- Workspaces share tools and panels without duplication.

---

## 26. Suggested Development Roadmap

### 26.1 Phase 0: Architectural scaffold

Goal:

> Establish the host, registries, state boundaries, module system, and project/session/UI state separation.

Deliverables:

- Host app lifecycle
- Internal module registration
- Workspace registry
- Panel registry
- Tool registry
- Command registry
- Job registry
- Basic project model
- Basic settings
- Theme tokens

### 26.2 Phase 1: Shared Sprite Editing Core

Goal:

> Build the shared editing foundation used by Draw and Animate.

Deliverables:

- Sprite document model
- Layers
- Frames
- Cels
- Active editing context
- Canvas renderer
- Tool system
- Basic tools
- Command system
- Undo/redo
- Palette system
- Save/load

### 26.3 Phase 2: Draw Workspace

Goal:

> Polish single-frame and general sprite editing.

Deliverables:

- Draw layout
- Tool shelf
- Tool options
- Palette panel
- Layers panel
- Sprite panel
- Compact frame strip
- Canvas polish
- Basic export

### 26.4 Phase 3: Animate Workspace

Goal:

> Add time to the shared editing core.

Deliverables:

- Large timeline
- Frame operations
- Playback
- Frame timing
- Animation clips
- Onion skin
- Animation inspector
- Drawing into active frames
- Animation export basics

### 26.5 Phase 4: Generate Workspace

Goal:

> Add AI-native generation without compromising manual editing.

Deliverables:

- Prompt recipes
- Templates/structures/styles
- Variables
- Mock provider
- Provider registry
- Generation jobs
- Result tray/grid
- Generated asset type
- Apply result commands
- Metadata/history

### 26.6 Phase 5: Pixel Art Mode

Goal:

> Add dedicated pixel art constraints and tools.

Deliverables:

- Indexed palette surfaces
- Pixel-perfect drawing
- Palette locking
- Dithering
- Color-count validation
- Pixel grid controls
- Palette-preserving AI constraints
- Pixel cleanup actions

### 26.7 Phase 6: Tiles Workspace

Goal:

> Support tile and terrain production workflows.

Deliverables:

- Tile document model
- Tile preview
- Seam validation
- Tile stamp tools
- Autotile rules
- Tile variants
- Tile export
- AI tile generation hooks

### 26.8 Phase 7: Export Workspace

Goal:

> Make production output first-class.

Deliverables:

- Export previews
- Spritesheet export
- GIF/video export
- Engine presets
- Metadata export
- Validation checklist
- Batch export

### 26.9 Phase 8: Advanced extensibility

Goal:

> Add future internal modules cleanly.

Candidates:

- Particle VFX workspace
- Sprite UI workspace
- Palette Lab
- Rigging/pose workspace
- Batch generation workspace
- Local model manager

---

## 27. Non-Negotiable Architecture Rules

1. Workspaces do not own core data models.
2. Draw and Animate share the same editing core.
3. Tools create commands; they do not randomly mutate project state.
4. AI generation creates results; applying results is a command.
5. The UI thread must not block on heavy work.
6. GPU textures are caches/views, not source data.
7. Pixel art is supported deeply but does not define the whole product.
8. Modules are internal, compiled-in, and registry-based.
9. Provider-specific logic stays behind provider modules.
10. Project format must be extensible and versioned.
11. Unknown future extension data should be preserved where possible.
12. egui must not become the architecture.
13. Every destructive action should be undoable unless explicitly impossible.
14. Workspaces are layouts over capabilities.
15. Agents should work within module/capability boundaries.
16. The app owns its state architecture; no generic egui state framework owns it.
17. Every mutable state has one owner; there is no single global locked app-state object.
18. Tokio is the async-I/O lane, owned by the binary — not the whole app.
19. Large projects open from index and metadata before all assets load.

---

## 28. Recommended Mental Model

Pixhaus is not one editor screen.

Pixhaus is a creative operating environment composed of:

- A native host
- Internal modules
- Shared creative core
- Workspaces
- Panels
- Tools
- Commands
- Jobs
- Assets
- Providers
- Renderers
- Importers/exporters

The simplest architecture diagram is:

```text
User intent
  ↓
Workspace / Panel / Tool
  ↓
Action, Command, or Job
  ↓
Creative Core / Job Result
  ↓
Renderer / Asset Library / UI State
  ↓
User sees result
```

For manual editing:

```text
Pointer input
  ↓
Tool
  ↓
Command
  ↓
Project mutation
  ↓
Renderer update
```

For AI generation:

```text
Prompt / recipe / context
  ↓
Generation job
  ↓
Provider/backend
  ↓
Generated asset
  ↓
User chooses result
  ↓
Apply command
  ↓
Project mutation
```

For animation:

```text
Active sprite + active frame + active layer
  ↓
Shared editing tools
  ↓
Frame/cel mutation through commands
  ↓
Timeline/playback/onion skin updates
```

---

## 29. Final Architectural Position

Pixhaus should be built as:

> A modular, native, Rust-based sprite creation and animation platform with a shared creative core, task-focused workspaces, internal capability modules, cross-platform GPU rendering, provider-based AI generation, command-based editing, job-based background execution, and art-mode-specific tooling for both general sprite art and pixel art.

The most important implementation decision is to build the shared editing core before over-specializing individual workspaces.

The most important product decision is to make manual creation excellent and AI optional-but-powerful.

The most important extensibility decision is to use internal modules and registries rather than external native dynamic plugins.

The most important UX decision is to treat workspaces as focus modes, not separate apps.

Pixhaus’ long-term strength will come from this combination:

- Manual editor credibility
- Animation-first production workflow
- AI-native recipe system
- Strong export pipeline
- Deep pixel art mode
- Multi-style sprite support
- Internal modular architecture
- Cross-platform native performance

This is the foundation that allows Pixhaus to grow from a sprite editor into a complete game-art production studio.

---

## 30. References and Technology Notes

These notes are included to ground current technology assumptions.

- `wgpu` is a cross-platform Rust graphics API that runs natively on Vulkan, Metal, DirectX 12, and OpenGL, aligning with Pixhaus’ Windows/macOS/Linux rendering needs.
- `eframe` supports renderer selection between `glow` and `wgpu` when corresponding features are enabled, and recent crate documentation indicates wgpu is the default renderer in current eframe releases.
- `egui_dock` provides docking support for egui — opening/closing tabs, moving/resizing, and undocking into egui windows. It is a candidate for when custom dock layouts land (section 20.3), not an adopted dependency; default layouts come first (section 20.1). The panel architecture leaves room for it.
- Rust native dynamic plugin systems are complex because Rust does not provide a stable general-purpose Rust ABI for arbitrary dynamic plugin boundaries. Pixhaus should therefore prefer internal modules, data/plugin packs, and out-of-process provider workers rather than external native dynamic plugins.

---

## 31. Runtime and concurrency architecture

Pixhaus is a native creative app on multi-core, GPU-backed workstations, and it should use that hardware. But concurrency belongs in jobs and services, not scattered through UI code. Organize it as a small set of execution lanes, each with one responsibility and a clear isolation rule.

### 31.1 Concurrency philosophy

Concurrency is organized through jobs and services, not spread across panels and tools. A panel collects intent; a command mutates; a job runs expensive work off the UI thread; a channel returns the result. When that discipline holds, the app stays responsive and the threading stays debuggable. When it breaks — a panel spawning its own thread, a tool blocking on I/O — responsiveness and undo correctness go with it.

### 31.2 Execution lanes

Pixhaus has five conceptual lanes. A lane is a responsibility plus an executor, not a literal thread count.

- **UI lane.** Runs the egui frame: input, lightweight state updates, command submission, job-progress display, and applying completed results through commands. One thread; it owns the document directly. It must stay responsive — never block it on I/O, generation, export, or a lock held across `.await`.
- **Render/GPU lane.** Texture uploads, canvas and preview rendering, wgpu submission, GPU resource and render-cache management. Isolated from project truth: GPU textures are caches and views, not source data (section 16, rule 6).
- **CPU worker pool.** Image ops, thumbnail batches, palette analysis, color reduction, compression, exports, validation, coverage. Today this runs off the UI thread via tokio `spawn_blocking`; `rayon` is a candidate for data-parallel iteration over independent assets, frames, tiles, or pixels if a workload earns it (section 33).
- **Async I/O runtime.** Remote provider calls, downloads, local-worker IPC, network and async file work. This is `tokio`, owned by the binary — one runtime, no scattered `#[tokio::main]`. The whole app does not become a Tokio app; Tokio is one lane.
- **AI/model workers.** Local model inference and provider-specific execution. Keep these out-of-process at first for crash, dependency, and memory isolation (section 14.8) — the app stays up when a model backend does not.

### 31.3 The golden state rule

The running app owns the authoritative project state. Background workers never mutate the live project directly. A worker receives immutable input, does its work, and returns a result; the app applies that result through a command, which records undo/redo and marks caches dirty. This protects undo correctness, save consistency, dirty tracking, cancellation, AI result review, and crash recovery — and it makes the threading predictable to debug. The full input/output contract is section 13.6.

### 31.4 Job results never mutate project state directly

A job produces a result; applying the result is a command (rule 4). The result enters a result store, the UI presents it, the user chooses, and a command applies it to the project under undo. This keeps the artist in control of every change a job proposes.

```text
GenerateSpriteJob   -> GeneratedAsset          -> user selects -> ApplyGeneratedAssetCommand
ImportAsepriteJob   -> ImportedAssetBundle      -> user confirms -> AddImportedAssetsCommand
ReducePaletteJob    -> PaletteReductionPreview  -> user accepts  -> ApplyPaletteReductionCommand
```

### 31.5 Channels and message passing

Workers talk to the UI lane by message passing, not by sharing a locked app-state object. The shell holds a background channel (today `std::sync::mpsc`) that the egui loop drains each frame, applying results through commands and requesting a repaint. Pass a snapshot, handle, or job request in; get a result, progress update, or error out. A heavier channel (`flume`, `crossbeam-channel`) and shared-cache primitives (`arc-swap`, `dashmap`) are candidates for when the messaging or a concurrent cache earns them (section 33); none is adopted yet.

---

## 32. Localization architecture

Pixhaus is not localized today, but fix the model now so strings do not get hardcoded into panels and project files do not store display text as truth. Localization is a service, not scattered string handling.

### 32.1 Localization is a service

The localization service owns string lookup. Core project logic never calls it — `core` stores stable ids and metadata, never localized strings. Display names are localized at render time, in the UI lane.

### 32.2 Stable keys and module namespaces

Strings are addressed by stable key in a module-owned namespace, so a module ships its own strings without colliding with another's:

```text
app.menu.file
workspace.draw.title
panel.layers.title
tool.pencil.label
command.undo.draw_pixels
provider.openai.label
export.png.label
job.generate_sprite.running
error.project.asset_missing
```

Project files store ids and metadata, never localized strings as the only source of truth (section 18.5). A renamed display string must not invalidate a saved project.

### 32.3 Requirements

The service should support runtime language switching, a fallback language, interpolation, pluralization where available, missing-key diagnostics, and a dev-mode key-display toggle. `egui-i18n` is a candidate to sit behind the Pixhaus-owned service (section 33); the service boundary stays even if the backing crate changes.

---

## 33. Runtime crate stack (reconciled appendix)

The authority for dependencies is the workspace `Cargo.toml` catalog, governed by the "Stack — locked" policy in the root `CLAUDE.md`. A crate not in the catalog is a candidate for when the capability lands — not an adopted dependency. Adding one is a decision: justify the need, check the license against the MIT lock with `cargo deny`, and load the matching `pixhaus-<dep>` skill before reaching for its API. This appendix maps the runtime concerns above onto that policy; it does not expand the locked stack.

### 33.1 Adopted (in the catalog)

| Concern | Crate |
|---|---|
| UI shell + window | `eframe`, `egui`, `egui_extras`, `egui-phosphor` |
| Canvas render + GPU | `wgpu` (pinned `=29.0.1`), `egui-wgpu`, `glam`, `bytemuck`, `pollster` |
| Async backbone | `tokio`, `tokio-util`, `futures` |
| Sync | `parking_lot` |
| Logging | `tracing`, `tracing-subscriber`, `tracing-appender`, `tracing-log` |
| Errors | `thiserror` (libs), `anyhow` (binary) |
| Serde backbone | `serde`, `serde_json` |
| Images | `image` (png feature) |
| Platform paths + dialogs | `directories`, `rfd` |
| Test stack | `rstest`, `proptest`, `insta`, `tempfile`, `mockall`, `image-compare`, `egui_kittest` |

### 33.2 Candidates (not adopted; pull when the capability lands)

| Concern | Candidate | Adoption trigger |
|---|---|---|
| Docking | `egui_dock` | when custom dock layouts land (section 20.3) |
| Localization | `egui-i18n` | when the localization service lands (section 32) |
| CPU data-parallelism | `rayon` | when a batch workload outgrows `spawn_blocking` (section 31.2) |
| Heavier channels | `flume`, `crossbeam-channel` | when std mpsc is outgrown (section 31.5) |
| Shared caches | `arc-swap`, `dashmap` | when a concurrent cache earns it |
| Ids + collections | `slotmap`, `uuid`, `indexmap`, `petgraph` | when the model needs stable arenas or a dependency graph |
| Settings + save format | `toml`, `rmp-serde`, `postcard`, `zstd`, `blake3` | when the project/save format lands (section 18) |
| Large blobs + watching | `memmap2`, `walkdir`, `notify` | when lazy asset loading or folder watching lands |
| More image formats | `gif`, `resvg` | when GIF export or SVG import lands |
| Clipboard + open | `arboard`, `open` | when clipboard or open-in-OS lands |
| Rich diagnostics | `miette`, `color-eyre` | when user-facing diagnostic reports earn it |
| Caching | `moka`, `lru` | when a bounded eviction cache earns it |
| Profiling | `puffin`, `criterion`, `divan` | when frame profiling or benchmarks land |

### 33.3 Corrections

- App directories use `directories` v6, not `directories-next` or `dirs-next`.
- PNG support comes from `image`'s png feature, not a standalone `png` crate.
- `egui_mobius` is rejected — Pixhaus keeps a custom state architecture and does not adopt a generic reactive egui framework.
- No channel crate is adopted; the current channel is `std::sync::mpsc`. `flume` and `crossbeam-channel` are candidates only.

### 33.4 Defaults

- Docking sits behind the workspace-layout abstraction (`egui_dock` candidate), and default layouts come first (section 20.1).
- Localization sits behind the Pixhaus localization service (`egui-i18n` candidate), never called from core logic (section 32.1).
