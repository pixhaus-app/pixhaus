# Code Animation Templates — CSS, GSAP, After Effects, Phaser

For code-based animation, the principles translate directly to easing functions, timing values, and keyframe positions. Here's the conversion table and templates.

## The principle → code mapping

| the classical tradition principle | Code equivalent |
|--------------------|------------------|
| Slow-in / slow-out | `ease-in-out`, `power2.inOut`, `cubic-bezier(0.4, 0, 0.6, 1)` |
| Slow-in (decel) | `ease-out`, `power2.out` |
| Slow-out (accel) | `ease-in`, `power2.in` |
| Anticipation | `ease: "back.in"` or manual reverse-direction preview |
| Overshoot / follow-through | `ease: "back.out(1.5)"` |
| Elastic bounce | `ease: "elastic.out(1, 0.3)"` |
| Multiple bounces | `ease: "bounce.out"` |
| Snap action | Custom curve with sharp acceleration |
| Hold / no motion | `duration: 0` or explicit hold timeline node |
| Constant speed | `ease: "none"` or `linear` |

## Universal animation template (GSAP)

```javascript
function animateAction(element, fromState, toState, options = {}) {
 const {
 totalDuration = 1,
 anticDuration = 0.4,
 holdAnticDuration = 0.05,
 actionDuration = 0.15,
 reactionDuration = 0.4,
 
 anticEase = "power2.out",
 actionEase = "power3.in",
 reactionEase = "elastic.out(1, 0.4)",
 } = options;

 const tl = gsap.timeline();
 
 // ANTICIPATION: opposite direction of toState
 const anticState = computeOppositeState(fromState, toState, 0.3);
 tl.to(element, { 
 ...anticState, 
 duration: anticDuration, 
 ease: anticEase 
 });
 
 // HELD ANTIC
 tl.to({}, { duration: holdAnticDuration });
 
 // ACTION: snap to toState
 tl.to(element, { 
 ...toState, 
 duration: actionDuration, 
 ease: actionEase 
 });
 
 // REACTION: settle with overshoot
 tl.to(element, { 
 ...toState, 
 duration: reactionDuration, 
 ease: reactionEase 
 });
 
 return tl;
}
```

## Template 1: Easing decision tree

```javascript
function chooseEasing(actionType) {
 switch (actionType) {
 case 'natural_motion':
 return 'power2.inOut'; // slow-in slow-out
 case 'acceleration':
 return 'power2.in'; // slow-out only (gravity, etc.)
 case 'deceleration':
 return 'power2.out'; // slow-in only (settling)
 case 'snap':
 return 'power4.in'; // very fast acceleration
 case 'whip':
 return 'power3.in'; // ramps up then sharp peak
 case 'bouncy':
 return 'bounce.out'; // multiple bounces
 case 'elastic':
 return 'elastic.out(1, 0.4)'; // overshoots and oscillates
 case 'overshoot_settle':
 return 'back.out(1.5)'; // overshoots by 50%, returns
 case 'mechanical':
 return 'none'; // constant speed
 case 'organic_pause':
 return 'sine.inOut'; // gentle organic curve
 }
}
```

## Template 2: Walk cycle in code

```javascript
function walkCycle(character, options = {}) {
 const {
 framesPerStep = 12, // standard natural walk
 stepDuration = framesPerStep / 24, // at 24fps
 headBobAmount = 8,
 armSwingAngle = 25,
 legSwingAngle = 35,
 } = options;
 
 const tl = gsap.timeline({ repeat: -1 });
 
 // Each step: contact → down → passing → up → next contact
 
 // CONTACT pose (right foot forward)
 tl.set(character, {
 rightLegRotation: -legSwingAngle, // extended forward
 leftLegRotation: legSwingAngle, // extended back
 rightArmRotation: armSwingAngle, // back (opposes leg)
 leftArmRotation: -armSwingAngle, // forward
 bodyY: 0,
 headY: 0,
 });
 
 // DOWN pose
 tl.to(character, {
 bodyY: headBobAmount,
 headY: headBobAmount,
 rightLegRotation: 0,
 duration: stepDuration * 0.25,
 ease: "power2.in"
 });
 
 // PASSING pose
 tl.to(character, {
 bodyY: 0,
 headY: 0,
 leftLegRotation: -10,
 rightLegRotation: 5,
 duration: stepDuration * 0.25,
 ease: "power2.out"
 });
 
 // UP pose
 tl.to(character, {
 bodyY: -headBobAmount,
 headY: -headBobAmount,
 duration: stepDuration * 0.25,
 ease: "power2.out"
 });
 
 // CONTACT pose (left foot forward) — mirror
 tl.to(character, {
 rightLegRotation: legSwingAngle, // extended back
 leftLegRotation: -legSwingAngle, // extended forward
 rightArmRotation: -armSwingAngle, // forward
 leftArmRotation: armSwingAngle, // back
 bodyY: 0,
 headY: 0,
 duration: stepDuration * 0.25,
 ease: "power2.in"
 });
 
 // Continue cycle...
 
 // Head lag (overlap) — head bobs are delayed slightly
 const headTl = gsap.timeline({ repeat: -1, delay: 2/24 });
 // ... similar timeline for head, offset by 2 frames
 
 return tl;
}
```

## Template 3: AAR (Anticipation-Action-Reaction)

```javascript
function aar(element, mainMove, options = {}) {
 const {
 totalDuration = 1,
 anticPortion = 0.4,
 actionPortion = 0.15,
 reactionPortion = 0.45,
 anticDirection = 'opposite', // 'opposite' / 'down' / 'up'
 } = options;
 
 const tl = gsap.timeline();
 
 // Compute antic state
 const anticState = {};
 Object.keys(mainMove).forEach(key => {
 if (typeof mainMove[key] === 'number') {
 // Anticipate in opposite direction (30% of main move)
 anticState[key] = mainMove[key] * -0.3;
 }
 });
 
 // ANTICIPATION
 tl.to(element, { 
 ...anticState, 
 duration: totalDuration * anticPortion, 
 ease: "power2.out" 
 });
 
 // ACTION
 tl.to(element, { 
 ...mainMove, 
 duration: totalDuration * actionPortion, 
 ease: "power4.in" 
 });
 
 // REACTION (overshoot then settle)
 const overshootState = {};
 Object.keys(mainMove).forEach(key => {
 if (typeof mainMove[key] === 'number') {
 overshootState[key] = mainMove[key] * 1.1; // 10% overshoot
 }
 });
 
 tl.to(element, { 
 ...overshootState, 
 duration: totalDuration * reactionPortion * 0.4, 
 ease: "power2.out" 
 });
 
 tl.to(element, { 
 ...mainMove, 
 duration: totalDuration * reactionPortion * 0.6, 
 ease: "elastic.out(1, 0.4)" 
 });
 
 return tl;
}
```

## Template 4: Take / Reaction (cartoon)

```javascript
function cartoonTake(character, options = {}) {
 const {
 setupDuration = 0.2, // see thing
 anticDuration = 0.25, // crouch
 accentDuration = 0.08, // pop up
 settleDuration = 0.5, // held shock
 } = options;
 
 const tl = gsap.timeline();
 
 // 1. SEE
 tl.to({}, { duration: setupDuration });
 
 // 2. ANTIC (crouch)
 tl.to(character, {
 y: 20, // body drops
 scaleY: 0.7, // squashed
 scaleX: 1.3,
 eyesScale: 0.7, // eyes squint
 duration: anticDuration,
 ease: "power2.out"
 });
 
 // 3. ACCENT (explosive pop)
 tl.to(character, {
 y: -50, // launches up
 scaleY: 1.4, // stretched tall
 scaleX: 0.7,
 duration: accentDuration,
 ease: "power4.in"
 });
 
 tl.to(character, {
 eyesScale: 2.5, // eyes pop
 mouthOpenness: 1, // mouth drops
 duration: accentDuration * 0.5,
 ease: "back.out(3)"
 }, '-=' + accentDuration);
 
 // 4. SETTLE
 tl.to(character, {
 y: 0,
 scaleY: 1,
 scaleX: 1,
 eyesScale: 1.2, // eyes still wide
 duration: settleDuration,
 ease: "elastic.out(1, 0.5)"
 });
 
 return tl;
}
```

## Template 5: Overlap / Follow-Through

```javascript
function bodyWithFollowThrough(parts, mainAction, options = {}) {
 const { duration = 0.5, easing = "power3.in" } = options;
 
 // parts is an object like:
 // {
 // body: bodyElement,
 // head: headElement,
 // hair: hairElement,
 // cape: capeElement,
 // }
 
 // Lag amounts (in frames at 24fps)
 const LAGS = {
 body: 0,
 head: 2 / 24, // 2-frame delay
 hair: 5 / 24, // 5-frame delay
 cape: 8 / 24, // 8-frame delay
 };
 
 // Overshoot amounts
 const OVERSHOOTS = {
 body: 1.0, // no overshoot
 head: 1.05, // 5% overshoot
 hair: 1.2, // 20% overshoot
 cape: 1.3, // 30% overshoot
 };
 
 const tl = gsap.timeline();
 
 Object.entries(parts).forEach(([name, element]) => {
 const overshootAction = {};
 Object.keys(mainAction).forEach(key => {
 overshootAction[key] = mainAction[key] * OVERSHOOTS[name];
 });
 
 // Movement with delay
 tl.to(element, {
 ...overshootAction,
 duration: duration,
 ease: easing,
 delay: LAGS[name]
 }, 0);
 
 // Settle back to actual target
 if (OVERSHOOTS[name] > 1) {
 tl.to(element, {
 ...mainAction,
 duration: duration * 0.5,
 ease: "elastic.out(1, 0.4)",
 delay: LAGS[name] + duration
 }, 0);
 }
 });
 
 return tl;
}
```

## CSS-only animations

For pure CSS (no JavaScript):

```css
/* Slow-in / slow-out (natural motion) */
.natural-motion {
 animation: move 1s cubic-bezier(0.42, 0, 0.58, 1);
}

/* Snap action */
.snap {
 animation: pop 0.2s cubic-bezier(0.55, 0.085, 0.68, 0.53);
}

/* Elastic / overshoot */
.elastic {
 animation: bounce 0.6s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

/* Squash and stretch keyframes */
@keyframes bounce {
 0% { transform: translateY(0) scaleY(1) scaleX(1); }
 45% { transform: translateY(200px) scaleY(1.2) scaleX(0.9); } /* stretch falling */
 50% { transform: translateY(220px) scaleY(0.5) scaleX(1.4); } /* squash impact */
 60% { transform: translateY(200px) scaleY(1.2) scaleX(0.9); } /* stretch rising */
 100% { transform: translateY(50px) scaleY(1) scaleX(1); } /* settle */
}

/* AAR pattern: pull back, then forward, then overshoot */
@keyframes punchAnim {
 0% { transform: translateX(0); }
 30% { transform: translateX(-30px); } /* anticipation */
 35% { transform: translateX(-30px); } /* hold antic */
 50% { transform: translateX(100px); } /* action */
 55% { transform: translateX(110px); } /* overshoot */
 100% { transform: translateX(95px); } /* settle */
}
```

## After Effects expression language

```javascript
// Easy Ease (slow-in slow-out)
ease(time, key(1).time, key(2).time, key(1).value, key(2).value)

// Bounce / overshoot
amp = .15;
freq = 2.0;
decay = 2.0;
n = 0;
if (numKeys > 0){
 n = nearestKey(time).index;
 if (key(n).time > time) n--;
}
if (n == 0){ t = 0;
}else{
 t = time - key(n).time;
}
if (n > 0 && t < 1){
 v = velocityAtTime(key(n).time - thisComp.frameDuration/10);
 value + v*amp*Math.sin(freq*t*2*Math.PI)/Math.exp(decay*t);
}else{
 value;
}
```

## Phaser game engine (for game animations)

```javascript
// Tweening with the Phaser tween system
this.tweens.add({
 targets: character,
 x: targetX,
 duration: 500,
 ease: 'Sine.easeInOut', // slow-in slow-out
});

// Anticipation-action-reaction with chained tweens
this.tweens.chain({
 targets: character,
 tweens: [
 { x: -10, duration: 200, ease: 'Power2.easeOut' }, // antic
 { x: 100, duration: 100, ease: 'Power3.easeIn' }, // action
 { x: 95, duration: 300, ease: 'Elastic.easeOut' }, // settle
 ]
});

// Cartoon take
this.tweens.timeline({
 tweens: [
 { targets: character, scaleY: 0.7, scaleX: 1.3, duration: 100, ease: 'Power2.easeOut' },
 { targets: character, scaleY: 1.5, scaleX: 0.7, y: -50, duration: 50, ease: 'Power4.easeIn' },
 { targets: character, scaleY: 1, scaleX: 1, y: 0, duration: 400, ease: 'Elastic.easeOut' },
 ]
});
```

## Linked concepts

- [[universal-prompt-skeleton]]
- [[sora-veo-kling-runway]]
- [[midjourney-flux-gpt-image]]
- [[../02_timing-and-spacing/slow-in-slow-out]]
- [[../02_timing-and-spacing/the-spacing-chart]]
