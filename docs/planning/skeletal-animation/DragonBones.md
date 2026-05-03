# DragonBones

## Quick facts

- Vendor / maintainer: DragonBones community (originally Adobe-sponsored, now open-source)
- License / pricing model: Free and open-source (MIT-like)
- Price point (current): Free
- Platforms: Windows, macOS, Linux
- First released: October 2012 (first 2D skeletal animation solution released)
- Last meaningful update: Actively maintained by community
- Source available: Yes (GitHub)
- Primary use case: Free 2D skeletal animation for mobile games and web applications, especially popular in Asia

## Origin and purpose

DragonBones emerged in October 2012 as the first commercial 2D skeletal animation solution, preceding Spine. It was originally developed with Adobe sponsorship and distributed as both a free Flash Pro plugin and standalone tool (DragonBones Pro). The software evolved into an open-source project with runtimes in multiple languages.

DragonBones targets game developers seeking free skeletal animation with broad engine support. The open-source model eliminated licensing friction, making it popular in mobile game development, particularly in Asian markets where cost-free tooling is preferred.

The design philosophy emphasizes ease of learning, fast iteration, and minimal file overhead. Unlike Spine's more feature-complete rigging model, DragonBones prioritizes simplicity and cross-platform compatibility.

## Rigging workflow

DragonBones uses a hierarchical bone system similar to Spine. Bones are created as parent-child chains, positioned over artwork, and given transform properties (position, rotation, scale).

Artwork attachment is slot-based: slots hold image attachments that are switched during animation (e.g., swapping between open and closed hands).

Bone weighting is automatic: vertices are assigned to the closest bone by distance. Manual weight painting is less refined than Spine's system.

Pose tool support allows multi-bone IK to quickly position character poses. Selecting multiple bones and dragging creates automatic IK solving, useful for fast pose setup.

Constraints are available but simpler than Spine's:

- IK chain constraints with rotation limits
- Transform constraints (copy parent transforms)
- Distance constraints (maintain fixed distance between bones)

The constraint system is adequate for typical game character animation but lacks Spine's breadth of specialized constraints (path, scale, shear).

## Mesh deformation

DragonBones supports linear blend skinning with automatic weight generation. Mesh vertices are bound to bones with weights computed by distance. Manual weight refinement is available through a weight editor, though not as sophisticated as Spine's weight painting.

Mesh topology is user-defined via triangle lists. Automatic triangulation of convex shapes is supported.

Free-form deformation (FFD) is available as an alternative to bone-driven deformation, allowing direct vertex manipulation for fine-tuning.

Mesh variants can be swapped during animation, enabling costume changes or deformable shape transitions.

The deformation approach is functional but less polished than Spine's, with fewer options for controlling normal maps or advanced bulge effects.

## Inverse kinematics and constraints

DragonBones implements forward kinematics by default. IK chains are created by marking a bone as an IK target and specifying the root bone of the chain.

IK features include:

- Multi-bone chain solving
- Rotation limits per bone
- Bend direction (left/right)
- Stretch enable/disable

The IK implementation is straightforward and adequate for most character animation tasks. Performance is good even with multiple IK chains.

Pose tool uses IK internally to quickly position entire limbs by dragging the endpoint, useful for keyframe setup.

## Animation timeline and tweening

DragonBones uses a traditional frame-based timeline. Artists set keyframes for bone transforms at specific frames and define interpolation between them.

Tweening options include:

- Linear interpolation
- Bezier curves for smooth easing
- Stepped transitions

Timeline display shows a hierarchical bone tree with each bone's keyframes visible. The dopesheet shows all keyframes across all bones simultaneously.

Multiple animations can be stored in a single project; each animation is an independent timeline.

Animation events are keyable, allowing triggering of game callbacks (sound effects, particle spawns, gameplay events) at specific frames. Events carry string or numeric data.

The timeline is functional and responsive, though the UI is less polished than Spine's.

## Skinning and weights

Vertex weighting uses automatic generation based on bone distance. Vertices are assigned to the closest bone(s) with falloff.

Manual weight refinement is available through a weight editor interface, though more limited than Spine's brush-based painting. Weight normalization is automatic.

Multi-bone influence is supported: vertices can be weighted to multiple bones, creating smooth deformation across joints.

Heat map visualization shows weight distribution.

The weighting system is adequate for typical mobile game characters but produces less refined results than Spine for complex organic shapes.

## Export and runtime integration (this section is critical for skeletal tools)

DragonBones exports to JSON format with accompanying texture atlas files. The JSON format is human-readable and portable, containing skeleton data, animation definitions, skin definitions, and bone constraints.

Runtime libraries are available in multiple languages:

- **JavaScript/TypeScript**: Primary runtime for web and Egret engine
- **ActionScript 3**: Flash and Starling engine support
- **C++**: Cocos2d-x engine native support
- **C#**: Unity integration
- **Lua**: Cocos2d-x scripting
- **Java**: Android native support
- **Python**: Limited community support

All runtimes are open-source and available on GitHub (DragonBones/DragonBonesJS, DragonBones/DragonBonesCPP, etc.).

The JavaScript runtime supports the Egret game engine (a TypeScript-based engine popular in Asia) with first-class integration. Egret has built-in DragonBones support and optimized rendering.

PixiJS (a 2D WebGL renderer) has official DragonBones support via a plugin, enabling web game development.

Cocos2d-x has native DragonBones support through the C++ runtime, making it seamless to load and animate DragonBones files.

Unity integration is available through the C# runtime, though not as polished as Spine's official Unity plugin.

Key runtime features:

- Skeleton animation playback with bone transforms
- Mesh deformation with automatic weighting
- Attachment/skin swapping
- Event callbacks
- Animation blending and layering
- IK application
- Slot visibility toggling

File sizes are small (typically 10-100 KB for complex characters) with good compression via JSON or binary export variants.

## Engine integration

Web integration is straightforward via the JavaScript runtime. Files are loaded asynchronously and rendered via WebGL. Performance is good across modern browsers.

Egret integration is native: the engine includes built-in loaders and renderers for DragonBones files, making it the most seamless integration.

Cocos2d-x integration is native through the C++ runtime, with no external dependencies needed.

Unity integration uses the C# runtime. Asset import is not as automated as Spine's, requiring manual setup of skeleton and animation components.

Mobile (iOS/Android) integration uses native runtimes (Objective-C, Java) or cross-platform code via C++ through game engines.

Cross-platform consistency is good: animations look identical across web, mobile, and desktop platforms.

## Scripting and extensibility

DragonBones' editor has no built-in scripting. The export format is documented, enabling custom loaders and tools.

Runtime scripting is available through each language's SDK:

- JavaScript: Direct control of skeleton playback, parameter access, event listening
- C++/C#: SDK APIs for loading, animation control, bone manipulation
- ActionScript 3: Runtime API for Flash integration

Animation blending and state management are typically implemented in game code rather than the editor.

The open-source runtimes are forkable, allowing custom modifications for specialized workflows.

## Workflow strengths

- Free and open-source: No licensing costs; source code available for customization
- Quick learning curve: Simpler than Spine; good for beginners and small teams
- Multiple runtime languages: Extensive platform coverage (web, mobile, desktop, consoles via third-party runtimes)
- Egret integration: Seamless integration with the Egret engine, popular in Asian mobile game development
- Community runtimes: PixiJS and Cocos2d-x integration is well-established and documented
- Compact files: Efficient skeletal data and animation compression
- Good documentation: Official guides and community tutorials available
- Asset store: Affordable pre-made characters and animations available for purchase

## Workflow gaps

- Less feature-rich rigging: Constraints are simpler than Spine; no specialized constraints like path or scale
- Weight painting is less refined: Automatic weighting requires more manual cleanup than Spine's brush-based approach
- Editor UI is dated: Design feels less modern compared to Spine; smaller user base means slower UI improvements
- No integrated procedural animation: No built-in automated walk cycles or physics-driven effects
- Limited physics support: No ragdoll or soft-body dynamics
- Weaker mainstream adoption: Spine dominates the western market; DragonBones is stronger in Asia
- No official Unreal support: Community runtimes exist but lack official plugin support like Spine

## Notable uses

DragonBones is widely used in mobile games, particularly in Asian markets. Many titles across Asia use DragonBones for character animation, especially games developed in China and Japan.

Web games using the Egret engine often rely on DragonBones. Examples include various browser-based and mobile web games popular in Asia.

The software is popular among indie developers due to its free and open-source nature.

## Community and ecosystem

DragonBones has an active community on GitHub (DragonBones organization). The official website (dragonbones.github.io) provides documentation and download links.

Community contributions to runtimes are encouraged. Many third-party plugins and tools have been built around DragonBones (e.g., automation scripts, asset converters).

A marketplace exists for DragonBones assets (Gumroad, BOOTH) where artists sell character animations and rigs.

The community is particularly strong in Asia, with Chinese and Japanese user groups and documentation available.

English documentation is available but sometimes less complete than Chinese or Japanese documentation.

## Pricing details

DragonBones is entirely free. No licensing costs, no subscription, no commercial restrictions.

The editor is available for download from dragonbones.github.io. Platform-specific installers exist for Windows, macOS, and Linux.

All runtimes are open-source (MIT-like licenses) and available on GitHub. No runtime licensing fees or commercial restrictions apply.

Educational use is free. Commercial use is free.

Redistribution of software containing DragonBones runtimes is unrestricted, provided the open-source license is preserved.

No trial period needed; full editor available immediately after download.
