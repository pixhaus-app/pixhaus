# Live2D Cubism

## Quick facts

- Vendor / maintainer: Live2D Inc. (Japan)
- License / pricing model: Freemium editor with commercial SDK licensing
- Price point (current): Free (editor with limitations), Pro $10/month or $200 one-time, SDK licensing variable
- Platforms: Windows, macOS
- First released: 2012
- Last meaningful update: Actively maintained with regular feature updates
- Source available: No (proprietary)
- Primary use case: 2.5D character animation for VTubers, gacha games, and interactive mobile apps

## Origin and purpose

Live2D Cubism was developed by Live2D Inc. and released in 2012 as a 2.5D character animation solution. Unlike full 3D or traditional 2D skeletal animation, Live2D creates the illusion of 3D movement by deforming a single 2D illustration using mesh manipulation. This approach became dominant in the VTuber industry (popularized by Hololive and Nijisanji) because it balances animation expressiveness with the minimal computational cost of 2D rendering.

The software targets content creators, VTubers, mobile game developers, and gacha game studios where character expressiveness and visual fidelity are paramount. The 2.5D approach preserves the original illustration style while enabling smooth head turns, body movement, facial expressions, and emotion-driven animations.

## Rigging workflow

Live2D Cubism's rigging process begins with a single 2D illustration, typically a PNG or PSD file. Unlike skeletal systems that require separate artwork for each limb, Cubism works with a flat composite image and partitions it into deformable regions.

The rigging workflow involves:

1. Importing the source illustration (usually a drawn or painted character)
2. Creating a deformation mesh overlaid on the image; this mesh defines how the image will stretch and warp
3. Placing deformation mesh bones within the mesh; these bones control local region deformation
4. Setting up parameters that drive bone movement (head X/Y rotation, eye blink, mouth open, etc.)
5. Binding bone movements to parameter values through the Mesh Transformation interface

Parameters are the core abstraction. Rather than animating bones directly, artists create sliders (parameters) like "head rotation" or "happiness" and map bone movements to those parameter ranges. This design supports interactive animation: the runtime applies parameter values (from facial tracking, game state, or user input) and interpolates bone positions in real-time.

Multi-parameter deformation allows layering multiple parameter influences on a single bone. For example, head rotation and flinch reaction might both drive eye position; the system blends them based on parameter values.

## Mesh deformation

Live2D uses Free-Form Deformation (FFD) via a triangulated mesh overlay. The mesh is user-defined, with vertices placed over the illustration to define deformable regions. Mesh triangulation can be automatic or manual.

Each mesh is bound to one or more deformation bones. When a bone moves, all vertices weighted to it deform proportionally. The weighting is automatic based on vertex distance from the bone.

Advanced deformation uses curve constraints: Bezier curves can be drawn over the illustration (e.g., along a limb outline) and bones are constrained to follow these curves. This ensures deformations respect the artist's drawn shape.

Rotation deformation is supported: bones can rotate to twist limbs or tilt the head. Combined with translation, this creates convincing bending and squashing effects.

Opacity masks allow selective deformation: some regions of the mesh can be opacity-masked to hide or reveal artwork during deformation (e.g., eyes behind closed eyelids).

Mesh switching enables swapping different mesh variants during animation (e.g., different mesh topologies for different head angles, or costume changes).

## Inverse kinematics and constraints

Live2D does not use traditional inverse kinematics in the sense of bone chains solving to an endpoint. Instead, it uses parameter-driven bone transforms and curve constraints.

Curve constraints are the closest analog: an artist draws a Bezier curve, and a bone is constrained to follow that curve. Animating along the curve produces natural motion without requiring explicit IK solves.

Rotation limits can be applied to prevent bones from rotating beyond plausible ranges (e.g., head rotation limited to ±45 degrees).

Blend mode constraints allow one parameter to influence another, enabling secondary motion effects (e.g., head tilt affecting ear position).

## Animation timeline and tweening

Live2D's animation model is parameter-centric rather than bone-centric. Instead of keyframing bone positions, artists create animation curves for parameters.

The timeline shows all parameters and their values over time. Each parameter has an animation curve with keyframes. Tweening options include:

- Linear interpolation
- Bezier easing curves
- Stepped (instant transitions)
- Additional specialized curves for common motion types

Parameter animation supports multiple curves simultaneously: a character can smile (mouth parameter), blink (eye parameter), and turn its head (rotation parameter) all at once, with independent easing for each.

Lip-syncing is built-in: mouth parameter values can be bound to phoneme timing from imported audio, allowing automatic mouth animation from speech.

Layered animation is supported through the "Expression" system: multiple animation layers can be blended together (e.g., idle animation + talking animation + emotion animation), with each layer modulating parameter values.

Animation preview runs at real-time framerate, allowing immediate visual feedback.

## Skinning and weights

Live2D does not use traditional skinning weights. Instead, vertices of the deformation mesh are bound to bones by proximity. A vertex is influenced by the closest bone(s) with weight decreasing by distance.

The binding is automatic but can be refined through a Weight Adjustment interface. Manual weight painting is not available; instead, artists use blend modes and constraint properties to refine deformation behavior.

Multi-bone influence is implicit: vertices near bone boundaries are influenced by multiple bones with distance-based falloff.

Normal map deformation can preserve silhouette sharpness: the mesh itself deforms smoothly, but surface normals adjust to maintain hard edges where needed.

## Export and runtime integration (this section is critical for skeletal tools)

Live2D exports to a proprietary binary format (.moc3 for Cubism 3, .moc for Cubism 2.1). The format includes mesh geometry, parameter definitions, animation curves, and texture references.

SDK licensing is complex and depends on use case:

**Editor Licensing (Cubism Editor):**
- Free version: Available perpetually with limitations (no SDK output, limited export)
- Pro version: $10/month or $200 one-time, includes SDK export and advanced features

**SDK/Runtime Licensing (for distributing applications):**

Live2D distinguishes between "indie" (under ¥10 million / ~$67k USD annual revenue) and commercial (¥10 million+) use.

- Indie: Free SDK use for individuals and small enterprises
- Commercial: Requires Publication License Agreement (PLA) with Live2D Inc. Licensing terms are negotiated per case, typically involving revenue sharing or fixed licensing fees
- VTuber streamers: If annual revenue from streaming exceeds thresholds, a commercial license is required even if earning under ¥10 million total

Runtime libraries are available for:

- Web (JavaScript/WebGL)
- iOS (Objective-C/Swift)
- Android (Java/Kotlin)
- PC (C++ and C#)
- Console (varies by console partnership with Live2D)
- Unreal Engine (plugin available)
- Unity (third-party plugins, not official)

Runtimes support:

- Parameter animation playback
- Audio-driven lip-syncing
- Facial motion capture integration (through third-party systems)
- Mouse/touch interaction (parameter driving)
- Texture swapping and costume changes

File sizes are modest (typically 1-5 MB for complex VTuber models) due to the 2D mesh and curve-based approach.

## Engine integration

Web integration is native via JavaScript runtime. The runtime loads .moc3 files and renders via WebGL, making Live2D suitable for browser-based applications, streaming overlays, and web games.

Mobile integration is well-established through official iOS and Android SDKs. VTuber streaming software (OBS, Streamlabs, Twitcasting) have built-in Live2D support.

PC integration uses C++ or C# runtimes. Games on Steam often use Live2D for character UI or animations.

Unreal Engine integration is available via official plugin, supporting real-time animation and interactive parameter control.

Godot and other engines are not officially supported; third-party plugins exist.

Cross-platform consistency is high: a Live2D model looks and animates identically across web, iOS, Android, and PC platforms.

## Scripting and extensibility

Live2D provides limited editor scripting. The export format is proprietary, and custom loaders are not commonly written.

Runtime scripting is available through platform-specific APIs:

- JavaScript: Direct DOM APIs to control Live2D runtime objects; listen to animation events; drive parameters from code
- C++/C#: SDK APIs to load models, update parameters, query animation state, and respond to events
- Mobile: Platform-specific SDKs (iOS/Android) expose animation control APIs

Parameter-driven design simplifies integration: game logic can set parameters to control animation without managing bone transforms or mesh state directly.

Motion capture integration is supported through third-party libraries (e.g., MediaPipe for facial tracking, or dedicated mocap systems) that drive parameter values in real-time.

## Workflow strengths

- VTuber industry standard: Dominates the 2D VTuber ecosystem; most professional VTubers use Live2D models
- Illustration preservation: Works with original 2D artwork; no need to separate artwork into body parts
- Parameter-driven design: Enables interactive animation driven by external input (facial tracking, game state, user interaction)
- Lip-syncing: Built-in audio-driven lip-syncing with phoneme timing
- Expressiveness: 2.5D deformation creates convincing head turns and body movement from a single image
- Fast iteration: Real-time preview and parameter animation enable quick refinement
- Cross-platform: Web, mobile, PC, and console support with consistent results
- Streaming integration: Native support in OBS, Twitcasting, and other streaming software

## Workflow gaps

- Single-image constraint: Works best with a single illustration per character; different expressions require multiple variants
- 2.5D limitations: Movement is restricted to mesh deformation of the original image; true 3D-like rotations beyond ~90 degrees don't look natural
- No skeletal hierarchy: Bones are positioned individually; no parent-child relationships or animation blending between bone hierarchies
- Manual mesh creation: Mesh topology must be hand-defined; automatic mesh generation is limited
- Curve constraint learning curve: Curve constraints are powerful but require understanding how to layer and blend them
- Limited physics: No built-in ragdoll or secondary motion dynamics (external physics systems can be integrated)
- SDK licensing complexity: Commercial use requires navigating complex VTuber licensing terms with Live2D Inc.

## Notable uses

Live2D is ubiquitous in the VTuber industry. Notable VTuber agencies using Live2D include:

- Hololive (virtually all talents use Live2D models)
- Nijisanji (large portion of talents)
- VShojo (multiple talents)

Game industry uses include:

- Genshin Impact (character UI and promotional artwork)
- Various gacha games (character animations, story sequences)
- Mobile games across Asia and the West

## Community and ecosystem

Live2D has a large community centered around VTuber and content creator ecosystems. Official documentation and tutorials are available from Live2D Inc. Third-party tutorial creators publish extensive guides on YouTube and blogs.

A marketplace exists for pre-made Live2D models (BOOTH, gumroad, ArtStation) where artists sell character models. Affordable licenses make custom Live2D models accessible to aspiring VTubers.

The rigging community is active, with experienced rigging artists offering services to create custom models. Rigging costs range from several hundred to several thousand USD depending on detail and expressiveness.

Official forums and social media (Twitter/X) are monitored by Live2D staff.

## Pricing details

Live2D Cubism Editor pricing:

- Free: Perpetual, with limitations on export formats and SDK output
- Pro: $10/month (subscription) or $200 USD one-time purchase (exact pricing varies by region)

A 42-day free trial of the Pro version is available; after expiration, the account reverts to Free version.

Student discounts: As of early 2026, students can purchase the 3-Year Pro Plan at 80% off (effectively $40 for three years of Pro access).

Seasonal sales: Live2D runs sales multiple times per year (Holiday, May, Summer, Autumn) offering 20% off.

SDK licensing is negotiated on a per-use basis. Indie developers (under ¥10 million annual revenue) use the SDK free. Commercial entities and high-earning VTubers must execute a Publication License Agreement with Live2D Inc. Terms vary widely; typical arrangements involve either:

- Revenue sharing (Live2D takes a percentage of earnings)
- Fixed annual licensing fee (varies by application tier)
- Per-copy licensing (rare, typically for gacha games)

Educational institutions: Live2D leases Pro licenses free to schools; over 200 institutions currently use Live2D in education.

All pricing in JPY (Japanese Yen) is listed on the official site; USD equivalents are approximate and subject to exchange rate fluctuation.
