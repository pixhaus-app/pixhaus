# Arm Swing — The Pendulum Above the Walk

Arms are not just decoration. They balance the body, signal personality, and provide overlap (arms always lag the body slightly).

## The basic rule: arms oppose legs

In a normal walk:
- **Right leg forward → left arm forward**
- **Left leg forward → right arm forward**

This is a fundamental biomechanical fact. The contralateral swing comes from the natural rotation of the spine and shoulders, which counter-rotate against the hips.

## The timing offset (y bit)

You'd think the arms are at peak swing during the CONTACT pose. They're not.

**The peak of the arm swing is at the DOWN pose** — one frame after contact. The arms lag the legs by a tiny amount.

```
Contact: arm swinging through middle of arc
Down: arm at peak forward / peak back ← ARM EXTREMES HERE
Passing: arm swinging back through middle
Up: arm at opposite peak ← OTHER ARM EXTREME
```

*"We could ignore the fact and keep going, but we might as well understand the rule before we start making a mess."*

## The arc of the arm swing

The hand follows a curved path — never straight. It traces an arc from back-extreme to forward-extreme around the shoulder pivot.

for any rotational motion: **trace the path of a single point** (the wrist) across all frames. If it's a straight line, the arc is wrong.

## Drag and overlap on the hand

The hand drags behind the arm. When the arm swings forward:
- Shoulder leads
- Elbow follows
- Wrist follows the elbow (slightly delayed)
- Hand trails the wrist (delayed further)
- Fingertips trail the hand (most delayed)

This creates a wave of motion through the limb. When the arm reaches its peak, parts are still moving:
- Shoulder is at peak
- Elbow is approaching peak
- Wrist is mid-motion
- Hand is still trailing

(This is overlap — see `06_flexibility-overlap-follow-through/`.)

## Arm swing amount by character

### Wide arm swing
- Macho swagger
- Confident / excited character
- Walking briskly with purpose
- Cartoon-style energy

### Medium arm swing (default)
- Normal walk
- Average adult moving at normal pace

### Small arm swing
- Sneaking / cautious
- Holding something
- Self-conscious character
- Tired character

### No arm swing (arms hang or held)
- Sneaking with hands held in front
- Pockets-in-jacket walk
- Carrying objects in both hands
- Cold/huddled posture

## The the classical tradition sneak exception

In a sneak walk, **the arms do not oppose the legs**. They balance — both arms move together to help maintain balance over the slow weight transfer.

This is a key tell of a sneaking gait. The arms are out for balance, swinging together with the body's lean, not against the legs.

## Reaction lag (the wave through the arm)

When the body changes direction (a sudden stop, a turn), the arms react last:

```
Frame 1: body decides to stop
Frame 2: shoulders begin to slow
Frame 3: upper arms still moving with momentum
Frame 4: forearms still going
Frame 5: wrists overshooting body's stop position
Frame 6: hands swing past
Frame 7: hands settle back
```

This is called **counter-reaction** or **follow-through**. Without it, the arms look stiff and stapled to the body.

## Arms in different walk speeds

| Walk frames/step | Arm swing amount | Arm timing |
|------------------|------------------|------------|
| 8 (fast cartoon) | Large | Tight, sharp |
| 12 (natural) | Medium | Standard |
| 16 (stroll) | Medium-small | Relaxed |
| 24 (slow heavy) | Small | Heavy, dragging |
| 32 (very slow) | Minimal | Almost still, lots of overlap drag |

## Prompt-ready language

### Video model — normal walk
> "Arms swing naturally in opposition to legs — right arm forward when left leg forward. Arms reach peak extension just after each foot contact. Wrists and hands drag slightly behind the elbows, creating soft overlap. Arc motion through shoulder pivot — never straight-line arm motion."

### Video model — confident swagger
> "Wide arm swings, hands move far forward and far back. Shoulders roll noticeably with each step. Some forward lean. Arms emphasized to communicate confidence."

### Video model — sneak
> "Arms do NOT oppose legs in normal fashion. Both arms held out slightly for balance, moving together in small motions. Hands held forward and slightly tense. Body weight stays back through each step."

### Code (GSAP)
```javascript
const armSwing = gsap.timeline({ repeat: -1 });
armSwing
 // rightArm forward, leftArm back (opposite to right leg, which is forward)
 .set(rightArm, { rotation: 30 })
 .set(leftArm, { rotation: -30 })
 // through one full step (12 frames at 24fps = 0.5s)
 .to(rightArm, { rotation: -30, duration: 0.5, ease: "sine.inOut" })
 .to(leftArm, { rotation: 30, duration: 0.5, ease: "sine.inOut" }, 0);

// hand drag — wrist follows arm with 2-frame delay
gsap.to(rightHand, {
 rotation: -30 - 10, // overshoots by 10 degrees (drag)
 duration: 0.5,
 ease: "sine.inOut",
 delay: 0.08, // 2 frames at 24fps
});
```

## Common mistake: arms in lockstep with legs

The most common AI prompting error: arms snap forward/back exactly when legs do. This looks robotic.

Always add the small timing offset between the leg's contact and the arm's peak. **The arm peaks 1-2 frames after the leg contacts.**

## Linked concepts

- [[the-standard-walk]]
- [[counter-reaction]]
- [[simple-overlap]]
- [[breaking-joints]]
