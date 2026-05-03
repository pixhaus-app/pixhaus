# Cavalry

## Quick facts
- Vendor / maintainer: Canva (acquired Scene Group in 2026)
- License / pricing model: Free (as of April 2026)
- Price point (current): Free (formerly $99 USD/month pro subscription)
- Platforms: macOS, Windows
- First released: 2020
- Last meaningful update: Acquisition by Canva (April 2026); ongoing development
- Source available: No
- Primary use case: Procedural motion graphics and animation; data visualization; generative design

## Origin and purpose

Cavalry was created by Scene Group as a modern motion graphics and animation tool combining 3D-like workflows with 2D simplicity. The software targets designers transitioning from After Effects to procedurally-driven animation. Cavalry emphasizes non-destructive, node-based workflows where animation properties are controlled via mathematical logic (Falloffs, Effectors) rather than manual keyframing. In April 2026, Canva acquired Cavalry and made the full software free, dramatically expanding accessibility. Cavalry is relevant to game development for procedural animation, data visualization, and motion-driven UI design.

## Drawing and painting tools

Cavalry includes basic shape and drawing tools (rectangle, circle, polygon, pen), but drawing is not the primary focus. The software assumes asset import: designers typically import vector artwork from Illustrator or Figma, or create simple shapes within Cavalry and animate them. Strokes and fills are colored via a color picker. Text tools create animated typography. The emphasis is on animation and motion design, not creation of original artwork.

## Animation timeline structure

The timeline displays frames horizontally with layers/objects vertically organized. Keyframes are marked; tweened frames are calculated automatically. The timeline is secondary to the node graph: most animation logic is driven by the node-based system (Falloffs, Effectors, mathematical expressions) rather than traditional timeline keyframing. Advanced animators work primarily in the node graph, not the timeline. Playback is real-time (not frame-by-frame like traditional animation tools).

## Frame-by-frame workflow (onion skin, lightbox, hold frames, blank frames)

Frame-by-frame animation is not a primary workflow in Cavalry. The software is designed for keyframe-based motion and procedural animation. Onion skin is not emphasized. Hold frames and blank frames are supported at the timeline level but are not central to Cavalry's philosophy. Artists using Cavalry rarely work frame-by-frame.

## Tweening and interpolation

Automatic keyframe interpolation is built-in. When animators set a property value at frame 1 and frame 10, Cavalry calculates frames 2-9 automatically with easing control (linear, ease-in, ease-out, custom curves). The timing editor visualizes and adjusts curves. However, Cavalry's procedural system (Duplicator, Falloffs, Effectors) often eliminates the need for manual keyframing entirely. For example, the Duplicator creates 100 instances of an object; Falloffs mathematically control each instance's position, rotation, scale without individual keyframes.

## Rigging and deformation

Rigging for character animation is not a primary feature. Cavalry is not designed for character skeleton rigging like Moho. However, Cavalry's deformation tools include:

- **Transform deformers**: Warp, twist, bend effects on artwork
- **Bone deformers**: Simple bones for deforming shapes (less sophisticated than Moho)

These are used for object deformation in motion graphics, not character performance animation. Game developers seeking character rigging should use Moho, Harmony, or Krita, not Cavalry.

## Vector vs raster

Cavalry works primarily with vector artwork. Imported vector files (SVG, Adobe Illustrator) are supported natively. Bitmap artwork (PNG, JPEG) can be imported but is treated as a static element. Strokes and fills in Cavalry are vector-based. This means scaled and rotated animation elements remain crisp.

## Color and palette workflow

Color management is basic. A color picker applies colors to shapes. Per-keyframe color changes are supported by keyframing color properties. There is no sophisticated palette system. For game development, this is adequate for simple sprite animation but limiting for character-heavy projects.

## Layer system

Objects (similar to layers) are displayed in the timeline with visibility, lock, opacity, and transform controls. Objects can be nested into groups. The hierarchy is straightforward. The node graph (distinct from the timeline) represents animation logic: nodes for position, rotation, scale, and custom effectors connect to define motion. This dual-view approach (timeline + node graph) is powerful for complex animation rigs but steeper for beginners.

## Export and import (critical: which formats game devs actually use)

Cavalry exports options:

- **MP4 / WebM**: Video formats for preview and delivery
- **PNG sequence**: Frame-by-frame PNG files
- **SVG sequence**: Vector format (one file per frame, unusual but supported)
- **JSON**: Custom format for procedural animation data
- **Lottie**: Web animation format (JSON-based, used in web and mobile)

For game developers:
- Sprite sheet export is not native. Developers export PNG sequences and use external tools (ImageMagick, Aseprite) to assemble into sprite sheets.
- **Lottie export is interesting for games**: Lottie is a popular format for lightweight web/mobile animation. Some indie game frameworks (e.g., web-based games) can render Lottie directly, eliminating the sprite sheet step.
- Direct game engine export is absent for traditional engines (Unity, Godot). For web-based games, Lottie + framework support provides direct integration.

This is a divergence point: Cavalry is better suited to web/mobile games than desktop games.

## Scripting and extensibility

Cavalry does not expose a public scripting API as of 2026. The node-based system is the primary extensibility mechanism. Advanced users create custom node structures to automate animation logic. Plugins or script extensions are not available. The node system is powerful enough for most procedural animation without additional scripting.

## Engine integration

No direct integration with traditional game engines (Unity, Godot, Unreal). However, Cavalry's Lottie export is compatible with game frameworks that support Lottie (e.g., Rive, which is used in some indie games). For web-based games, Lottie + Cavalry → direct web animation without sprite sheets is a viable workflow.

## Workflow strengths

1. Free (as of April 2026; major change)
2. Procedural animation via Duplicator and Effectors is dramatically faster than manual keyframing
3. Node-based system is powerful for complex, reusable animation rigs
4. Real-time preview without render steps
5. Professional motion graphics quality suitable for commercials and broadcasting
6. Lottie export enables direct web/mobile animation integration
7. Non-destructive workflow (changes to nodes don't destructively alter frames)
8. Suitable for data visualization and generative animation

## Workflow gaps

1. No sprite sheet export (requires external tool chaining for game integration)
2. Not designed for character animation (lacking skeletal rigging and facial controls)
3. Learning curve is steep (node-based workflows require understanding of graph logic)
4. Frame-by-frame animation is not optimized (use TVPaint or Harmony)
5. No professional support or training (given recent acquisition, infrastructure may change)
6. Onion skin and traditional animation tools are absent
7. Not suitable for hand-drawn or traditional animation workflows

## Notable uses (especially game-related uses)

- **Motion graphics for advertising**: Professional-grade animated ads and commercials
- **Data visualization**: Animated charts, graphs, and infographics
- **Web animation**: Animated graphics for websites and online experiences
- **UI motion design**: Animated interactions for apps and interfaces
- **Generative art**: Procedural animation and visual effects
- **Indie web games**: Lottie export enables direct integration into web-based games

Game adoption is niche. Cavalry is better suited to web/mobile games and motion graphics than to traditional sprite-based games.

## Community and ecosystem

- Active community due to free release in 2026 (recent adoption surge)
- Official Cavalry Academy provides tutorials and documentation
- YouTube tutorials are increasing as the tool gains visibility
- Forums and Discord communities are growing
- School of Motion and other design education platforms teach Cavalry
- No third-party plugin ecosystem (not applicable to node-based software)

## Pricing details

Cavalry is now free (as of April 2026), a major change from prior subscription model:

### Previous Model (pre-April 2026)
- Pro plan: $99 USD/month (subscription)
- Shared features with Figma Pro and other Canva products

### Current Model (April 2026 onward)
- **Free**: Full version available at no cost, including all features (motion design, procedural animation, Lottie export, professional export options)
- Commercial use permitted
- No forced updates or locked features

The free release was a strategic move by Canva to integrate Cavalry into its ecosystem and build adoption. Users can now access professional motion graphics software without subscription barriers.
