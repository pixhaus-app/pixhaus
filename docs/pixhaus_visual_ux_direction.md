# Pixhaus Visual UX Direction

**Version:** 1.0  
**Product:** Pixhaus  
**Platform:** Native desktop application for Windows, macOS, and Linux  
**Primary domain:** Sprite creation, sprite editing, sprite animation, and AI-assisted asset generation  
**Design reference points:** Blender, Aseprite, Photoshop, animation timelines, game asset pipelines  

---

## 1. Executive Summary

Pixhaus should become a **native sprite production studio**: a professional, craft-first desktop application for creating sprites and sprite animations, with AI deeply integrated as an accelerator rather than as the center of the product.

The product should serve two audiences equally well:

1. **Game developers with limited artistic skill**  
   They need to generate usable sprites, animations, props, tilesets, icons, and variations quickly.

2. **Experienced pixel artists and animators**  
   They need precise manual tools, fast workflows, strong palette control, animation timelines, and AI assistance that stays out of the way unless requested.

The goal is not to build “an AI image generator with pixel tools.”  
The goal is to build:

> **A professional sprite editor and animation studio with AI-native acceleration.**

A stronger framing:

> **Manual-first. AI-assisted. Artist-respecting. Game-production ready.**

Pixhaus should feel dense, powerful, native, and serious like Blender, while remaining direct and craft-oriented like Aseprite.

---

## 2. Product Positioning

### 2.1 Core Positioning

Pixhaus is a native desktop application for creating, editing, animating, organizing, and exporting sprites for games.

AI is a first-class part of the workflow, but the canvas remains the source of truth.

The product should be able to say:

> **Whether you can draw or not, Pixhaus helps you get production-ready sprites into your game faster, without taking away control.**

### 2.2 What Pixhaus Is

Pixhaus is:

- a manual sprite editor
- a sprite animation tool
- a game asset production workspace
- an AI-assisted sprite generator
- an AI composition and prompt recipe system
- an animation coverage and production planning tool
- a palette-aware pixel art environment
- a native professional creative app

### 2.3 What Pixhaus Is Not

Pixhaus should not feel like:

- a web dashboard
- a chatbot wrapped around an image generator
- a toy pixel editor
- a Midjourney-style prompt box with a canvas attached
- an AI-first app that treats manual artists as secondary users
- a generic image editor without game-production thinking

---

## 3. Core Design Philosophy

### 3.1 The Canvas Is Sovereign

The canvas is always the primary creative surface.

AI can generate, suggest, clean, fill, interpolate, and vary, but the user decides what enters the final artwork.

AI output should always be:

- previewable
- editable
- undoable
- reproducible
- palette-aware
- selectable
- non-destructive where possible

### 3.2 AI Proposes, The Artist Decides

AI should not silently overwrite work. It should propose options.

The user should be able to:

- compare generated variants
- pick one result
- insert as a new sprite
- insert into a selected region
- place as a new layer
- regenerate with the same seed
- generate variations from a selected result
- discard everything cleanly

### 3.3 Manual Workflows Must Be Excellent Without AI

Pixhaus must still be a credible sprite editor if AI is disabled.

This is critical for trust with professional artists.

Manual workflows need to be polished:

- pencil
- eraser
- fill
- line
- shapes
- selection
- lasso
- transform
- layers
- palette editing
- frame editing
- onion skin
- animation timing
- export
- keyboard shortcuts
- right-click menus
- drag-and-drop

### 3.4 AI Should Be Contextual, Not Intrusive

AI should appear when it is useful:

- when the user selects a region
- when the user is on an animation frame
- when the user opens Generate mode
- when a tile is selected
- when a sprite has missing animations
- when a palette has too many colors
- when seams or jitter are detected

AI should not constantly compete with the drawing tools.

---

## 4. Target Audiences

## 4.1 Game Developers With Limited Art Skill

These users want assets quickly.

They may not know terms like:

- hue shifting
- dithering
- indexed palettes
- silhouette readability
- onion skin
- animation arcs
- tileset rules
- 47-tile autotiles
- ramp
- anti-aliasing

They need friendly, guided creation flows.

### Needs

- Generate character sprites from prompts
- Generate full animation sets
- Generate props and items
- Generate tilesets and environments
- Generate UI icons
- Choose common game styles
- Export directly to their game engine
- Understand what assets are missing
- Reuse prompts and recipes
- Keep style consistent across assets

### Ideal Experience

A developer should be able to say:

> “I need a goblin enemy with idle, walk, attack, hurt, and death animations in a dark fantasy 32x32 style.”

Pixhaus should guide them from this idea to production-ready assets.

---

## 4.2 Experienced Pixel Artists

These users can draw manually and care deeply about control.

They may dislike tools that shove generation into the foreground.

They need:

- speed
- precision
- predictable editing
- palette discipline
- timeline control
- no hidden AI changes
- high-quality export
- powerful shortcuts
- non-intrusive assistance

### Needs

- Draw manually all day without AI in the way
- Use AI only on selected areas
- Generate in-betweens from existing frames
- Clean up stray pixels
- Reduce colors to palette
- Suggest ramps and harmonies
- Detect animation jitter
- Fix tile seams
- Create variations without losing the original
- Preserve style, silhouette, and palette

### Ideal Experience

A pro artist should think:

> “This still feels like my hand made the work. Pixhaus just saved me 30 minutes.”

---

## 5. Visual Direction

## 5.1 Desired Feel

Pixhaus should feel like:

- a serious native creative application
- a compact production cockpit
- a professional game asset tool
- a dark studio environment
- precise and fast
- quiet but powerful
- technical enough for experts
- approachable enough for non-artists

The app should have the density and workspace structure of Blender, but with the immediacy and pixel-art focus of Aseprite.

### Keywords

- native
- professional
- compact
- precise
- dark
- layered
- modular
- production-ready
- craft-first
- AI-native
- artist-respecting

---

## 5.2 Avoided Feel

Avoid:

- overly glossy web-app UI
- giant prompt boxes in the main editor
- empty SaaS dashboard layouts
- excessive neon AI styling
- huge rounded-card mobile design
- playful toy-like UI
- flat admin panels
- debug-tool aesthetics
- overly sparse screens

---

## 5.3 Visual Identity

Pixhaus already has a strong identity through:

- dark UI
- violet accent
- pixel art mascot
- compact editor layout
- native desktop density

The UI should preserve that identity but make hierarchy clearer.

Recommended accent usage:

- active workspace
- active tool
- selected layer/frame/sprite
- AI affordances
- focused input
- primary actions
- generated result selection

The violet should be used confidently but sparingly.

AI should glow softly, not scream.

---

## 6. App-Level Layout

## 6.1 Primary Regions

The application should be organized into stable regions:

```text
Top Bar       App menus, workspace tabs, global status
Tool Options  Active tool settings
Left Shelf    Manual tools
Center Stage  Canvas / editor / workspace content
Right Panel   Inspector / layers / sprites / AI context
Bottom Tray   Timeline / frames / assets / AI results
Status Bar    zoom, grid, sprite size, backend status, console, AI status
```

Each region should have slightly different visual treatment so the user immediately understands the structure of the app.

---

## 6.2 Region Hierarchy

Current UI issue: many areas share similar contrast and visual weight.

Recommended hierarchy:

| Region | Visual Treatment |
|---|---|
| App frame | darkest background |
| Top bars | slightly elevated dark surface |
| Left toolbar | compact vertical rail |
| Canvas stage | deep neutral, checker/grid visible |
| Artboard | framed, subtly shadowed |
| Right panels | card-like dark panels with headers |
| Bottom tray | timeline/editor surface, visually connected to canvas |
| Active items | violet highlight |
| AI actions | violet + sparkle marker |

---

## 6.3 Native Desktop Feel

Since Pixhaus is a native app, lean into desktop expectations:

- menu bar commands
- keyboard shortcuts
- dockable panels
- resizable split panes
- context menus
- command palette
- drag-and-drop
- status bar
- workspace tabs
- multi-document tabs
- local project assets
- precise input fields
- compact controls

Do not make it feel like a browser app pretending to be a desktop app.

---

## 7. Workspace Modes

Pixhaus should adopt task-focused workspaces, inspired by Blender.

The purpose of workspaces is not to split the product into separate apps. It is to focus the UI around the task at hand.

Recommended workspace tabs:

```text
Draw | Animate | Tiles | Generate | Export
```

Optional future workspaces:

```text
Compose | Palette | Rig | Effects | Debug
```

But the first five are the core.

---

## 7.1 Draw Workspace

### Purpose

Manual sprite creation and editing.

This should be the default workspace.

### Primary User

- experienced pixel artist
- game developer editing generated output
- anyone doing precise manual work

### Main UI Focus

- canvas
- pencil/eraser/fill tools
- selections
- layers
- palette
- sprite list
- grid controls
- zoom controls

### AI Role

Quiet and contextual.

AI actions in Draw should include:

- fill selected area
- clean stray pixels
- reduce to palette
- suggest palette ramp
- create selected-region variations
- fix outline readability
- add lighting
- remove background
- make tile seamless if selection is tile-sized

### Draw Workspace Principle

> Manual tools are primary. AI is available when requested.

---

## 7.2 Animate Workspace

### Purpose

Sprite animation, timing, clips, frame editing, and animation polish.

### Primary User

- pixel animator
- game developer producing animation sets
- users editing AI-generated animation

### Main UI Focus

- canvas preview
- timeline
- frame thumbnails
- animation clips
- onion skin
- playback
- FPS / timing
- layer tracks

### AI Role

Animation assistant.

AI actions in Animate should include:

- generate in-between frames
- extend animation
- create idle/walk/run/attack cycles
- detect jitter
- loop polish
- propagate edits across frames
- clean all frames
- reduce all frames to palette
- generate missing animation from current sprite
- maintain silhouette across frames

### Animate Workspace Principle

> Animation is a production timeline, not just a row of frames.

---

## 7.3 Tiles Workspace

### Purpose

Tilesets, autotiles, terrain, seamless materials, and map-ready assets.

### Primary User

- game developer
- environment artist
- pixel artist making terrain sets

### Main UI Focus

- tile canvas
- tile preview grid
- seamless preview
- random patch preview
- tileset browser
- autotile rules
- edge matching

### AI Role

Tileset generator and validator.

AI actions in Tiles should include:

- generate single tile
- generate seamless tile
- generate 3x3 autotile
- generate 47-tile blob set
- generate terrain variants
- detect seams
- fix edge mismatch
- reduce repetition
- generate tile transitions
- create material variants

### Tiles Workspace Principle

> Generate, preview, validate, and fix tile behavior visually.

---

## 7.4 Generate Workspace

### Purpose

Guided AI generation for sprites, animations, props, tilesets, icons, and backgrounds.

### Primary User

- game developers with limited art skill
- artists exploring ideas
- users producing starting points

### Main UI Focus

- asset type selection
- prompt
- style
- structure
- palette behavior
- sprite size
- animation type
- results grid
- generation history
- insert/apply actions

### AI Role

Primary creative generator.

Generate workspace should include:

- Character
- Animation
- Prop / Item
- Tileset
- UI Icon
- Background
- Environment
- Effect

### Generate Workspace Principle

> This is where AI can be prominent because the user explicitly came here to generate.

---

## 7.5 Export Workspace

### Purpose

Prepare game-ready output.

### Primary User

- game developers
- technical artists
- anyone exporting assets into engines

### Main UI Focus

- spritesheet packing
- animation metadata
- game engine presets
- file naming
- JSON export
- GIF/video preview
- PNG export
- trimming
- padding
- pivot points
- hitboxes / hurtboxes eventually

### AI Role

Production validator.

AI actions in Export should include:

- detect inconsistent frame sizes
- find stray transparent pixels
- check palette count
- validate animation naming
- suggest missing animations
- flag non-looping loops
- optimize spritesheet layout

### Export Workspace Principle

> Make assets game-ready and catch production mistakes before export.

---

## 8. Top Bar Design

## 8.1 Structure

Recommended top layout:

```text
Pixhaus   File  Edit  Sprite  Layer  Frame  Select  View  Window  Help

Draw  Animate  Tiles  Generate  Export

Tool Options: Pencil | 1px | Opacity 255 | Pixel-perfect | Dither None | Mirror X | Mirror Y
```

Currently, menus and modes visually blend together. The workspace tabs should have more presence.

---

## 8.2 Workspace Tab Styling

Active workspace:

- violet text or pill
- subtle underline
- slightly brighter background

Inactive workspace:

- low contrast but readable

AI-related workspace naming:

Use **Generate**, not **AI**.

Reason:

- “Generate” describes the user’s goal
- “AI” describes the implementation
- “Generate” feels less gimmicky
- artists are less likely to feel pushed into AI

---

## 8.3 Tool Options Bar

The tool options bar should change depending on active tool and workspace.

Examples:

### Pencil Tool

```text
Pencil | Size 1 px | Opacity 255 | Pixel-perfect | Dither None | Mirror X | Mirror Y
```

### Selection Tool

```text
Selection | Mode Replace | Feather 0 | Snap Pixel | Transform Origin Center
```

### AI Fill Selection

```text
AI Fill | Use Palette | Preserve Outline | Variations 4 | Strength 0.65
```

---

## 9. Left Toolbar

## 9.1 Purpose

The left toolbar should remain focused on direct tools.

Recommended tools:

- Pencil
- Eraser
- Fill
- Line
- Rectangle
- Ellipse
- Eyedropper
- Selection
- Lasso
- Move
- Transform
- Text
- Hand
- Zoom
- AI Brush / Assistive Tool

---

## 9.2 Active Tool State

Active tool should be more obvious than today.

Recommended active state:

- violet-tinted background
- bright icon
- small left or top accent line
- tooltip with shortcut

Example tooltip:

```text
Pencil Tool (B)
Draw individual pixels. Hold Shift for straight line.
```

---

## 9.3 AI Tool in Toolbar

The toolbar can include one AI tool, but it should not dominate.

Suggested name:

```text
AI Brush
```

AI Brush modes:

- Fill
- Clean
- Variations
- Lighting
- Material
- Repair

The AI Brush is for targeted manual-assist work, not full generation.

---

## 10. Canvas Stage

## 10.1 Current Issue

The canvas is large and clean, but it needs more visual ceremony.

It should feel like the central artboard, not just a grid floating in a dark area.

---

## 10.2 Recommended Canvas Treatment

Add:

- subtle canvas frame
- slight shadow around artboard
- darker stage area behind artboard
- checkerboard outside transparent bounds
- stronger major grid every 8 or 16 pixels
- lighter minor grid
- floating zoom controls
- floating canvas HUD

---

## 10.3 Canvas HUD

A small unobtrusive HUD could show:

```text
64 × 64   1600%   Grid 8px   Pixel Perfect   Palette: Bit
```

For animation:

```text
Frame 11 / 15   Clip: jump   12 FPS   Onion Skin: Off
```

For Generate preview:

```text
Preview result 3 / 8   Seed 123456   Style: Pixel Art
```

---

## 10.4 Grid Controls

Grid should be customizable:

```text
Grid: Off / Light / Normal / Strong
Major Grid: 8px / 16px / 32px
```

At high zoom, the current grid can visually overpower the artwork. The major grid should help orientation while the minor grid remains subtle.

---

## 11. Right Panel / Inspector

## 11.1 Current Issue

The right panel is useful but can feel like a stack of forms.

It should behave like a contextual inspector.

---

## 11.2 Recommended Right Panel Structure

Use task-based panels depending on workspace.

### Draw

- Layers
- Sprites
- Palette
- Selection Actions
- AI Assistant

### Animate

- Layers
- Sprites
- Frames
- Clip Properties
- AI Animation Assistant

### Tiles

- Tileset
- Rule Type
- Material
- Seam QA
- AI Tile Assistant

### Generate

- Prompt
- Recipe
- Structure
- Style
- Palette Behavior
- Advanced Settings

### Export

- Export Format
- Engine Preset
- Animation Metadata
- QA Warnings

---

## 11.3 Contextual AI Assistant

In editing workspaces, AI should appear as a contextual assistant panel.

Example in Animate workspace:

```text
AI Assistant

Quick Actions
[ In-between frames ]  Generate missing frames
[ Variations ]         Create sprite variations
[ Extend animation ]   Add more frames
[ Clean up ]           Remove stray pixels
[ Reduce colors ]      Match current palette
[ Make seamless ]      Tile / edge match
```

This is much better than burying AI under props/autotile sections.

---

## 12. Palette System

## 12.1 Palette Is Core, Not Secondary

For pixel art, palette is not a minor setting. It is part of the art direction.

The palette panel should feel important.

---

## 12.2 Palette UI Recommendations

Include:

- palette name
- swatch grid
- locked colors
- foreground/background color
- recent colors
- ramps
- harmony tools
- sort options
- reduce-to-palette action
- add generated colors review

---

## 12.3 AI Palette Behavior

AI generation should expose palette behavior clearly:

```text
AI Palette Behavior
[x] Use current palette only
[ ] Add colors automatically
[ ] Suggest palette expansion
[ ] Dither to palette
[ ] Reduce after generation
```

When AI adds colors:

```text
3 new colors added by AI
[ Review ] [ Accept all ] [ Revert ]
```

If generation exceeds the palette:

```text
Generated sprite uses 37 colors.
Current palette has 8 colors.

[ Reduce to palette ] [ Keep colors ] [ Create new palette ]
```

---

## 13. Timeline Design

## 13.1 Current Timeline Strength

The existing timeline is already one of the strongest parts of Pixhaus.

It includes:

- playback controls
- frame controls
- animation tags
- timing
- onion skin
- layer frame grid
- linked frames
- animation labels

This should become a major product differentiator.

---

## 13.2 Timeline Problem

The current timeline is powerful but visually dense and somewhat flat.

The user needs clearer visual separation between:

- playback
- animation clips
- frame ruler
- layer tracks
- selected frame
- timing
- AI-generated frames

---

## 13.3 Recommended Timeline Bands

Structure timeline into clear horizontal bands:

```text
Playback Bar
Animation Clips
Frame Ruler
Layer Tracks
```

Example:

```text
[ Play ] [ Prev ] [ Next ]   100ms   1.00x   12 FPS   Loop

Animations
idle | walk | run | jump | fall | attack | hurt | custom clip

Frames
0  1  2  3  4  5  6  7  8  9  10  11  12  13  14

Tracks
Body      □ □ □ □ □ □ □ □ □ □ □ □ □ □ □
Effects   □ □ □ □ □ □ □ □ □ □ □ □ □ □ □
Shadow    □ □ □ □ □ □ □ □ □ □ □ □ □ □ □
```

---

## 13.4 Animation Tags Become Clips

The colored animation tags should visually behave like timeline clips.

Instead of small loose labels, they should span frame ranges.

Example:

```text
idle       walk       run        jump       attack
[---]      [---]      [---]      [----]     [-----]
```

Each clip should be clickable.

Clicking a clip opens clip properties:

```text
Animation: jump
Frames: 8–15
FPS: 12
Loop: false
Export name: bit_jump
Source recipe: Bit — jump
Prompt: Bit, the Pixhaus mascot...
```

Actions:

```text
Regenerate clip
Create variations
Extend animation
Clean frames
Lock manual edits
Export clip
```

---

## 13.5 Playhead

The playhead should be more visible.

Recommended:

- full-height vertical line through timeline
- selected frame cell has violet outline
- frame number is highlighted
- canvas HUD shows current frame

---

## 13.6 Frame Cell States

Frame cells should communicate state.

| State | Visual Indicator |
|---|---|
| Empty frame | dark empty cell |
| Drawn frame | thumbnail or filled marker |
| AI-generated frame | small sparkle marker |
| AI-generated but edited | sparkle + pencil marker |
| Linked frame | chain icon |
| Onion skin enabled | ghost marker |
| Timing override | small clock marker |
| Selected frame | violet outline / fill |
| Part of selected clip | tinted clip background |

---

## 13.7 AI in Timeline

AI should be integrated into timeline workflows.

Examples:

- select two keyframes → `Generate in-betweens`
- select clip → `Create variations`
- select frames → `Clean selected frames`
- right-click empty range → `Generate missing frames`
- right-click animation tag → `Regenerate animation from recipe`

---

## 14. Bottom Tray

## 14.1 Purpose

The bottom area should become a flexible production tray.

It can show different content depending on workspace:

```text
Timeline | Frames | Assets | AI Results | Console
```

---

## 14.2 Draw Workspace Bottom Tray

Default:

- frames strip
- small playback controls if sprite has animation
- recent assets

---

## 14.3 Animate Workspace Bottom Tray

Default:

- full timeline
- animation clips
- layer tracks
- onion skin controls

---

## 14.4 Generate Workspace Bottom Tray

Default:

- generation results
- variation grid
- selected result preview
- seed/history

Actions:

```text
Use selected
Insert as new sprite
Place on canvas
Generate more
Create variations
Save to assets
```

---

## 14.5 Tiles Workspace Bottom Tray

Default:

- tile variations
- terrain patch preview
- seamless test
- edge QA

---

## 15. Generate Workspace

## 15.1 Goal

Generate workspace is the friendly, guided place for AI creation.

This is where non-artists should feel empowered.

---

## 15.2 Recommended Layout

```text
Left:   What are you making?
Center: Prompt, style, size, animation settings
Right:  Context and advanced settings
Bottom: Results grid
```

---

## 15.3 Asset Type Selection

Use visual cards:

```text
Character
Animation
Prop / Item
Tileset
UI Icon
Background
Environment
Effect
```

Each card should have a tiny pixel-art icon.

---

## 15.4 Friendly Generation Flow

Questions should be simple:

```text
What do you want to make?
[ Character ] [ Animation ] [ Prop ] [ Tileset ] [ UI Icon ]

Style
[ Pixel Art ] [ Retro ] [ Cute ] [ Dark Fantasy ] [ Sci-Fi ] [ Custom ]

Size
[ 16x16 ] [ 32x32 ] [ 64x64 ] [ 128x128 ] [ Custom ]

Animation
[ None ] [ Idle ] [ Walk ] [ Run ] [ Jump ] [ Attack ] [ Custom ]
```

---

## 15.5 Advanced Settings Should Be Collapsed

Advanced settings are important, but they should not dominate the flow.

Advanced settings:

- seed
- model
- steps
- strength
- negative prompt
- palette mode
- outline behavior
- dithering
- transparency
- reference strength
- variation strength

---

## 15.6 Generation Results

AI results should be visual and comparable.

Each result card should show:

- thumbnail
- result number
- seed
- favorite/star
- palette warning if any
- selected state
- source recipe

Actions:

```text
Use selected
Insert as new sprite
Place as layer
Create variations
Regenerate
Save to assets
```

---

## 16. AI Composition Library

## 16.1 Current Strength

The existing AI Studio / Composition Library is a very strong idea.

It includes:

- Templates
- Structures
- Styles
- Built-in items
- Custom user items
- Variables
- Prompt text
- Default structure
- Default style

This is more than prompt management. It is a recipe system.

---

## 16.2 Recommended Framing

Use this language:

```text
Composition Library
Recipes
Templates
Structures
Styles
Variables
Coverage
```

Recommended explanation:

> Templates define the subject. Structures define the output. Styles define the visual treatment.

---

## 16.3 Templates

Templates answer:

> What are we making?

Examples:

- Bit — idle
- Bit — walk
- Bit — run
- Bit — jump
- Bit — attack
- Bit — hurt
- Bit — fall
- Byte — companion bot
- Circuit-grid — tileset

---

## 16.4 Structures

Structures answer:

> What form should the output take?

Examples:

- Character
- Custom
- Item
- Single image
- Tall object
- Tileset
- Wide object

---

## 16.5 Styles

Styles answer:

> How should it look?

Examples:

- Clean HD
- Default
- Map style
- Pixel art
- Pixel inspired
- Retro pixel

---

## 16.6 Composition Library Layout

Recommended layout:

```text
Left: Recipe browser
Center: Preview / details / test generations
Right: Inspector
```

Example:

```text
┌────────────────────┬───────────────────────────────┬────────────────────┐
│ Composition Library │ Preview                       │ Inspector          │
│                    │                               │                    │
│ Templates          │ Bit — jump                    │ Name               │
│ Structures         │ [last generated thumbnails]    │ Prompt             │
│ Styles             │                               │ Variables          │
│ Coverage           │ Used by: Bit sprite            │ Defaults           │
│                    │ Coverage: 7/9 animations       │                    │
│ Bit — idle         │                               │ [Save] [Duplicate] │
│ Bit — jump         │ [Test generate]                │                    │
│ Bit — attack       │                               │                    │
└────────────────────┴───────────────────────────────┴────────────────────┘
```

---

## 16.7 Add Preview Cards

The Composition Library should not be a pure table.

Add thumbnails to:

- templates
- styles
- structures
- recent test generations

Example template row:

```text
[ thumbnail ] Bit — jump
              Animation · built-in · uses: Bit mascot
```

Example style row:

```text
[ thumbnail ] Retro pixel
              hard edges · limited palette · 8-bit contrast
```

---

## 16.8 Variable Chips

Variables should be visual chips.

Example:

```text
Variables
[ character ] [ pose ] [ expression ] [ equipment ] [+ Add]
```

Inside the prompt editor, variables should be highlighted:

```text
{character} performs a {pose} with a {expression} expression
```

---

## 16.9 Built-In vs User Recipes

Built-ins should feel like locked factory presets.

Recommended behavior:

- built-ins are read-only
- primary action is `Duplicate to edit`
- user recipes can be edited directly
- modified recipes show unsaved state
- reverted recipes show clear confirmation

Visual states:

```text
Built-in      locked badge
User          editable badge
Modified      dot indicator
Deprecated    warning badge
```

---

## 16.10 Test Generation Inside Library

When editing a recipe, user should be able to test it immediately.

Right panel or center preview:

```text
Test Generate
[ Generate Preview ]
[ Generate More ]
[ Use in Current Sprite ]
```

This makes the library feel creative, not administrative.

---

## 17. Coverage System

## 17.1 Why Coverage Matters

Coverage could become one of Pixhaus’ strongest differentiators.

It turns AI generation into a game-production workflow.

Instead of asking users to manually remember which animations or assets they need, Pixhaus can show what exists and what is missing.

---

## 17.2 Animation Coverage Example

For a character sprite:

```text
Bit

Animations
✓ idle
✓ walk
✓ run
✓ jump
✓ fall
✓ attack
✓ hurt
✕ death
✕ climb
✕ interact

Coverage: 7 / 10

[ Generate missing ]
```

---

## 17.3 Asset Coverage Example

For a platformer character:

```text
Required Set: Platformer Character

Required:
✓ idle
✓ walk
✓ run
✓ jump
✓ fall
✓ land
✓ attack
✕ crouch
✕ climb
✕ death

Optional:
✕ wall slide
✕ dash
✕ celebrate
```

---

## 17.4 Tileset Coverage Example

For a tileset:

```text
Required Set: 3x3 Terrain

✓ center
✓ top
✓ bottom
✓ left
✓ right
✓ corners
✕ inner corners
✕ transitions

[ Generate missing tiles ]
```

---

## 17.5 Coverage Actions

Coverage panel actions:

```text
Generate missing
Mark as complete
Ignore item
Add custom requirement
Create requirement set
Export checklist
```

---

## 17.6 Coverage As UX Bridge

Coverage serves both target audiences:

- non-artists get guidance on what to generate
- artists get production checklists
- teams get consistency across assets
- AI gets a structured generation target

---

## 18. Asset Browser

## 18.1 Purpose

Pixhaus should help users manage a project, not just one sprite.

The asset browser should include:

```text
Sprites
Animations
Tilesets
Props
Items
UI
Effects
Palettes
References
Generated
Favorites
Recent
```

---

## 18.2 Asset Browser Layout

Recommended layout:

```text
Left: category tree
Center: asset grid
Right: asset inspector
```

Example asset card:

```text
[ thumbnail ]
bit_jump
Animation · 15 frames · 512x512 · 12 FPS
```

---

## 18.3 AI Results Become Assets

Generated outputs should not disappear after use.

They should be storable as:

- candidate
- favorite
- rejected
- inserted
- source of variation

This creates a useful production memory.

---

## 19. AI Assistant Behavior

## 19.1 AI Levels

AI should exist at three levels.

### Level 1: Invisible Assistance

Quiet QA and suggestions.

Examples:

- stray pixel detection
- color count warning
- palette mismatch warning
- seam detection
- jitter detection
- non-looping animation warning

### Level 2: Contextual Actions

Appears based on selection/context.

Examples:

- Fill selection
- Clean selected area
- Generate in-betweens
- Fix seams
- Create variations

### Level 3: Full Generation

Explicit Generate workspace.

Examples:

- Prompt-to-sprite
- Prompt-to-animation
- Prompt-to-tileset
- Generate missing coverage

---

## 19.2 AI Action Naming

Use user-goal language, not model language.

Prefer:

- Generate sprite
- Create variations
- Clean up
- Reduce colors
- Make seamless
- Generate in-betweens
- Extend animation
- Match palette

Avoid making model details primary:

- Run model
- Invoke pipeline
- Diffuse
- Sampler
- Steps
- CFG

Those can live in advanced settings.

---

## 19.3 AI Markers

Use a consistent sparkle marker for AI actions:

```text
✦ Generate
✦ Variations
✦ Clean up
✦ In-between
```

But keep it visually restrained.

---

## 20. Command Palette

## 20.1 Purpose

A command palette gives Pixhaus professional speed.

Shortcut:

```text
Ctrl/Cmd + K
```

---

## 20.2 Command Examples

```text
Generate sprite from prompt
Create variations from selection
Clean selected pixels
Reduce sprite to current palette
Generate in-between frames
Make selected tile seamless
Export selected animation
Open Composition Library
Create new animation clip
Show coverage
```

---

## 20.3 Context Awareness

Command palette should know current context.

Examples:

- If region selected: prioritize selection actions
- If animation clip selected: prioritize animation actions
- If tile selected: prioritize tile actions
- If Generate workspace: prioritize recipes and prompts

---

## 21. Context Menus

Right-click menus are essential in native creative tools.

---

## 21.1 Canvas Selection Context Menu

```text
Cut
Copy
Paste
Transform

AI Fill Selection
Create Variations
Clean Up
Reduce to Palette
Add Lighting

Save Selection as Sprite
Export Selection
```

---

## 21.2 Timeline Clip Context Menu

```text
Rename Clip
Set FPS
Set Looping
Duplicate Clip
Export Clip

Generate In-betweens
Extend Animation
Create Variations
Clean Frames
Regenerate From Recipe
```

---

## 21.3 Palette Color Context Menu

```text
Set Foreground
Set Background
Find Usage
Replace Color
Lock Color
Generate Ramp
Generate Harmony
Remove Color
```

---

## 21.4 Asset Context Menu

```text
Open
Duplicate
Rename
Create Variation
Generate Missing Animations
Reveal in Project
Export
Delete
```

---

## 22. Right Panel Visual Redesign

## 22.1 Current Right Panel Content

Current panel includes:

- Palette
- Ramp
- Harmony
- Pages
- Animation
- Import / Export
- Layers
- Sprites
- Props
- Autotile

This is powerful but too many unrelated concepts live in one column.

---

## 22.2 Recommended Panel Organization

Use either:

### Option A: Workspace-specific panels

Each workspace decides what the right panel shows.

### Option B: Inspector tabs

```text
Inspector | Palette | Layers | Assets | AI
```

For native density, Option A is probably better, with optional tabs inside the right panel.

---

## 22.3 Visual Improvements

- stronger section headers
- more whitespace inside groups
- clearer active item rows
- reduce always-visible prompt fields outside Generate workspace
- move props/autotile into Generate or Tiles workspace
- make AI Assistant a distinct panel
- use icon + title headers

---

## 23. AI Prompt Templates Visual UX

## 23.1 Current Prompt Template UX

The current system is already strong:

- templates
- structures
- styles
- variables
- built-ins
- project-level recipes

The visual goal is to make it feel creative and reusable.

---

## 23.2 Recommended UX Copy

Use this explanation in the Composition Library:

```text
Saved prompts, structures, and styles for this project.
Built-ins are read-only — duplicate one to make it yours.
```

And this conceptual explanation:

```text
Templates define the subject.
Structures define the output.
Styles define the visual treatment.
```

---

## 23.3 Recipe Detail Panel

For a selected template:

```text
Name
Bit — jump

Type
Animation Template

Prompt
Bit, the Pixhaus mascot — a small retro robot...

Variables
[ character ] [ pose ] [ expression ] [ equipment ]

Defaults
Structure: Single image
Style: Pixel art
Palette: Current project palette

Usage
Used by: Bit sprite
Last used: Today
Coverage: 1 / 1 sprite

Actions
[Test Generate] [Duplicate] [Save]
```

---

## 23.4 Prompt Compilation Preview

Show the resolved prompt:

```text
Compiled Prompt
Bit, the Pixhaus mascot — a small retro robot with a boxy CRT head, jumping upward, happy expression, knees tucked, arms raised...
```

This helps users debug recipes.

---

## 24. Animation + AI Recipe Integration

## 24.1 Key Opportunity

Timeline clips and AI templates should be connected.

Example:

- timeline clip: `jump`
- source recipe: `Bit — jump`
- style: `Pixel art`
- structure: `Single image`
- generated frames: `8–15`
- manual edits: `frames 10, 11`

This creates reproducibility and trust.

---

## 24.2 Clip Inspector

When selecting an animation clip:

```text
Clip: jump
Frames: 8–15
FPS: 12
Loop: false

Source
Recipe: Bit — jump
Style: Pixel art
Structure: Single image
Seed: 123456

Manual edits
Frames 10, 11, 12

Actions
[Regenerate safely]
[Create variations]
[Extend]
[Clean]
[Export]
```

---

## 24.3 Protect Manual Edits

If a user manually edits generated frames, Pixhaus should preserve that work.

When regenerating:

```text
This clip has manual edits on 3 frames.

[ Preserve edited frames ] [ Regenerate all ] [ Cancel ]
```

This is essential for artist trust.

---

## 25. Non-Destructive AI Workflow

## 25.1 AI Result States

Generated content should have state:

- generated
- selected
- inserted
- edited
- accepted
- rejected
- saved as asset

---

## 25.2 Safe Apply Options

When applying AI output:

```text
Apply as:
[ New sprite ]
[ New layer ]
[ Replace selection ]
[ New frame range ]
[ New animation clip ]
```

---

## 25.3 AI History

AI history should store:

- prompt
- compiled prompt
- template
- structure
- style
- palette
- seed
- model
- source selection
- output size
- created assets
- whether output was edited

Actions:

```text
Reuse prompt
Regenerate
Create variations
Copy seed
Open source asset
```

---

## 26. Visual Language Details

## 26.1 Color Palette

Suggested UI palette direction:

| Role | Suggested Direction |
|---|---|
| App background | near-black warm slate |
| Panel background | dark charcoal/slate |
| Elevated panel | slightly lighter slate |
| Borders | low-contrast cool gray |
| Text primary | soft white |
| Text secondary | muted gray |
| Accent | Pixhaus violet |
| Success | muted green |
| Warning | muted amber |
| Error | muted red |
| AI marker | violet sparkle |

Avoid oversaturated neon except for intentional selection states.

---

## 26.2 Spacing

Use compact native spacing, but avoid cramped sections.

Recommended:

- tool buttons: compact
- panel groups: slightly more padding
- timeline cells: clear hit areas
- side panel rows: readable selected state
- bottom tray: enough vertical height to show meaningful content

---

## 26.3 Typography

Use a crisp UI font with high legibility.

Style:

- small but readable labels
- stronger section headers
- monospace only for values/code/prompt snippets if needed
- avoid overly playful fonts

---

## 26.4 Icons

Icons should have:

- consistent stroke width
- consistent size
- clear active/disabled states
- tooltips with shortcuts

AI actions should share a consistent sparkle icon.

---

## 27. Status Bar

## 27.1 Current Status Bar

Current status includes things like:

- Theme: System
- backend ready
- sprite size
- frames
- zoom

This is useful.

---

## 27.2 Recommended Status Bar

Make it compact and professional:

```text
64×64 | 15 frames | 1600% | Pixel Grid On | Onion Skin Off | Snap Off | AI Ready | Console
```

Move `backend ready` into a small status indicator:

```text
● AI Ready
```

Instead of bright text.

---

## 28. Export UX

## 28.1 Export Should Be Production-Oriented

Export workspace should support:

- PNG
- spritesheet
- GIF
- APNG
- JSON metadata
- engine presets
- per-animation export
- trimmed frames
- padding
- spacing
- pivot/origin
- hitbox/hurtbox metadata eventually

---

## 28.2 Engine Presets

Potential presets:

```text
Generic PNG + JSON
Godot
Unity
Unreal Paper2D
Phaser
PixiJS
Love2D
Bevy
Custom
```

---

## 28.3 Export QA

Before export, show warnings:

```text
✓ All frames same size
✓ Transparent background
✓ Palette under 32 colors
⚠ Animation “jump” does not loop
⚠ Sprite has 2 missing expected animations
```

Actions:

```text
Fix automatically
Ignore warning
Open issue
```

---

## 29. Docking and Customization

## 29.1 Blender-Inspired Docking

Eventually, Pixhaus should support:

- resizable panels
- collapsible sidebars
- saved layouts
- workspace-specific panel arrangements
- detachable panels if feasible

This increases professional credibility.

---

## 29.2 Layout Presets

Recommended presets:

```text
Default
Compact
Animation Focus
Palette Focus
Generate Focus
Single Monitor
Wide Monitor
```

---

## 30. Keyboard Shortcuts

## 30.1 Importance

Professional artists and animators rely heavily on shortcuts.

Every major action should have shortcuts and tooltips.

---

## 30.2 Suggested Shortcut Areas

- tool selection
- frame navigation
- playback
- onion skin toggle
- grid toggle
- palette actions
- layer actions
- generate variations
- command palette
- switch workspace

---

## 30.3 Workspace Switching

Example:

```text
Ctrl/Cmd + 1  Draw
Ctrl/Cmd + 2  Animate
Ctrl/Cmd + 3  Tiles
Ctrl/Cmd + 4  Generate
Ctrl/Cmd + 5  Export
```

---

## 31. Manual Artist Trust Rules

These rules should be treated as product principles.

### Rule 1: Never overwrite manual edits silently

Always warn or apply to new layer/sprite/frame range.

### Rule 2: Always show AI provenance

Users should know which frames/assets came from AI.

### Rule 3: Always preserve editability

AI output becomes normal pixel data after acceptance.

### Rule 4: Palette discipline matters

AI should respect palette constraints.

### Rule 5: The app must be useful without AI

Manual editor quality is non-negotiable.

---

## 32. Game Developer Guidance Rules

These rules help non-artists.

### Rule 1: Ask what they are making, not which model they want

Use asset types and game concepts.

### Rule 2: Provide sensible presets

Character, enemy, prop, tileset, icon, platformer, RPG, etc.

### Rule 3: Show coverage

Tell them what animations/assets are missing.

### Rule 4: Offer production export

Help them get assets into the game engine.

### Rule 5: Explain advanced art terms gently

Use tooltips and progressive disclosure.

---

## 33. Suggested MVP Visual Improvements

These are high-impact improvements that do not require inventing new systems.

### 33.1 Improve Region Separation

- stronger top bar hierarchy
- more distinct canvas stage
- panel background differences
- clearer bottom timeline region

### 33.2 Improve Active States

- selected workspace
- selected tool
- selected frame
- selected layer
- selected sprite
- selected generated result

### 33.3 Improve Timeline Visual Hierarchy

- playback band
- animation clips band
- frame ruler band
- layer tracks band
- stronger playhead

### 33.4 Make AI Studio More Visual

- add thumbnails to templates
- add preview/test generation area
- show variables as chips
- show compiled prompt preview

### 33.5 Add AI Results Tray

- thumbnail grid
- selected result
- use selected
- insert as new sprite
- create variations

---

## 34. Medium-Term Product Improvements

### 34.1 Contextual AI Assistant

Add right-panel assistant that changes by workspace.

### 34.2 Asset Browser

Add project asset browser with sprites, animations, props, tilesets, palettes, references, generated assets.

### 34.3 Coverage Panel

Add coverage for characters, animations, tilesets, and asset packs.

### 34.4 Recipe-to-Timeline Integration

Link animation clips to AI recipes.

### 34.5 Export Workspace

Build proper export QA and game-engine metadata.

---

## 35. Long-Term Differentiators

### 35.1 Project-Level Art Direction

Project style memory:

- palette
- outline rules
- shading rules
- sprite proportions
- animation style
- material language

### 35.2 Style Lock

AI can generate new assets while matching the project’s established style.

### 35.3 Batch Generation

Generate a whole character pack:

```text
Character: Forest Goblin
Animations: idle, walk, run, attack, hurt, death
Style: Dark fantasy 32x32
Palette: Forest Dungeon
```

### 35.4 Manual-AI Hybrid Animation

Artist draws keyframes. AI drafts in-betweens. Artist polishes.

### 35.5 QA Assistant

AI checks:

- readability
- silhouette
- palette drift
- inconsistent lighting
- animation jitter
- tile seams
- missing coverage

---

## 36. Screen-Specific Recommendations

## 36.1 Main Editor Screen

Current strengths:

- clean canvas
- compact tools
- palette panel
- layer/sprite structure
- native density

Recommended changes:

- make canvas stage more framed
- move AI props/autotile into Generate/Tiles context
- improve right panel grouping
- strengthen selected tool state
- make bottom tray more meaningful
- reduce visual dominance of debug/status text

---

## 36.2 Animate Screen

Current strengths:

- strong timeline foundation
- animation tags already exist
- frame/layer grid is promising
- playback controls exist

Recommended changes:

- transform tags into clips
- stronger playhead
- clearer layer tracks
- thumbnails or state markers in cells
- right panel focused on clip/frame properties
- AI assistant for animation-specific actions

---

## 36.3 AI Studio / Composition Library Screen

Current strengths:

- templates/structures/styles architecture
- built-in recipe system
- variables
- default structure/style
- project library concept

Recommended changes:

- add thumbnails/previews
- add center preview/test area
- make right inspector richer
- add compiled prompt preview
- make built-ins visibly read-only
- make Coverage a real panel
- visually connect recipes to assets and timeline clips

---

## 37. Proposed Navigation Model

Top-level workspace tabs:

```text
Draw | Animate | Tiles | Generate | Export
```

Within Generate:

```text
Create | Results | Recipes | Coverage | History
```

Within Recipes / Composition Library:

```text
Templates | Structures | Styles
```

Within Asset Browser:

```text
Sprites | Animations | Tilesets | Props | Palettes | References | Generated
```

This creates a clean information architecture.

---

## 38. Terminology Recommendations

## 38.1 Prefer

- Generate
- Composition Library
- Recipes
- Templates
- Structures
- Styles
- Coverage
- Variations
- Clean up
- Make seamless
- In-between frames
- Extend animation
- Use selected
- Insert as new sprite

## 38.2 Avoid As Primary Labels

- AI model
- sampler
- inference
- pipeline
- CFG
- steps
- tensor
- backend

These can exist in advanced/debug settings.

---

## 39. Possible Product Taglines

```text
Sprite creation for the AI-native era.
```

```text
A craft-first sprite studio with AI-native acceleration.
```

```text
Create, animate, and generate game-ready sprites.
```

```text
Manual when you want control. AI when you want leverage.
```

```text
A professional sprite editor for artists and game developers.
```

Best internal product principle:

```text
Manual-first. AI-assisted. Artist-respecting.
```

---

## 40. Recommended Design North Star

Pixhaus should become:

> **A native sprite production studio with a craft-first editor, a timeline built for game animation clips, and an AI composition library for reusable generation recipes.**

This is the strongest product shape observed from the current UI.

The three pillars are:

```text
1. Manual Sprite Editor
2. Animation Timeline
3. AI Composition Library
```

Everything in the visual design should reinforce those pillars.

---

## 41. Practical Implementation Sequence

## Phase 1: Visual Hierarchy Pass

- stronger workspace tabs
- clearer top/tool bars
- canvas frame and stage
- improved active states
- panel header styling
- better status bar

## Phase 2: Timeline Polish

- timeline bands
- animation clips
- stronger playhead
- frame cell states
- clip inspector

## Phase 3: Generate Workspace

- guided generation cards
- result grid
- AI result tray
- palette behavior controls
- advanced settings collapse

## Phase 4: Composition Library Upgrade

- thumbnails
- preview/test area
- variable chips
- compiled prompt preview
- built-in/user visual states

## Phase 5: Coverage + Asset Browser

- animation coverage
- asset browser categories
- generated asset states
- missing animation generation

## Phase 6: Deep Integration

- recipe-to-timeline linking
- protect manual edits
- AI provenance
- project art direction memory
- export QA

---

## 42. Final Product Principle

Pixhaus should not ask users to choose between manual craft and AI generation.

It should let them move fluidly between both.

For artists:

> “I draw. Pixhaus helps me move faster.”

For developers:

> “I describe what I need. Pixhaus helps me get usable game assets.”

For both:

> **Pixhaus keeps the work editable, understandable, and production-ready.**

That is the visual and UX direction.
