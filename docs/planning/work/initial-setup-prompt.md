# Initial setup prompt for Claude Code

The single prompt to dispatch to Claude Code that does all the initial repo setup, hook configuration, ralph-loop infrastructure, and pre-build skill authoring. After this runs, the project is ready to dispatch its first feature stream.

## Manual steps before running the prompt

These can't be automated by Claude Code — they need your auth.

1. Create the GitHub organization: `https://github.com/organizations/new` → name "pixhaus".
2. Reserve the npm package: `npm login` then `npm publish` a stub package or use `https://www.npmjs.com/` to claim "pixhaus".
3. Point pixhaus.app at a Cloudflare Pages placeholder (or just a 200 OK page) so the domain resolves.
4. Create the empty repo on GitHub: `https://github.com/organizations/pixhaus/repositories/new` → name "pixhaus" → MIT license → no README (we'll add one).
5. Decide where the local repo lives. Recommend `~/code/pixhaus` on macOS/Linux or `C:\Users\luism\Documents\GitHub\pixhaus` on Windows.
6. Install prerequisites if not already: Rust toolchain via rustup (channel: stable), Node 20 LTS, pnpm 9, git 2.40+.
7. Open a terminal in that directory and run `claude --model claude-opus-4-7`.

Then paste the prompt below.

## Why Opus for this run

This is bedrock-tier setup. The skills authored here shape every subsequent agent's behavior. The hook configuration shapes the development loop. The CI shapes what merges. Get this right once with Opus; iterate with Sonnet later.

## The prompt

Copy everything between the lines below into Claude Code:

---

```
You are setting up a new open-source project called Pixhaus — an AI-native pixel art editor for sprites, animations, and tilemaps. The complete planning documentation lives at:

    C:\Users\luism\Documents\Claude\Projects\SpriteMaster\

Read these planning docs in this order before doing anything else:

  1. README.md — folder map
  2. product/scope.md — what we're building
  3. architecture/stack.md — locked tech (Tauri 2 + Rust + TS/Solid + WebGL2)
  4. architecture/rust-vs-electron.md — why Rust
  5. work/dev-workflow.md — Claude Code + ralph loop + hooks + model strategy
  6. work/bedrock.md — bedrock specs B1-B8 (you're going to deliver B1 and B8)
  7. work/skills.md — the four pre-build skills you'll author
  8. ecosystem/06-rust-best-practices-2026.md — distill into the rust-conventions skill
  9. ecosystem/01-foundations.md — Tauri patterns reference
 10. ecosystem/04-scripting-and-testing.md — testing patterns reference

After reading, build the initial repository in the current working directory.

USE the TodoWrite tool to plan this work. The phases are:

PHASE 1: Repo scaffold (B1)
- Initialize git, create the workspace structure described in architecture/stack.md
- Cargo workspace at the root with crates: core, io, ai, scripting, app
- pnpm workspace with ui/ subfolder using Solid.js + Vite + TypeScript 5
- Tauri 2.x app shell that opens an empty window titled "Pixhaus"
- Each Rust crate has a stub lib.rs with a single placeholder function and one passing test
- Configure rust-toolchain.toml pinning stable 1.82+
- Configure rustfmt.toml and clippy.toml with project lint set (forbid unwrap in non-test code, forbid panic in non-test code, enable clippy::pedantic with reasonable allows)
- Configure tsconfig.json with strict mode
- Configure ESLint and Prettier for the UI
- Place a stub Unity package skeleton in unity/ (UPM manifest, Editor/, Runtime/, Samples~/)

Acceptance for Phase 1:
  - cargo build --workspace succeeds
  - cargo test --workspace passes
  - cargo clippy --workspace -- -D warnings passes
  - pnpm install && pnpm typecheck passes
  - pnpm dev opens an empty Pixhaus window
  - pnpm build produces the production bundle
  - git init done, initial commit created

PHASE 2: License, README, contributing
- LICENSE: MIT, copyright Luis (luis@agsense.es) 2026
- README.md: brief project intro, status (in active development), link to docs/planning/
- CONTRIBUTING.md: factual and operational. Cover: how to contribute, branch naming (feat/sNN-slug), commit format (Conventional Commits), PR template expectations, code review bar, the human + AI agent contribution model. One-line reference to CODE_OF_CONDUCT.md. Avoid extended behavior-policy language here — keep it short and procedural.
- CODE_OF_CONDUCT.md: short stub only — do NOT write the verbatim Contributor Covenant text (content filters reject the verbatim covenant due to its prohibited-behavior descriptions). Use this exact short content:

  # Code of conduct
  Pixhaus follows the Contributor Covenant version 2.1.
  Full text: https://www.contributor-covenant.org/version/2/1/code_of_conduct/
  Report violations to luis@agsense.es. Maintainers commit to a 72-hour first response and confidential handling.

  Then add a TODO comment pointing at the canonical URL so a future maintainer can paste the full text by hand if they want it inline.

- SECURITY.md: how to report security issues (luis@agsense.es). Brief, procedural — no extended threat-language.
- .gitignore: comprehensive Rust + Node + OS-specific
- .editorconfig: 2 spaces TS, 4 spaces Rust, LF line endings, UTF-8

PHASE 3: GitHub configuration
- .github/PULL_REQUEST_TEMPLATE.md: template per work/dev-workflow.md (what changed, why, test plan, screenshots if UI, references to streams.md)
- .github/ISSUE_TEMPLATE/bug.md, feature.md, plugin-idea.md
- .github/dependabot.yml: weekly cargo + npm + github-actions updates
- .github/workflows/ci.yml: on every PR — cargo fmt --check, cargo clippy --workspace -- -D warnings, cargo test --workspace, pnpm typecheck, pnpm lint, pnpm test, pnpm build. Cache cargo registry, target dir, pnpm store. Target: under 8 minutes.
- .github/workflows/release.yml: stub (TODO: full per-OS builds when S50 lands). For now, just runs on tag push and creates a GitHub Release with the changelog.
- .github/workflows/docs.yml: stub (deploys docs site when S41 lands)

PHASE 4: Hook configuration and dev scripts
- .claude/settings.json: PostToolUse hook on Edit|Write that calls scripts/post-edit.sh
- scripts/post-edit.sh: detects file type. For .rs files: cargo fmt on the file, then cargo check --tests -p <crate> (use a helper to map file → crate). For .ts/.tsx: prettier --write, then tsc --noEmit. Echo any errors so Claude sees them in the next turn.
- scripts/find-crate-for-file.sh: walks up the directory tree from a given file to find the nearest Cargo.toml and prints the crate name from it.
- scripts/install-tools.sh: cargo install --locked cargo-nextest cargo-deny cargo-audit cargo-machete cargo-watch typos-cli bacon. Plus pnpm add -g if needed.
- scripts/setup-git-hooks.sh: copies .githooks/pre-commit to .git/hooks/pre-commit (or sets core.hooksPath = .githooks)
- .githooks/pre-commit: cargo fmt --check, cargo clippy --workspace -- -D warnings, typos --check, pnpm prettier --check ui/, pnpm eslint ui/

PHASE 5: Ralph loop infrastructure
- scripts/ralph.sh: per the skeleton in work/dev-workflow.md. Take a worktree name argument. Loop forever picking tasks from work/queue.md, dispatching Claude with the task as prompt, opening a PR on success, marking the task done.
- scripts/claim-next-task.sh: atomic claim of the next unclaimed task from work/queue.md. Use flock for atomicity. Marks the task with [CLAIMED:worktree-name] and prints the task path/contents.
- scripts/finalize-task.sh: marks a task done if CI is green, else returns it to the queue with the failure reason.
- scripts/new-worktree.sh: helper to create a new git worktree at ../pixhaus-worktrees/<name> on a new branch.

PHASE 6: Pre-build skills (the highest-leverage deliverable in this whole prompt)
Create four skills under .claude/skills/, each as <skill-name>/SKILL.md with proper Claude Code skill frontmatter:

(a) .claude/skills/pixhaus-rust-conventions/SKILL.md
- Distilled from C:\Users\luism\Documents\Claude\Projects\SpriteMaster\ecosystem\06-rust-best-practices-2026.md
- Length target: 400-600 lines
- Bias toward code examples over prose
- Cover: thiserror+anyhow split, no unwrap rule, async patterns, lock-across-await footguns, single-owner principle, common agent mistakes with corrections (Box<dyn Trait> overuse, Vec<Vec<T>> for 2D, premature Arc<Mutex<>>), the newtype/sealed-trait/type-state idioms, the code review checklist

(b) .claude/skills/pixhaus-claude-code-workflow/SKILL.md
- Distilled from C:\Users\luism\Documents\Claude\Projects\SpriteMaster\work\dev-workflow.md
- Length target: 300-400 lines
- Cover: branch naming (feat/sNN-slug), Conventional Commits, PR template, ralph loop expectations (mark done only after CI passes), worktree discipline (don't cd to other worktrees), what cargo check errors mean and how to act, "open PRs not merges", when to escalate to human

(c) .claude/skills/pixhaus-tauri-patterns/SKILL.md
- Distilled from C:\Users\luism\Documents\Claude\Projects\SpriteMaster\ecosystem\01-foundations.md (Tauri sections)
- Length target: 300-500 lines
- Cover: IPC command signatures, tauri::State<T> pattern, event emission/subscription, window management, cross-thread main-thread issues, tauri-specta typed-IPC pattern with worked examples, native menu integration, per-OS quirks

(d) .claude/skills/pixhaus-testing-conventions/SKILL.md
- Distilled from C:\Users\luism\Documents\Claude\Projects\SpriteMaster\ecosystem\04-scripting-and-testing.md
- Length target: 400-600 lines
- Cover: inline #[cfg(test)] vs tests/ directory, the rstest fixture pattern, the proptest property-based test pattern with examples, the insta snapshot pattern, visual regression with image-compare, mocking with mockall (the trait-then-mock pattern), wiremock-rs for HTTP mocking, "every public function has a test" rule, cargo nextest usage, fast-vs-comprehensive local test workflows

Each skill must have YAML frontmatter at the top:
---
name: <skill-name>
description: <one-line trigger description for skill activation>
---

PHASE 7: Documentation snapshot
- Create docs/planning/ in the new repo
- Copy these directories from the workspace into docs/planning/ verbatim:
  * pixel-art-editors/, general-purpose/, skeletal-animation/, frame-by-frame/, engine-integrated/, tilemap-level/, ai-native/ (the ~60 tool research files)
  * synthesis/
  * product/
  * architecture/
  * ecosystem/
  * work/
  * README.md, index.md, _research-template.md
- This way the planning docs travel with the codebase. Future agents in the repo have full context without depending on the user's specific filesystem layout.

PHASE 8: Task queue
- Create work/queue.md with the initial task list:
  * UNCLAIMED: B2 (core data model) — highest priority, single agent
  * UNCLAIMED: B3 (project file format)
  * UNCLAIMED: B4 (IPC command catalog)
  * UNCLAIMED: B5 (verb plugin protocol) — flag as Opus-required, highest leverage
  * UNCLAIMED: B6 (Unity handoff format)
  * UNCLAIMED: B7 (Aseprite compat spec)
- Each entry has: task ID, link to the brief in docs/planning/work/bedrock.md, current status, claim slot (worktree name when claimed)
- Note that B2 should land before B3-B7 fan out (everyone depends on the data model)
- Below bedrock, list the critical-path streams from streams.md as queued-but-blocked-on-bedrock: S01, S02, S05, S06, S07, S08, S10, S13, S14, S21, S22, S39, S49

PHASE 9: Verification
Run all of these and confirm green:
  - cargo build --workspace
  - cargo test --workspace
  - cargo clippy --workspace -- -D warnings
  - cargo fmt --check
  - pnpm install
  - pnpm typecheck
  - pnpm lint
  - pnpm test
  - pnpm build
  - chmod +x scripts/*.sh
  - bash scripts/setup-git-hooks.sh
  - Create a test edit to a Rust file, save, verify scripts/post-edit.sh fires and reports cargo check output
  - Try to commit broken-formatting code, verify pre-commit hook rejects it
  - Verify .claude/skills/ is recognized (run a quick `claude` prompt that exercises one skill)

PHASE 10: Initial commit and push
- git add everything
- git commit with message: "chore: initial repo scaffold (B1) + agent handbook (B8) + four pre-build skills"
- Print clear instructions for pushing to GitHub: git remote add origin git@github.com:pixhaus/pixhaus.git && git push -u origin main

CONSTRAINTS:
- Use only crates listed in docs/planning/ecosystem/*.md as approved dependencies. If a stream needs a crate not in the ecosystem docs, surface it for human review rather than silently adding.
- Tone in all README/CONTRIBUTING/skill content: Pragmatic Leader voice — direct, declarative, no LLM tells (avoid "moreover", "furthermore", "comprehensive", "robust", "powerful", "intuitive"). Sentence-case headings. Straight quotes. No emojis.
- License headers in source files: not required (MIT is permissive enough that file-level headers are optional).
- File mode for shell scripts: chmod +x.
- Commit message format: Conventional Commits (chore:, feat:, fix:, docs:).

WHEN TO STOP AND ASK ME:
- A planning doc says one thing but the ecosystem doc contradicts it — don't silently pick. Ask.
- You can't determine which crate owns a piece of code (e.g., palette ops between core/ and io/). Ask.
- A dependency in the ecosystem docs is no longer available on crates.io. Ask.
- You hit a Tauri 2 API surface that's changed since the 2.x version pinned in stack.md. Ask.

REPORT WHEN DONE:
- The list of paths created/touched
- Verification command outputs (final lines, not full traces)
- Any decisions you made that weren't fully specified
- Any stuck points where you'd want a human eye

Begin by reading the planning docs and producing the TodoWrite plan.
```

---

## After the prompt completes

You should have:

- A working repo at the local path you ran Claude Code in
- An empty Pixhaus window that opens via `pnpm dev`
- Green CI configuration ready to run as soon as you push to GitHub
- The four pre-build skills loaded so future agents inherit conventions
- The task queue populated with B2-B7 ready to claim
- A working ralph loop you can spin up via `./scripts/ralph.sh main` (after you push to GitHub)

What to do next, in order:

1. Push the initial commit to `github.com/pixhaus/pixhaus` and verify GitHub Actions runs green.
2. Set up your first git worktree: `./scripts/new-worktree.sh stream-b2`.
3. Open a second terminal, cd into the worktree, run `claude --model claude-opus-4-7 --print "$(cat work/queue.md | grep B2 -A 50 | head -100)"` — let Opus draft B2 (the core data model). Review the PR before merging.
4. After B2 lands and merges, set up worktrees for B3-B7 and run them in parallel with Sonnet via the ralph loop.
5. Once bedrock is done, the critical-path streams (S01, S02, S05, S07, S08, S13, S14, S21, S22, S39, S49) unlock for parallel dispatch.

## Watch out for

- The setup prompt is long. Expect 30-90 minutes of Claude Code session time. Opus will be careful and methodical — that's correct for this work.
- The agent will probably want to verify each phase before moving on. Let it. The verification is what catches "I wrote it but it doesn't actually work" mistakes.
- If the agent asks a clarifying question, answer with the actual answer rather than "use your judgment." This is the highest-leverage code in the whole project.
- After the agent claims success, manually verify by:
  - Closing and reopening Claude Code so it loads the skills fresh
  - Running `cargo build --workspace` and `pnpm dev` yourself
  - Spot-checking one of the skills to make sure it's substantive and matches the source ecosystem doc
  - Trying the post-edit hook by editing a Rust file and saving — confirm `cargo check` fires
- If anything is off, push back rather than accepting it. The setup is the foundation; foundation mistakes compound.

## What this prompt does not do

- Does not write any feature code. The streams (S01-S52) are explicitly out of scope here.
- Does not author the stream-triggered skills (image-processing, aseprite-format, verb-protocol, solid-ui, ai-backend-adapter). Those come when their streams come online.
- Does not set up the website, Discord, or marketing infrastructure (S46-S48 streams).
- Does not write the user docs site (S41). Just `docs/planning/` snapshot.
- Does not set up release packaging (S50). Just stub workflows.

Those are all separate dispatches when their time comes.
