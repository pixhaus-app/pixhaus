# Rive

## Quick facts

- Vendor / maintainer: Rive (formerly Flare, founded by Two Chairs Inc.)
- License / pricing model: Freemium cloud editor with free runtime libraries
- Price point (current): Free (editor with basic features), Rive Subscription (Infinity plan, pricing varies)
- Platforms: Web-based editor (Windows, macOS, Linux via browser); runtimes across web, mobile, desktop, game engines
- First released: 2016 (as Flare); rebranded as Rive in 2020
- Last meaningful update: Actively maintained with regular feature releases
- Source available: Runtimes are open-source (GitHub); editor is proprietary web app
- Primary use case: Interactive state machine-driven animation for web, mobile apps, and games

## Origin and purpose

Rive (formerly called Flare) was created by Two Chairs Inc. to solve animation delivery for interactive experiences. Unlike traditional skeletal animation tools optimized for game characters, Rive targets interactive UI, mascots, loading states, and animated graphics that respond to user input or real-time data.

The software's core innovation is state machines: instead of linear animation timelines, Rive structures animation as a graph of states (idle, hover, pressed, loading, success, error) connected by transitions triggered by input or events. This enables animations that adapt dynamically to application state without requiring frame-by-frame pre-baking of every possible outcome.

The emphasis on runtimes (shipped as open-source libraries) rather than proprietary formats makes Rive portable: a single animation can ship on web, iOS, Android, and game engines without re-exporting or format conversion.

## Rigging workflow

Rive does not use traditional skeletal rigging. Instead, it uses shape-based animation with bones that deform shapes, similar to mesh deformation systems.

The workflow begins with creating vector shapes or importing artwork:

1. Draw or import shapes (rectangles, circles, paths, or images)
2. Create bones that can control shape position, rotation, and scale
3. Define how shapes deform when bones move (through constraint-like relationships)
4. Create animations (timelines) that set keyframes for bone transforms
5. Build a state machine layer that combines animations with transition logic

Bones in Rive are simpler than in skeletal animation tools: they primarily control transform (position, rotation, scale) rather than mesh deformation. However, Rive supports shape constraints that create complex deformation effects (e.g., IK-like behavior through constraints).

Nested artboards allow building complex hierarchies: a character can be composed of multiple artboards (body, head, limbs) that animate together, useful for modularity.

## Mesh deformation

Rive supports mesh deformation through shape manipulation. Shapes (paths) can have vertices that are influenced by bones. When bones move, vertices deform, creating natural limb bending and character movement.

Constraint system allows non-bone-driven shape behavior:

- Distance constraints: Keep vertices at fixed distances
- Angle constraints: Maintain angle between points
- Transform constraints: Link shape transforms to bone transforms

These constraints enable soft-body-like effects and complex deformation without explicit mesh weighting.

Blend shape support allows morphing between different shape configurations, useful for facial expressions or costume changes.

Mesh deformation is less sophisticated than dedicated skeletal tools (Spine, DragonBones) but sufficient for interactive UI and moderately complex characters.

## Inverse kinematics and constraints

Rive uses constraints rather than traditional IK. Constraints define relationships between elements:

- Distance constraints: Maintain distance between two points
- Angle constraints: Fix angle between bones
- Transform constraints: Copy transforms from one bone to another

These constraints are applied in real-time during animation playback, enabling IK-like behavior without explicit solving. For example, a distance constraint between hand and target point creates automatic hand-to-target following.

Bone chains can be rigged using multiple constraints, allowing complex limb behavior driven by constraint resolution.

The constraint system is flexible but requires more manual setup than Spine's built-in IK.

## Animation timeline and tweening

Rive's animation model combines timelines with state machines. Timelines define keyframe animation (similar to traditional tools), but timelines are wrapped within state machine states.

Timeline features:

- Keyframes for bone transforms, shape properties, and parameters
- Bezier curve tweening with adjustable handles
- Multiple animation layers within a single artboard

State machine layer connects animations:

- States represent animation sequences (idle, walk, jump, land)
- Transitions define how states connect with conditions
- Conditions are driven by inputs (user gestures, game variables, time-based triggers)
- State machines can be nested (a state can contain a sub-state machine)

Parameter system allows data-driven animation:

- Define parameters (numeric values, booleans, trigger events)
- Keyframe parameter values in timelines
- Use parameters to drive state transitions or shape deformation

Real-time parameter updates allow animation to respond immediately to external data: update a parameter from code, and the animation adapts instantly.

## Skinning and weights

Rive uses automatic shape deformation without traditional weight painting. Shapes are bound to bones through proximity or explicit constraints.

When a bone moves, all vertices of shapes near it move with distance-based falloff. This automatic approach requires less manual setup but provides less control than Spine's weight painting.

Multi-bone influence is implicit: vertices near bone boundaries are influenced by multiple bones with automatic blending.

Shape blending allows morphing between different geometry for expressions or costume swaps.

## Export and runtime integration (this section is critical for skeletal tools)

Rive exports to a proprietary binary format (.riv) with an accompanying JSON sidecar for metadata. The binary format is compact and optimized for fast loading.

Runtime libraries are open-source and available on GitHub across multiple languages and platforms:

- **Web (JavaScript/WASM)**: Official runtime with WebGL rendering; tiny footprint (~300 KB gzipped for full feature set)
- **iOS (Swift)**: Official native runtime
- **Android (Kotlin/Java)**: Official native runtime
- **Flutter**: Official Flutter plugin for cross-platform mobile
- **React Native**: Official React Native plugin
- **Unity**: Official Unity plugin with editor integration
- **Unreal Engine**: Official Unreal plugin (UE4/5)
- **C++ (custom engines)**: Official C++ runtime for custom game engines
- **Python**: Community runtime for scripting
- **Go, Rust, C#, Java**: Community runtimes available

All runtimes are under the MIT or Unlicense, allowing unrestricted distribution.

Runtime capabilities:

- Load .riv files and playback animations
- Evaluate state machines in real-time
- Parameter-driven animation with instant updates
- Event triggering (callbacks from animations)
- Bone and shape querying (get current positions, scales, rotations)
- Custom constraint types through scripting

File sizes are exceptionally small. A complex interactive character animation can be 30-50 KB, compared to megabytes for video or animated GIF equivalents.

## Engine integration

Web integration is seamless via the JavaScript runtime. Applications can instantiate Rive artboards, drive parameters from code, and listen to animation events.

React integration: Official React component (react-rive-canvas) simplifies embedding animations in React apps.

Vue, Angular, Svelte: Community integrations available.

Mobile integration via official iOS and Android SDKs is native and performant. Runtimes use hardware acceleration where available.

Flutter integration enables cross-platform mobile animation with a single codebase.

Unity integration is via official plugin. Animations can be embedded in UI or world space with full parameter control from C# scripts.

Unreal integration uses the official plugin, supporting both Blueprint and C++ workflows.

Game engine consistency is high: a Rive animation behaves identically across web, mobile, and game engine platforms.

## Scripting and extensibility

Rive editor has limited scripting (no visual scripting language built into the editor).

Runtime scripting is available through each platform's SDK:

- JavaScript: Direct parameter control, event listening, animation playback control
- C++/C#: SDK APIs for the same features
- Mobile: Platform-specific SDKs (iOS/Android) expose the same control surface

Custom constraints can be created in some runtimes (especially C++ and web), allowing specialized animation behaviors.

The open-source runtimes are forkable, enabling custom modifications.

Plugins (for various app frameworks like React, Vue) extend Rive's capabilities in specific environments.

## Workflow strengths

- State machines: Industry-leading approach to interactive animation; intuitive visual paradigm for defining animation logic
- Real-time parameter updates: Animation responds instantly to code-driven parameter changes without re-baking
- Small file sizes: Exceptional compression; ideal for web and bandwidth-limited scenarios
- Cross-platform consistency: Single animation file runs identically on web, iOS, Android, Unity, Unreal
- Open-source runtimes: No licensing restrictions; full source available
- Modern tooling: Polished, browser-based editor with responsive UI and intuitive design
- Low-cost entry: Free editor and runtimes reduce barrier to adoption
- Enterprise adoption: Used by major companies (Spotify, Duolingo, Disney, Google, automakers)
- Event system: Built-in event triggering enables game-like interactivity without code

## Workflow gaps

- Not designed for frame-by-frame animation: Better suited to interactive UI than complex character animation sequences
- Limited mesh deformation: Less sophisticated than dedicated skeletal tools; complex organic deformation requires workarounds
- Shape-based (not bone-based): Bone hierarchies are simpler; no advanced constraints like path or scale
- No procedural animation generation: Must manually define all animations; no automated walk cycle or procedural effects
- Learning curve for state machines: State machines are powerful but require different thinking than timeline-based tools
- Limited physics: No built-in ragdoll or soft-body dynamics (though external physics can be integrated)
- File format lock-in: Proprietary binary format; no open standard equivalent

## Notable uses

Rive is used by major companies for interactive experiences:

- **Duolingo**: Character animation system for learning app mascots; uses Rive state machines for character expressions and interactions
- **Spotify**: Interactive animated assets in the app
- **Disney**: Interactive experiences and UI animation
- **Google**: Interactive documentation and UI
- **Major automakers**: Interactive in-car entertainment and UI

The software is popular for UI animation, loading states, and interactive mascots across web and mobile applications.

## Community and ecosystem

Rive has an active community on GitHub, Twitter/X, and a web-based discussion forum. Official documentation and tutorials are comprehensive.

Community contributions to runtimes are welcome. Third-party plugins and integrations (React components, Vue plugins) are actively developed.

Asset marketplace: Rive files can be shared and sold through gumroad and other platforms.

Educational resources: YouTube tutorials and written guides from Rive and the community.

Discord community: Active Discord server for questions, feedback, and networking.

## Pricing details

Rive offers a freemium model:

**Free Plan**: Includes:
- Cloud-based editor with all features
- Unlimited projects
- Unlimited sharing of public animations
- Full runtime access (all open-source libraries)
- Community support

**Rive Infinity**: Premium subscription with additional features:
- Private projects
- Team collaboration (multiple users per project)
- Priority support
- Advanced features (varies by plan tier)

Specific pricing for Rive Infinity is not published on the main website; it appears to be available through inquiry or subscription tiers on their site. Pricing is likely in the range of $10-30/month based on industry standards, but exact rates should be verified on rive.app.

No per-runtime licensing: All runtimes are free and open-source, with no restrictions on commercial use or distribution.

Educational discounts: Not published, but likely available upon inquiry.

Free trial: The free plan is perpetual; no trial period needed.

All pricing is transparent regarding runtime use: Rive makes revenue through editor subscriptions and premium features, not through runtime licensing, making it cost-effective for developers at scale.
