# PixelLab

## Quick facts

- Vendor / maintainer: PixelLab (independent developer-led team)
- Status (active / acquired / shut down): Active, growing
- License / pricing model: Proprietary SaaS, freemium
- Price point (current): Free tier (limited generations); Tier 1 $9/month; Tier 2 $30/month; Tier 3 $50/month (Pixel Architect with team features)
- Platforms: Web browser, Aseprite plugin, SDK (JavaScript, Python)
- First released: 2023
- Last meaningful update: 2024-2025 (animation tools, directional generation, API expansion)
- Source available: No, but public API
- Primary use case: AI-native pixel art generation and animation for indie and mid-size game studios

## Origin and purpose

PixelLab launched in 2023 explicitly as a pixel-art-first tool, filling a gap left by general-purpose generators (Midjourney, Stable Diffusion) that struggle with authentic retro sprites. The founding team identified that game developers needed fast, grid-respecting, palette-aware pixel art generation—not just "pixel" styles from generic models.

## Generation model and approach

Uses a custom fine-tuned diffusion model trained specifically on pixel art. Unlike Scenario and Layer (which use multiple vendor models), PixelLab runs a proprietary model optimized for pixel grids and limited color palettes. The model is trained to avoid anti-aliasing artifacts and sub-pixel defects common in diffusion output.

Does not publicly disclose the base model (likely Stable Diffusion derivative) but emphasizes the fine-tuning work as key to quality.

## What it generates

- Individual character sprites (any resolution, typically 64x256 to 256x256 for characters)
- Sprite sheets with directional variants (4 or 8 directions for top-down, isometric, side-scroll)
- Animations (multi-frame sequences for walk, run, attack, idle, etc.)
- Items, UI elements, and smaller assets
- Tileset and environment maps (less emphasized than characters)
- Isometric tiles and perspective variants

PixelLab's unique selling point: directional rotation. In one generation, you can ask for "a knight facing left, right, up, down" and get a coherent set. This is a direct answer to game developer needs.

## Editing capabilities post-generation

Robust post-gen editing via in-app tools:
- **Inpainting**: Regenerate specific parts of a sprite by selecting a region and describing the change (e.g., "change the sword to a staff")
- **Color adjustment**: Palette editing and hue-shift
- **Manual pixel editing**: Draw/erase pixels in the editor for final tweaks
- **Aseprite integration**: Export to Aseprite for advanced animation sequencing

The combination of AI and manual control is PixelLab's strength; you can use AI for rough generation, then refine by hand in minutes rather than hours.

## Style control and consistency

Style control is addressed via:
1. **Text prompts** with detailed style descriptors ("8-bit NES-style" vs "Game Boy monochrome" vs "32-bit Sega Genesis")
2. **Reference images**: Upload an existing character or sheet and ask for variations maintaining that aesthetic
3. **Style templates**: Pre-defined styles (fantasy pixel, sci-fi, cute, dark, etc.)
4. **Inpainting with conditioning**: Regenerate a sprite while keeping certain elements fixed

Consistency across a full character set (idle, walk, run, all 8 directions) is reported by users as strong when prompts are specific. Less reliable than Scenario's custom models, but more practical for quick iteration.

Shortfall: no per-project style training like Scenario. Style consistency depends on prompt quality and reference images.

## Animation capabilities

Animation is a core feature:
- **Text-to-animation**: Describe an action (walk cycle, jump, attack) and generate 4-8 frames
- **Skeleton-based animation**: Specify limb positions and PixelLab animates the character between poses
- **Animation-to-animation**: Condition new frames on existing ones to extend or refine animation sequences
- **Sprite sheet generation**: Output is a ready-to-use sprite sheet with rows for each animation and columns for frames

This is significantly ahead of Scenario and Layer, which can generate frames but not with frame-consistency guarantees.

Output quality: smooth animations with consistent pixel sizes, though some users report occasional frame glitches requiring manual fixes in Aseprite.

## Pixel art handling

This is PixelLab's core strength and primary differentiator.

- **Grid-respecting output**: Respects pixel dimensions; no sub-pixel anti-aliasing
- **Palette awareness**: Generates images within specified or detected color palettes
- **Authentic retro feel**: Trained on pixel art, so aesthetics match 8-bit to 32-bit game eras
- **Resolution flexibility**: Can generate 16x16 up to 512x512 or higher with output grid guarantees
- **No cleanup required**: Unlike Stable Diffusion pixel art, requires minimal post-gen touching up

This is the primary reason PixelLab ranks highest among pixel-art tools.

## Export and import

Exports to:
- PNG (with transparency)
- Aseprite project files (.ase)
- Sprite sheets (auto-compiled with configurable grid and spacing)
- GIF (for animation preview)
- JSON metadata for game engine import

Supports importing existing assets and using them as reference or conditioning input.

## Scripting / API

Yes. PixelLab provides:
- **Public API**: Generate sprites, animations, and rotations programmatically
- **JavaScript SDK**: Official client library for Node.js and browser
- **Python SDK**: Official PyPI package for automation scripts
- **MCP (Model Context Protocol) integration**: Usable as a tool within Claude Code and other AI assistants

API is well-documented with clear pricing per endpoint.

## Engine integration

No official plugins, but API access + SDKs enable custom integrations. Example workflows:
- Godot GDScript calling PixelLab API to generate assets at runtime (uncommon, but possible)
- Batch generation scripts in Python feeding assets into a game project
- Aseprite plugin allows direct save-to-engine workflows

## Workflow strengths

- **Pixel-art specialization**: No competitor does this as well
- **Animation support**: Text-to-animation and skeleton-based systems rival or exceed specialized animation tools
- **Quick iteration**: Inpainting + manual editing loop is fast for single-asset tweaking
- **Directional generation**: Unique feature for top-down and isometric games
- **Affordable pricing**: $9-50/month covers indie to mid-size studio needs
- **Accessibility**: Web-based, no setup; Aseprite plugin lowers friction
- **API + SDKs**: Automation and integration are straightforward

## Workflow gaps

- **No custom model training**: Unlike Scenario, you can't train a model on your specific art style; you're limited to prompt refinement
- **No collaboration canvas**: Tier 3 has team features, but real-time co-editing is not mentioned
- **Export to 3D**: Unlike Layer, no 3D asset generation (sprites are 2D only)
- **Limited style templates**: Fewer pre-built styles than Layer; requires more prompt engineering for niche aesthetics
- **Inpainting limitations**: Works well for broad changes (swap weapon) but struggles with fine details (adjust facial features)

## Notable uses

Indie game developers and small studios are the primary user base. No major AAA case studies (these prefer in-house pipelines or Scenario). Reported use in 2024-2025 indie game projects, particularly for rapid prototyping and asset expansion.

## Community and ecosystem

Active Discord with user-shared prompts and tips. GitHub repositories for SDKs. Minimal third-party integrations; most ecosystem value comes from Aseprite synergy.

## Pricing details

**Free Tier**:
- Limited fast generations per month
- Access to creator tool
- Suitable for evaluation

**Tier 1**: $9/month
- 40 fast generations/month
- Unlimited slow generations
- Single user

**Tier 2**: $30/month
- 200 fast generations/month
- Multiple simultaneous jobs
- Enhanced priority

**Tier 3 (Pixel Architect)**: $50/month
- 20 concurrent jobs
- Team collaboration (multiple users)
- API priority
- Highest priority queue

API is metered separately: per-call costs vary by operation (sprite generation ~$0.01-0.05, animation ~$0.05-0.10).

## Pixel art consistency verdict

PixelLab excels at pixel-art consistency through specialization. For projects that require authentic, grid-aware sprites and animations, PixelLab is the leader. Trade-off vs. Scenario: less custom training depth, but faster iteration and better out-of-the-box pixel quality. For pixel-art-heavy projects, PixelLab is the first choice. For mixed 2D/3D pipelines or general game art, Layer or Scenario may be better fits.

## Verdict for SpriteMaster

PixelLab's animation and directional-generation capabilities are directly relevant to a sprite editor. If SpriteMaster targets pixel-art or retro 2D games, PixelLab's workflow (AI generation → Aseprite for animation refinement) is the gold standard to study. If SpriteMaster aims to be a standalone editor with built-in generation, PixelLab's API and model specialization are valuable reference points.
