# Pixhaus Project & Save File Format Architecture

## Purpose

This document defines a recommended save-file and project-storage architecture for **Pixhaus**, a native Rust + egui sprite creation and sprite animation application.

Pixhaus is expected to support:

- Multiple sprites per project.
- Multiple art styles, not only pixel art.
- Dedicated pixel-art tooling where required.
- Manual editing and AI-assisted generation.
- Animation timelines, clips, frames, layers, cels, masks, palettes, references, generated assets, recipes, and future asset types.
- Large projects that may contain many heavy image assets.
- Efficient partial loading without requiring a multi-gigabyte monolithic file to be loaded into memory.
- Cross-platform operation on Windows, macOS, and Linux.
- Long-term compatibility and migration.
- Internal extensibility for future workspaces such as particles, UI sprites, VFX, tilesets, rigging, or game-engine export systems.

The core recommendation is:

> Use a **folder-based project format as the primary working format**, with binary asset chunks and lightweight metadata indexes. Optionally support a packaged `.pixhaus` bundle for sharing, archiving, and publishing.

Pixhaus should not use a single giant binary file as the everyday working format. It should use a project directory that supports lazy loading, streaming, partial saves, crash recovery, asset-level versioning, thumbnails, and future extension data.

---

# 1. Core Design Philosophy

## 1.1 Project folder first

Pixhaus projects should be stored as folders during active work.

Example:

```text
MyGame.pixhaus/
  project.pxmeta
  index.pxidx
  assets/
  sprites/
  palettes/
  recipes/
  generated/
  cache/
  history/
  extensions/
```

This gives Pixhaus several important advantages:

- The app can open the project quickly by loading only metadata and indexes.
- Individual sprites and assets can be loaded on demand.
- Large binary surfaces can stay on disk until needed.
- Saves can be incremental and asset-local.
- Corruption is less catastrophic than in one huge file.
- Source control and backup tools can work better.
- Future asset types can live alongside core assets.
- Heavy generated assets can be managed without bloating the whole project manifest.

## 1.2 Packaged file second

Pixhaus can also support a single-file package for portability:

```text
my_project.pixhaus
```

This should be treated as a **bundle/archive/export format**, not necessarily the primary editing format.

The packaged file can be useful for:

- Sharing a project with another artist.
- Uploading/downloading from cloud storage.
- Archiving a milestone.
- Sending a bug report reproduction project.
- Publishing sample projects.

The packaged format should be equivalent to zipping the project directory with a known structure, manifest, and integrity checks.

## 1.3 Binary where it matters, readable where it helps

Not everything should be custom binary.

Use binary storage for:

- Large pixel surfaces.
- Layer/cel image chunks.
- Thumbnail atlases.
- Cached composites.
- Generated images.
- Large masks.
- Tilemaps.
- Future particle simulation caches.

Use readable or semi-readable structured metadata for:

- Project manifest.
- Asset registry.
- Sprite document metadata.
- Layer lists.
- Frame lists.
- Animation clips.
- Recipes.
- Styles.
- Export presets.
- Plugin/module extension metadata.

A good rule:

> The more often humans need to inspect, diff, repair, or migrate it, the more readable it should be. The larger and more performance-sensitive it is, the more binary it should be.

---

# 2. Main Recommendation

Pixhaus should define three related formats:

## 2.1 `.pixhaus/` project folder

The primary editable format.

```text
MyProject.pixhaus/
```

Despite looking like a file extension, this is a directory bundle. On macOS it could eventually behave like a package. On Windows/Linux it remains a normal folder.

## 2.2 `.pxdoc` document files

Binary or hybrid document files for individual heavy assets, such as sprites, tilesets, animations, or future VFX documents.

Examples:

```text
sprites/hero/sprite.pxdoc
sprites/enemy_slime/sprite.pxdoc
ui/buttons/ui_sprite.pxdoc
particles/fire_burst/particle.pxdoc
```

These are asset-local documents.

## 2.3 `.pixhaus` package file

A portable single-file package for sharing or archiving.

```text
MyProject.pixhaus
```

If using the same extension for folders and packages causes confusion, use:

```text
MyProject.pixhaus/     # folder project
MyProject.pxpack       # packaged archive
```

My recommendation:

- Use `.pixhaus/` for project folders.
- Use `.pxpack` for single-file portable bundles.
- Use `.pxdoc` for internal asset documents.

This avoids ambiguity and makes the architecture clearer.

---

# 3. Why Not One Giant Binary File?

A single giant binary file may seem attractive because it gives full control, but it creates problems for Pixhaus.

## 3.1 Slow open times

Large projects may contain many sprites, generated assets, references, timelines, thumbnails, and cached outputs.

The user should be able to open the project quickly and see the asset browser without loading every pixel of every sprite into memory.

## 3.2 Memory pressure

A multi-gigabyte project cannot be fully loaded on lower-end machines.

Pixhaus should only load:

- Project manifest.
- Asset index.
- Thumbnails.
- Currently open sprite documents.
- Visible frame/layer/cel surfaces.
- Nearby timeline frames if needed.

## 3.3 Risk of corruption

If one large file becomes corrupted, the entire project may be affected.

With a folder-based format, damage can often be isolated to one asset or one binary chunk.

## 3.4 Poor incremental saves

A huge binary file encourages full rewrites or complex in-place mutation.

A project folder allows Pixhaus to save only the changed asset or chunk.

## 3.5 Harder extensibility

Future workspaces may introduce new document types.

A folder format allows new internal modules to create new asset folders without redesigning the whole global binary file.

---

# 4. Project Folder Layout

Recommended top-level layout:

```text
MyProject.pixhaus/
  project.pxmeta
  index.pxidx
  lock.pxlock
  previews.pxthumbs

  assets/
    assets.pxidx
    references/
    external/
    imported/

  sprites/
    hero/
      asset.pxasset
      sprite.pxdoc
      thumbs.pxthumbs
      cache/
    slime/
      asset.pxasset
      sprite.pxdoc
      thumbs.pxthumbs
      cache/

  palettes/
    palettes.pxmeta
    dusk_forest.pxpal
    ui_neon.pxpal

  recipes/
    recipes.pxmeta
    templates/
    structures/
    styles/
    packs/

  generated/
    gen_01HXYZ/
      asset.pxasset
      result.pximg
      generation.pxmeta
      source_context.pxmeta

  exports/
    presets.pxmeta
    last_outputs.pxmeta

  cache/
    composites/
    thumbnails/
    gpu/
    ai/

  history/
    journal.pxlog
    autosaves/
    recovery/

  extensions/
    com.pixhaus.particles/
    com.pixhaus.ui_sprites/
```

Not every project needs every folder. Pixhaus can create folders lazily.

---

# 5. File Roles

## 5.1 `project.pxmeta`

The project manifest.

Contains:

- Project ID.
- Project name.
- Pixhaus project format version.
- Created/modified timestamps.
- Author metadata if desired.
- Active workspace preferences.
- Project-level settings.
- Project color/profile assumptions.
- Asset registry pointer.
- Feature flags used by this project.
- Required Pixhaus version range.
- Extension data registry.

This file should be small and loaded immediately.

## 5.2 `index.pxidx`

A fast global asset index.

Contains enough information to populate project browser views without loading every asset document.

Contains:

- Asset IDs.
- Asset type.
- Asset display name.
- Asset path.
- Thumbnail pointer.
- Tags.
- Last modified time.
- Dependency list.
- Workspace/module owner.
- Whether asset is missing/corrupt/stale.

This can be binary for speed or structured data if simplicity is preferred. The index is rebuildable from asset manifests, so it should never be the only source of truth.

## 5.3 `asset.pxasset`

Asset-local manifest.

Each major asset should have one.

Contains:

- Asset ID.
- Asset type.
- Name.
- Path to primary document.
- Thumbnail info.
- Tags.
- Dependencies.
- Creation source.
- AI provenance if relevant.
- Module-specific metadata.

Examples of asset types:

```text
sprite
animation_clip
tileset
palette
reference_image
generated_asset
particle_system
ui_sprite
prompt_recipe
export_preset
```

## 5.4 `.pxdoc`

Asset-local document format.

This is where heavy structured document data lives.

Examples:

- Sprite document.
- Tileset document.
- UI sprite document.
- Particle VFX document.
- Future rigging document.

`.pxdoc` should support lazy loading internally.

## 5.5 `.pximg`

Pixhaus binary image/surface format.

Used for:

- Generated outputs.
- Raster cels.
- Masks.
- Cached image data.
- Possibly embedded surfaces inside `.pxdoc`.

## 5.6 `.pxpal`

Palette format.

Contains:

- Palette ID.
- Name.
- Colors.
- Optional ramps.
- Optional harmony groups.
- Pixel-art mode metadata.
- Locked colors.
- AI palette behavior.

## 5.7 `.pxthumbs`

Thumbnail cache or thumbnail atlas.

Loaded quickly by asset browser.

Can be regenerated if missing.

## 5.8 `.pxlog`

Append-only project journal.

Useful for:

- Crash recovery.
- Autosave recovery.
- Debugging save operations.
- Optional command history persistence.

This is not necessarily the same as the user-facing undo stack.

---

# 6. Binary Format Strategy

Pixhaus should not invent one giant all-purpose binary blob.

Instead, define a small set of chunked binary formats.

Recommended binary properties:

- Magic number.
- Format version.
- Endianness marker.
- Header length.
- Table of contents.
- Chunk directory.
- Checksums per chunk.
- Optional compression per chunk.
- Optional external blob references.
- Forward-compatible unknown chunks.

## 6.1 Chunked file structure

Conceptual layout:

```text
Header
Chunk Directory
Chunk 1: Document metadata
Chunk 2: Layer table
Chunk 3: Frame table
Chunk 4: Cel table
Chunk 5: Surface blob A
Chunk 6: Surface blob B
Chunk 7: Animation clips
Chunk 8: Extension data
Footer / checksum
```

Why chunked?

- Load metadata without loading surfaces.
- Load a specific cel without loading all frames.
- Skip unknown chunks from future versions.
- Validate and repair individual chunks.
- Compress large chunks independently.
- Support partial rewriting later if desired.

## 6.2 Chunk IDs

Chunk IDs should be stable symbolic identifiers internally, encoded compactly on disk.

Example conceptual chunk types:

```text
PXDOC_HEADER
PXDOC_METADATA
PXDOC_ASSET_REF
PXDOC_LAYERS
PXDOC_FRAMES
PXDOC_CELS
PXDOC_SURFACES
PXDOC_ANIMATION_CLIPS
PXDOC_PALETTES
PXDOC_SELECTIONS
PXDOC_GUIDES
PXDOC_COLOR_PROFILE
PXDOC_AI_PROVENANCE
PXDOC_EXTENSION_DATA
PXDOC_THUMBNAILS
PXDOC_DEPENDENCY_TABLE
```

## 6.3 Unknown chunk policy

Unknown chunks should be preserved when possible.

This is critical for future internal modules.

Example:

- Pixhaus v1 opens a project containing v2 particle metadata.
- v1 does not understand it.
- v1 should preserve the unknown chunk on save if the related asset is not modified destructively.

This allows backward/forward compatibility in realistic workflows.

---

# 7. Metadata Format Recommendation

For metadata, choose a format based on these priorities:

- Versioning and migration.
- Rust support.
- Reasonable performance.
- Human inspectability where useful.
- Stable schema evolution.

Options:

## 7.1 JSON

Pros:

- Human-readable.
- Easy to debug.
- Easy to generate from tools and agents.
- Great during early development.

Cons:

- Larger files.
- Slower than binary formats.
- Less strict schema evolution.

Good for:

- Early manifests.
- Recipes.
- Export presets.
- Debuggable metadata.

## 7.2 MessagePack / CBOR

Pros:

- Compact.
- Fast enough.
- Supports structured data.
- Easy Rust serialization.

Cons:

- Not human-readable.
- Requires tooling to inspect.

Good for:

- Asset indexes.
- Document metadata.
- Project manifests once stable.

## 7.3 FlatBuffers / Cap'n Proto

Pros:

- Schema evolution.
- Efficient random access.
- Potential zero-copy reads.

Cons:

- More complex.
- Less flexible early.
- Harder to evolve casually while product is changing.

Good for:

- Later-stage stable binary document metadata.
- Large indexes.

## 7.4 SQLite

Pros:

- Mature storage engine.
- Great indexing/querying.
- Handles partial reads.
- Good for asset registry and metadata.
- Can store blobs, though that should be used carefully.

Cons:

- Adds database semantics to project files.
- Concurrent access and migrations require discipline.
- Less transparent than folders of metadata files.

Good for:

- Large project asset indexes.
- Search metadata.
- AI generation history.
- Cache database.

## 7.5 Recommendation

For Pixhaus, I recommend this staged approach:

### Early development

Use:

```text
JSON/TOML/RON-like readable metadata + binary surface files
```

Why:

- Agents can inspect and modify it easily.
- Debugging is simple.
- The schema will change often.
- You avoid premature binary complexity.

### Production format

Move performance-sensitive metadata to:

```text
MessagePack/CBOR or a custom chunked binary encoding
```

Keep recipes/export presets optionally readable.

### Large indexes/caches

Consider:

```text
SQLite or rebuildable binary indexes
```

Only if asset search, tagging, and browsing become large enough to justify it.

---

# 8. Sprite Document Model

A sprite document must support multiple art styles.

It should not assume pixel art only.

Pixhaus should support:

- Pixel art sprites.
- Painted sprites.
- HD sprites.
- Vector-like raster workflows if needed.
- AI-generated sprites.
- Hybrid manually edited AI output.
- Animation frames.
- Layers.
- Masks.
- Palettes where relevant.
- Style metadata.
- Per-document editing mode.

## 8.1 Sprite document metadata

A sprite document contains:

```text
sprite_id
name
canvas_size
art_mode
color_mode
layers
frames
cels
animation_clips
palettes
reference_images
metadata
extension_data
```

## 8.2 Art mode

Art mode should be explicit.

Examples:

```text
PixelArt
Raster
HDIllustration
Painted
VectorLikeRaster
UI
Tileset
```

Art mode affects:

- Tool defaults.
- Scaling behavior.
- Grid behavior.
- Palette behavior.
- Export validation.
- AI prompt defaults.
- Brush engine options.

Pixel art is a mode, not the whole app.

## 8.3 Pixel art mode

Pixel art mode should enable:

- Indexed palette options.
- Strict palette mode.
- No-antialias drawing.
- Pixel-perfect transforms.
- Grid and major grid.
- Dithering tools.
- Ramp/harmony tools.
- Tile seam tools.
- Palette reduction.
- Nearest-neighbor scaling.

## 8.4 Raster/HD mode

Raster or HD mode should allow:

- Larger canvases.
- Anti-aliased brushes.
- Soft masks.
- Larger surfaces.
- Brush textures.
- Alpha gradients.
- Higher bit-depth later if needed.
- AI-assisted cleanup and style transfer.

This means the file format must not hardcode everything as indexed pixel art.

---

# 9. Surface Storage

Surface storage is the most important heavy-data problem.

A surface may represent:

- A cel.
- A layer mask.
- A generated result.
- A reference image.
- A cached composite.
- A thumbnail source.
- A brush texture.

## 9.1 Surface types

Recommended surface modes:

```text
RGBA8
RGBA16F, future optional
Indexed8
Alpha8
Mask8
NormalMap, future optional
TileIndexMap, future optional
```

## 9.2 Pixel-art indexed surfaces

For pixel art, support indexed surfaces.

Benefits:

- Small size.
- Palette discipline.
- Fast palette swaps.
- Easier palette tools.
- Better retro/game workflows.

Indexed surfaces reference a palette ID.

## 9.3 Raster surfaces

For general sprite art, use RGBA surfaces.

Benefits:

- Supports multiple art styles.
- Works naturally with AI outputs.
- Allows antialiasing and soft edges.
- Easier interoperability with PNG and game engines.

## 9.4 Sparse cels

Do not require every frame/layer to have a full surface.

Use sparse cels.

A sprite with 20 layers and 100 frames should not require 2,000 full surfaces if most are empty.

Each cel can be:

```text
missing
empty
linked to another cel
own surface
procedural/generated reference
```

## 9.5 Surface chunking

Large surfaces can be chunked by tiles.

This matters for:

- Large HD sprites.
- Large background assets.
- Large reference images.
- Future painting workflows.

Example tile chunking:

```text
surface 2048x2048
chunks 256x256
only dirty chunks are saved
only visible chunks are loaded
```

For small sprites, chunking may be unnecessary overhead. Pixhaus can use whole-surface storage for small documents and chunked storage for large ones.

---

# 10. Compression Strategy

Compression should be per chunk, not whole project.

Different data compresses differently.

## 10.1 Recommended compression modes

```text
none
lz4/zstd-fast for interactive saves
zstd-high for packaged archives
png-compatible compression for external image export only
```

## 10.2 Interactive save

For active project editing:

- Prefer fast compression.
- Avoid blocking the UI.
- Save changed chunks only.
- Use background save workers.

## 10.3 Archive/package export

For `.pxpack`:

- Use stronger compression.
- Include checksums.
- Include all dependencies or declare external references.
- Optimize for portability rather than edit speed.

## 10.4 Pixel-art optimization

Indexed pixel art can compress extremely well.

Potential optimizations:

- RLE for simple surfaces.
- Dirty rectangle storage.
- Palette index compression.
- Frame delta compression.

Be careful: clever compression can make editing and recovery harder. Prefer simple chunk compression first.

---

# 11. Lazy Loading Model

Pixhaus should open projects in stages.

## 11.1 Open project flow

```text
1. Read project.pxmeta
2. Read global index.pxidx
3. Load thumbnails/previews
4. Populate project browser
5. Load active workspace layout
6. Load currently active/open asset document metadata
7. Load only visible/current frame surfaces
8. Load nearby timeline frames in background
9. Load AI/generation history only when needed
```

## 11.2 Asset loading levels

Each asset should have loading levels:

```text
Unloaded
Indexed
MetadataLoaded
PreviewLoaded
DocumentLoaded
SurfacesPartiallyLoaded
FullyLoaded
Dirty
Saving
```

This lets Pixhaus manage memory deliberately.

## 11.3 Active document cache

Pixhaus should keep recently used documents in memory with an LRU-style policy.

Cache examples:

- Current sprite fully active.
- Recently opened sprites partially active.
- Thumbnails always cheap.
- Generated assets lazy.
- Cached composites disposable.

## 11.4 Memory pressure behavior

When memory pressure is high:

- Drop cached composites.
- Drop thumbnails that can be regenerated.
- Unload inactive surfaces.
- Keep dirty data until saved.
- Warn before unloading unsaved changes.

---

# 12. Incremental Save Architecture

Pixhaus should support incremental saves.

## 12.1 Dirty tracking

Track dirty state at multiple levels:

```text
project manifest dirty
asset index dirty
asset manifest dirty
sprite document metadata dirty
cel surface dirty
thumbnail dirty
cache dirty
generated result metadata dirty
recipe dirty
```

Do not mark the entire project dirty when a single cel changes.

## 12.2 Save process

Recommended safe-save process:

```text
1. Determine dirty assets/chunks
2. Write changed chunks to temporary files
3. Validate written files
4. Flush to disk as appropriate
5. Atomically replace old files where possible
6. Update indexes
7. Append save event to journal
8. Clear dirty flags
```

## 12.3 Atomicity

On cross-platform desktop apps, safe save behavior matters.

Use temp files and atomic rename where possible:

```text
sprite.pxdoc.tmp
sprite.pxdoc
```

For multi-file saves, use a transaction marker:

```text
save_transaction.pxtxn
```

This allows recovery after crash during save.

## 12.4 Autosave

Autosave should not constantly rewrite the main project.

Use:

```text
history/autosaves/
  autosave_2026_06_01_1530/
```

or per-asset autosave deltas.

Autosave should be recoverable but not make the project folder noisy forever. Old autosaves should be pruned according to user settings.

---

# 13. Crash Recovery

Pixhaus should assume crashes can happen during:

- AI generation.
- GPU work.
- Save operations.
- Large imports.
- Export operations.
- User closing laptop.

## 13.1 Journal file

A lightweight journal can record:

```text
project opened
asset modified
save started
file written
save completed
job started
job completed
job failed
autosave created
```

## 13.2 Recovery scan

On project open:

```text
1. Check for lock file
2. Check for incomplete transaction marker
3. Check journal tail
4. Check autosave folder
5. Check temp files
6. Offer recovery options
```

User-facing recovery:

```text
Pixhaus found unsaved recovery data for this project.

[Restore recovery] [Compare] [Discard]
```

## 13.3 Recovery priority

Never silently discard recovery data.

Never silently overwrite the main project with recovery data.

---

# 14. Asset Identity and References

Every major asset needs a stable ID.

Do not rely on file paths as identity.

Use:

```text
asset_id
asset_type
human_name
relative_path
```

File paths can change. IDs should remain stable.

## 14.1 References

Sprites can reference:

- Palettes.
- Reference images.
- Recipes.
- Generated assets.
- Parent/source assets.
- Export presets.
- Future particle assets.

References should be by stable asset ID, not fragile relative path.

## 14.2 Missing references

If an asset is missing, Pixhaus should show it clearly:

```text
Missing reference: dusk_forest_palette
Last known path: palettes/dusk_forest.pxpal
```

## 14.3 Dependency graph

Maintain dependency information in the index.

This enables:

- Find assets using a palette.
- Find sprites generated from a recipe.
- Package only selected assets and dependencies.
- Warn before deleting assets.
- Rebuild cache when dependencies change.

---

# 15. AI Provenance and Generation Metadata

AI-native workflows need provenance.

Every generated asset should optionally store:

```text
provider
model
prompt
negative_prompt
compiled_recipe_id
template_id
structure_id
style_id
variables
seed
input images
selected region
source sprite/frame/layer
palette behavior
timestamp
user-edited-after-generation flag
```

This is important for:

- Regeneration.
- Variations.
- Auditability.
- Debugging.
- Style consistency.
- User trust.

## 15.1 AI result should not directly mutate source

AI generation result should first be saved as a generated asset.

Then applying it to the project creates a command:

```text
GeneratedAsset -> ApplyToSpriteFrameCommand -> Sprite document changes
```

This means the original result can remain available even if the user modifies the applied version.

## 15.2 Generated asset folder

Example:

```text
generated/gen_01HX9M2Z/
  asset.pxasset
  result.pximg
  preview.png
  generation.pxmeta
  inputs/
    source_region.pximg
    palette_snapshot.pxpal
```

This makes generation reproducible and inspectable.

---

# 16. Undo/Redo vs Persistent History

Undo/redo and persistent save history are different systems.

## 16.1 Undo/redo

In-memory or session-local editing feature.

Stores command patches, pixel diffs, and operation data.

## 16.2 Persistent history

Optional project-level history for recovery, snapshots, or versioning.

Could store:

- Autosaves.
- Milestone snapshots.
- Command journal for recovery.
- AI generation history.

Do not require full persistent undo across app restarts in v1. It is complex and may create huge storage overhead.

A good v1 target:

```text
Undo/redo during session
Autosave recovery across restart
Manual snapshots/checkpoints
AI generation history
```

---

# 17. Snapshots and Versioning

Pixhaus should support project snapshots eventually.

Examples:

```text
history/snapshots/
  snapshot_001/
  snapshot_002/
```

Snapshot types:

```text
manual checkpoint
autosave
before destructive operation
before major import
before AI batch apply
before migration
```

A snapshot does not need to duplicate everything. It can use copy-on-write or hard links where available, but portability and simplicity matter.

For v1, simple full or asset-level snapshots are acceptable.

---

# 18. Project Migration

The save format will evolve.

Pixhaus needs explicit migration rules from the beginning.

## 18.1 Version every format

Version:

```text
project manifest format
asset manifest format
sprite document format
image/surface format
palette format
recipe format
index format
package format
```

Do not use one global version for everything.

## 18.2 Migration flow

When opening an old project:

```text
1. Detect project format version
2. Check supported range
3. If migration needed, offer backup
4. Create pre-migration snapshot
5. Run migrations in sequence
6. Validate migrated project
7. Save new version
```

## 18.3 Unknown future versions

If project was created by a newer Pixhaus:

- Open read-only if safe.
- Warn clearly.
- Do not overwrite unknown data unless explicitly allowed.

## 18.4 Extension data migration

Each internal module should own migration of its own extension data.

Core app should route module data to the responsible migrator.

---

# 19. External Assets

Projects may reference external files.

Examples:

- Reference images.
- Imported PSD/Aseprite files.
- Audio timing references.
- Model files.
- Game engine assets.

Pixhaus should support two modes:

```text
Embedded asset
Linked external asset
```

## 19.1 Embedded assets

Copied into the project folder.

Pros:

- Portable.
- Safer.
- Package export is easy.

Cons:

- Larger project.
- Duplicates files.

## 19.2 Linked assets

Reference original path.

Pros:

- Avoids duplication.
- Useful for large references.

Cons:

- Can break if moved.
- Harder to package.

## 19.3 Recommended UX

When importing, ask or use project setting:

```text
[Copy into project] [Link to original]
```

When packaging, offer:

```text
Include linked external assets?
```

---

# 20. Cache Policy

Caches should be disposable.

Cache folders may contain:

- Composite previews.
- Timeline frame thumbnails.
- Asset browser thumbnails.
- GPU-prepared data.
- AI intermediate previews.
- Import previews.

Rules:

- Cache can be deleted safely.
- Cache should be rebuildable.
- Cache should not be the only copy of important creative data.
- Cache should have versioning so stale cache can be invalidated.

Potential cache layout:

```text
cache/
  thumbnails/
  composites/
  timeline/
  ai/
  import/
  gpu/
```

---

# 21. Thumbnails and Previews

Thumbnails are critical for asset-heavy projects.

The asset browser should not need to open every sprite document.

Each asset should have:

```text
small thumbnail
medium preview
optional animated preview
```

Examples:

```text
thumb_64
thumb_256
animated_preview_webp_or_pxanim
```

Thumbnails can be stored in:

- Asset-local `.pxthumbs`.
- Global thumbnail atlas.
- Cache folder.

Recommended:

```text
asset-local canonical thumbnail + global cache/atlas for speed
```

If the cache disappears, asset-local thumbnails still help.

---

# 22. Animation Storage

Animation data should be first-class.

A sprite document should store:

```text
frames
frame durations
animation clips
clip ranges
loop modes
clip tags
onion skin preferences, maybe UI-level
frame labels
linked cels
```

## 22.1 Animation clips

Each clip should have:

```text
clip_id
name
frame_range
fps or frame durations
loop mode
export name
tags
source recipe, optional
AI provenance, optional
```

## 22.2 Frame surfaces

Frames should not necessarily store full composites.

Store source cels/layers, and generate composites as cache.

## 22.3 Delta compression

For animation, frame delta compression can save space, but it complicates random access.

Recommendation:

- Store editable cels independently.
- Optionally compress internally.
- Use cached/delta-encoded previews only for performance.
- Do not make editable data depend on a fragile delta chain in v1.

---

# 23. Pixel Art-Specific Storage

Pixel art mode deserves special support.

Pixel-art sprite documents may store:

```text
indexed surfaces
palette ID
palette snapshots
ramps
color harmony groups
dither settings
tile grid settings
major grid size
strict palette flag
nearest-neighbor export defaults
```

## 23.1 Palette snapshots

If a sprite references a project palette, changes to that palette may affect the sprite.

Pixhaus should support:

```text
live palette reference
palette snapshot
locked palette
```

This prevents accidental palette changes destroying old assets.

## 23.2 Palette migration

If a palette color is removed or reordered, indexed surfaces need careful handling.

Store palette entries with stable color IDs, not only numeric positions, if possible.

---

# 24. Multi-Style Raster Storage

For non-pixel-art sprites, Pixhaus should support larger RGBA surfaces and future enhancements.

Consider future compatibility for:

```text
soft brushes
masks
alpha gradients
layer effects
adjustment layers
blend modes
high-res sprite source + exported downscale
normal maps
emissive maps
```

Do not overbuild all of this now, but the file format should have room for it.

For example, layers should have a `kind` field:

```text
raster
indexed_raster
mask
reference
adjustment, future
folder/group, future
procedural, future
```

---

# 25. Tilesets and Future Asset Types

The save format should not assume everything is a sprite.

Future asset types may include:

```text
tileset
autotile rule set
particle system
sprite UI component
nine-slice sprite
rigged sprite
brush pack
material pack
style pack
prompt recipe pack
```

Each asset type should have:

```text
asset manifest
primary document
optional binary blobs
optional previews
extension data
```

This gives future workspaces a predictable storage pattern.

---

# 26. Internal Module Extension Data

Pixhaus will use internal modules rather than external dynamic plugins.

Still, modules need storage space.

Each module should have a namespace.

Examples:

```text
com.pixhaus.core
com.pixhaus.sprite
com.pixhaus.animation
com.pixhaus.generation
com.pixhaus.tiles
com.pixhaus.export
com.pixhaus.particles
com.pixhaus.ui_sprites
```

Documents and assets may include module extension sections:

```text
extension_data:
  com.pixhaus.generation:
    source_recipe_id: ...
    seed: ...
  com.pixhaus.tiles:
    autotile_rule: ...
```

Unknown module data should be preserved when possible.

---

# 27. File Locking and Multi-Process Safety

Pixhaus should create a lock file when opening a project for editing.

```text
lock.pxlock
```

Contains:

```text
machine name
user name, optional
process ID
Pixhaus version
timestamp
```

If another Pixhaus instance opens the project:

```text
Project appears to be open elsewhere.

[Open read-only] [Take over lock] [Cancel]
```

Do not attempt full real-time collaborative editing in the save format v1.

---

# 28. Cross-Platform Path Rules

Project files must use portable paths.

Rules:

- Store internal paths as UTF-8 relative paths.
- Use `/` as logical separator in metadata.
- Normalize paths.
- Avoid absolute paths except for linked external assets.
- Handle case sensitivity carefully.
- Do not rely on platform-specific hidden folder behavior.

External linked assets may store:

```text
original absolute path
relative path if inside project parent
file fingerprint/hash
last known size/modified time
```

This helps find moved files.

---

# 29. Integrity and Validation

Pixhaus should validate projects.

Validation checks:

```text
manifest exists
index can be read
asset manifests exist
referenced documents exist
binary chunks have valid checksums
sprites reference existing palettes
animation clips reference valid frames
generated assets reference available results
external links exist or are marked missing
unknown extension data is preserved
cache version is valid
```

Offer repair tools:

```text
Rebuild index
Regenerate thumbnails
Find missing assets
Remove stale cache
Validate binary chunks
Recover from autosave
```

This should eventually be available from the Export or Project Health panel.

---

# 30. Package Format `.pxpack`

A package should be a portable archive of a project folder.

Recommended properties:

```text
contains project folder structure
includes package manifest
includes checksums
optionally includes linked external assets
optionally excludes cache
optionally includes generated history
compression optimized for portability
```

Package manifest:

```text
package version
created by Pixhaus version
project ID
included assets
excluded assets
external references included or omitted
checksums
```

Packaging options:

```text
Full project
Project without cache
Selected assets only
Selected sprite and dependencies
Bug report package
Archive with generated history
Archive without generated history
```

---

# 31. Import/Export Relationship

Save format is not export format.

Pixhaus project data preserves editable structure.

Exports produce flattened or game-engine-ready outputs.

Examples:

```text
PNG
GIF
APNG
spritesheet PNG + JSON
Godot atlas metadata
Unity sprite metadata
TexturePacker JSON
Aseprite export
```

Never design the project format around a single export target.

The project format should preserve source truth. Exporters transform it.

---

# 32. Save Format and Agent-Driven Development

Because agents will be used extensively, the save format should be agent-friendly.

That means:

- Clear manifest files.
- Stable schemas.
- Small bounded files.
- Asset-local documents.
- Human-readable metadata during development.
- Validation commands.
- Migration scripts.
- Golden test fixtures.
- Format documentation.

Agents should be able to work on:

```text
palette format
sprite document metadata
recipe storage
generated asset storage
index rebuild command
project validation
without touching rendering or UI code
```

This is another reason folder-based storage is superior.

---

# 33. Recommended V1 Format

Do not build the final perfect binary format immediately.

Recommended v1:

```text
Project folder
JSON metadata
Binary `.pximg` surface blobs
Asset-local manifests
Rebuildable index
Simple thumbnails
Autosave/recovery folder
Packaged archive later
```

V1 priorities:

```text
correctness
partial loading
simple debugging
safe saves
schema migration
asset identity
lazy loading
AI provenance
multi-style support
pixel-art mode support
```

Do not over-optimize compression, chunking, or delta encoding until real project sizes prove the need.

---

# 34. Recommended V2 Format

Once the product stabilizes:

```text
Chunked `.pxdoc` binary documents
Chunked `.pximg` binary surfaces
Fast binary/global index
Optional SQLite metadata cache
Package format `.pxpack`
Better compression
Per-chunk checksums
Unknown chunk preservation
```

V2 priorities:

```text
fast loading
large project scalability
stable binary schema
archive packaging
asset dependency graph
cache optimization
```

---

# 35. Recommended V3+ Direction

Future capabilities:

```text
copy-on-write snapshots
asset-level version history
large tiled surfaces
streaming document loading
cloud sync support
collaboration-aware file structure
partial package export
content-addressed blobs
deduplicated generated assets
```

Do not design v1 around all of this, but avoid blocking it.

---

# 36. Proposed Format Stack

Recommended long-term stack:

```text
Project folder:         primary editable format
Project manifest:       readable structured metadata initially
Asset index:            rebuildable binary or SQLite later
Asset manifests:        readable/semi-readable metadata
Sprite documents:       chunked `.pxdoc`
Image surfaces:         chunked `.pximg`
Thumbnails:             `.pxthumbs` or cache atlas
Package export:         `.pxpack` archive
Caches:                 disposable folder
Recovery:               journal + autosaves
```

---

# 37. Example Project: Small

```text
SlimeGame.pixhaus/
  project.pxmeta
  index.pxidx

  sprites/
    hero/
      asset.pxasset
      sprite.pxdoc
      thumbs.pxthumbs
    slime/
      asset.pxasset
      sprite.pxdoc
      thumbs.pxthumbs

  palettes/
    main.pxpal

  recipes/
    recipes.pxmeta

  generated/
    gen_slime_walk_001/
      asset.pxasset
      result.pximg
      generation.pxmeta

  cache/
  history/
```

Open behavior:

- Load project metadata.
- Load index.
- Show hero and slime thumbnails.
- Load hero document only when selected.
- Load slime later if opened.

---

# 38. Example Project: Large

```text
RPG_Assets.pixhaus/
  project.pxmeta
  index.pxidx
  previews.pxthumbs

  sprites/
    player/
    enemies/
      slime/
      skeleton/
      bat/
      boss_01/
    npcs/
    items/
    ui/

  tilesets/
    forest/
    dungeon/
    cave/

  generated/
    batch_001/
    batch_002/
    batch_003/

  references/
    moodboards/
    sketches/

  recipes/
    platformer_pack/
    rpg_items_pack/
    ui_icon_pack/

  cache/
    thumbnails/
    composites/
    ai/

  history/
    autosaves/
    snapshots/
```

Open behavior:

- Load project and index.
- Show asset tree and thumbnails.
- Do not load every sprite document.
- Do not load every generated asset.
- Load selected workspace data only.

---

# 39. Project Health Tooling

Pixhaus should eventually have a project health/check command.

It can report:

```text
Project size
Asset count
Generated asset count
Missing references
Cache size
Recoverable autosaves
Corrupt chunks
Outdated format versions
Large unused assets
Duplicate generated results
Unreferenced blobs
```

Actions:

```text
Rebuild index
Clean cache
Compact project
Package project
Remove orphaned generated assets
Repair missing thumbnails
Migrate project
```

This is especially important for asset-heavy projects.

---

# 40. Final Recommendation

Pixhaus should use a **hybrid project-folder format**:

- Folder-based project as the primary working format.
- Asset-local manifests and documents.
- Binary surface/blob storage for heavy data.
- Rebuildable global index for fast browsing.
- Lazy loading by asset, frame, layer, cel, and surface.
- Project-level autosave and recovery.
- Optional single-file package for sharing and archive.
- Explicit format versioning and migration.
- Extension-friendly asset/document model.
- Multi-style sprite support, with pixel art as a dedicated mode.

The save format should treat Pixhaus projects as **asset libraries**, not single images.

The guiding principle:

> Open fast, load lazily, save incrementally, preserve editability, and never make one huge file the only source of truth.

That design will support small hobby projects, large game asset libraries, AI-heavy generation workflows, future internal modules, and long-term project compatibility.
