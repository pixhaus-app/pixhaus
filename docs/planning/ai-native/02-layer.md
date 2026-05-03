# Layer

## Quick facts

- Vendor / maintainer: Layer (formerly layerdev)
- Status (active / acquired / shut down): Active, well-funded
- License / pricing model: Proprietary SaaS, freemium
- Price point (current): Free tier; Pro $10/month (300 CUs); higher tiers $60+ per 1,000 additional CUs
- Platforms: Web browser, API, node-based editor
- First released: 2021-2022 (initial game-art focus)
- Last meaningful update: 2024-2025 (major platform expansion to video, 3D, audio; pricing restructuring)
- Source available: No
- Primary use case: Multi-modal AI asset generation for game studios and creative teams

## Origin and purpose

Layer started as a game-art-focused platform but has pivoted to position itself as a broader "AI Operating System for Creative Teams." The shift from specialization to generalization happened in 2024-2025, broadening from images to video, 3D, and audio generation. The game-development heritage remains visible in the feature set and marketing to game studios.

## Generation model and approach

Layer aggregates 149+ generative models from multiple vendors (Google, OpenAI, Flux, Runway, Kling, and others). It does not train its own base models; instead, it provides a unified interface and workflow engine.

Models are organized into categories: image generation (Flux, SDXL, others), video (Runway, Kling), 3D (Luma, Meshy), and audio (Eleven Labs and others). Underneath is a node-based workflow system that lets users chain models together and automate complex generation pipelines.

Custom model training is available but appears to be a secondary feature (less marketed than Scenario's equivalent).

## What it generates

- Images (2D sprites, characters, environments, concept art)
- Animation frames and short video sequences
- 3D models and meshes
- Audio (voice, sound effects)
- Sprite sheets (via multi-frame generation)
- Tilesets and environments

The breadth is Layer's strength; a game studio can generate art, animations, and audio from one platform. The downside: depth in any one modality is shallower than single-purpose tools.

## Editing capabilities post-generation

Layer provides:
- Natural-language editing (regenerate specific areas by describing them)
- Upscaling and background removal
- Image-to-image editing
- Multi-modal workflows (e.g., generate an image, then animate it)

Editing is more integrated than Scenario's (workflows live in the app), but pixel-level precision is not the goal.

## Style control and consistency

Layer offers 40+ pre-built "custom models inspired by successful game visual styles." This is a middle ground: not as deep as Scenario's per-studio training, but more curated than Leonardo's generic models.

Users can upload reference images to bias generation, and the platform supports a workflow concept called "Style Locking" (light on official docs), but details are sparse. Reports from users suggest style consistency is better than Midjourney but not as reliable as Scenario or PixelLab.

Strength for studios: pre-built game-art styles are ready to use immediately. Weakness: if your aesthetic doesn't match the 40 templates, you're limited to prompt engineering.

## Animation capabilities

Layer supports video generation (via Runway, Kling) and can produce short animated sequences. This is newer (2024-2025) and less documented than image generation.

For sprite animation, users report mixed results: the tool can generate multi-frame sequences, but frame-to-frame consistency and pixel-perfection are not guaranteed.

## Pixel art handling

Not specialized for pixel art. Layer's strengths are photorealism, illustration, and stylized animation. Pixel-art prompts work but are unreliable. No palette constraints or pixel-grid enforcement.

For authentic retro sprite work, use PixelLab or Retro Diffusion instead.

## Export and import

Standard formats (PNG, JPG for images; MP4, WebM for video; OBJ, GLB for 3D). Batch export from workflows. API access for programmatic export.

## Scripting / API

Yes. Layer API supports workflow definition, generation, and asset retrieval. Node-based workflows can be constructed visually or programmatically.

The API is less documented than Scenario's (as of 2025) but appears to support enterprise integration patterns.

## Engine integration

No official Unity/Unreal plugins as of 2025. Workflows are accessible via API; studios have built custom integrations.

Layer is attempting to position itself as a "creative infrastructure" layer that game engines plug into, but the integrations are early.

## Workflow strengths

- Multi-modal generation (images, video, audio, 3D from one platform)
- Pre-built game-art styles get you started fast
- Node-based workflow UI is powerful for non-technical creatives
- 40+ models available, reducing vendor lock-in compared to Scenario
- Team collaboration features (multiple users sharing CUs)
- Free tier is more generous than Scenario's

## Workflow gaps

- Less specialization than single-purpose tools (PixelLab for sprites, Midjourney for illustration)
- Pixel art is not a focus; results are inconsistent
- Style consistency is less reliable than Scenario (no per-studio custom training equivalent)
- Documentation for advanced features (workflows, API) lags behind web UI
- Animation is newer and less mature than static image generation

## Notable uses

Emerging use in indie studios and creative agencies. Less public case study material than Scenario, but user growth reported (blog posts Dec 2024, 2025 offsite announcement).

## Community and ecosystem

Moderate Discord presence. Community workflows are shared but centralized documentation is limited. Ecosystem is younger than Scenario's.

## Pricing details

**Free Tier**:
- 300 Creative Units per month
- Access to basic features (Forge image gen, Realtime live canvas, Canvas editor)
- Limited to lower-priority models

**Pro**: $10/month
- 300 CUs base + subscription fee
- Access to higher-priority generation and more models

**Team/Higher Tiers**: Consumption-based scaling
- $60 per additional 1,000 CUs/month
- No per-seat charges; users share a CU pool
- Monthly tier adjustment based on usage needs

Creative Units (CUs) cost varies by model and output resolution. Image generation: ~5-50 CUs per image. Video generation: ~50-500 CUs per clip. 3D: ~50-200 CUs per model.

Pay-as-you-go is available; annual discounts (20-30%) offered.

## Overall positioning

Layer is pitching itself as the "Swiss Army knife" of AI asset generation for creative teams: multi-modal, team-friendly, pre-styled. It sacrifices depth in any one area (vs. Scenario for custom models, vs. PixelLab for pixel art, vs. Runway for video) for breadth. Best for studios that need diverse asset types and want to avoid switching between multiple vendors. Weaker for pixel-art-heavy or highly specialized projects.

## Style consistency verdict

Layer's 40+ pre-built game styles provide quick-start convenience but less long-term consistency than Scenario's custom training. For studios that fit one of the pre-existing templates, Layer is competitive. For unique aesthetics, Scenario wins.
