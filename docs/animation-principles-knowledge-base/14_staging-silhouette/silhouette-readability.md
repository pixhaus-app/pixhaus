# Silhouette Readability — The Two-Tone Test

> Instant readability
The silhouette test: if you fill the character's outline with solid black, can you still read the pose?

If yes — the silhouette works. If no — fix the pose.

## The principle

Audiences read silhouettes BEFORE they read details. A character against any background is first perceived as a shape. That shape carries the pose, the action, the intent.

A character whose silhouette is ambiguous (arms hidden behind body, head merged with shoulders) will not read on screen — no matter how beautiful the rendering.

## The two-tone test

Take any pose you've drawn or any prompt you're generating. Mentally fill it with solid black. Does the action read?

### Good silhouettes (test passes)
- A pointing arm extends clearly to one side of the body
- A leaping pose has clear gap between legs
- A character holding object: object visible against background
- A surprised pose: arms thrown out away from body

### Bad silhouettes (test fails)
- Pointing arm overlaps body — looks like a single blob
- Object held in front of chest — disappears into body shape
- Character with hands in pockets — body becomes amorphous
- Head buried in shoulders — character looks decapitated

## The "profile rule"

> Profiles for legibility.
Sometimes the strongest silhouette is the side view. A pure profile shows:
- Nose direction (which way is character looking)
- Chest/back shape
- Front of body / back of body
- Arms separated from body

For key action poses, consider angling the character into profile. It's almost always more readable than 3/4 or front view.

## How to design for readable silhouettes

### 1. Keep limbs OFF the body
When designing a pose, push arms and legs AWAY from the torso silhouette. A character pointing should have a clear arm extension OUTSIDE the body outline.

### 2. Use clear directional limbs
A leg lifted at an angle reads better than a leg pulled tight against the body. An arm at 45 degrees reads better than an arm at 5 degrees.

### 3. Avoid "hidden" body parts
If a hand is behind the back, it doesn't exist in the silhouette. Make sure all important parts are visible.

### 4. Don't cross the centerline
A right hand crossing in front of the body, ending up on the left side of the silhouette, confuses the eye. Keep right things on the right of the silhouette.

### 5. Differentiate adjacent parts
Don't let the head merge into the shoulders. Don't let the elbow disappear into the torso. Each major part should have a clear separation in the silhouette.

## The the classical tradition checklist for any pose

Before committing to a pose:

1. ☐ Is the head clearly separated from the shoulders?
2. ☐ Is each arm visible in the silhouette (not hidden behind torso)?
3. ☐ Are the legs differentiated (not parallel or twinned)?
4. ☐ Are any held objects visible against the silhouette?
5. ☐ Does the silhouette tell the story?

If you can fill the pose with black and still read what's happening, you've succeeded.

## Prompt-ready language

### Video model — strong silhouette
> "Character in a strong action pose with clear silhouette readability. Pointing dramatically off-screen left. Arm extended fully clear of body silhouette. Legs in dynamic stance — one forward, one back, clearly separated. Head tilted, clearly separated from shoulders. The full pose readable even as a black silhouette."

### Video model — silhouette-first staging
> "Stage the character so the action reads from the silhouette alone. Character is celebrating — arms thrown wide overhead, legs apart in jumping pose, head tilted back. Even if rendered as pure black on white, the joy is unmistakable. Avoid hands or objects buried against the body."

### Code (when designing character rigs)
For any major key pose, the rigging/animation system should be able to:
1. Render the character as a flat fill
2. Verify the pose is readable
3. Adjust pose offsets if needed

```javascript
// Pseudocode for a silhouette check
function checkSilhouette(pose) {
 const blackPose = renderAsFill(pose);
 return analyzeReadability(blackPose);
 // Returns metric for how clearly the pose reads
}
```

## Common silhouette mistakes in AI generation

### Limbs disappearing into torso
AI image models often pull limbs in close to the body for "natural" poses. Override:
> "Arms held clearly away from body, never overlapping the torso silhouette."

### Both legs in identical position
AI image models love symmetric standing poses. Override:
> "Asymmetric leg pose — one leg forward, one leg back, hips tilted."

### Hands hidden
AI image models hide hands they don't want to draw. Override:
> "Hands visible and prominent in silhouette, clearly extended from arms."

## Linked concepts

- [[line-of-action]]
- [[C-and-S-curves]]
- [[avoiding-twins]]
- [[the-spacing-chart]]
