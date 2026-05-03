# Pyxel Edit

## Quick facts
- Vendor / maintainer: Developer (independent, name not widely publicized)
- License / pricing model: Commercial, one-time purchase (beta pricing model)
- Price point (current): $9 USD (one-time, beta pricing)
- Platforms: Windows, macOS (via Wine on Linux)
- First released: Unknown (released as free, commercial version began development)
- Last meaningful update: Unknown (active development, beta status maintained)
- Source available: No (proprietary)
- Primary use case: Tile-aware pixel art and tilemap design for game development

## Origin and purpose

Pyxel Edit was originally released as freeware for creating pixel art and tilesets. As development expanded to add tile-specific features and stability improvements, the creator transitioned to a beta pricing model. Early adopters purchasing the beta receive all future updates at no additional cost, with pricing expected to increase with version milestones. The tool targets game developers needing integrated tile and tilemap workflows rather than artists focusing solely on animation.

## Drawing and painting tools

Standard pixel art tools include pencil, eraser, brush, line, rectangle, and ellipse. Fill tool (bucket fill) accelerates large area painting. Custom brush creation is supported. Selection tools (rectangular, freehand) isolate regions. The focus is on tiling-aware functionality rather than brush sophistication; drawing tools are functional but not as extensive as animation-focused editors.

## Pixel-specific features

Grid-based editing with snapping ensures clean tile alignment. Zoom levels support detailed pixel work and overview. Pattern fill enables quick tessellation of repeated motifs. Symmetry tools reduce work on symmetrical designs. Tile editing directly on the canvas (see changes in real-time across all instances) is a core feature. Tiles can be flipped and rotated while remaining linked to the original, maintaining real-time sync across the tileset. Import and auto-identification of unique tiles from mockups or existing tilesets speeds tileset extraction from reference images.

## Color and palette workflow

Pyxel Edit includes palette management and per-tile color customization. Color swatches and palette selection. RGB and indexed color modes. Palette import from images. Color picker (eyedropper) for sampling. The color workflow is functional but not as refined as dedicated animation tools; emphasis is on tileset organization rather than palette animations.

## Layer system

Multiple tilemap layers enable game level composition with depth and organization. Each layer can be toggled on/off for editing. Layers represent different tile planes (background, foreground, collision, etc.). Layer opacity control allows visibility adjustment. Merging and flattening operations combine layers. Layer-based organization suits level design more than sprite animation workflows.

## Animation features

Tile-based animation support allows individual tiles to animate independently (frame sequences per tile). Animation timeline shows frame sequences for selected tile. Frame duration is customizable. Animated tiles display in the level preview. Export includes animation data embedded in tilemap. Animation is tile-focused rather than sprite-focused; multi-frame sprites would use Aseprite or similar, while Pyxel Edit handles tile animation within levels.

## Export and import

Tilemap export formats include XML, JSON, and plain text, enabling quick game engine integration. Tileset PNG export with metadata. Frame data for animated tiles. Collision layer export as separate data. Batch export of multiple tilemaps. PNG and image sequence import for mockup conversion to tilesets. Backward compatibility with older Pyxel Edit project files during updates.

## Scripting and extensibility

Unknown scripting or plugin support based on available documentation. The focus is on visual editing rather than programmability. Community feature requests and development suggestions drive evolution.

## Engine integration

Pyxel Edit exports are compatible with game engines through standard formats (JSON, XML). Tiled compatibility enables interop with Tiled Map Editor. Level data suitable for tile-based 2D engines. Sprite sheets export for use in custom pipelines. Collision layer export suits physics engine integration. No native plugin system for engine-specific bridges.

## Workflow strengths

Integrated tile and tilemap editing eliminates tool-switching. Direct tile editing with real-time propagation to all instances speeds iteration. Tile flipping and rotation while maintaining linkage reduces asset bloat. Import and auto-identify of unique tiles from mockups accelerates tileset extraction. JSON/XML export integrates cleanly with game engines and custom tools. Affordable beta pricing supports hobbyist budgets. Cross-platform availability (Windows, macOS, Linux via Wine) suits diverse teams.

## Workflow gaps

No character animation timeline; sprite animation requires Aseprite or similar. Text tool is absent. Limited brush customization compared to animation-focused tools. No layer blend modes or advanced compositing. Scripting and plugin system not available. No collaboration or version control integration. GUI responsiveness and polish lag modern tools. Documentation and tutorials are sparse compared to Aseprite/Aseprite equivalents. Community size is small, limiting third-party resources.

## Notable uses

Indie game developers creating tile-based games (platformers, top-down adventures, roguelikes) use Pyxel Edit for tileset and level design. Game jams feature Pyxel Edit workflow documentation. Educational game development projects benefit from its focused scope and low cost. Retro game recreation and ROM hacking communities use it for tileset editing.

## Community and ecosystem

Small but dedicated community. Official website (pyxeledit.com) hosts downloads, features list, and user guide. YouTube tutorials exist but are limited. GitHub discussions and issue tracking (if publicly available) drive feature requests. Itch.io hosts games created with Pyxel Edit. Community sharing of tilemaps and templates is minimal compared to Aseprite. Development roadmap is community-driven through feedback.

## Pricing details

Pyxel Edit costs $9 USD one-time purchase, currently at beta pricing. Price is expected to increase with stable releases. Purchasing beta grants all future updates and versions at no additional cost. No subscription or per-seat licensing. Created artwork has no commercial restrictions. Free version (older) may still be available for download but receives no support or updates. Educational discounts are not advertised.
