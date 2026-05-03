# Procreate

## Quick facts
- Vendor / maintainer: Savage Interactive (Australian software company)
- License / pricing model: One-time purchase for iOS
- Price point (current): $12.99 USD (one-time purchase on App Store)
- Platforms: iPad (iOS 17.4 and later)
- First released: 2011
- Last meaningful update: Regular updates; current version maintains feature parity with industry standards
- Source available: No. Proprietary.
- Primary use case: Digital illustration and painting on iPad with support for animation via separate Dreams app

## Origin and purpose

Procreate was first released in 2011 by Australian software company Savage Interactive for iPad. The initial release focused on digital painting and illustration. Procreate 2 launched in June 2013 alongside iOS 7, introducing higher resolution support and expanded brush libraries. Over 10+ years of development, Procreate has become the dominant iPad painting application for professional artists. The underlying Valkyrie graphics engine provides real-time performance on iPad hardware. Procreate remains a one-time purchase application with no subscription component, contrasting with Adobe Creative Cloud's subscription model. In 2023, Savage Interactive released Procreate Dreams as a dedicated companion animation application.

## Drawing and painting tools

Procreate includes over 300 hand-crafted brushes covering pencils, inks, charcoals, oils, watercolors, pastels, airbrushes, and artistic textures. Brushes can be customized using the Brush Editor, which supports brush size, opacity, dynamics (pressure, tilt, speed, rotation), texture application, and dual-texture blending. Pressure sensitivity is first-class; Apple Pencil and other styluses leverage Pro Motion displays (on compatible iPads) for low-latency input. Stroke stabilization reduces jitter. Symmetry and perspective guides assist composition. Color dynamics and granulation settings simulate natural media behavior. The QuickShape tool allows rapid geometric shape creation. The interface is gesture-based; standard iOS multi-touch interactions (two-finger tap to undo, three-finger tap to redo) are native.

## Pixel-specific features (or "How artists use it for sprite work")

Procreate does not have a dedicated pixel-art mode. However, artists can work with pixel constraints by:
- Using a small hard brush sized to 1 pixel.
- Enabling grid display and snapping to grid.
- Disabling anti-aliasing per brush.
- Working at integer zoom levels (100%, 200%, 400%) to maintain pixel alignment.

Many illustrators use Procreate for character design and concept art that later becomes sprite assets in games. However, for frame-by-frame pixel-art animation, Procreate's primary limitation is the lack of onion skinning and timeline controls—Procreate Dreams (the separate animation app) is the intended tool for this workflow.

## Color and palette workflow

Procreate supports RGB and Display P3 color spaces. The color picker provides standard hue/saturation/brightness sliders and a full spectrum picker. Color swatches can be saved to a palette. The Palettes panel allows import and export of palette files. Color Dynamics settings let brushes pick colors from gradients or other sources during painting. The color history shows recently used colors. Palette management is streamlined for iPad touch interaction; palette switching during work is instantaneous. Unlike desktop tools, Procreate does not support indexed color mode or strict palette restrictions—this is a limitation for retro game sprites with fixed color budgets, though such constraints can be enforced externally before export.

## Layer system

Procreate's layer system supports standard hierarchical organization. Layers can be grouped in folders. Clipping masks bind layers to predecessors. Adjustment layers provide non-destructive color correction (Hue/Saturation, Curves, Levels, Color Balance, Posterize, Desaturate). Layer opacity and blend modes (Multiply, Screen, Overlay, Color Dodge, etc.) are standard. Layer masks allow selective transparency. Stroke shape, color dynamics, and texture layers can be applied. For illustration work, layers are well-suited. For animation, Procreate itself offers only basic frame-by-frame support via stacking layers; Procreate Dreams is the dedicated animation tool.

## Animation features

Procreate's animation capabilities within the main app are limited:
- The Animation Assist feature (beta in recent versions) provides basic frame-by-frame drawing aid with onion skin preview.
- Layers can be stacked to simulate frame sequences, but playback is not optimized for animation review.
- Timeline and keyframe features are absent from the main Procreate app.

For full animation capability, users must purchase Procreate Dreams, a separate $12.99 app released in 2023. Procreate Dreams is the dedicated animation tool designed for frame-by-frame and keyframe animation. See the "Procreate Dreams" section for full animation details.

The separation of illustration (Procreate) and animation (Procreate Dreams) into two applications reflects a design choice by Savage Interactive to keep the main app focused and lightweight while providing a specialized tool for animators.

## Export and import

Procreate natively saves to PSD (Photoshop format) and Procreate's native format. Supports import of PNG, JPEG, TIFF, GIF, and PSD. Export options include PNG, JPEG, TIFF, PDF, and PSD.

For sprite work:
- PNG export with transparency for game asset import.
- PSD export for further editing in desktop tools.
- No animated GIF or video export from Procreate itself (animation exports handled by Dreams app).
- No sprite sheet packing native to Procreate.

## Scripting and extensibility

Procreate does not support plugins or scripting in the traditional sense. Workflows can be extended via keyboard shortcuts, gesture customization, and brush creation. The Procreate API is closed; third-party developers cannot write extensions. This contrasts with desktop tools like Krita or GIMP, which allow plugin development.

## Engine integration

Procreate is a creative tool, not a game engine. Assets created in Procreate are exported as PNG or PSD and imported into game engines (Unity, Godot, Unreal, GameMaker) via their respective asset importers. Professional game studios often use Procreate for character concept art and illustration that later becomes sprite assets, but animation is handled in a separate tool.

## Workflow strengths

- Highly optimized for iPad; real-time performance even on older iPad models.
- Gesture-based interface is intuitive for touch input.
- One-time purchase with no subscription; lifetime access.
- Over 300 hand-crafted brushes with deep customization.
- Excellent pressure sensitivity and response.
- Growing professional adoption; industry-standard for iPad painting.
- Regular updates with new features.

## Workflow gaps

- No animation timeline or keyframe support in main app.
- Animation features require purchasing separate Dreams app.
- No pixel-art-specific tools (pixel mode, dedicated pixel brushes).
- No indexed color support for retro sprite constraints.
- Limited scripting/extensibility for custom workflows.
- iPad-only; no desktop version.
- Layers as frames approach does not scale well for complex animations.

## Notable uses

Procreate is widely used by professional illustrators, concept artists, and character designers for character and asset creation that feeds into game development pipelines. Studios like Superbrothers and independent developers use Procreate for initial character design; assets are exported and animated in dedicated tools or game engines. The iPad platform provides mobility; artists can iterate designs on-site or during travel.

## Community and ecosystem

Large community of professional artists on Procreate's forums, Discord, and social media. Extensive tutorials on YouTube and Procreate's official website. Brush packs and asset stores available on Gumroad and other platforms. Active community sharing on Instagram and ArtStation.

## Pricing details

Procreate is available exclusively on Apple App Store for a one-time purchase of $12.99 USD. Regional pricing may differ. No subscriptions, in-app purchases, or premium features beyond the initial purchase price. All updates are free. Works on all iPads running iOS 17.4 or later (most iPad Pro, iPad Air, iPad, and iPad mini models from recent years).

Procreate Dreams, the animation companion app, is a separate $12.99 purchase (see Procreate Dreams section).
