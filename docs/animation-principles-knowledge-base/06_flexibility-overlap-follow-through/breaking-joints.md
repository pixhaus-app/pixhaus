# Breaking Joints — The the classical tradition Flexibility Trick

> Break the joints. Let them go past their natural extension for one frame.
One of the most useful tricks: at the apex of any limb motion, let the joint "overextend" beyond its natural position for 1-2 frames. This is called breaking the joint, and it's what makes cartoon motion feel rubbery, alive, and weighted.

## The principle

Real joints don't bend backward. Real elbows don't hyperextend. But for ONE FRAME at the peak of a motion, you can violate this and the audience reads it as "more energy, more snap."

```
Frame 1: arm at rest (elbow at natural 90 degrees)
Frame 2: arm at full extension (elbow at 180 degrees — straight)
Frame 3: arm BROKEN (elbow at 185 degrees — overextended) ← BROKEN JOINT FRAME
Frame 4: arm back at full extension (180 degrees)
Frame 5: arm settling back
```

The 1-frame break adds invisible "snap" energy to the motion.

## Where to break joints

### The elbow
At the peak of a punch, throw, or reach — the elbow bends backward 5-10 degrees beyond straight for 1 frame.

### The wrist
At the peak of a wave or salute — the wrist hyperflexes beyond natural range for 1 frame.

### The knee
At the peak of a kick — the knee straightens past 180 degrees for 1 frame.

### The fingers
At the apex of a finger snap — the fingers fly past their natural extension.

### The neck
On a sudden head turn — the neck extends past its natural rotation for 1 frame.

## The break is **1 frame only**, then immediately snaps back to the natural position. If you hold the broken pose, it looks wrong (anatomically impossible). One frame is invisible to conscious viewers but charges the motion.

```
WRONG:
Frame 1: arm straight
Frame 2-5: arm broken (held — looks deformed)
Frame 6: arm settles

RIGHT:
Frame 1: arm straight
Frame 2: arm broken (1 frame only)
Frame 3: arm at full extension again
Frame 4: arm settling
```

## Where this comes from

But the modern application is more subtle: realistic-looking limbs that briefly break for energy.

It's used heavily in:
- Looney Tunes (a Bugs Bunny pointing arm always slightly hyperextends)
- Tom and Jerry (whip-fast motion always has broken joints at peaks)
- Modern 3D animation (rigs that allow slight overextension for "snap")

## Combined with overshoot

Breaking joints is related to overshoot but distinct:

- **Overshoot**: the limb travels past its target by 10-20% then returns
- **Broken joint**: the joint angle exceeds its natural range by 5-10 degrees for 1 frame

You can combine them: a limb both overshoots its target AND breaks the joint at that overshoot.

## the face application

For ONE FRAME, push the mouth WIDER than natural.
Then settle back to the natural wide.
```

This 1-frame "over-stretched" mouth adds energy to vowels like "WOW" or "WHOA." The audience doesn't see the impossibility — they feel the snap.

Same for eyes (1 frame of impossibly wide), nostrils (1 frame of impossibly flared), eyebrows (1 frame above the forehead).

## The "where can I break it?" checklist

For any pose change, ask:
1. Where is a joint reaching its maximum extension?
2. Can I push it 1 frame past natural?
3. Will that 1 frame add energy?

If yes, break the joint for 1 frame.

## Prompt-ready language

### Video model — break joints
> "Character points emphatically at the door. At the peak of the pointing motion (frame of full extension), the elbow briefly hyperextends slightly — 1 frame of impossible overextension that adds snap to the gesture. Then the arm settles back to natural full-extension."

### Video model — break face
> "Character expresses shock with a 'WOW' face. Mouth opens to maximum natural width, then for 1 single frame at the peak, mouth is pushed even WIDER than natural — clearly impossible but only for 1 frame. Then settles to natural wide. The 1-frame impossibility makes the shock read 2x stronger."

### Code (broken joint at extension peak)
```javascript
const pointWithBrokenElbow = gsap.timeline();
pointWithBrokenElbow
 // Initial reach
 .to(elbowAngle, { value: 170, duration: 0.15, ease: "power3.in" })
 // BROKEN JOINT — 1 frame
 .to(elbowAngle, { value: 195, duration: 1/24, ease: "none" })
 // Settle back to natural
 .to(elbowAngle, { value: 180, duration: 0.08, ease: "back.out(1.5)" });
```

## When NOT to break joints

- **Realistic style** — looks like a deformity
- **Slow gentle motion** — no need for snap
- **Stiff / paralyzed character** — wouldn't have the snap
- **Heavy character** — they don't have the speed for it

Break joints sparingly for stylized cartoon work. Skip them entirely for realistic work.

## Linked concepts

- [[simple-overlap]]
- [[counter-reaction]]
- [[invisible-anticipation]]
- [[squash-and-stretch]]
