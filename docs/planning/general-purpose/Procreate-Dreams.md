# Procreate Dreams

## Quick facts
- Vendor / maintainer: Savage Interactive
- License / pricing model: One-time purchase for iOS
- Price point (current): $12.99 USD (one-time purchase on App Store)
- Platforms: iPad (iPadOS 16.3 or later)
- First released: 2023
- Last meaningful update: Dreams 2 released in 2025 with redesigned timeline and advanced features
- Source available: No. Proprietary.
- Primary use case: Frame-by-frame and keyframe animation for iPad with native sprite export

## Origin and purpose

Procreate Dreams was released in 2023 as a dedicated companion animation application to the main Procreate painting app. Savage Interactive designed Dreams to address the gap between Procreate's illustration focus and the animation needs of game developers and motion designers. Rather than bloating the main Procreate app with animation tools, the company created a specialized, gesture-driven animation suite optimized for iPad interaction. Dreams 2, released in early 2025, significantly expanded capabilities with a redesigned timeline, multiple composition modes, and advanced export options. The application maintains Procreate's philosophy of one-time purchase with no subscriptions.

## Drawing and painting tools

Procreate Dreams includes the full Procreate brush library (300+ brushes) for frame-by-frame drawing. Brushes can be customized in the Brush Editor. Each frame can be drawn with any available brush, allowing artists to layer textures, effects, and details across frames. Pressure sensitivity and Apple Pencil tilt are fully supported. Stroke stabilization reduces jitter. Layers within each frame support blending modes and opacity adjustment. The drawing experience mirrors Procreate's familiar interface, reducing switching friction for artists moving between illustration and animation.

## Pixel-specific features (or "How artists use it for sprite work")

Procreate Dreams does not have a dedicated pixel-art mode. However, pixel-art sprite animation is achievable via:
- Using 1-pixel hard brushes with anti-aliasing disabled.
- Enabling grid display and snapping to grid.
- Working at integer zoom levels.

Many indie game developers use Dreams for sprite animation, particularly for character animation, particle effects, and UI animations. The full brush library allows stylized sprite work beyond strict pixel constraints. For retro pixel-perfect games, artists may need to enforce color and dimension constraints externally.

## Color and palette workflow

Procreate Dreams inherits Procreate's color system. Full RGB and Display P3 color space support. Color picker with hue/saturation/brightness sliders. Palette import/export for consistency across frames. Color Dynamics for brush effects. No indexed color mode or strict palette restrictions; artists must enforce color budgets externally if required for retro game compatibility.

## Layer system

Within each frame, Procreate Dreams supports multiple layers. Layers can be grouped. Adjustment layers provide non-destructive color correction. Layer masks enable selective transparency. Blend modes (Multiply, Screen, Overlay, Color Dodge, etc.) are standard. For multi-layer animation (e.g., character with separate limbs, effects, backgrounds), layers within frames allow complex compositions. Layer organization is critical for large animation projects; users can organize layers by body part, effect, or other logical grouping.

## Animation features

Procreate Dreams is animation-first and includes comprehensive timeline controls.

**Timeline**: The redesigned timeline in Dreams 2 (2025) displays frames horizontally with a playhead. The timeline shows multiple tracks for different animation sequences or layers (e.g., character walk cycle, attack animation, idle loop). Tracks can be named and color-coded for organization. Frame duration is adjustable per-frame (100ms, 200ms, etc.) or globally. Looping behavior is configurable (loop, one-shot, bounce).

**Keyframe animation**: Supports keyframe-based animation for motion graphics. Properties like scale, rotation, position, opacity, and blend modes can be keyframed. Dreams automatically interpolates between keyframes with easing options (linear, ease-in, ease-out, custom).

**Onion Skin**: The onion skin docker shows semi-transparent previews of surrounding frames. Opacity, frame count, and color tinting are adjustable. This is essential for frame-by-frame animation quality and consistency.

**Flipbook**: The Flipbook mode provides a lightweight frame-by-frame drawing mode focused on rapid sketch and iteration. Flipbook supports infinite tracks in Dreams 2, allowing multiple animation sequences within a single project.

**Playback**: Real-time playback within the app allows instant preview of animations. Playback speed is adjustable. The Squeeze feature allows editing while playback continues, keeping artists in creative flow.

**Export**: File > Export offers multiple formats:
- GIF (animated, loopable).
- Transparent video (ProRes or H.264 with transparency).
- MP4 video.
- Frame sequence (individual PNG files).
- Sprite sheet (PNG with automatic grid layout, suitable for game engine import).

Sprite sheet export is native and optimized; no external plugins required.

**Composite and effects**: Dreams supports layer compositing with blend modes, masks, and adjustment layers per-frame. Video editing features allow importing video clips, adding audio, and integrating hand-drawn animation with filmed elements.

## Export and import

Procreate Dreams natively saves to the Dreams project format (proprietary). Supports import of Procreate PSD files, PNG sequences, and video files. Export options include:
- Animated GIF.
- Transparent video (ProRes MOV or H.264 MP4).
- Frame sequence (PNG, JPEG, TIFF).
- Sprite sheet PNG (with automatic frame grid layout suitable for game engines).

For sprite work:
- Sprite sheet export directly generates a single PNG with all frames tiled in a grid, eliminating manual layout work.
- PNG sprite sheet includes frame position data suitable for game engine sprite importers.
- Transparent video export for engine integration.
- Individual frame sequence for custom processing.

## Scripting and extensibility

Procreate Dreams does not support plugins or scripting. Workflows are extended via keyboard shortcuts, gesture customization, and brush creation. The API is closed; third-party developers cannot write extensions.

## Engine integration

Procreate Dreams is not a game engine. Animations exported as sprite sheets or video integrate directly into game engines (Unity, Godot, Unreal, GameMaker). Sprite sheet export is optimized for game-engine sprite importers, making integration straightforward. Many indie game studios use Dreams for sprite animation and export directly to their engine's asset pipeline.

## Workflow strengths

- Dedicated animation tool; all features are animation-focused.
- Native sprite sheet export with no plugins required.
- Full keyframe and onion-skin support.
- Real-time playback and Squeeze feature for continuous editing.
- Intuitive iPad interface with gesture controls.
- One-time purchase with free updates; no subscription.
- Seamless integration with Procreate for illustration and animation pipeline.
- Advanced export options (GIF, transparent video, sprite sheet, frame sequence).
- Professional-grade timeline and composition tools.

## Workflow gaps

- iPad-only; no desktop version.
- No rigged or skeletal animation.
- No audio sync or sound layer integration.
- Sprite sheet export is automatic but not customizable (grid dimensions, padding).
- No multi-artiste collaboration features.
- Limited undo depth on some operations compared to desktop tools.

## Notable uses

Procreate Dreams is increasingly used by indie game developers and animation studios for sprite animation, particularly for character animation, VFX, and UI animation in mobile and indie games. Professional animation studios also use Dreams for previs and concept animation on iPad. The seamless workflow between Procreate and Dreams makes it a natural choice for iPad-first game development pipelines.

## Community and ecosystem

Growing community of animators and game developers on Procreate's forums, Discord, and social media. YouTube tutorials on Dreams animation techniques. Brush packs and animation templates shared on Gumroad and other platforms. Integration with engine-specific workflows (e.g., importing Dreams sprite sheets into Godot) discussed in game development communities.

## Pricing details

Procreate Dreams is available exclusively on Apple App Store for a one-time purchase of $12.99 USD. Regional pricing may differ. No subscriptions, in-app purchases, or premium features beyond the initial purchase. All updates are free and delivered via App Store.

Works on all iPads running iPadOS 16.3 or later. Compatible with most iPad Pro, iPad Air, iPad, and iPad mini models released in recent years. The app takes advantage of larger screens on iPad Pro for timeline and docking panels but runs on any compatible iPad.

Procreate (the main painting app) is a separate $12.99 purchase; both are required for a complete illustration-to-animation workflow.
