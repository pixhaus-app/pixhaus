# Cascadeur

## Quick facts

- Vendor / maintainer: Nekki (Cascadeur team)
- Status (active / acquired / shut down): Active, actively developed
- License / pricing model: Proprietary, subscription and perpetual licenses
- Price point (current): Free tier; Standard $15/month; Professional $50/month; Educational discounts available
- Platforms: Windows, macOS (beta), Linux (beta)
- First released: 2020
- Last meaningful update: 2025.2 (AI Inbetweening, AutoPosing, Quadruped support)
- Source available: No
- Primary use case: AI-assisted keyframe animation for 3D character rigs (not pixel art)

## Origin and purpose

Cascadeur is an AI-assisted animation software for creating character animations via keyframe positioning. Founded to accelerate animation production through physics-aware automation and AI-assisted inbetweening.

**Important caveat**: Cascadeur is primarily for 3D skeleton animation, not 2D pixel art. It's included here because:
1. Sprite animation and 3D animation have overlapping concerns (keyframing, pose control, motion flow)
2. Cascadeur's AI inbetweening could theoretically be adapted for pixel-art frame interpolation
3. For comparison with animation-capable tools in the game-art space

## Generation model and approach

Uses machine learning (not traditional diffusion) trained on real-world motion capture and animation data. The AI learns physics-aware motion patterns and inbetweening (interpolation between key poses).

Workflow:
1. Artist sets key poses manually (frame 1: idle, frame 20: walking stride)
2. AI automatically generates intermediate frames respecting physics constraints
3. Artist refines as needed

Approach is physics-first (respect character constraints) rather than prompt-first (describe the motion).

## What it generates

- Inbetween frames (interpolate between two poses)
- AutoPosing suggestions (AI suggests next pose based on motion flow)
- Physics validation (ensures bones don't break, feet slide, etc.)
- QuickRigging (automated rig creation for bipeds and quadrupeds)

For pixel sprites, only AutoPosing and frame-count suggestions would apply (not physics simulation).

## Editing capabilities post-generation

Full manual editing post-AI:
- Adjust individual bone positions
- Refine trajectory and motion curves
- Physics constraints (IK, pole vectors)
- Motion graph and curve editor
- Ragdoll simulation for realistic secondary motion

Cascadeur is animation-focused; editing is first-class.

## Style control and consistency

Not applicable in traditional sense. Cascadeur doesn't have "styles." Consistency comes from:
- Character rig (defines body proportions, constraints)
- Manual pose refinement
- Physics constraints (ensure believable motion)

A well-built rig and careful pose setup ensure consistency across animations.

## Animation capabilities

This is Cascadeur's core. Features:
- **AI Inbetweening**: Generate smooth interpolation between poses, 2-120 frames in one click
- **AutoPosing**: Suggest next pose in a motion sequence
- **Physics simulation**: Validate motion doesn't violate constraints
- **Ragdoll**: Secondary motion (cloth, hair) via simulation
- **Motion graph editor**: Visual editing of motion flow

Quality: Excellent for 3D bipedal and quadrupedal characters. Suitable for game animation production.

## Pixel art handling

Not applicable. Cascadeur is 3D-only.

## Export and import

Exports to:
- FBX (animated 3D model)
- Alembic (animation data)
- Game engines (Unreal, Unity) via import
- Image sequences (if rendering within Cascadeur)

Can export pre-rendered animations as sprite sheets if you render the 3D character from a fixed camera angle (unusual workflow but possible).

## Scripting / API

Limited. Cascadeur is primarily UI-driven. Python scripting is mentioned in docs but not extensively used by community.

## Engine integration

Supports FBX export to Unity and Unreal. No real-time live-link as of 2025.2 (though an Epic MegaGrant announced plans for Unreal Live Link plugin).

## Workflow strengths

- **AI inbetweening**: One-click frame interpolation is powerful
- **Physics-aware**: Ensures believable motion without manual tweaking
- **Rapid prototyping**: Quick pose-to-pose animation for game mechanics
- **Professional-grade**: Used in studios for game and film
- **Affordable entry**: Free tier and $15/month standard plan are accessible
- **Active development**: Frequent updates with new features (2025.2 brings Quadruped support)

## Workflow gaps

- **3D-only**: No pixel-art or 2D sprite support
- **Manual pose input**: Must set keyframes; can't describe motion in text
- **No style control**: Aesthetics depend on character rig and rendering
- **Rig-dependent**: Quality depends on character setup; requires rigging knowledge

## Notable uses

Professional game studios and animation houses use Cascadeur for game animation production (cutscenes, character loops). Notable use in indie game development for character animation.

## Community and ecosystem

Small but engaged community on Discord and forums. Some shared rigs and tutorials. Minimal integration with other tools.

## Pricing details

**Free Tier**:
- Limited features (basic inbetweening, no AutoPosing)
- Suitable for evaluation

**Standard**: $15/month
- Full AI inbetweening and AutoPosing
- Professional export

**Professional**: $50/month
- Team features and higher priority

Educational: Deep discounts for students and educators.

Perpetual licenses available (one-time purchase) but details are enterprise-only.

## Verdict for SpriteMaster

Cascadeur is **not directly applicable** to pixel-art sprite editing. It's a 3D animation tool, not a 2D tool.

However, the conceptual approach—AI-assisted keyframing and inbetweening—could inspire similar features for 2D sprite animation. For example: if SpriteMaster had pose-based sprite animation, an AI inbetweening system could interpolate between poses.

## Relevance to SpriteMaster

**Very low**, but architecturally interesting. Cascadeur demonstrates:
- Physics-aware animation (respecting body constraints)
- Pose-to-pose animation workflows
- AI inbetweening as a distinct feature

If SpriteMaster supports skeleton-based 2D animation (less common but possible), Cascadeur's approach could be adapted. But for frame-by-frame pixel art, Cascadeur's techniques don't transfer.

## Why included

Cascadeur is here as a reference point for animation workflows in game development, not as a direct competitor or model. It shows how animation tools in adjacent domains (3D) approach AI-assisted motion, which could inform design decisions for 2D animation features if SpriteMaster goes there.
