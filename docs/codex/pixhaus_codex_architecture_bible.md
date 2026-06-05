# Pixhaus Codex Architecture Bible

**Document status:** Product architecture / UX architecture / implementation guidance  
**Scope:** Pixhaus Codex, Codex Workspace, `@` references, anchors, and Generate Workspace integration  
**Audience:** Product, design, engineering, AI/workflow agents, future contributors  
**Core idea:** The Codex is the creative knowledge backbone of a Pixhaus project.

---

## 1. Executive Summary

Pixhaus should not treat AI prompting as a flat list of prompt templates. That is too small for the kind of product Pixhaus is becoming.

The right abstraction is the **Codex**.

The Codex is a project-level creative bible where the user can define everything that matters for a game world's visual production:

- characters
- enemies
- NPCs
- creatures
- props
- weapons
- items
- materials
- palettes
- styles
- vibes
- factions
- locations
- biomes
- animations
- poses
- UI sprite systems
- particles and VFX
- rules of the world
- scale conventions
- visual constraints
- lore and personality notes
- reference images
- generation anchors
- reusable AI recipes

The Codex becomes the backbone of the Generate workspace, but it is more than an AI feature. It is a **production knowledge system** for visual consistency.

The Codex should let users write prompts like:

```text
Generate @bit doing @animation.jump in @style.clean_hd with @palette.moonlit_ruins
```

or:

```text
Create 8 props for @location.fungal_catacombs using @material.mossy_stone and @vibe.cozy_dark_fantasy
```

The `@` reference system should turn Codex entries into reusable, typed, inspectable prompt context. When a user references `@bit`, Pixhaus should know that this means a specific character with canonical proportions, personality, palette preferences, visual anchors, animation coverage, and style rules.

The Codex should support both major Pixhaus audiences:

1. **Game developers with little artistic knowledge**  
   They use the Codex to define their world once, then generate consistent assets from it.

2. **Experienced artists**  
   They use the Codex as an art bible, production checklist, consistency tool, and AI assistant that respects their manual work.

The Codex should be treated as a first-class project system, with its own workspace, data model, validation, versioning, anchors, references, and integration with AI generation.

---

## 2. Product Philosophy

### 2.1 The Codex is not a prompt library

A prompt library is usually just:

```text
Name → Prompt text
```

That is not enough for Pixhaus.

Pixhaus needs:

```text
World knowledge → structured references → generation context → production assets
```

The Codex is closer to:

- a game art bible
- a lore bible
- a visual direction system
- a style guide
- an asset registry
- a generation memory layer
- a project-specific knowledge graph

Raw prompts are still useful, but they should be compiled from structured Codex entries, not treated as the source of truth.

### 2.2 The artist remains in control

The Codex should never become a system where AI silently decides the world. It should help the user define, preserve, and reuse creative intent.

The rule is:

> **The Codex remembers. AI proposes. The artist decides.**

This means:

- Codex entries are editable by the user.
- AI can suggest entries, but canonical entries require user approval.
- Generated results can reference Codex entries, but should not mutate them automatically.
- AI should expose what Codex context was used.
- Users should be able to lock visual anchors, palettes, proportions, and style rules.

### 2.3 The Codex supports many art styles

Pixhaus is not only a pixel art tool.

Pixhaus is a sprite creation and animation tool that can support many visual styles:

- pixel art
- high-definition hand-painted sprites
- painterly fantasy sprites
- clean cartoon sprites
- anime-inspired sprites
- vector-like UI sprites
- retro-inspired sprites
- low-res chunky game art
- dark fantasy sprites
- sci-fi sprites
- cozy game sprites
- stylized VFX sprites

Pixel art deserves dedicated tooling, but it should be a mode/style/tooling layer inside a broader sprite production system.

The Codex should therefore support style-specific constraints, including but not limited to pixel art constraints.

---

## 3. Core Definition

### 3.1 What is the Codex?

The Codex is a project-level database of creative entities and rules.

A Codex entry can represent:

```text
Character
Creature
NPC
Enemy
Prop
Item
Weapon
Material
Palette
Style
Vibe
Location
Biome
Faction
Animation
Pose
Effect
UI Element
Rule
Lore Note
Reference Board
Generation Recipe
Anchor
```

Each entry can contain:

```text
Name
Stable ID
Aliases
Type
Description
Lore
Visual description
Prompt fragments
Negative prompt fragments
Reference images
Color palettes
Style constraints
Animation notes
Scale rules
Relationships
Tags
Status
Version history
Generation history
Anchors
```

### 3.2 The Codex as a graph

The Codex should be modeled as a graph, not a flat list.

Examples:

```text
@bit is a Character
@bit belongs to @faction.pixhaus_team
@bit uses @palette.bit_default
@bit supports @animation.idle, @animation.walk, @animation.jump
@bit often appears in @vibe.playful_arcade
@bit has companion @byte
```

Or:

```text
@fungal_catacombs is a Location
@fungal_catacombs uses @palette.moss_and_amber
@fungal_catacombs contains @material.mossy_stone
@fungal_catacombs contains @prop.cracked_pillar
@fungal_catacombs has vibe @vibe.cozy_dark_fantasy
```

This relationship model makes the Codex powerful because generation can pull relevant context from related entries.

### 3.3 The Codex as a reference resolver

When the user types:

```text
@bit
```

Pixhaus should resolve it to a stable Codex entry, not merely insert text.

The display name can change. The internal reference should remain stable.

This means:

```text
@bit → codex_entry_id: character.bit.7f3a...
```

The user sees a friendly chip. The system stores a stable reference.

---

## 4. Codex Entry Types

The Codex should support typed entries. Types help the UI, prompt compiler, validation, and Generate workspace understand how an entry should be used.

### 4.1 Character

Represents a playable character, NPC, enemy character, mascot, companion, or named entity.

Recommended fields:

```text
Name
Aliases
Short description
Lore description
Personality
Role in game
Visual identity
Silhouette notes
Proportions
Scale
Default outfit
Variant outfits
Facial features
Hair / head features
Body shape
Signature props
Color palette
Style anchors
Animation set
Allowed styles
Forbidden traits
Reference images
Generated examples
Canonical sprite assets
```

Example use:

```text
@character.bit
@bit
```

Generation meaning:

> Use the canonical identity, proportions, visual anchors, palette, and animation rules for Bit.

### 4.2 Creature

For monsters, animals, fantasy beings, familiars, bosses, pets.

Recommended fields:

```text
Species name
Anatomy
Movement style
Silhouette
Size category
Threat level
Material/skin/fur/scales
Palette
Biome
Behavior
Attack types
Idle behavior
Animation needs
```

### 4.3 Prop

For environmental objects and interactive props.

Recommended fields:

```text
Name
Category
Scale
Material
World usage
Shape language
Damage/wear rules
Palette
Style
Allowed variations
Tile compatibility
Collision expectation
```

Examples:

```text
@prop.cracked_pillar
@prop.torch
@prop.wooden_crate
```

### 4.4 Item

For collectables, inventory items, powerups, icons, loot.

Recommended fields:

```text
Name
Gameplay role
Rarity
Icon silhouette
Sprite scale
Material
Glow/effect rules
Palette
UI representation
World representation
Animation notes
```

### 4.5 Weapon

Could be a specialized Item or Prop, but it may deserve its own type because weapons often need animation compatibility.

Recommended fields:

```text
Name
Weapon class
Grip point
Swing arc
Scale relative to character
Material
Damage type
Icon version
World version
Animation compatibility
VFX references
```

### 4.6 Material

Defines surface appearance.

Recommended fields:

```text
Name
Base colors
Texture description
Detail frequency
Damage rules
Lighting response
Pixel art constraints if applicable
Tile compatibility
Examples
```

Examples:

```text
@material.mossy_stone
@material.rusted_iron
@material.glowing_crystal
```

### 4.7 Palette

Defines a color system.

A palette can be:

```text
Global project palette
Character palette
Biome palette
Material palette
UI palette
Temporary generation palette
Pixel-art indexed palette
Mood palette
```

Recommended fields:

```text
Name
Colors
Color roles
Ramp definitions
Usage rules
Locked colors
Optional colors
Generated colors policy
Compatible styles
Compatible locations
```

Palette roles matter:

```text
shadow
midtone
highlight
outline
skin
cloth
metal
magic glow
danger
healing
UI accent
```

### 4.8 Style

Defines how art should look.

Recommended fields:

```text
Name
Visual description
Rendering rules
Line treatment
Detail level
Lighting rules
Texture rules
Resolution expectations
Anti-aliasing rules
Palette behavior
Shading method
Reference images
Negative prompt rules
Compatible output types
```

Examples:

```text
@style.pixel_art_32
@style.clean_hd_sprite
@style.hand_painted_fantasy
@style.retro_rpg
@style.cartoon_vector_like
```

### 4.9 Vibe

A Vibe is not exactly a style. It is mood, atmosphere, and creative feeling.

Examples:

```text
@vibe.cozy_dark_fantasy
@vibe.lonely_ruins
@vibe.arcade_playful
@vibe.neon_melancholy
@vibe.sunny_adventure
```

Recommended fields:

```text
Mood description
Lighting mood
Color tendency
Shape tendency
Emotional keywords
Forbidden mood drift
Compatible locations
Compatible music/sound references if useful
```

### 4.10 Location

For areas, levels, towns, dungeons, biomes, rooms.

Recommended fields:

```text
Name
Lore
Function in game
Biome
Architecture
Materials
Lighting
Palette
Props commonly found
Characters commonly found
Hazards
Tile rules
VFX rules
```

### 4.11 Biome

A reusable natural/environmental category.

Examples:

```text
@biome.fungal_caves
@biome.ashen_forest
@biome.floating_islands
```

Recommended fields:

```text
Terrain materials
Vegetation
Lighting
Color tendencies
Common props
Hazards
Ambient particles
Tile material rules
```

### 4.12 Faction

For groups, teams, races, organizations, kingdoms.

Recommended fields:

```text
Name
Lore
Members
Symbol
Shape language
Color identity
Clothing/material rules
Architecture style
Prop style
UI/emblem assets
```

### 4.13 Animation

Defines an animation concept, not necessarily a concrete clip.

Examples:

```text
@animation.idle
@animation.walk
@animation.run
@animation.jump
@animation.attack_light
@animation.hurt
@animation.death
@animation.cast_spell
```

Recommended fields:

```text
Name
Purpose
Looping behavior
Recommended frame count
Timing
Pose beats
Motion arcs
Silhouette rules
Secondary motion
Style-specific notes
Character compatibility
AI generation hints
```

This is critical for Generate and Animate integration.

### 4.14 Pose

A specific body pose or action beat.

Examples:

```text
@pose.ready_stance
@pose.jump_apex
@pose.attack_windup
@pose.hit_recoil
```

Fields:

```text
Description
Body orientation
Limb placement
Silhouette notes
Camera angle
Applicable characters
Reference images
```

### 4.15 Effect / VFX

For sprite-based visual effects.

Examples:

```text
@vfx.spark_burst
@vfx.healing_glow
@vfx.slash_arc
@vfx.smoke_puff
```

Fields:

```text
Effect type
Frame count
Timing
Color palette
Blend mode expectation
Scale
Looping behavior
Attachment point
Compatible weapons/actions
```

### 4.16 UI Element

For game UI sprites.

Examples:

```text
@ui.button_primary
@ui.health_icon
@ui.dialog_frame
@ui.inventory_slot
```

Fields:

```text
Component role
States
Nine-slice rules
Scale rules
Palette
Text compatibility
Icon rules
Export constraints
```

### 4.17 Rule

Rules define constraints that should apply across assets.

Examples:

```text
@rule.no_realistic_gore
@rule.readable_at_32px
@rule.all_characters_face_right_by_default
@rule.no_extra_colors_in_pixel_mode
@rule.transparent_background_for_sprites
```

Fields:

```text
Rule statement
Scope
Severity
Applies to entry types
Validation method
Prompt fragment
Negative prompt fragment
```

### 4.18 Reference Board

A group of visual/lore references.

Fields:

```text
Name
Images
Notes
Tags
Referenced entries
Usage scope
```

### 4.19 Generation Recipe

A structured reusable recipe for creating assets.

Fields:

```text
Name
Template
Structure
Style
Variables
Default context
Required Codex references
Output expectations
Provider preferences
Validation rules
```

---

## 5. Anchors

Anchors are one of the most important Codex concepts.

An anchor is a stable creative reference that generation should preserve.

Anchors answer:

> “What must remain consistent?”

Examples:

```text
Bit's head shape
Bit's color palette
The silhouette of the main sword
The UI button style
The mossy stone material language
The walk cycle timing
The world’s cozy-dark tone
```

### 5.1 Why anchors exist

AI generation often drifts.

Common problems:

```text
character changes shape
palette changes unexpectedly
style becomes inconsistent
prop scale changes
animation does not match existing frames
materials look different between generations
```

Anchors are how Pixhaus controls drift.

### 5.2 Anchor types

#### Identity Anchor

Defines who or what an entity is.

Used for:

```text
characters
creatures
factions
signature props
bosses
mascots
```

Contains:

```text
canonical description
aliases
visual must-haves
forbidden changes
canonical references
```

#### Visual Anchor

Defines visual appearance.

Contains:

```text
reference images
silhouette
shape language
line treatment
proportions
detail density
```

#### Palette Anchor

Defines colors that must be respected.

Contains:

```text
required colors
optional colors
forbidden colors
palette expansion policy
color role mapping
```

#### Style Anchor

Defines rendering language.

Contains:

```text
shading style
edge treatment
anti-aliasing rules
texture language
lighting rules
camera/view rules
```

#### Animation Anchor

Defines motion consistency.

Contains:

```text
pose beats
timing
looping rules
motion arcs
feet/contact rules
silhouette rules
```

#### Scale Anchor

Defines size and proportion relationships.

Contains:

```text
sprite size
relative scale
grid size
world unit size
collision expectation
```

#### Lore Anchor

Defines story/personality/world constraints.

Contains:

```text
personality
role
faction
behavior
world logic
forbidden contradictions
```

#### Negative Anchor

Defines what must not happen.

Examples:

```text
Do not give Bit realistic human eyes.
Do not add extra limbs.
Do not use neon colors in the mossy ruins palette.
Do not make this prop symmetrical.
Do not add weapons to idle animations.
```

### 5.3 Anchor strength

Anchors should have strength levels.

```text
Loose       Influence only
Normal      Prefer strongly
Strong      Preserve unless impossible
Locked      Must not change
```

The Generate workspace should expose these as user-friendly controls:

```text
Reference strength: Loose / Normal / Strong / Locked
```

### 5.4 Anchors and manual artists

For artists, anchors are not “AI prompts.” They are art direction locks.

An artist may manually draw a character, mark the frame as canonical, then create an anchor from it.

Example:

```text
Right-click sprite → Create Codex Anchor → Identity Anchor for @bit
```

Then future AI operations can preserve the manually drawn work.

---

## 6. The `@` Reference System

The `@` reference system is how users connect prompts, recipes, and generation requests to the Codex.

### 6.1 Basic behavior

When typing in any Codex-aware prompt field, the user can type:

```text
@
```

Pixhaus opens autocomplete.

Example suggestions:

```text
@bit                         Character
@byte                        Character
@palette.moonlit_ruins       Palette
@style.pixel_art_32          Style
@animation.jump              Animation
@material.mossy_stone        Material
@location.fungal_catacombs   Location
```

Selecting an item inserts a reference chip.

### 6.2 Reference chips

A reference should not be plain text after insertion. It should become a chip/token with:

```text
icon
name
type color
dropdown
hover preview
stable ID
```

Examples:

```text
[@bit Character]
[@moonlit_ruins Palette]
[@jump Animation]
```

Hovering should show a mini-card:

```text
Bit
Character
Playable mascot character
Palette: Bit Default
Animations: idle, walk, jump, attack
Anchors: identity locked, palette strong
```

### 6.3 Typed namespaces

Users should be able to reference entries loosely or specifically.

Loose:

```text
@bit
```

Specific:

```text
@character.bit
@palette.moonlit_ruins
@animation.jump
@style.clean_hd_sprite
```

The typed form avoids ambiguity in large projects.

### 6.4 Aliases

Entries should support aliases.

Example:

```text
Canonical: @character.bit
Aliases: @bit, @mascot, @main_hero
```

Aliases are convenient, but compilation should resolve them to stable entry IDs.

### 6.5 Reference resolution

The prompt compiler resolves references in stages:

```text
1. Parse prompt text and chips
2. Resolve aliases to stable Codex IDs
3. Load referenced entries
4. Load related required anchors
5. Apply scope rules
6. Compile provider-ready prompt/context
```

### 6.6 Broken references

If an entry is deleted or renamed, references should not silently break.

The UI should show:

```text
@old_character_name could not be resolved
[Relink] [Remove] [Create new entry]
```

### 6.7 Deprecated references

Codex entries can be deprecated.

Example:

```text
@style.old_pixel_style is deprecated.
Suggested replacement: @style.pixel_art_32_v2
```

### 6.8 Reference scopes

Some references are project-wide. Others are local to a workspace, sprite, or asset.

Examples:

```text
Project scope: @style.clean_hd_sprite
Character scope: @bit.palette_default
Location scope: @fungal_catacombs.prop_set
Animation scope: @bit.animation.jump
```

The UI should make scope understandable without overwhelming users.

---

## 7. Prompt System Upgrade

The Codex does not remove prompts. It upgrades them.

### 7.1 Prompt input should become a Prompt Composer

A basic text box is not enough.

The Generate workspace and Codex workspace should use a Prompt Composer with:

```text
rich text chips for @ references
variable chips
prompt sections
negative prompt section
style/context sidebar
compiled preview
validation warnings
prompt history
```

### 7.2 Prompt layers

A final generation prompt should be composed from layers:

```text
User request
+ Codex references
+ selected recipe
+ style rules
+ structure rules
+ project rules
+ workspace context
+ provider-specific formatting
+ negative constraints
```

This means the user might type only:

```text
@bit jumping over a broken bridge in @location.fungal_catacombs
```

But Pixhaus compiles a much richer generation request.

### 7.3 Prompt compiler output

The compiled output should include:

```text
positive prompt
negative prompt
reference images
palette constraints
output structure
animation requirements
seed/provider settings
validation rules
metadata
```

### 7.4 Inspectable compiled prompt

Users should be able to inspect what Pixhaus is sending.

This is important for trust.

UI action:

```text
[Inspect Compiled Prompt]
```

Shows:

```text
User text
Resolved references
Included anchors
Style fragments
Negative constraints
Provider payload summary
```

### 7.5 Context budget

Some providers will have prompt/context limits. The Codex must support context budgeting.

Each entry field should have an inclusion priority:

```text
Critical
Important
Normal
Optional
Debug only
```

The compiler can include the most relevant pieces first.

### 7.6 Prompt modes

Different users want different amounts of control.

Recommended modes:

```text
Simple
Guided
Advanced
Debug
```

Simple:

```text
Describe what you want.
```

Guided:

```text
What are you making?
Which character?
Which style?
Which animation?
```

Advanced:

```text
Prompt composer with @ references, variables, anchors, structure settings.
```

Debug:

```text
Compiled prompt, provider payload, logs, reference resolution.
```

---

## 8. Codex Workspace

The Codex deserves its own workspace.

The Codex Workspace is where users define, organize, validate, and evolve the creative world bible.

### 8.1 Purpose

The Codex Workspace should help users answer:

```text
Who exists in this world?
What do they look like?
What styles are allowed?
What palettes define the world?
What animations does each character need?
What props/materials/locations exist?
What has already been created?
What is missing?
What should AI preserve?
```

### 8.2 Workspace layout concept

Recommended layout:

```text
Left: Codex Navigator
Center: Entry Editor / Board / Graph
Right: Inspector / Anchors / References
Bottom: Coverage / Generation Tests / History
```

More concretely:

```text
┌──────────────────────┬──────────────────────────────────────────────┬──────────────────────┐
│ Codex Navigator       │ Entry Editor / Visual Board                  │ Inspector             │
│                      │                                              │                      │
│ Search               │ @bit                                         │ Type: Character       │
│ Filters              │ Description                                  │ Status: Canonical     │
│ Entry Types          │ Lore                                         │ Anchors               │
│ Tags                 │ Visual Identity                              │ Relations             │
│                      │ Reference Images                             │ Used By               │
│ @bit                 │ Animation Coverage                           │ Prompt Preview        │
│ @byte                │ Generated Examples                           │                      │
│ @mossy_stone         │                                              │                      │
│ @fungal_catacombs    │                                              │                      │
├──────────────────────┴──────────────────────────────────────────────┴──────────────────────┤
│ Coverage / Validation / Test Generation / History                                           │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.3 Codex Navigator

The navigator should support:

```text
search
filter by type
filter by tag
filter by status
favorites
recently edited
missing coverage
broken references
built-in vs project entries
```

Entry grouping:

```text
Characters
Locations
Props
Materials
Palettes
Styles
Animations
Rules
Recipes
References
```

### 8.4 Entry Editor

The center editor changes depending on entry type.

A Character editor should not look identical to a Palette editor.

Each entry type should have a specialized form, while still sharing common blocks:

```text
Overview
Visual Identity
Lore
Prompt Context
Anchors
Relationships
References
Generated Assets
Coverage
History
```

### 8.5 Visual Board Mode

Many artists think visually. The Codex should support a board-style view.

A board may contain:

```text
reference images
sprites
palette swatches
notes
generated variations
canonical examples
rejected examples
relationship pins
```

This is especially useful for:

```text
characters
locations
styles
vibes
factions
materials
```

### 8.6 Graph Mode

Graph Mode shows relationships between entries.

Example:

```text
@bit → uses → @palette.bit_default
@bit → belongs_to → @faction.pixhaus_team
@bit → supports → @animation.walk
@bit → appears_in → @location.workshop
@bit → companion_of → @byte
```

This graph should be useful, not decorative.

Actions:

```text
Find all assets using this palette
Find all characters missing attack animation
Find all props used in this location
Find all deprecated style references
```

### 8.7 Coverage Mode

Coverage Mode tracks what exists and what is missing.

For a character:

```text
Animations
✓ idle
✓ walk
✓ run
✓ jump
✕ fall
✕ attack
✕ hurt
✕ death
```

For a location:

```text
Environment Pack
✓ ground tiles
✓ wall tiles
✓ props
✕ doors
✕ hazards
✕ ambient particles
✕ background layers
```

For UI:

```text
Button States
✓ normal
✓ hover
✕ pressed
✕ disabled
```

Coverage is one of the strongest bridges between Codex and Generate.

### 8.8 Test Generation Panel

The Codex Workspace should let users test entries without leaving the workspace.

Examples:

```text
Generate sample @bit idle frame
Generate 4 @material.mossy_stone variations
Generate @palette.moonlit_ruins prop preview
```

Test results should not modify canonical assets unless the user chooses to promote them.

Actions:

```text
Promote to reference
Save as generated example
Create anchor from result
Reject and remember as negative example
```

---

## 9. Generate Workspace Integration

The Generate workspace should be powered by the Codex.

### 9.1 Generate should start from project knowledge

Instead of asking only:

```text
What do you want to generate?
```

Generate should also ask:

```text
What Codex entries should this use?
```

Suggested Generate workflow:

```text
1. Choose output type
2. Choose Codex references
3. Choose style/structure
4. Add user prompt
5. Review anchors
6. Generate results
7. Apply or save
```

### 9.2 Context stack

The Generate workspace should show a Context Stack.

Example:

```text
Context Stack
- @bit                      Character, identity locked
- @animation.jump           Animation, normal strength
- @style.pixel_art_32       Style, strong
- @palette.bit_default      Palette, locked
- @rule.transparent_bg      Rule, locked
```

Users should be able to remove, reorder, or adjust strength.

### 9.3 Reference chips in Generate

The prompt composer should allow:

```text
@bit doing @animation.jump with @weapon.training_sword
```

This is natural and powerful.

### 9.4 Generate from coverage

The strongest Generate entry point may not be a blank prompt.

It may be:

```text
Generate missing assets from Codex coverage.
```

Examples:

```text
Generate missing animations for @bit
Generate missing props for @location.fungal_catacombs
Generate missing UI button states for @ui.button_primary
Generate missing material tiles for @material.mossy_stone
```

### 9.5 Generated result metadata

Every generated result should store:

```text
Codex references used
Anchor strengths
Compiled prompt
Provider
Seed
Generation settings
Output structure
Style
Palette behavior
Source coverage item
Time/date
User modifications after generation
```

This is important for reproducibility.

---

## 10. Relationship Between Codex, Recipes, and Prompts

The Codex should absorb and upgrade the earlier Prompt Library idea.

### 10.1 Old model

```text
Prompt Template
Structure
Style
Variables
```

### 10.2 New model

```text
Codex Entry
  can contain prompt fragments
  can contain anchors
  can contain relationships
  can contain style rules
  can contain output rules
  can be referenced with @
```

Templates, Structures, and Styles still exist, but they become Codex entry types or Codex subtypes.

### 10.3 Recommended naming

Use:

```text
Codex
Codex Entries
Recipes
Anchors
References
Coverage
Prompt Composer
```

Avoid overusing:

```text
Prompt Library
Prompt Templates
```

Those terms make the system sound smaller than it is.

### 10.4 Recipe as an executable Codex entry

A Recipe is a Codex entry that knows how to generate something.

Example:

```text
@recipe.character_walk_cycle
```

It may require:

```text
character
style
palette
frame count
direction
```

Then it outputs:

```text
animation frames
clip metadata
preview thumbnails
```

---

## 11. Data Model Principles

This document is not prescribing code, but the architecture should support these concepts.

### 11.1 Codex root

A project has one Codex.

The Codex contains:

```text
entries
relationships
anchors
reference assets
coverage plans
entry versions
indexes
```

### 11.2 Stable IDs

Every entry needs a stable ID.

Display names can change.

References should not break when display names change.

### 11.3 Human-readable handles

Every entry should have a handle.

Examples:

```text
bit
mossy_stone
fungal_catacombs
pixel_art_32
```

Handles power `@` mentions.

### 11.4 Entry status

Entries should have status.

Recommended statuses:

```text
Draft
Candidate
Canonical
Deprecated
Archived
Rejected
```

### 11.5 Entry ownership

Entries may be:

```text
Built-in
Project-defined
Imported pack
Generated suggestion
User-created
Team-shared
```

### 11.6 Entry locking

Important entries can be locked.

Lock types:

```text
Content lock
Visual anchor lock
Palette lock
Name/handle lock
Reference lock
```

### 11.7 Version history

Entries should keep version history because creative bibles evolve.

Minimum version metadata:

```text
version number
timestamp
author/source
change summary
previous version reference
```

### 11.8 Relationships

Relationships should be typed.

Examples:

```text
uses
belongs_to
appears_in
compatible_with
incompatible_with
inherits_from
variant_of
requires
contains
replaces
inspired_by
```

### 11.9 Field priorities

Fields should have prompt inclusion priority.

```text
Critical
Important
Normal
Optional
Never include in prompt
```

This lets the prompt compiler manage context size.

---

## 12. Codex Storage and Project Format Implications

Because Pixhaus projects may be asset-heavy, the Codex should not require loading all art assets into memory.

### 12.1 Codex metadata vs asset blobs

Codex metadata should be lightweight.

Reference images, generated examples, sprites, and large assets should be external blobs referenced by ID/hash.

Example conceptual layout:

```text
project.pixhaus/
  manifest.phxbin
  codex/
    codex_index.phxbin
    entries/
      character.bit.phxentry
      palette.moonlit_ruins.phxentry
      location.fungal_catacombs.phxentry
    relationships.phxbin
    coverage.phxbin
  assets/
    images/
    sprites/
    references/
    generated/
  thumbnails/
```

### 12.2 Lazy loading

Opening the Codex should load:

```text
entry metadata
names
handles
types
tags
relationships summary
thumbnail references
```

It should not immediately load:

```text
all reference images
all generated examples
all sprite blobs
all animation frames
```

Those should load on demand.

### 12.3 Search index

The Codex should maintain a search index for:

```text
names
aliases
tags
descriptions
handles
relationships
entry type
status
```

This makes `@` autocomplete fast.

### 12.4 Thumbnail cache

The Codex workspace will be visual. It needs thumbnails.

Thumbnails should be cached and regenerated in background jobs.

---

## 13. UX Details for `@` Mentions

### 13.1 Autocomplete behavior

When the user types `@`, autocomplete appears.

Ranking should consider:

```text
exact handle match
recently used entries
current workspace context
entry type expected by field
project favorites
canonical status
relationship relevance
```

Example: inside an animation field, `@ju` should rank `@animation.jump` above unrelated locations.

### 13.2 Type filtering

Users should be able to type:

```text
@character:
@palette:
@style:
@animation:
@location:
```

Then autocomplete filters to that type.

### 13.3 Fuzzy matching

Autocomplete should support fuzzy search.

Examples:

```text
@fung cat → @location.fungal_catacombs
@moss stone → @material.mossy_stone
@bit jump → @bit + @animation.jump suggestions
```

### 13.4 Multi-reference suggestions

Pixhaus can suggest bundles.

Example:

User types:

```text
@bit jump
```

Suggestion:

```text
Use @bit + @animation.jump + @palette.bit_default
```

### 13.5 Hover cards

Hover card should show:

```text
entry name
type
thumbnail
short description
anchors
status
used by
quick actions
```

Quick actions:

```text
Open in Codex
Pin to context
Change strength
Remove reference
```

### 13.6 Mention chips and text export

In the editor, mentions appear as chips.

When copied as plain text, they can become:

```text
@character.bit
```

When saved, they remain stable references.

---

## 14. Codex-Aware Prompt Composer UX

The Prompt Composer should be used in Generate, Codex, and eventually contextual AI panels.

### 14.1 Core features

```text
Rich text with @ chips
Variables
Prompt sections
Negative prompt section
Context stack
Anchor controls
Compiled preview
Validation warnings
Provider settings summary
History
```

### 14.2 Prompt sections

Instead of one long text box, advanced mode can show sections:

```text
Subject
Action
Environment
Style
Constraints
Negative constraints
Notes
```

For simple users, these can be hidden.

### 14.3 Variable and mention difference

Variables are placeholders.

```text
{character}
{weapon}
{location}
```

Mentions are references.

```text
@bit
@weapon.training_sword
@location.fungal_catacombs
```

A Recipe may have variables that require Codex references.

Example:

```text
Generate {character} performing {animation} in {style}
```

The user fills:

```text
{character} = @bit
{animation} = @animation.jump
{style} = @style.pixel_art_32
```

### 14.4 Prompt linter

The composer should warn about:

```text
unresolved @ reference
ambiguous reference
deprecated reference
conflicting styles
palette mismatch
missing required variable
too many context entries
locked anchor conflict
output structure mismatch
```

Example warning:

```text
@style.clean_hd_sprite conflicts with @rule.no_antialiasing_pixel_mode.
```

### 14.5 Compiled preview

The user can inspect:

```text
Raw user prompt
Resolved Codex references
Included anchors
Excluded optional fields
Final positive prompt
Final negative prompt
Provider payload summary
```

This is critical for debugging and trust.

---

## 15. Codex Coverage System

Coverage makes the Codex production-oriented.

### 15.1 What is coverage?

Coverage tracks required or desired assets for a project.

Examples:

```text
Every playable character needs idle, walk, run, jump, hurt, death.
Every biome needs ground, wall, decoration, hazard, background, ambient particles.
Every UI button needs normal, hover, pressed, disabled.
Every weapon needs icon, world sprite, swing VFX.
```

### 15.2 Coverage templates

Coverage can be defined as templates.

Example:

```text
Platformer Character Coverage
- idle
- walk
- run
- jump
- fall
- land
- attack
- hurt
- death
```

Apply to:

```text
@bit
@enemy.slime
@enemy.goblin
```

### 15.3 Coverage statuses

Each coverage item can be:

```text
Missing
Draft
Generated
Needs Review
Approved
Manually Finalized
Deprecated
```

### 15.4 Generate missing

Coverage should directly connect to generation.

Action:

```text
Generate Missing
```

The system uses:

```text
entry
coverage item
anchors
style
palette
generation recipe
```

### 15.5 Manual completion

Artists can mark manually created assets as satisfying coverage.

Example:

```text
Right-click animation clip → Mark as @bit / @animation.jump coverage complete
```

This is important because the Codex should support manual-first workflows.

---

## 16. Codex Validation

The Codex should include validation tools.

### 16.1 Entry validation

Checks:

```text
missing required fields
duplicate handles
broken references
deprecated references
missing thumbnails
missing anchors
invalid palette references
missing coverage plans
```

### 16.2 Creative consistency validation

Checks:

```text
character uses multiple conflicting palettes
location references deprecated vibe
sprite asset violates style constraints
pixel art asset contains too many colors
animation missing required frames
UI element missing required states
```

### 16.3 Generation readiness

Before generation, validate:

```text
all required variables filled
all @ references resolved
provider supports requested output structure
anchors available
palette behavior defined
output size defined
```

### 16.4 Validation severities

```text
Info
Warning
Error
Blocking
```

Blocking errors prevent generation. Warnings allow generation with caution.

---

## 17. Codex and Manual Art Workflows

The Codex must not be AI-only.

### 17.1 Create Codex entry from manual art

Users should be able to:

```text
Select sprite → Create Codex Entry
Select frame → Create Pose Anchor
Select animation clip → Create Animation Entry
Select palette → Create Palette Entry
Select layer group → Create Prop Entry
```

### 17.2 Promote manual work to canonical

Manual work can become the truth.

Example:

```text
Artist draws Bit's final idle frame.
Right-click → Promote as Canonical Reference for @bit.
```

Future AI generation respects that.

### 17.3 Reject AI output and learn from it

Rejected examples can be stored as negative references.

Example:

```text
Reject because: changed character proportions
Reject because: wrong palette
Reject because: too realistic
```

This can become negative prompt context.

---

## 18. Codex and Pixel Art Mode

Since Pixhaus supports many styles, pixel art should be represented as a dedicated style/tooling mode.

### 18.1 Pixel art style entry fields

Pixel art style entries may include:

```text
canvas resolution
sprite resolution
grid size
palette limit
anti-aliasing rule
dithering rule
outline rule
subpixel rule
color ramp rules
readability constraints
export constraints
```

### 18.2 Pixel art generation constraints

Example rules:

```text
No anti-aliasing
Use only current palette
Transparent background
Readable silhouette at 32x32
Hard pixel edges
No painterly gradients
```

### 18.3 Pixel art validation

Validation can check:

```text
color count
off-palette colors
semi-transparent pixels
unwanted anti-aliasing
grid alignment
sprite bounds
outline consistency
```

### 18.4 Other art style validation

Non-pixel styles may care about:

```text
alpha quality
edge softness
texture consistency
lighting direction
resolution
normal map compatibility later
```

---

## 19. Codex as Agent Backbone

Because agents will be used extensively in development and perhaps eventually inside Pixhaus, the Codex should have clear agent interaction rules.

### 19.1 Agent permissions

Agents can:

```text
read Codex entries
suggest new entries
draft coverage plans
generate candidate assets
flag inconsistencies
propose anchors
summarize lore
compile prompts
```

Agents should not silently:

```text
overwrite canonical entries
delete anchors
change locked palettes
promote generated assets to canonical
remove manual artist work
```

### 19.2 Proposal workflow

Agent-generated Codex changes should appear as proposals.

Example:

```text
AI suggests new prop: @prop.glowing_mushroom_lantern
Status: Candidate
[Accept] [Edit] [Reject]
```

### 19.3 Agent tasks

Possible future actions:

```text
Create a full Codex for this game concept
Generate missing coverage for all enemies
Find inconsistent style references
Suggest palette improvements
Create a prop pack for this location
Create animation coverage for all playable characters
Convert loose notes into structured Codex entries
```

### 19.4 Agent auditability

All agent edits should include:

```text
source agent
timestamp
reason
changed fields
references used
```

---

## 20. UX Flows

### 20.1 Creating a character

Flow:

```text
Codex Workspace
→ New Entry
→ Character
→ Name: Bit
→ Add description/lore
→ Add reference sprite
→ Create identity anchor
→ Assign palette
→ Assign style
→ Add animation coverage
→ Generate samples
→ Promote best sample to canonical reference
```

### 20.2 Generating a character animation

Flow:

```text
Generate Workspace
→ Output: Animation
→ Character: @bit
→ Animation: @animation.jump
→ Style: @style.pixel_art_32
→ Palette: @palette.bit_default
→ Review anchors
→ Generate
→ Select result
→ Apply to Animate timeline
→ Mark coverage item complete
```

### 20.3 Creating a location art pack

Flow:

```text
Codex Workspace
→ New Location: @location.fungal_catacombs
→ Add vibe: @vibe.cozy_dark_fantasy
→ Add palette: @palette.moss_and_amber
→ Add materials: @material.mossy_stone, @material.old_wood
→ Define coverage: tiles, props, background, VFX
→ Generate missing assets from coverage
```

### 20.4 Artist-first anchor flow

Flow:

```text
Draw/Animate Workspace
→ Artist draws a sprite manually
→ Select sprite
→ Create Codex Entry from Sprite
→ Add anchors from manual art
→ Later use Generate for variations/in-betweens
```

### 20.5 Developer-first game world flow

Flow:

```text
Codex Workspace
→ Describe game world
→ AI proposes Codex structure
→ User edits/approves characters, locations, styles
→ Apply coverage templates
→ Generate missing assets
→ Review and export
```

---

## 21. UI Design Direction

### 21.1 Codex should feel like a creative library

Avoid making it look like a database admin screen.

It should feel like:

```text
art bible
asset board
production checklist
creative graph
reference library
```

### 21.2 Entry cards

Codex entries should have cards with:

```text
thumbnail
name
type
status
short description
anchor indicators
coverage indicators
```

### 21.3 Entry detail pages

Entry pages should be rich and visual.

For characters:

```text
hero image / canonical sprite
identity summary
visual notes
palette
anchors
animations
relationships
generated examples
```

For palettes:

```text
swatches
ramps
roles
usage
compatible styles
examples
```

For animations:

```text
pose beats
timing chart
frame count
loop mode
example clips
compatible characters
```

### 21.4 Status indicators

Use clear visual status:

```text
Draft
Candidate
Canonical
Locked
Deprecated
Missing Coverage
Broken Reference
```

### 21.5 Anchors in UI

Anchors should be visible as chips or badges.

Example:

```text
Identity: Locked
Palette: Strong
Style: Normal
Animation: Loose
```

### 21.6 Visual hierarchy

The Codex UI should have a slightly calmer, library-like feel compared to the Draw/Animate workspaces.

It should still use the Pixhaus dark theme and accent color, but emphasize:

```text
cards
boards
tabs
relationship chips
thumbnail previews
coverage checklists
```

---

## 22. Codex Workspace Modes

The Codex Workspace can have internal modes.

### 22.1 Browse Mode

Find and inspect entries.

### 22.2 Edit Mode

Modify one entry deeply.

### 22.3 Board Mode

Arrange visual references and notes.

### 22.4 Graph Mode

Explore relationships.

### 22.5 Coverage Mode

Track missing assets.

### 22.6 Test Mode

Generate sample outputs from entries.

These modes do not have to be separate top-level workspaces. They can be tabs inside the Codex workspace.

---

## 23. Importing and Exporting Codex Data

### 23.1 Import sources

Potential import sources:

```text
Markdown lore docs
JSON/YAML design docs
CSV asset lists
image reference folders
palette files
existing Pixhaus projects
recipe packs
style packs
```

### 23.2 Export formats

Potential exports:

```text
Markdown art bible
JSON Codex package
HTML reference site
asset coverage report
prompt recipe pack
style pack
palette pack
```

### 23.3 Codex packs

Codex packs can be shared or reused.

Examples:

```text
Platformer Animation Coverage Pack
Dark Fantasy Materials Pack
Cozy Farming UI Pack
Retro RPG Enemy Pack
Pixel Art Rules Pack
```

Codex packs should not automatically overwrite project entries. They should import as candidates or namespaces.

---

## 24. Namespaces

Large projects need namespaces.

Examples:

```text
@character.bit
@enemy.slime
@prop.torch
@ui.button.primary
@palette.moonlit_ruins
@location.fungal_catacombs
```

Namespaces help:

```text
avoid ambiguity
organize large worlds
support imported packs
support built-in libraries
```

Suggested top-level namespaces:

```text
character
enemy
npc
creature
prop
item
weapon
material
palette
style
vibe
location
biome
faction
animation
pose
vfx
ui
rule
recipe
reference
```

---

## 25. Safety, Review, and Canonical Truth

### 25.1 Canonical vs candidate

Generated entries should usually start as Candidate.

The user promotes them to Canonical.

### 25.2 Manual edits have priority

If a human artist marks an asset as canonical, AI should respect it.

### 25.3 Reversible changes

Codex changes should be undoable where practical and versioned where important.

### 25.4 Review queues

The Codex workspace can include a review queue:

```text
New AI suggestions
Broken references
Missing coverage
Deprecated entries
Conflicting anchors
```

---

## 26. Search and Discovery

The Codex should have excellent search.

Search by:

```text
name
alias
type
tag
relationship
status
coverage state
palette
style
location
asset usage
```

Examples:

```text
all characters using @palette.moonlit_ruins
all props in @location.fungal_catacombs
all deprecated styles
all enemies missing death animation
all assets generated from @recipe.platformer_enemy
```

---

## 27. Relationship to Asset Browser

The Codex and Asset Browser are related but not the same.

### Codex

Defines creative meaning.

```text
Who is Bit?
What is mossy stone?
What does jump mean?
What style is clean HD?
```

### Asset Browser

Stores concrete files/assets.

```text
bit_idle.png
bit_walk.anim
mossy_stone_tile_01.png
button_primary_hover.png
```

A Codex entry can link to many assets.

An asset can satisfy one or more Codex coverage items.

---

## 28. Relationship to Workspaces

### Draw Workspace

Uses Codex for:

```text
reference lookup
palette/style constraints
create entry from manual art
anchor creation
```

### Animate Workspace

Uses Codex for:

```text
animation definitions
coverage
pose anchors
in-between context
clip metadata
```

### Generate Workspace

Uses Codex as primary context source.

### Tiles Workspace

Uses Codex for:

```text
materials
biomes
locations
tile rules
palette/style constraints
```

### Export Workspace

Uses Codex for:

```text
coverage validation
naming conventions
asset metadata
engine export grouping
```

---

## 29. Future Advanced Features

### 29.1 Codex-aware batch generation

Examples:

```text
Generate all missing props for @location.fungal_catacombs
Generate all weapon icons for @faction.iron_order
Generate all enemy hurt/death animations
```

### 29.2 Codex style drift detection

Analyze generated or imported assets and warn:

```text
This sprite no longer matches @style.pixel_art_32.
This prop uses colors outside @palette.moss_and_amber.
This character silhouette differs from @bit identity anchor.
```

### 29.3 Codex-driven procedural generation

The Codex can power procedural generators.

Examples:

```text
Generate tile variations using @material.mossy_stone
Generate random shop props using @location.workshop rules
Generate NPC variants using @faction.moon_guild
```

### 29.4 Team collaboration

Future collaborative features:

```text
entry authors
review comments
approval workflow
locked canonical entries
branch/merge Codex changes
```

### 29.5 External game engine integration

Codex metadata can inform export:

```text
Unity animation names
Godot sprite animation resources
Tiled tileset metadata
LDtk entity sprites
UI atlas grouping
```

---

## 30. Implementation Boundaries

The Codex should integrate with the rest of Pixhaus through stable boundaries.

### 30.1 Codex service

Responsible for:

```text
entry CRUD
reference resolution
search/autocomplete
relationship queries
validation
versioning
```

### 30.2 Prompt compiler

Responsible for:

```text
resolving @ references
collecting anchors
building context stack
compiling provider-ready prompts
building negative prompts
producing debug output
```

### 30.3 Coverage service

Responsible for:

```text
coverage templates
coverage item state
missing asset detection
asset-to-coverage linking
```

### 30.4 Anchor service

Responsible for:

```text
anchor creation
anchor strength
anchor validation
reference asset loading
```

### 30.5 Generate integration

Responsible for:

```text
turning Codex context into jobs
storing generation metadata
applying results through commands
```

---

## 31. MVP Scope

The full Codex vision is large. Build it in phases.

### 31.1 Codex MVP

Must have:

```text
Codex workspace
entry list
entry types: Character, Style, Palette, Animation, Prop, Location, Recipe
basic entry editor
@ autocomplete
reference chips
prompt composer
anchors: identity, palette, style
Generate workspace context stack
coverage for characters/animations
save/load support
```

### 31.2 Codex V1

Add:

```text
visual boards
reference images
coverage templates
test generation panel
graph relationships
validation panel
entry version history
import/export Codex packs
```

### 31.3 Codex V2

Add:

```text
agent proposals
style drift detection
advanced relationship queries
batch generation
team review workflows
external engine metadata
advanced namespace management
```

---

## 32. Acceptance Criteria

The Codex system is successful when:

```text
A user can define a character once and generate consistent assets later.
A user can reference Codex entries with @ in prompts.
The Generate workspace can use Codex references as structured context.
The app can track missing animations/assets through coverage.
Manual artists can promote their own work into Codex anchors.
AI can generate candidates without overwriting canonical truth.
Users can inspect what Codex context was used in a generation.
The project can save/reload Codex entries without loading all large assets.
The Codex feels like an art bible, not a database table.
```

---

## 33. Core Product Statement

The Codex is the creative memory of a Pixhaus project.

It defines the world, the art direction, the characters, the assets, the rules, the anchors, and the reusable generation context.

The Generate workspace should not start from an empty prompt box.

It should start from the Codex.

> **The Codex turns AI generation from one-off prompting into project-aware art production.**

---

## 34. Final Recommendation

Replace the idea of a simple Prompt Library with a full Codex system.

The Codex should become a top-level workspace alongside:

```text
Draw
Animate
Codex
Generate
Tiles
Export
```

The Codex is where the user defines the world.  
Generate is where Pixhaus uses that world to create assets.  
Draw and Animate are where artists refine and create by hand.  
Export is where assets become production-ready.

The Codex is not optional polish. For an AI-native sprite production tool, it is one of the main pillars of the product.

