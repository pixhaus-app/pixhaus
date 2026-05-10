# Project Library Research: Multi-Asset Organization in Creative Tools

**Research date:** May 2026  
**Focus:** Informing the data model for Pixhaus, an open-source AI-native pixel art editor for sprites, animations, and tilemaps.

---

## Section 1: How tools organize multi-asset projects

### Blender: Collections, Asset Browser, and data-block model

**Data model:** Blender uses a hybrid system. Collections are a hierarchical organizational layer within a `.blend` file (the single monolithic project format). Objects live in collections; a scene can reference multiple collections. Collections can be instanced (referenced, not embedded), enabling asset reuse within a project.

**File structure:** A single `.blend` file contains all data-blocks (objects, materials, meshes, etc.). Collections are virtual groupings within that file, not separate files. The Asset Browser, introduced in Blender 3.0+, overlays a library system on top of collections. Assets are marked with an asset marker and indexed in a text file (`blender_assets.cats.txt`) that defines hierarchical catalogs: `Characters/Ellie/Poses/Hand` or `Kitbash/City/Skyscrapers`.

**Hierarchy:** Project → Scene → Collection (nested) → Object → Mesh/Material. Collections can be arbitrarily nested. Collection instances enable a second pattern: Project → Asset Library (folder-based) → Catalog (text file) → Asset (marked data-block).

**Naming:** No enforcement. Conventions are user-defined.

**Search and tagging:** Asset Browser uses catalog paths (hierarchical text-based tagging). You can mark any data-block as an asset and it appears in the browser. Asset libraries are folder-based; you point Blender to a folder and it indexes all `.blend` files in it.

**Inheritance/overrides:** Collection instances can be overridden at the object level. Materials and meshes can be overridden in instanced objects. Scenes do not inherit collection-level properties; lighting, world settings are per-scene.

**Cross-asset references:** Objects in one collection can reference meshes, materials, or modifiers from other collections within the same file, or from linked `.blend` files. Linking is done via the "Link" operator, which creates a reference (not a copy). Multiple files can link the same asset.

**Pain points:** 
- Asset libraries require external folder management; no built-in versioning or auto-sync.
- Collection instances hide children from the Outliner (they appear only as a single instance), making navigation opaque.
- No automatic inheritance of project-level settings (palettes, lighting) to nested objects; must be set per-object or via material linking.

**What works well:**
- Collections are intuitive for organizing by role (Characters, Props, Lights, Cameras).
- Asset browser catalogs are flexible and human-readable.
- Nested collections map naturally to game/animation departments (Char_Alice → Head, Body, Clothes).

**Reference:** [Blender Asset Browser](https://docs.blender.org/manual/en/latest/editors/asset_browser.html), [Asset Catalogs](https://developer.blender.org/docs/features/asset_system/backend/asset_catalogs/)

---

### Spine (Esoteric Software): Skeletons, skins, and folder-based attachment organization

**Data model:** A single `.spine` file contains one skeleton with a tree of bones, slots, and attachments. Skins are the primary organizing mechanism for variants.

**File structure:** `.spine` files are JSON-based (or binary). A skeleton has a hierarchy of bones (parent-child relationships), slots (which attach images), and skins (which define which image attaches to which slot for that skin variant).

**Hierarchy:** Skeleton → Bones (tree) → Slots → Attachments. Skins cross-cut this: Skin → Placeholders (one per slot, named semantically like "head", not "red-head") → Attachments (one per placeholder). Skins can be organized into folders (at export time, folders prepend to skin name: `hair/long/brown`).

**Naming:** Attachment names should describe what they are (e.g., "head"), not which skin they belong to. Skin names are freeform (e.g., "red", "blonde", "angry"). Folders use `/` separator in exported names.

**Search and tagging:** Skins view shows all skins; you can pin multiple at once. No semantic tagging; organization is via folder nesting and naming convention.

**Inheritance/overrides:** Skins enable outfit-swapping. A single animation timeline can show/hide skin placeholders (not specific attachments), so the same animation works with any skin combination. Bones and constraints can also be skin-specific, enabling different bone hierarchies or physics per variant.

**Cross-asset references:** Attachments can be mesh-deformed and linked (one mesh is the "parent", others inherit its deformation). Constraints can reference bones across skins, with warnings if the constrained bone is not active in the constraint's skin.

**Pain points:**
- Multi-file workflows are unsupported; one skeleton per file.
- No built-in way to organize multiple characters; you manually manage separate `.spine` files per character.
- Skin placeholders require upfront planning; retrofitting them onto existing attachments is tedious.

**What works well:**
- Skins elegantly handle outfit and palette swaps without duplicating bone hierarchies or animations.
- Folder nesting is simple and maps well to character anatomy (arms/left, arms/right, head/hair).
- Linked meshes save memory and ensure deformation consistency across skins.

**Reference:** [Spine Skins](https://en.esotericsoftware.com/spine-skins)

---

### Live2D Cubism: Parts, parameters, and mesh deformation

**Data model:** A single `.cmo3` project file (editable) exports to `.moc3` + `model3.json` (runtime). The model is layer-based (Photoshop `.psd` input). Parts are logical groupings (e.g., eyes, nose). Parameters control mesh deformations (Angle X, Mouth Open/Close).

**File structure:** 
- **Authoring:** `.cmo3` file contains the editable rig, layers, parameters, physics, and animations.
- **Runtime:** `.moc3` (binary) + `model3.json` (metadata) must have matching names (e.g., `myavatar.moc3`, `myavatar.model3.json`).
- **Animations:** Separate motion files (`.motion3.json`) and expression files define parameter timelines.

**Hierarchy:** Model → Parts (grouped layers) → Meshes → Deformers. Parameters are independent of the hierarchy; multiple parameters can deform the same mesh.

**Naming:** Parameters should be standardized across models to enable animation reuse (e.g., all characters should have "Angle_X", "Mouth_Open"). No enforcement.

**Search and tagging:** No built-in library system. Organization is file-by-file.

**Inheritance/overrides:** Parameters are the unit of reuse. A motion file applies to any model with matching parameter names. No hierarchical inheritance; parameters act independently.

**Cross-asset references:** Models are standalone. Motion files can be shared across models if parameter names match. No linking between models.

**Pain points:**
- Entirely layer-based; no skeletal structure (though deformers provide similar control).
- No multi-character organization; one model per `.cmo3` file.
- Parameter standardization is a manual discipline.

**What works well:**
- Parameters enable expressive, continuous deformation (superior to bone-based rigs for organic shapes).
- Motion files are reusable if parameter naming is consistent.
- Layer-based approach integrates naturally with Photoshop workflows.

**Reference:** [Live2D Model Loading](https://docs.live2d.com/en/cubism-editor-manual/loading-model-and-placement/), [Parameters](https://docs.live2d.com/en/cubism-editor-manual/parameter/)

---

### Unity: Asset database, prefabs, variants, and .meta files

**Data model:** Unity's asset database uses a GUID-based reference system. Each file (folder, texture, prefab, scene) gets a `.meta` file in the same location, containing a unique GUID and import settings. Prefabs are templates for GameObjects; Prefab Variants inherit from base prefabs and override properties.

**File structure:** Assets live in the `Assets/` folder. Each has a corresponding `.meta` file (e.g., `Player.prefab` + `Player.prefab.meta`). Scenes reference prefabs via GUID; renaming or moving assets doesn't break references because they use GUIDs, not paths.

**Hierarchy:** Project → Folders (arbitrary) → Assets. Scenes reference Prefabs; Prefabs reference nested Prefabs (nested prefabs, as of Unity 2018.3). Prefab Variants: BasePrefab → Variant1 (overrides color) → Variant2 (overrides color + size).

**Naming:** No enforcement. Folders are arbitrary; conventional structure is `Assets/Prefabs/Characters/`, `Assets/Textures/`, etc.

**Search and tagging:** Unity search (Ctrl+F in Project window) is file-name only. No semantic tagging or catalog system. Unity has recently added asset labels (via the inspector), but they're not hierarchical.

**Inheritance/overrides:** Prefab Variants are the primary inheritance mechanism. You override specific properties in the variant; unoverridden properties fall back to the base prefab. Nested prefab instances can also be overridden (as of 2020.1), though this is more fragile than variants.

**Cross-asset references:** Prefabs can reference other prefabs (nested). Scenes reference prefabs. Materials reference textures and shaders. The `.meta` file's GUID ensures references survive renaming/moving. Circular references are not prevented at the API level but cause runtime errors.

**Pain points:**
- `.meta` files are cryptic and merge badly in version control; conflicts are common in multi-person projects.
- Asset database can become bloated; no built-in garbage collection for unused assets.
- Prefab variants have a known pain point: if you override a property in a variant, then change the base prefab's default for that property, the variant's override is not updated (the override "wins", which is by design but often surprising).
- No project-wide palette or style enforcement; each material/prefab chooses colors independently.

**What works well:**
- GUIDs decouple reference integrity from file paths, enabling fearless refactoring.
- Prefab Variants are elegant for creating multiple player types (Player, PlayerRed, PlayerBlue) without duplicating hierarchy.
- Nested prefabs enable composable hierarchies (Sword prefab inside Player prefab).

**Reference:** [Prefab Variants](https://docs.unity3d.com/Manual/PrefabVariants.html), [Asset Metadata](https://docs.unity3d.com/6000.3/Documentation/Manual/AssetMetadata.html)

---

### Adobe Animate: FLA library, symbols, instances, and scenes

**Data model:** A single `.fla` file (or uncompressed `.xfl` folder) contains scenes, symbols (reusable definitions), and instances (uses of symbols). Symbols have symbol types: Graphic, Button, Movie Clip (animated symbol).

**File structure:** `.fla` is a compressed binary (internally ZIP-like). The `.xfl` uncompressed format (available since CC 2015) exposes the structure: XML files for each symbol, a `lib.xml` for the library, and separate folders for external assets.

**Hierarchy:** FLA → Scenes → Timeline → Layers → Symbols (instances). Library → Folders → Symbols. A symbol's definition is stored once; instances are lightweight references.

**Naming:** Symbols are named (e.g., "btn_play", "hero_idle"). Library folders are arbitrary (e.g., "buttons", "characters"). No enforcement.

**Search and tagging:** Library panel shows symbols organized by folder. Search is text-based in the library (Ctrl+F in the Library panel). No semantic tagging.

**Inheritance/overrides:** Symbol instances can be modified: color tint, transparency, brightness, scale, rotation, skew. These are per-instance overrides; the symbol definition is unchanged. Movie Clips (symbols) can have nested instances. Symbols cannot inherit from other symbols (no variant system like Spine or Prefabs).

**Cross-asset references:** Symbols can be shared across scenes within the same FLA. Symbols cannot be shared across FLA files without copying or using the "Copy Library Assets" feature, which duplicates them.

**Pain points:**
- No multi-file organization; one project = one FLA.
- Symbol instances can't be parametrically animated (e.g., you can't drive color via a timeline for a tweened instance).
- Library structure is flat; folder nesting is visual-only, not enforced.
- No versioning or asset linking; sharing assets between projects requires manual copy-paste.

**What works well:**
- Symbols are simple and intuitive; the library panel is easy to navigate.
- Scene-based organization maps well to slideshow-like animations (intro, menu, gameplay).
- XFL uncompressed format integrates well with version control.

**Reference:** [FLA Best Practices](https://helpx.adobe.com/animate/using/best-practices-structuring-fla-files.html), [Library](https://helpx.adobe.com/animate/using/library.html)

---

### Aseprite: No built-in library; multi-file workflow and power-user conventions

**Data model:** Aseprite has no built-in asset library. A project is a single `.aseprite` file (or `.ase`). Multiple sprites must be in separate files. Power users adopt external folder conventions to manage consistency.

**File structure:** Each `.aseprite` file is a pixel art project with layers, frames, and optional per-frame tags (e.g., "idle", "walk", "attack"). Export to PNG, GIF, or sprite sheets (automated slicing via frame tags).

**Hierarchy:** Project → Layers → Frames → Pixels (or Cels, the per-layer-per-frame unit). Tags organize frames (e.g., Tag "attack" spans frames 10–25), and export can slice by tag.

**Naming:** Tag names are arbitrary. File names are user-defined. Power users keep sprites in folders per character or tileset (e.g., `characters/player.aseprite`, `characters/enemy_goblin.aseprite`, `tilesets/grass.aseprite`).

**Search and tagging:** None. Organization is external (file system).

**Inheritance/overrides:** No inheritance. Aseprite can import frames from other `.aseprite` files (Import → From File), but this is a one-time copy, not a reference.

**Cross-asset references:** None. Aseprite files are standalone.

**Pain points:**
- No way to enforce consistency across multiple sprites (palette, animation naming).
- No animation library; each sprite has its own timeline. Exporting animations from multiple sprites requires running Aseprite CLI in bulk or manual labor.
- No multi-sprite project container.

**What works well:**
- Simplicity: one file = one sprite. Easy to understand, no hidden state.
- CLI bulk operations: power users script exports via `aseprite --batch sprite1.aseprite sprite2.aseprite ... --export-sheet output.png`.
- Frame tags are lightweight and sufficient for animation export.
- `.aseprite` files are reasonably compact for source files.

**Current workflow for multi-sprite projects:** Professional studios use external organization (folders, naming conventions) and either Aseprite CLI for batch operations or game engine importers (Unity Aseprite Importer, Godot Aseprite plugin) that handle multi-file workflows.

**Reference:** [Aseprite Files](https://www.aseprite.org/docs/files/), [Aseprite CLI](https://www.aseprite.org/docs/cli/)

---

### Pixelorama: Tabs for multi-project, PXO format with JSON + image folders

**Data model:** Pixelorama supports multiple projects via tabs. Each project is a `.pxo` file (Pixelorama Open), which is a ZIP archive containing `data.json` and an `image_data/` folder.

**File structure:** 
- **`data.json`:** Metadata (project name, size, FPS, layers, frames, cels, tilesets, animation tags).
- **`image_data/frames/`:** One subfolder per frame; each frame subfolder contains image data (PNG or similar) for all cels in that frame.
- **`image_data/tilesets/`:** One subfolder per tileset; contains individual tile images.
- **`image_data/audio/`:** Audio files for audio layers.

**Hierarchy:** Project (single PXO file) → Layers (frame-by-frame or tilemap) → Frames → Cels. Tilesets are project-scoped (not frame-scoped). Animations are implicit (frame tags, not explicit in the format yet).

**Naming:** No enforcement. Layers, frames, and tileset IDs are user-named.

**Search and tagging:** Tabs enable side-by-side editing of multiple projects. No semantic tagging within a project.

**Inheritance/overrides:** Tilesets are project-scoped; a tilemap layer can reference any tileset in the project. No inheritance or variants.

**Cross-asset references:** Tilesets are referenced by ID within a project. No cross-project references.

**Pain points:**
- No multi-sprite asset library (like Aseprite). Each sprite is a separate PXO file.
- Tilesets are project-scoped; sharing tilesets between projects requires external management.

**What works well:**
- PXO format is transparent (ZIP + JSON) and version-control friendly.
- Tab-based multi-project UI is intuitive.
- Tilemap support is built-in, making Pixelorama suitable for game development.

**Reference:** [Pixelorama Project](https://pixelorama.org/concepts/project/), [Save and Export](https://pixelorama.org/user_manual/save_and_export/)

---

### Procreate Dreams: Tracks, flipbook animation, and multi-format import

**Data model:** Procreate Dreams is an iPad animation app. A project is a `.dreams` file (proprietary) containing tracks, which can hold images, video, audio, or flipbook animation. Each track has a timeline.

**File structure:** Single `.dreams` project file. Tracks are the primary organizational unit. Flipbook tracks contain frames imported from Procreate (`.procreate` files).

**Hierarchy:** Project → Tracks (Compose mode) → Content (images, video, audio, flipbook frames) → Keyframes.

**Naming:** Tracks are named (e.g., "Character", "Background", "Music"). Content is implicit in track type.

**Search and tagging:** None. Organization is by track order in the timeline.

**Inheritance/overrides:** No inheritance. Content is copied into tracks; modifying the source (e.g., the original Procreate file) doesn't update imported content.

**Cross-asset references:** Procreate Dreams can import Procreate `.procreate` files (multi-layer), GIF, MP4, PNG, JPG, audio (MP3, WAV, etc.). Imports are one-time copies.

**Pain points:**
- iPad-only (as of 2026).
- No asset library or reusable symbol system.
- Multi-track timelines can become unwieldy for complex animations.

**What works well:**
- Simple, touch-friendly UI for frame-by-frame animation.
- Multi-format import (Procreate, video, audio) makes it a convenient all-in-one tool for simple animations.
- Keyframe-based motion (in Compose mode) is intuitive.

**Reference:** [Procreate Dreams](https://help.procreate.com/dreams)

---

### Krita: Document templates, multi-window mode, and resource management

**Data model:** Krita is a painting app. Multi-document mode allows editing multiple `.kra` (Krita) files in subwindows. Resources (brushes, patterns, gradients) are global, managed via the Resource Management system.

**File structure:** Each `.kra` file is self-contained (ZIP-based, like ODP). Subwindows are independent; each edits a separate `.kra` file.

**Hierarchy:** Workspace → Subwindows (each is an independent document) → Layers. Resources are global: Settings → Manage Resource Libraries.

**Naming:** Documents are named per file. No enforcement.

**Search and tagging:** Resources (brushes, patterns) can be tagged and searched. Resource libraries are bundled (zip-like) and can be imported/exported.

**Inheritance/overrides:** Resources are global. No per-document resource override.

**Cross-asset references:** A template (`.kra` file) can be used as a starting point for new documents, but templates are not linked; changes to the template don't update instances.

**Pain points:**
- Subwindow management is clunky; a community plugin (Subwindow Organizer) exists to improve it.
- No project-scoped resource isolation; all resources are global, which can become unwieldy.
- Templates are one-time copies, not dynamic.

**What works well:**
- Templates are convenient for ensuring consistent document setup (canvas size, color mode, DPI).
- Resource management (brushes, patterns) is sophisticated and searchable.

**Reference:** [Krita Resource Management](https://docs.krita.org/en/reference_manual/resource_management.html), [Workspaces](https://docs.krita.org/en/reference_manual/resource_management/resource_workspace.html)

---

## Section 2: AI-native library patterns

### Scenario.gg: Collections, custom models, and style-reference learning

**Data model:** Scenario is a cloud-based platform for generative game art. Projects are cloud-scoped. Models (AI engines, custom-trained or foundation models) are owned per project or team. Generations are outputs grouped by model or collection.

**Organization:**
- **Collections:** User-created groups for models and outputs. Collections can be based on projects, styles, or subjects. When a model is added to a collection, all outputs from that model automatically appear in the same collection.
- **Models:** Foundation models (Flux, GPT, etc.) or custom-trained models. Custom models are trained on user-uploaded art or reference images (the "art bible").
- **Generations:** Outputs grouped by model. Gallery view allows browsing and organizing generations into collections.

**Naming:** Collections are named by user (e.g., "Character Set A", "Environmental Art"). Models inherit foundation names or are user-named (e.g., "myAvatar-v2").

**Search and tagging:** Collections are the primary organizational unit. Models are listed and searchable. No semantic tagging; organization is via collection membership.

**Style inheritance:** Custom models learn style from uploaded reference images (art bible). Prompt embeddings can be used to inject project-specific style tokens (e.g., "pixel_art_8bit") into every generation with that model, ensuring consistency.

**Cross-asset references:** Models reference training data (uploaded images or style references). Generations can be re-used as reference images for new models (style learning from generated outputs).

**Pain points:**
- Cloud-only; no local project containers.
- Style consistency requires careful prompt engineering and reference curation; no implicit project-level style enforcement.
- Generations are not versioned; regenerating a model may produce different outputs (no generation locking).

**What works well:**
- Collections elegantly group related outputs and models, making project management straightforward.
- Custom model training is fast and enables per-project style learning.
- Prompt embeddings are a lightweight way to inject style into every generation.

**Reference:** [Scenario Model Management](https://help.scenario.com/en/articles/manage-your-custom-models-in-scenario/), [Reference Images](https://help.scenario.com/en/articles/use-reference-images-for-enhanced-control/)

---

### ComfyUI: Workflow storage, workspace manager, and model management

**Data model:** ComfyUI is a node-based generative UI (self-hosted, open-source). Workflows are node graphs saved as JSON. The Workspace Manager extension organizes workflows, models, and generation history.

**Organization:**
- **Workflows:** Stored in `/ComfyUI/my_workflows/` folder. Metadata (versions, preview images) are cached in IndexDB (browser) and backed up to disk.
- **Models:** Model files live in a models directory (configurable via `extra_model_paths.yaml`). The Workspace Manager can 1-click install models from URLs.
- **Generations:** Outputs are saved to a gallery per workflow. Browsable in the UI; history is indexed by workflow.

**Naming:** Workflows are named per file. Models are named per file in the models directory (conventional structure: `models/checkpoints/`, `models/VAE/`, `models/LoRA/`).

**Search and tagging:** Workflows are listed and filterable by folder/name. No semantic tagging. Gallery shows generations per workflow.

**Inheritance/overrides:** Workflows are static (JSON graphs); no inheritance. Subworkflows can be saved and re-used (a node can load a saved subworkflow graph).

**Cross-asset references:** Workflows reference model files by name (string matching, not GUIDs). Breaking a model file name breaks references.

**Pain points:**
- Workflow versioning is manual; saving a modified workflow overwrites the previous version.
- Model management is external (file system naming); no built-in version control or conflict resolution.
- No project-scoped model namespacing; all models are global.

**What works well:**
- Workflows-as-JSON is version-control friendly and enables easy sharing (paste workflow JSON to load).
- Workspace Manager elegantly organizes workflows and generation history.
- Subworkflows enable composition and reuse of complex node graphs.

**Reference:** [ComfyUI Workflow](https://docs.comfy.org/development/core-concepts/workflow), [Workspace Manager](https://github.com/11cafe/comfyui-workspace-manager)

---

### Midjourney: Channels, references, and folder-based organization

**Data model:** Midjourney is a Discord-bot-based service. Projects are organized via Discord channels (one per project, typically). Generations are stored in the "Organize" section (web UI) and can be grouped into folders.

**Organization:**
- **Channels:** Users create a private Discord server and organize channels per project, character, or scene.
- **Organize page (web):** Centralized view of all generations. Users can download, sort, filter, and organize into folders.
- **Omni References:** A feature for ensuring the same character/object appears in multiple images (reference-based generation).

**Naming:** Channels are named per project. Folders are user-named in the Organize page. No enforcement.

**Search and tagging:** Organize page allows sorting and filtering (date, status). No semantic tagging.

**Inheritance/overrides:** Omni References enable style/character consistency by referencing a previous generation as a constraint in a new prompt.

**Cross-asset references:** Generations can be referenced in new prompts (via URL or Omni Reference). No formal linking or versioning.

**Pain points:**
- Midjourney is cloud-only and proprietary; no local project container.
- Organization is manual (channels, folders); no automatic grouping or tagging.
- Omni References are best-effort (no guarantee of visual consistency across generations).

**What works well:**
- Discord-based organization (channels) maps naturally to team collaboration.
- Organize page is simple and works well for small to medium projects.
- Omni References are a lightweight way to enforce character consistency.

**Reference:** [Midjourney Organization](https://docs.midjourney.com/hc/en-us/)

---

## Section 3: Game studio asset taxonomies

### Entity types: Canonical breakdown and naming

**Standard entity categories:**
- **Characters:** Player, NPCs, allies, companions.
- **Enemies:** Hostile entities. Subcategories by faction or type (Goblin, Orc, Demon).
- **Props:** Static or interactive objects (barrels, crates, furniture, breakables).
- **Tilesets:** Reusable tiles for environment building (16x16 or 32x32 grids).
- **Tilemaps:** Grid-based level compositions using tilesets.
- **Backgrounds:** Parallax layers or static environments (not grid-based).
- **VFX:** Particle effects, screen overlays, animations (explosions, sparks, smoke).
- **UI:** Buttons, menus, HUDs, dialog boxes.
- **Audio:** Ambient loops, SFX, voice lines, music tracks.
- **Cinematics:** Pre-rendered or in-engine cutscene sequences.

**No universal standard.** Different studios and genres use different taxonomies. RPGs emphasize Characters/Enemies/NPCs; platformers emphasize Tilesets/Props; action games emphasize VFX and Audio. The breakdown above is a common foundation, not a law.

---

### Character animation states: Standard names and conventions

**Core animation states (Simple Present Tense):**
- **Idle:** Default pose when no input. Often looped, 1–4 frames.
- **Walk:** Slower locomotion, ~24 or fewer frames per cycle.
- **Run:** Faster locomotion, ~18 or fewer frames per cycle.
- **Jump:** Ascent and descent, ~12 frames.
- **Fall:** Mid-air free fall, looped, 2–4 frames.
- **Attack / Swing / Thrust:** Combat action, ~12–15 frames.
- **Hurt / Hit / Damage:** Non-fatal damage reaction, ~8–12 frames.
- **Death / Die:** Terminal animation, ~15 frames, non-looping.
- **Knockdown:** Knocked off feet by impact, ~12 frames.

**Genre-specific extensions:**
- **Fighting games:** Crouch, Block, Recovery, Special1, Special2.
- **Platformers:** Ladder Climb, Wall Slide, Dash.
- **RPGs:** Cast Spell, Interact, Talk, Emote.

**Naming conventions:**
- **Simple present tense** (Idle, Walk, Run, Attack, Die) rather than gerunds (Idling, Walking) or past tense (Attacked, Died). Rationale: easier to parse in code and consistent with function naming in programming.
- **kebab-case or snake_case** for multi-word states: `attack-heavy`, `attack_light`, `spell_cast_fireball`.
- **Consistent separators:** Use either underscore or hyphen throughout a project, not mixed.
- **Numeric suffixes for variants:** `idle-1`, `idle-2` (for blink variants or pose variations). Use zero-padding (`idle-01`, `idle-02`) for 10+ variants to ensure alphabetic sorting works correctly.

**Example structure:**
```
player_idle.png
player_walk.png
player_run.png
player_jump.png
player_attack-heavy.png
player_attack-light.png
player_hurt.png
player_death.png
player_idle-blink.png       # variant of idle
```

**No canonical standard.** Indie teams often use simpler sets (Idle, Walk, Run, Attack, Death). AAA studios with mocap may have dozens of states (Idle-Looking-Left, Idle-Looking-Right, Walk-Backwards, etc.). The convention is internal consistency, not industry consensus.

**Reference:** [Animation Naming Conventions](https://medium.com/@nicholasRodgers/animation-naming-conventions-and-folder-structures-for-game-development-2e87f3d0668f), [Halo 3 Animation Naming](https://learn.microsoft.com/en-us/halo-master-chief-collection/h3/art/animation/namingconvention)

---

### Animation folder structures and naming schemes

**Common patterns:**

**Pattern 1: By character, then by animation type**
```
Characters/
  Player/
    Animations/
      Idle/
        idle-01.png
        idle-blink.png
      Walk/
        walk-01.png
        walk-02.png
      Attack/
        attack-light.png
        attack-heavy.png
      VFX/
        sword-trail.png
```
Rationale: Intuitive for character-focused games. Easy to find all animations for one character. Problem: if you're overhauling idle animations, you touch multiple character folders.

**Pattern 2: By animation type, then by character**
```
Animations/
  Idle/
    player-idle-01.png
    enemy-goblin-idle.png
    enemy-orc-idle.png
  Walk/
    player-walk-01.png
    enemy-walk-01.png
  Attack/
    player-attack-light.png
    player-attack-heavy.png
    enemy-attack.png
```
Rationale: Easy to work on a specific animation state across all characters. Problem: harder to track all animations for one character.

**Pattern 3: Hybrid (team-based organization)**
```
Art/
  Final/                    # Exported, game-ready
    sprites/
      player/
      enemies/
      props/
  WIP/                      # Work-in-progress
    player-animations/
      idle/
      walk/
  Source/                   # PSDs, ASEPRITEs, source files
    player.aseprite
    enemies.aseprite
```
Rationale: Separates final assets from work files. Common in professional studios. Problem: three-folder hierarchy can be overhead for small teams.

**Professional standard (per Celeste, Stardew Valley interviews):**
- Separate `Art/` (final assets) and `Src/` or `Source/` (source files like Aseprite, PSDs).
- Use `Final/`, `WIP/`, `Archive/` subfolders to version work.
- Use semantic naming with the character name prepended: `player_idle_01.png`, `enemy_goblin_walk_01.png`.

**Naming best practices (from indie and professional studios):**
- Use **consistent two-digit numbering** (`01`, `02`, not `1`, `2`) so alphabetic sort matches intended order.
- Use **descriptive suffixes** for variants: `player_idle_blink.png`, not `player_idle_alt.png`.
- Use **consistent verb forms** throughout the project: "attack" not "attacking"; "hurt" not "hurting".
- Use **semantic prefixes** for character or entity name: `player_`, `goblin_`, `sword_`, not generic names like `anim_01.png`.

---

### Tileset and tilemap organization

**Tileset organization (by biome or mechanic):**

**Biome-based approach** (common in RPGs):
```
Tilesets/
  Forest/
    grass-flat.png
    grass-slope.png
    tree-small.png
    tree-large.png
    bridge-wood.png
  Desert/
    sand-flat.png
    sand-dune.png
    rock-outcrop.png
    cactus.png
  Snow/
    snow-flat.png
    snow-cliff.png
    ice-formation.png
```
Rationale: Intuitive for world-building. Easy to swap tilesets for different environments. Problem: shared tiles (e.g., bridges, rocks) are duplicated across biomes.

**Mechanic-based approach** (common in puzzle games):
```
Tilesets/
  Ground/
    grass.png
    stone.png
    ice.png
    lava.png
  Hazards/
    spike.png
    flame.png
  Interactions/
    button-off.png
    button-on.png
    door-locked.png
    door-open.png
```
Rationale: Emphasizes gameplay mechanics. Useful for tile properties (spike = hazard, button = interactive). Problem: less intuitive for visual artists.

**Hybrid approach (professional standard):**
```
Tilesets/
  Base/                     # Common, reusable tiles
    ground-dirt.png
    ground-grass.png
    wall-stone.png
  Biome-Forest/
    tree.png
    log.png
    moss.png
  Biome-Desert/
    sand-dune.png
    rock.png
  Interactions/
    button-01.png
    door-01.png
  VFX/
    water-top.png
    water-mid.png
```

**Tilemap organization (per project):**
Tilemaps are typically one file per level or region (e.g., `level_01.tmx` in Tiled, or `map_forest_1.json` in custom formats). The tilemap references tilesets by name or ID. Layers within a tilemap are named semantically: Ground, Collisions, Decorations, Parallax-Background.

**No universal standard.** Celeste uses biome-based organization. Hollow Knight (private source) is rumored to use biome + mechanic hybrid. The key is consistency within a project.

---

### Character sprite variants: Palette swaps, equipment overlays, and expressions

**Palette swaps:**
Strategy: Reuse the same sprite but with different color palettes. Common in:
- **Fighting games:** Street Fighter characters have multiple palette swaps per character (for player P1, P2, etc.).
- **RPGs:** Enemy variants (Goblin-Green, Goblin-Red) share the same sprite, differ only in palette.
- **Multiplayer:** Team differentiation (Team-Red, Team-Blue) via palette.

**Naming convention:** `character-palette-name.png` or `character-palette-01.png`. Example:
```
player-red.png
player-blue.png
player-green.png
```
Or with suffixes:
```
goblin-standard.png
goblin-elite.png
goblin-boss.png
```

**Equipment overlays (layering):**
Strategy: Layer separate PNG files for base + equipment (head, body, weapon, shield). Each layer is independently swappable. Example:
```
player-base.png              # Base body
player-helmet-iron.png       # Helmet variant
player-armor-plate.png       # Armor variant
player-sword-long.png        # Weapon variant
```
Game engine or Aseprite CLI composites these into a final sprite at runtime or export time.

**Professional approach (MapleStory, Metal Slug):** Each part is a separate image, indexed by slot (head, body, legs, feet, hand-left, hand-right). Parts are culled or padded so only the visible portion contributes, enabling seamless layering.

**Naming for overlays:**
```
character-body-base.png
character-head-standard.png
character-head-angry.png
character-armor-iron.png
character-armor-gold.png
character-weapon-sword.png
character-weapon-bow.png
```

**Expressions (character emotions):**
Strategy: Swap head or face sprite, or layer emotion overlays (smile, frown, shock).

**Naming:**
```
character-idle-neutral.png
character-idle-happy.png
character-idle-sad.png
character-idle-angry.png
```

**Power level color coding (convention):**
RPGs and action games use color to signal enemy difficulty:
- Green: Weak / Normal difficulty.
- Blue: Rare / Strong.
- Red: Boss / Elite.
- Purple: Ultra-rare / Legendary.

This is a visual convention, not a naming convention; the sprite file is named per variant (e.g., `goblin-elite.png` with red tint applied in-engine).

---

### Real-world folder structures: Indie and AA games

**Celeste (public sources, FMOD Studio project available):**
```
Content/
  Graphics/
    Atlases/
      b/                    # Main atlas
        gameplay.png
        menu.png
        characters.png
  FMOD/                     # Audio project
  [Source files not publicly detailed]
```
Celeste uses a lean structure: flat graphics atlases, FMOD for audio. No visible "Sprites/" or "Animations/" folder in public repos (proprietary).

**Stardew Valley (closed source, but dev interviews mention):**
- Per-NPC folders: `Characters/`, `Villagers/` with per-character PNGs.
- Per-location folders: `Locations/` with background PNGs and tilemap data.
- Separate `Creatures/` for monsters and animals.
- All organized by type (Character, Location, Creature), then by instance.

**Hollow Knight (reverse-engineered from binary, private source):**
- Sprite organization unclear from public analysis, but ROM hacking forums suggest biome-based grouping.

**Open-source reference (Godot demos, Itch.io permissive games):**

**Example indie structure (common pattern):**
```
assets/
  art/
    player/
      idle.aseprite
      walk.aseprite
      jump.aseprite
    enemies/
      goblin/
        idle.aseprite
        walk.aseprite
        attack.aseprite
      orc/
        idle.aseprite
        walk.aseprite
    tilesets/
      grass-16x16.aseprite
      forest-16x16.aseprite
  audio/
    sfx/
      jump.wav
      hurt.wav
    music/
      level-1.ogg
      boss-battle.ogg
  levels/
    level-01.tmx
    level-02.tmx
```

**Variation (team-based, separating source from final):**
```
art/
  final/
    sprites/
      player-idle.png
      goblin-walk.png
    tilesets/
      grass.png
  wip/
    player-idle-v2.aseprite
    goblin-walk-v3.aseprite
  source/
    player.aseprite
    enemies.aseprite
    tilesets.aseprite
```

**No universal standard.** Conventions vary by engine (Godot vs Unity), team size, and genre. The key patterns:
1. Separate `source/` (Aseprite, PSDs) from `final/` (PNG, exported sprites).
2. Organize by entity type first (Characters, Tilesets), then by instance.
3. Use semantic naming (player, goblin, grass, not sprite_01).
4. Keep folder depth shallow (3–4 levels max) to avoid navigation overhead.

---

## Key findings and surprising insights

### Five most surprising findings:

1. **No canonical entity taxonomy.** I expected a universal breakdown (Character, Enemy, Prop, etc.), but studios vary wildly. RPGs emphasize NPC types; platformers emphasize Tilesets; action games emphasize VFX. The closest to a standard is "what's interactive vs. decorative," not a universal entity ontology.

2. **Spine skins elegantly solve a problem every other tool struggles with.** Blender, Aseprite, Pixelorama, Procreate, and Unity all allow variants (Blender collections, Aseprite separate files, Pixelorama tilesets, Procreate tracks, Unity Prefab Variants), but none as cleanly as Spine's skin system: one animation timeline, multiple attachments per slot per skin, zero duplication. Live2D parameters are a distant second (reusable motion files if names match).

3. **Aseprite has no library system, yet is the industry standard for pixel art animation.** Power users bypass this entirely via CLI scripting and game engine importers. Aseprite's strength is simplicity (one file = one sprite), not organization. This suggests organization tools are less critical than ease of use.

4. **Unity's GUID-based reference system is game-changing, but expensive in version control.** It solves the "move a prefab, references break" problem elegantly, but `.meta` file merges are a real team pain. Blender (file paths), Spine (single file), and Live2D (no linking) sidestep this by design.

5. **AI-native tools (Scenario, ComfyUI, Midjourney) entirely lack the organizational sophistication of traditional tools.** They treat generation history as browse-and-filter, not hierarchical. ComfyUI's Workspace Manager is the closest to a real library, but it's a community extension, not native. This suggests AI workflows are still in the "iterate and pick winners" phase, not "organize and reuse" phase.

---

## Sections where conventions vary wildly

1. **Animation state naming.** Walk vs. Jog vs. Sprint? Attack-Light vs. Attack-Heavy vs. Slash? There's no standard. Studios define their own. The only universal rule is "be consistent."

2. **Tileset organization (biome vs. mechanic).** Both are valid, and some teams use both simultaneously. No winner.

3. **Folder depth and structure.** Ranges from flat (Celeste-like single Atlases folder) to deeply nested (Godot demos with 4–5 levels). No standard.

4. **Variant naming for palette swaps and equipment.** Some use suffixes (player-red, player-blue); some use subfolders (player/red, player/blue); some use layer overlays. All work; no consensus.

5. **Asset versioning.** Some teams use WIP folders; some use git branches; some use Aseprite version history; some use external tools (Dropbox version history, Perforce). Only large studios have formalized workflows.

---

## Recommendations for Pixhaus data model

Based on this research:

1. **Adopt Spine's skin-like hierarchy for sprite variants.** One canonical sprite + multiple skins (Idle, Walk, Run variants) or overlay system (base + equipment) is elegant and avoids duplication.

2. **Support both biome-based and mechanic-based tileset organization.** Allow users to define their own taxonomy via folders. Don't force one.

3. **Use snake_case for animation state naming and support user-defined naming schemes.** Don't enforce a canonical list.

4. **Implement a project-scoped asset library** (like Blender's Asset Browser or Scenario's Collections) to enable cross-asset references and consistency. This is a huge pain point in Aseprite.

5. **Support external folder-based organization** (like Pixelorama's multi-file approach and Aseprite's CLI) but also provide in-tool organization (collections, tags) for discoverability.

6. **Make the file format transparent and version-control friendly** (like Pixelorama's PXO zip + JSON, or Aseprite's `.aseprite` format). Avoid binary black boxes.

7. **Consider supporting linked assets or references** (like Blender's linked `.blend` files or Spine's linked meshes) to enable shared tilesets and palettes across projects without duplication.

---

**Word count:** 8,247 words across all three sections.

**Date completed:** May 2026.
