# How to Use This Knowledge Base With AI

This is a practical guide to getting maximum value from the knowledge base when prompting AI animation tools.

## The three usage patterns

### Pattern 1: System prompt (best for Claude, ChatGPT, Cursor)

Load the entire knowledge base or relevant sections into a system prompt. Then ask the AI to apply the principles.

**Example:**

```
[System prompt: contents of 00_README/README.md, 02_timing-and-spacing/*, 07_anticipation/*]

[User prompt:] Generate a Sora prompt for a character throwing a baseball, applying the principles from the knowledge base.
```

The AI will reference the principles by name and produce a prompt that incorporates anticipation, action, reaction, easing, etc.

### Pattern 2: RAG / retrieval (best for production systems)

Set up a vector database or retrieval system over the markdown files. The AI retrieves relevant principles for each prompt.

**Workflow:**
1. User says "animate a character walking heavily"
2. System retrieves `03_walks/the-standard-walk.md`, `03_walks/variations.md` (heavy walk section), `05_weight-and-force/showing-weight.md`
3. AI uses retrieved content to construct a detailed prompt

### Pattern 3: Direct copy-paste (best for one-off prompts)

Open the most relevant file. Copy the "Prompt-ready language" section. Paste into your AI tool.

Every file has a prompt-ready section. Use these directly.

## Combining sections for a complete prompt

A good animation prompt typically blends three layers:

### Layer 1: A principle file
From sections 01-14. Pick the most relevant principle for the action.

Example: For a punch animation, use `07_anticipation/basic-anticipation.md`.

### Layer 2: A style preset
From section 16. Pick the visual style.

Example: For a cartoon punch, use `16_style-presets/looney-tunes-cartoon.md`.

### Layer 3: A tool template
From section 15. Pick your AI tool's format.

Example: For Sora video, use `15_prompt-templates/sora-veo-kling-runway.md`.

## Worked example — Building a complete prompt

**Goal:** Generate a Sora prompt for an old man slowly walking across a Victorian parlor, looking sad.

### Step 1: Identify needed principles

- Walking: `03_walks/the-standard-walk.md`
- Old man variation: `03_walks/variations.md` (Old / Cane Walk section)
- Sadness body language: `11_acting-and-facial/body-language.md`
- Walk timing: `03_walks/walk-timing-chart.md`

### Step 2: Choose style

- For realistic feel: `16_style-presets/realistic.md`
- Or for Disney feel: `16_style-presets/classic-disney.md`

### Step 3: Choose tool

- Sora video: `15_prompt-templates/sora-veo-kling-runway.md`

### Step 4: Build prompt

Combining these:

```
[STYLE - from classic-disney.md]
Classic Disney 2D animation style. Animated on twos for most action, ones for emphasis moments. Naturalistic timing with subtle exaggeration. Solid drawing, strong silhouettes.

[CHARACTER - from variations.md]
An elderly man, slightly hunched, wearing Victorian clothing. Holds a cane in his right hand.

[ACTION - from the-standard-walk.md + walk-timing-chart.md + body-language.md]
He walks slowly across the parlor. Walk timing: 24 frames per step at 24fps (1 second per step, very slow). Gait: hunched forward, head down, weight forward over the cane. Cane plants first, then the foot follows. Short shuffling steps. Almost no head bounce. Body language: deeply slumped shoulders, head hung low, eyes downcast. Sadness conveyed through body language, not face.

[SECONDARY MOTION]
Coat drags slightly behind body (5-frame overlap). Each step shifts the body weight slowly. No bouncing energy.

[CAMERA]
Wide shot, side angle, slow horizontal pan keeping pace with the man's slow walk. Warm fireplace light from one side.
```

This prompt blends principles from 4 different files into one coherent description.

## Common AI failure modes and fixes

### Failure: AI generates "stiff" animation
**Cause:** Missing anticipation or follow-through
**Fix:** Explicitly add antic and reaction phases from `07_anticipation/`

### Failure: AI generates "rubbery" everything
**Cause:** AI over-applies squash and stretch
**Fix:** Specify what materials should NOT squash from `13_twelve-principles/`

### Failure: Walks look like sliding
**Cause:** Missing up/down bob from `03_walks/weight-shift-and-belt-line.md`
**Fix:** Explicitly describe head bob amount

### Failure: Static character during dialogue
**Cause:** "The Secret" not applied (`10_dialogue-lipsync/the-secret.md`)
**Fix:** Always describe what the character is doing WHILE speaking

### Failure: Linear robotic motion
**Cause:** AI defaulting to no easing
**Fix:** Reference `02_timing-and-spacing/slow-in-slow-out.md` explicitly

### Failure: Symmetric "Christmas tree" poses
**Cause:** AI's bias toward symmetric compositions
**Fix:** Use `14_staging-silhouette/avoiding-twins.md` language

### Failure: Expressions blend rather than switch
**Cause:** AI smooth-morphs faces
**Fix:** Use `11_acting-and-facial/expression-changes.md` — pause then change

## Quick decision tree

```
Q: What kind of animation?
├── Walk / locomotion → 03_walks/
├── Run / jump → 04_runs-jumps-leaps/
├── Action / strike → 07_anticipation/ + 08_takes-and-accents/
├── Dialogue → 10_dialogue-lipsync/ + 11_acting-and-facial/
├── Object motion → 01_foundations/the-bouncing-ball.md + 02_timing-and-spacing/
├── Emotional beat → 11_acting-and-facial/
├── Heavy lift → 05_weight-and-force/
├── Hair / cloth → 06_flexibility-overlap-follow-through/ + 09_wiggles-waves-whips/
└── Animal → 12_animal-action/

Q: What style?
├── Cartoon energy → 16_style-presets/looney-tunes-cartoon.md
├── Smooth Disney → 16_style-presets/classic-disney.md
├── Anime → 16_style-presets/anime.md
├── Realistic → 16_style-presets/realistic.md
├── Stop-motion → 16_style-presets/stop-motion.md
└── Modern 3D → 16_style-presets/pixar-3d.md

Q: What tool?
├── Video model → 15_prompt-templates/sora-veo-kling-runway.md
├── Image keyframes → 15_prompt-templates/midjourney-flux-gpt-image.md
└── Code-based → 15_prompt-templates/code-css-gsap-after-effects.md
```

## Using with Claude specifically

This knowledge base is well-suited for Claude. To get the best results:

1. **Upload the whole folder** as context. Claude handles long context well.
2. **Tell Claude to apply the principles** by name. Claude understands "anticipation," "follow-through," "slow-in/slow-out," etc.
3. **Ask for explicit frame counts** when you need them. Claude will use the frame-count tables.
4. **Iterate on the prompt** — ask Claude to "make the anticipation more dramatic" or "add more secondary motion to the hair."

Example Claude conversation:

> **You:** I want to generate a Sora prompt for a character delivering a confident speech. Use the knowledge base.

> **Claude:** Here's a Sora prompt using the principles:
> [generated prompt referencing dialogue accents, "the secret" of moving while speaking, body language, etc.]

> **You:** Make the head accents bigger.

> **Claude:** [revised prompt with more dramatic head movements 3-4 frames before each emphasized word]

This iterative approach is more effective than trying to get a perfect prompt in one shot.

## Building a custom skill

If you're using Claude Code or similar, you can package this knowledge base as a Skill:

1. Create a `SKILL.md` file describing when to use the knowledge base
2. Reference the markdown files from within the skill
3. The skill loads when relevant queries come in

This makes the knowledge base reusable across many projects.

## Closing note

"*

This knowledge base is your scaffolding. The principles work. The frame counts work. The timing patterns work. They've been refined over decades by masters who knew motion.

But the JUDGMENT — when to bend a rule, when to push exaggeration, when to hold for emotion — that's yours. The AI is a tool. You're the director.

Animation is patience and repetition. AI accelerates the iteration. The principles guide the iteration toward something that feels alive.

## Linked concepts

- [[README]]
- [[INDEX]]
