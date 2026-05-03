# Recraft

## Quick facts

- Vendor / maintainer: Recraft (venture-backed)
- Status (active / acquired / shut down): Active
- License / pricing model: Proprietary SaaS, freemium
- Price point (current): Free tier; paid plans start around $10-20/month (exact pricing not fully transparent in public docs)
- Platforms: Web browser, API
- First released: 2023
- Last meaningful update: 2024-2025 (Style Sharing, Advanced Style Control, vector generation)
- Source available: No
- Primary use case: AI design with vector export, pixel art support, and team style consistency

## Origin and purpose

Recraft launched in 2023 as an AI design tool for designers and creatives, emphasizing style control and brand consistency. The company's unique angle: AI-generated vector graphics, not just raster. For game art, Recraft offers pixel-art generation and notably strong style-consistency features (2024 updates).

## Generation model and approach

Uses multiple underlying diffusion models (details not fully public). The distinguishing feature is the output format: Recraft can generate images as vectors (SVG) in addition to raster (PNG). For game art, this is less relevant (most game pipelines use raster), but the underlying model quality affects both output types.

Approach is similar to Leonardo or Layer (model curation) but with vector as a first-class output format.

## What it generates

- Raster images (PNG, JPG)
- Vector graphics (SVG) — unique feature
- Pixel art (via pixel-art model)
- Game assets (sprites, backgrounds, UI)
- Icons and design elements
- Multiple style variations

The vector capability is most relevant for UI and icons; for sprites, raster output is typical.

## Editing capabilities post-generation

In-app tools:
- Style editing and refinement
- Manual adjustments to vector output
- Regeneration with style-preservation
- Batch editing (apply changes to multiple assets simultaneously)

Not as deep as PixelLab's inpainting, but sufficient for broad adjustments.

## Style control and consistency

This is Recraft's flagship feature as of 2024-2025.

**Advanced Style Control** (2024):
- Upload brand colors, icon geometry, line styles
- Recraft biases all generated art toward those specifications
- Style Mixing: combine multiple styles into a single output
- Style Sharing: create shareable style templates for teams

**For game art specifically**:
- Select from preset game styles or upload reference images
- Generate multiple assets with style-locking across the batch
- "Harmonize" elements (color palette, line style) across a group of sprites

Users report strong style consistency when using style templates or uploading references. This is a direct competitor to Scenario's custom training, using a different mechanism (reference-image biasing vs. model fine-tuning).

Strength: Fast to set up (upload images vs. 30-minute training). Weakness: may be less fine-grained than per-project model training.

## Animation capabilities

No native animation generation. Can generate multi-frame sequences, but frame-to-frame consistency is not guaranteed. Not a focus for Recraft.

## Pixel art handling

Recraft offers a pixel-art model, but quality is not comparable to PixelLab or Retro Diffusion. Pixel art is a supported style, not a core specialization.

Output from the pixel-art model:
- More reliable than generic diffusion (Stable Diffusion)
- Less grid-aware and palette-conscious than PixelLab
- Suitable for casual pixel-art projects or concept art

For production pixel art, use PixelLab.

## Export and import

- Raster: PNG, JPG
- Vector: SVG (unique to Recraft)
- Batch export
- API access for programmatic workflows

Vector export is useful for scalable UI elements and icons but less relevant for pixel-art sprites (which should remain raster-only).

## Scripting / API

Yes. API supports generation, style management, and batch operations. Documentation is moderate; less comprehensive than PixelLab's.

## Engine integration

No official plugins. API access allows custom integration.

## Workflow strengths

- **Style consistency tools**: Advanced Style Control and Style Sharing are industry-leading for design teams
- **Vector export**: Unique capability for scalable graphics (less relevant for game art)
- **Team collaboration**: Style Sharing is built for teams
- **Batch operations**: Generate multiple assets with consistent style
- **Brand-aligned generation**: Color codes and style images can be encoded into generation
- **Free tier available**: Good for evaluation

## Workflow gaps

- **Not pixel-art specialized**: Pixel-art quality is below dedicated tools
- **No animation support**: Can't generate sprite animations
- **Limited game-art focus**: Positioning is broader (design, branding)
- **No custom training**: Unlike Scenario, you can't train a project-specific model (though Style Sharing is a partial alternative)
- **Pricing opacity**: Exact pricing for game-art workflows is not clear from public docs

## Notable uses

Designers and creative agencies using Recraft for brand identity and UI design. Limited adoption by game studios specifically (unlike Scenario or PixelLab). Emerging use for indie game UI design.

## Community and ecosystem

Moderate community; primarily designers rather than game developers. Minimal game-art-specific resources.

## Pricing details

**Free Tier**: Available, but limits on generations and model access are not clearly specified.

**Paid Plans**: Start around $10-20/month, with higher tiers for teams and enterprise. Annual discounts available.

Exact per-generation costs and token metering are not publicly documented; likely usage-based but less transparent than Scenario or PixelLab.

## Verdict for SpriteMaster

Recraft is strongest for teams building consistent design systems and UI (where vector export shines). For pure pixel-art sprite generation, PixelLab or Retro Diffusion are better. For style consistency across a game's art (characters, backgrounds, UI), Recraft's tools are competitive with Scenario's custom training, using a reference-image approach instead of fine-tuning.

Positioning: "Design tool that supports game art and pixel art" vs. "Game-art tool with design features" (which is Scenario's positioning).

## Relevance to SpriteMaster

If SpriteMaster targets mixed-media projects (pixel art + UI + vector icons), Recraft's workflow is relevant. The Style Sharing and Advanced Style Control features demonstrate how to maintain consistency across a large sprite set without custom model training. Worth studying for style-consistency mechanisms.
