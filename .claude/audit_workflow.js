export const meta = {
  name: 'pixhaus-skill-compliance-audit',
  description: 'Crate-by-crate, file-by-file audit of Pixhaus against its skills, CLAUDE.md rules, and architecture boundaries',
  phases: [
    { title: 'Audit', detail: 'one agent per audit unit, each loaded with the skills + CLAUDE.md that apply to its files' },
    { title: 'Verify', detail: 'adversarial re-check of every finding to drop hallucinations and misapplied rules' },
    { title: 'Write', detail: 'per-crate markdown audit sections from verified findings' },
  ],
}

const ROOT = '/Users/luismorales/project/pixhaus-app/pixhaus'
const DATE = '2026-06-07'

// The master rubric every audit agent checks against. Drawn from root CLAUDE.md,
// the crate CLAUDE.md files, the locked Cargo.toml lints, and the skills.
const CHECK = `
COMPLIANCE CHECKLIST (apply only where relevant to the files in this unit):

A. Error / panic policy (clippy-DENIED workspace-wide; tests are exempt):
   - No unwrap() / expect() / panic! / todo! / unimplemented! / dbg! / println! / eprintln! in non-test code.
   - Library crates use thiserror; ONLY the app/ binary uses anyhow. No Box<dyn Error> in public APIs.
   - Result/Option taken out with ? / let-else / match / combinators, never unwrap. _else variants used for lazy/expensive defaults; eager forms only for cheap defaults.

B. Ownership / memory:
   - Signatures borrow (&str, &[T]) not over-own (&String, &Vec<T>); no .clone() of a borrowed arg; no clone() of large pixel buffers to dodge the borrow checker.
   - No Box<dyn Trait> for monomorphic params (use impl Trait / generics); dyn only at genuine runtime-polymorphism boundaries (registries, heterogeneous collections).
   - No Vec<Vec<T>> for 2D / pixel data; flat Vec<u8> with explicit stride.
   - No premature Arc<Mutex<>> for single-owner state. Correct mutex: parking_lot (sync short), tokio::sync (across .await), never std unless poison needed. NEVER hold a lock across .await.
   - Copy derived only on small, all-Copy, heap-free types. Newtypes for ids/indices.

C. Style:
   - Iterator chains for transforms, for-loops for early-exit/side-effects; no throwaway collect() in hot paths; .iter().copied() over .into_iter() for Copy.
   - Comments explain WHY not WHAT; no prose // TODO (file an issue); "record the why" at EVERY spot a non-obvious decision shaped; a reversed decision takes its stale comments with it.
   - use groups: std / external / workspace (pixhaus_*) / local (crate::), blank line between groups.
   - Every public item has a /// doc (missing_docs = warn); # Errors / # Panics where relevant.

D. Tests (pixhaus-testing-conventions):
   - Every public function has >=1 test (may live in a tests/ dir — note, don't over-claim). Property tests (proptest) for image/pixel ops; insta snapshots for text; image-compare for visual; mockall trait-then-mock; rstest fixtures. nextest layout.

E. Tracing (pixhaus-tracing):
   - Libraries emit events + #[instrument] on fallible/expensive/job bodies; NEVER install a subscriber; never println!. The app/ binary owns the ONE subscriber.
   - No per-frame tracing in the 60fps egui loop. core/render: a coarse debug! at most, no per-pixel/per-scanline spans. info! when a module registers capabilities. NEVER log API keys/secrets.

F. i18n (pixhaus-i18n):
   - No hardcoded user-facing string literals. Strings are stable KEYS in the right namespace (panel.<id>.*, tool.<id>.*, app.menu.*, command.<id>, provider.<id>.label), values in crates/services/locales/*.yaml, resolved at render time via MsgKey::tr().
   - core & render NEVER localize (store keys/ids only). Never put user data (file names, prompts, project content) inside a key. Provider-returned text / model names / API keys are DATA, not keys.

G. UI design system (crates/ui + any module with egui; pixhaus-ui-conventions / egui):
   - Theme tokens (theme.surfaces/roles/accent/spacing/type_scale/radius/elevation) — NEVER a hex / Color32::from_rgb literal in panel/region/widget code. Violet accent reserved for active tab/tool/selection, primary buttons, AI affordances.
   - Shared widgets (card, section_header, tool_button, workspace_tab, tray_tab, mock_*) — not bespoke frames; card already draws the panel title (don't re-draw).
   - Phosphor icons via crate::icons::* — NEVER emoji (render as tofu). Brand via crate::brand with TextureOptions::NEAREST.
   - Deferred-intent model: panels are &self, read through read-only ContribCtx, push an Intent for ALL mutation, never mutate the model directly; only PanelScope.scratch is mutable. No panel-to-panel coupling.
   - egui 0.34 API: ctx.global_style_mut/global_style (not style_mut/style); egui::SidePanel/TopBottomPanel patterns; ctx.text_edit_focused() for shortcut focus-gating. Flag older-API usage.

H. Architecture boundaries (CLAUDE.md + bible):
   - core & render are egui/wgpu-UI-free permanently. core also: no wgpu, no I/O, no network, no other workspace-crate dep. Strict acyclic deps: never depend on app/, modules never depend on each other unless deliberate/one-directional.
   - ALL project-state mutation goes through a Command in core. Tools/panels/AI results never mutate the model directly.
   - Long/expensive/external work is a Job producing a result; applying a result is a Command. AI generation never touches the canvas directly.
   - GPU textures are caches/views; the project model is the source of truth. Capabilities via registries; no dynamic plugins. Modules own their namespace, don't monopolize a workspace, no hidden global state.

I. Per-dependency idioms: judge version-correct, idiomatic use of THIS unit's dependency skills (egui 0.34, wgpu =29.0.1, eframe 0.34, tokio feature minimalism in libs, serde, image: FilterType::Nearest for pixel art + a Limits guard on untrusted decode, directories: create_dir_all before first write + macOS config==data collision, openrouter 0.10 shape, base64, parking_lot, async-trait Send-by-default trap / native async-fn-in-trait, etc.). Flag deprecated/older-version API and the footguns each skill calls out.

J. Stack discipline: no dependency beyond the locked Cargo.toml catalog without justification; no GPL/LGPL/AGPL; NO unsafe (workspace forbids — there should be zero unsafe and zero SAFETY comments).
`

const AUDIT_SCHEMA = {
  type: 'object',
  required: ['unit', 'compliance_summary', 'strengths', 'findings'],
  properties: {
    unit: { type: 'string' },
    files_reviewed: { type: 'number' },
    compliance_summary: { type: 'string', description: '2-4 sentence honest assessment of this unit' },
    strengths: { type: 'array', items: { type: 'string' }, description: 'exemplary compliance worth keeping' },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'file', 'lines', 'severity', 'category', 'source', 'issue', 'recommendation'],
        properties: {
          id: { type: 'string', description: 'unit-local id, e.g. U1-1' },
          file: { type: 'string' },
          lines: { type: 'string' },
          severity: { type: 'string', enum: ['critical', 'high', 'medium', 'low', 'info'] },
          category: { type: 'string', description: 'short tag e.g. no-unwrap, i18n, ui-tokens, wgpu, docs, boundary, tracing, tests' },
          source: { type: 'string', description: 'the exact skill / CLAUDE.md / bible rule violated' },
          issue: { type: 'string' },
          evidence: { type: 'string', description: 'the offending code snippet' },
          recommendation: { type: 'string' },
        },
      },
    },
  },
}

const VERIFY_SCHEMA = {
  type: 'object',
  required: ['unit', 'verdicts'],
  properties: {
    unit: { type: 'string' },
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'verdict', 'note'],
        properties: {
          id: { type: 'string' },
          verdict: { type: 'string', enum: ['confirmed', 'false_positive', 'needs_human_review'] },
          confidence: { type: 'string', enum: ['high', 'medium', 'low'] },
          note: { type: 'string', description: 'cite the code AND the rule for confirmed; reason for rejection otherwise' },
          corrected_severity: { type: 'string', enum: ['critical', 'high', 'medium', 'low', 'info'] },
        },
      },
    },
  },
}

// crates/core/src/commands/codex — 35 files, split across two units.
const CC = (n) => `crates/core/src/commands/codex/${n}.rs`
const CC_A = ['add_alias', 'add_coverage_slot', 'add_entry', 'add_entry_custom_slot', 'add_relationship', 'apply_builtin_coverage_template', 'apply_coverage_template', 'change_relationship_kind', 'clear_coverage', 'create_coverage_template', 'create_folder', 'delete_coverage_template', 'delete_entry', 'delete_folder', 'duplicate_entry', 'mod', 'remove_alias', 'remove_anchor'].map(CC)
const CC_B = ['remove_coverage_slot', 'remove_entry_custom_slot', 'remove_relationship', 'rename_coverage_slot_label', 'rename_coverage_template', 'rename_entry_custom_slot', 'rename_folder', 'reorder_coverage_slots', 'set_anchor', 'set_coverage_status', 'set_details', 'set_entry_folder', 'set_folder_parent', 'set_fragments', 'set_handle', 'set_status', 'update_entry'].map(CC)

const UNITS = [
  // ---- core (egui-free; serde + thiserror; no tracing, no i18n) ----
  { id: 'U1', group: 'core', label: 'core/domain model', claudeMd: ['crates/CLAUDE.md', 'crates/core/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'serde', 'type-state', 'generics-dispatch', 'testing-conventions'],
    files: ['crates/core/src/lib.rs', 'crates/core/src/document.rs', 'crates/core/src/pixel.rs', 'crates/core/src/ids.rs', 'crates/core/src/animation.rs', 'crates/core/src/composite.rs', 'crates/core/src/buffer_store.rs', 'crates/core/src/command.rs', 'crates/core/src/test_support.rs', 'crates/core/tests/undo_round_trip.rs'],
    focus: 'Pixel buffers must be flat Vec<u8> with explicit stride. Commands own all mutation. core stores keys/ids, never display text, never localizes, no tracing beyond coarse debug. Verify Copy/newtype discipline on ids and the no-egui/no-io boundary.' },
  { id: 'U2', group: 'core', label: 'core/codex model', claudeMd: ['crates/CLAUDE.md', 'crates/core/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'serde', 'thiserror', 'type-state', 'testing-conventions'],
    files: ['crates/core/src/codex/mod.rs', 'crates/core/src/codex/anchor.rs', 'crates/core/src/codex/coverage.rs', 'crates/core/src/codex/details.rs', 'crates/core/src/codex/entry.rs', 'crates/core/src/codex/entry_type.rs', 'crates/core/src/codex/folder.rs', 'crates/core/src/codex/handle.rs', 'crates/core/src/codex/ids.rs', 'crates/core/src/codex/priority.rs', 'crates/core/src/codex/relationship.rs', 'crates/core/src/codex/root.rs', 'crates/core/src/codex/status.rs'],
    focus: 'Pure domain data: serde derives, typed ids, invariants enforced by constructors/mutators. No display text, no localization, no I/O. Every public item documented and tested.' },
  { id: 'U3', group: 'core', label: 'core/commands (non-codex) + macros', claudeMd: ['crates/CLAUDE.md', 'crates/core/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'serde', 'generics-dispatch', 'testing-conventions'],
    files: ['crates/core/src/commands/mod.rs', 'crates/core/src/commands/macros.rs', 'crates/core/src/commands/add_sprite.rs', 'crates/core/src/commands/apply_generated_animation.rs', 'crates/core/src/commands/apply_generated_asset.rs'],
    focus: 'Command trait impls: undoable, mutation-owning, well-tested round-trips. Judge the swap-command macro for clarity and the apply_generated_* path (AI result applied AS a command, never touching state directly).' },
  { id: 'U4', group: 'core', label: 'core/codex commands (A)', claudeMd: ['crates/CLAUDE.md', 'crates/core/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'testing-conventions'],
    files: CC_A,
    focus: 'Each command must be undoable and own its mutation, return typed errors not panics, and carry a test. Watch for unwrap/expect, missing docs, and repetitive code that should use the swap macro.' },
  { id: 'U5', group: 'core', label: 'core/codex commands (B)', claudeMd: ['crates/CLAUDE.md', 'crates/core/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'testing-conventions'],
    files: CC_B,
    focus: 'Same as the A batch: undoable single-responsibility commands, typed errors, tests, docs, no panic paths.' },

  // ---- render (egui-free; wgpu; perf-critical) ----
  { id: 'U6', group: 'render', label: 'render/wgpu viewport', claudeMd: ['crates/CLAUDE.md', 'crates/render/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'wgpu', 'bytemuck', 'glam', 'pollster', 'performance', 'testing-conventions'],
    files: ['crates/render/src/lib.rs'],
    focus: 'wgpu 29.0.1 idioms, Pod/Zeroable via bytemuck derive (no unsafe), glam column-major + GPU alignment, textures-as-caches. MUST be egui-free. Perf: no per-frame allocation, dirty-rect discipline. pollster only in dev/tests.' },

  // ---- platform (directories; thiserror; tracing) ----
  { id: 'U7', group: 'platform', label: 'platform/dirs', claudeMd: ['crates/CLAUDE.md', 'crates/platform/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'directories', 'tracing', 'rfd', 'testing-conventions'],
    files: ['crates/platform/src/lib.rs', 'crates/platform/src/dirs.rs'],
    focus: 'directories: create_dir_all before first write, macOS config_dir == data_dir collision risk. Typed errors, #[instrument] on disk-touching fns, no subscriber install.' },

  // ---- io (stub) ----
  { id: 'U8', group: 'io', label: 'io (stub)', claudeMd: ['crates/CLAUDE.md', 'crates/io/CLAUDE.md'],
    skills: ['rust-conventions', 'thiserror', 'result-handling'],
    files: ['crates/io/src/lib.rs'],
    focus: 'This is a deliberate compiling stub. Verify it is a clean stub (compiles, documented, no half-built bodies, no todo!/unimplemented! that would trip the Stop gate), not an accidental gap. Note what the bible says it should become.' },

  // ---- services (tokio[rt,macros,time] + tokio-util + parking_lot + serde + rust-i18n + tracing + thiserror) ----
  { id: 'U9', group: 'services', label: 'services/core (jobs, undo, providers, store)', claudeMd: ['crates/CLAUDE.md', 'crates/services/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'tokio', 'tokio-util', 'parking-lot', 'futures', 'generics-dispatch', 'async-trait', 'serde', 'tracing', 'performance', 'testing-conventions'],
    files: ['crates/services/src/lib.rs', 'crates/services/src/error.rs', 'crates/services/src/job.rs', 'crates/services/src/history.rs', 'crates/services/src/transaction.rs', 'crates/services/src/provider.rs', 'crates/services/src/result_store.rs', 'crates/services/src/generated.rs', 'crates/services/build.rs'],
    focus: 'Job system: spawns onto the binary runtime (never creates one), CancellationToken with biased select!, no lock across .await, parking_lot guards the shared result store, channels drain to the UI. Provider dispatch: generics vs dyn boundary, async-trait Send trap. Lib tokio features stay minimal (rt/macros/time), not full.' },
  { id: 'U10', group: 'services', label: 'services/i18n service', claudeMd: ['crates/CLAUDE.md', 'crates/services/CLAUDE.md'],
    skills: ['rust-conventions', 'i18n', 'result-handling', 'tracing', 'serde', 'testing-conventions'],
    files: ['crates/services/src/i18n.rs'],
    focus: 'The ONE localization service wrapping rust-i18n behind a Pixhaus boundary. tr/tr_args/tr_plural/set_language, missing-key warns on pixhaus::i18n target then falls back en then key. Verify the boundary is clean (rust-i18n swappable) and locale-global tests are serialized.' },
  { id: 'U11', group: 'services', label: 'services/codex (compiler, coverage, search, validation, health)', claudeMd: ['crates/CLAUDE.md', 'crates/services/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'thiserror', 'serde', 'tracing', 'generics-dispatch', 'performance', 'testing-conventions'],
    files: ['crates/services/src/codex/mod.rs', 'crates/services/src/codex/compiler.rs', 'crates/services/src/codex/coverage.rs', 'crates/services/src/codex/folder.rs', 'crates/services/src/codex/reference.rs', 'crates/services/src/codex/search.rs', 'crates/services/src/codex/validation.rs', 'crates/services/src/codex/health.rs', 'crates/services/src/codex/test_support.rs'],
    focus: 'Service-layer logic over the core Codex model. Typed errors, tracing on expensive passes, no panics, performance of search/health passes. Keys vs data discipline (it may build user-facing strings — those must be keys).' },
  { id: 'U12', group: 'services', label: 'services/codex demo data', claudeMd: ['crates/CLAUDE.md', 'crates/services/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'i18n', 'serde'],
    files: ['crates/services/src/codex/demo/mod.rs', 'crates/services/src/codex/demo/entries.rs', 'crates/services/src/codex/demo/animations.rs', 'crates/services/src/codex/demo/recipes.rs', 'crates/services/src/codex/demo/rules.rs'],
    focus: 'Demo/seed content. Distinguish DATA (entry names, prompt fragments = legitimately literal user-content) from user-facing UI strings that must be keys. Flag panics/unwraps; large literal tables are fine if they are content.' },
  { id: 'U13', group: 'services', label: 'services tests', claudeMd: ['crates/CLAUDE.md', 'crates/services/CLAUDE.md'],
    skills: ['rust-conventions', 'testing-conventions', 'serde-json'],
    files: ['crates/services/tests/bit_demo.rs', 'crates/services/tests/history_round_trip.rs'],
    focus: 'Integration tests. unwrap/expect are allowed here. Judge coverage quality, rstest/serial_test usage, and whether the JSON round-trip patching of pub(crate) fields is sound.' },

  // ---- ui (egui + egui-wgpu + phosphor + image + wgpu + serde + tracing + i18n) ----
  { id: 'U14', group: 'ui', label: 'ui/lib + registries + brand/icons/playback', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'egui', 'i18n', 'rust-conventions', 'result-handling', 'tracing', 'generics-dispatch'],
    files: ['crates/ui/src/lib.rs', 'crates/ui/src/registry/mod.rs', 'crates/ui/src/registry/resolve.rs', 'crates/ui/src/region.rs', 'crates/ui/src/brand.rs', 'crates/ui/src/icons.rs', 'crates/ui/src/playback.rs'],
    focus: 'Registry-driven shell surface. brand uses NEAREST + install once; icons are phosphor consts. No per-frame tracing. Registries carry MsgKey, resolved render-time.' },
  { id: 'U15', group: 'ui', label: 'ui/contrib_api (Panel/Tool/Workspace traits)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'egui', 'i18n', 'rust-conventions', 'generics-dispatch', 'type-state', 'tracing'],
    files: ['crates/ui/src/contrib_api/mod.rs', 'crates/ui/src/contrib_api/context.rs', 'crates/ui/src/contrib_api/ids.rs', 'crates/ui/src/contrib_api/module.rs', 'crates/ui/src/contrib_api/panel.rs', 'crates/ui/src/contrib_api/tool.rs', 'crates/ui/src/contrib_api/tool_rail.rs', 'crates/ui/src/contrib_api/workspace.rs'],
    focus: 'The contribution traits. Panels &self + read-only ContribCtx + Intent for mutation. dyn vs generic boundary for the registries. MsgKey defined here. Trait object-safety and the deferred-intent contract.' },
  { id: 'U16', group: 'ui', label: 'ui/canvas (egui<->wgpu paint callback)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['egui', 'egui-wgpu', 'wgpu', 'bytemuck', 'glam', 'ui-conventions', 'performance', 'rust-conventions', 'result-handling'],
    files: ['crates/ui/src/canvas/mod.rs', 'crates/ui/src/canvas/onion.rs', 'crates/ui/src/canvas/overlay.rs', 'crates/ui/src/canvas/view.rs'],
    focus: 'egui-wgpu CallbackTrait prepare/finish_prepare/paint order, ViewportInPixels scissor math, RenderState access, no panic on surface error. glam transforms for view/zoom. Overlays via egui shapes. Performance of per-frame paint.' },
  { id: 'U17', group: 'ui', label: 'ui/theme (tokens, palettes, fonts, contrast)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'egui', 'rust-conventions'],
    files: ['crates/ui/src/theme/mod.rs', 'crates/ui/src/theme/contrast.rs', 'crates/ui/src/theme/fonts.rs', 'crates/ui/src/theme/palettes.rs', 'crates/ui/src/theme/tokens.rs'],
    focus: 'This is the source of truth for tokens, so raw color literals are EXPECTED here (this is where they are allowed). Judge token structure, contrast helpers, font/phosphor merge, egui 0.34 style API. Flag literals only if they leak OUT of the token layer.' },
  { id: 'U18', group: 'ui', label: 'ui/widgets (shared design-system widgets)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'egui', 'i18n', 'rust-conventions'],
    files: ['crates/ui/src/widgets/mod.rs', 'crates/ui/src/widgets/busy.rs', 'crates/ui/src/widgets/card.rs', 'crates/ui/src/widgets/codex.rs', 'crates/ui/src/widgets/layout.rs', 'crates/ui/src/widgets/placeholder.rs', 'crates/ui/src/widgets/section_header.rs', 'crates/ui/src/widgets/tool_button.rs', 'crates/ui/src/widgets/tray_tab.rs', 'crates/ui/src/widgets/workspace_tab.rs'],
    focus: 'Widgets must consume theme tokens (not hex), phosphor icons (not emoji). card draws its own title. The big widgets/codex.rs (1306 lines): watch for bespoke chrome duplicating card, hardcoded strings that should be keys, and &self/intent discipline.' },
  { id: 'U19', group: 'ui', label: 'ui/shell (runtime, menus, palette, splash, about)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'egui', 'eframe', 'i18n', 'rust-conventions', 'tracing', 'result-handling'],
    files: ['crates/ui/src/shell/mod.rs', 'crates/ui/src/shell/about.rs', 'crates/ui/src/shell/command_palette.rs', 'crates/ui/src/shell/menus.rs', 'crates/ui/src/shell/runtime.rs', 'crates/ui/src/shell/shortcuts.rs', 'crates/ui/src/shell/splash.rs'],
    focus: 'Shell runtime drives the egui loop region layout. Menus/palette labels are keys resolved render-time. Shortcut focus-gating via text_edit_focused. Coarse debug! tracing only, no per-frame flood.' },
  { id: 'U20', group: 'ui', label: 'ui/shell/regions (docks, rails, bars, stage)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'egui', 'i18n', 'rust-conventions', 'tracing'],
    files: ['crates/ui/src/shell/regions/mod.rs', 'crates/ui/src/shell/regions/bottom_tray.rs', 'crates/ui/src/shell/regions/canvas_stage.rs', 'crates/ui/src/shell/regions/center_stage.rs', 'crates/ui/src/shell/regions/left_dock.rs', 'crates/ui/src/shell/regions/left_rail.rs', 'crates/ui/src/shell/regions/right_dock.rs', 'crates/ui/src/shell/regions/scope_split.rs', 'crates/ui/src/shell/regions/status_bar.rs', 'crates/ui/src/shell/regions/tool_options.rs', 'crates/ui/src/shell/regions/top_bar.rs'],
    focus: 'Region chrome must use theme elevation tiers + tokens + shared widgets, labels via keys (status-bar items were recently keyed — verify none remain raw). Accent restraint. egui 0.34 panel API.' },
  { id: 'U21', group: 'ui', label: 'ui/state/intent (the 2243-line intent system)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'rust-conventions', 'i18n', 'generics-dispatch', 'result-handling'],
    files: ['crates/ui/src/state/intent.rs'],
    focus: 'The deferred-intent core: Intent enum + apply_intent. This is the seam where intents become Commands. Verify all mutation routes through Commands, no direct model writes, exhaustive matching, no unwrap, and that it is not carrying business logic that belongs in modules/services. Size alone may warrant a split recommendation.' },
  { id: 'U22', group: 'ui', label: 'ui/state (session, edit_session, ui_state)', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['ui-conventions', 'rust-conventions', 'i18n', 'tokio', 'serde', 'result-handling', 'parking-lot'],
    files: ['crates/ui/src/state/mod.rs', 'crates/ui/src/state/edit_session.rs', 'crates/ui/src/state/session.rs', 'crates/ui/src/state/ui_state.rs'],
    focus: 'The five state buckets (session/UI/derived). EditSession owns the document (single owner, &mut, no needless Arc<Mutex>) and holds the job/provider systems; drains channels each frame. serde Prefs round-trip. No durable data here (that is core).' },
  { id: 'U23', group: 'ui', label: 'ui tests', claudeMd: ['crates/CLAUDE.md', 'crates/ui/CLAUDE.md'],
    skills: ['testing-conventions', 'i18n', 'egui', 'tokio', 'rust-conventions'],
    files: ['crates/ui/tests/codex_layout_snapshot.rs', 'crates/ui/tests/generate_loop.rs', 'crates/ui/tests/i18n_keys.rs', 'crates/ui/tests/resolve_layout_snapshot.rs', 'crates/ui/tests/smoke.rs', 'crates/ui/tests/support/mod.rs'],
    focus: 'insta snapshots for layout, the i18n_keys lint test (catches hardcoded strings), the end-to-end generate loop with a runtime + mock provider. Judge coverage and that the i18n_keys test is comprehensive.' },

  // ---- modules ----
  { id: 'U24', group: 'mod-sprite-edit', label: 'mod/sprite-edit (draw, tools)', claudeMd: ['modules/CLAUDE.md', 'modules/sprite-edit/CLAUDE.md'],
    skills: ['egui', 'ui-conventions', 'i18n', 'tracing', 'performance', 'rust-conventions', 'result-handling'],
    files: ['modules/sprite-edit/src/lib.rs', 'modules/sprite-edit/src/draw.rs', 'modules/sprite-edit/src/tools.rs'],
    focus: 'Tools create Commands, never mutate the model. Registers with keys + info! on register. draw.rs hot path: no per-pixel tracing, flat buffers. Theme tokens + phosphor in any UI. Deferred-intent.' },
  { id: 'U25', group: 'mod-animation', label: 'mod/animation', claudeMd: ['modules/CLAUDE.md', 'modules/animation/CLAUDE.md'],
    skills: ['egui', 'ui-conventions', 'i18n', 'tracing', 'rust-conventions'],
    files: ['modules/animation/src/lib.rs', 'modules/animation/src/animate.rs'],
    focus: 'Sibling workspace over the same sprite core; composes, does not monopolize. Keys, tokens, intent discipline, #[instrument] on jobs.' },
  { id: 'U26', group: 'mod-codex', label: 'mod/codex workspace (editor, coverage)', claudeMd: ['modules/CLAUDE.md', 'modules/codex/CLAUDE.md'],
    skills: ['egui', 'ui-conventions', 'i18n', 'tracing', 'rust-conventions', 'result-handling'],
    files: ['modules/codex/src/codex_ws/editor.rs', 'modules/codex/src/codex_ws/coverage.rs'],
    focus: 'editor.rs is 1223 lines — watch for bespoke chrome vs card/section_header, hardcoded user-facing strings vs keys (keys.rs exists — confirm it is used), direct-mutation vs Intent, and whether logic belongs in services not the panel.' },
  { id: 'U27', group: 'mod-codex', label: 'mod/codex workspace (details, navigator, inspector, tray, keys)', claudeMd: ['modules/CLAUDE.md', 'modules/codex/CLAUDE.md'],
    skills: ['egui', 'ui-conventions', 'i18n', 'tracing', 'rust-conventions'],
    files: ['modules/codex/src/lib.rs', 'modules/codex/src/codex_ws/mod.rs', 'modules/codex/src/codex_ws/details.rs', 'modules/codex/src/codex_ws/inspector.rs', 'modules/codex/src/codex_ws/keys.rs', 'modules/codex/src/codex_ws/navigator.rs', 'modules/codex/src/codex_ws/tray.rs'],
    focus: 'Module owns its namespace via keys.rs. Panels &self + Intent. Tokens + phosphor + shared widgets. info! on register. No hidden global state.' },
  { id: 'U28', group: 'mod-generation', label: 'mod/generation (generate, prompt kb, codex context)', claudeMd: ['modules/CLAUDE.md', 'modules/generation/CLAUDE.md'],
    skills: ['egui', 'ui-conventions', 'i18n', 'tracing', 'rust-conventions', 'result-handling', 'generics-dispatch'],
    files: ['modules/generation/src/lib.rs', 'modules/generation/src/generate.rs', 'modules/generation/src/codex_context.rs', 'modules/generation/src/prompt/mod.rs', 'modules/generation/src/prompt/defaults.rs', 'modules/generation/src/prompt/kb.rs', 'modules/generation/src/prompt/types.rs'],
    focus: 'Generation is a Job; AI results apply via a Command, never touch the canvas. Prompt text / KB content is DATA, not i18n keys; UI labels ARE keys. The Generate workspace asks for a capability, not a specific provider.' },
  { id: 'U29', group: 'mod-providers', label: 'mod/providers (mock + openrouter)', claudeMd: ['modules/CLAUDE.md', 'modules/providers/CLAUDE.md'],
    skills: ['rust-conventions', 'result-handling', 'openrouter', 'reqwest', 'image', 'tokio', 'tokio-util', 'async-trait', 'thiserror', 'tracing', 'keyring', 'serde-json', 'i18n', 'testing-conventions'],
    files: ['modules/providers/src/lib.rs', 'modules/providers/src/mock.rs', 'modules/providers/src/openrouter.rs', 'modules/providers/src/postprocess.rs', 'modules/providers/tests/mock_generation_loop.rs', 'modules/providers/tests/openrouter_live.rs'],
    focus: 'NEVER log API keys (keyring/OS vault, not the log). Provider failure must not crash — isolated, surfaced as keyed error strings. Span per request with duration; error! on failure. openrouter 0.10 shape (.modalities/.image_config/ContentPart::ImageUrl/message.images). image: nearest for pixel art, Limits guard on decode. base64 data-URLs. async-trait Send trap or native async fn. CancellationToken. Provider text/model names are DATA not keys.' },
  { id: 'U30', group: 'mod-export-tiles', label: 'mod/export + mod/tiles', claudeMd: ['modules/CLAUDE.md', 'modules/export/CLAUDE.md', 'modules/tiles/CLAUDE.md'],
    skills: ['egui', 'ui-conventions', 'i18n', 'tracing', 'rust-conventions'],
    files: ['modules/export/src/lib.rs', 'modules/export/src/export_ws.rs', 'modules/tiles/src/lib.rs', 'modules/tiles/src/tiles_ws.rs'],
    focus: 'Workspace panels: tokens, phosphor, shared widgets, keys, deferred-intent. export must keep format logic out (that is io); a module wires capabilities, it does not reimplement them. info! on register.' },
  { id: 'U31', group: 'mod-stubs', label: 'mod/core + mod/pixel-art (stubs)', claudeMd: ['modules/CLAUDE.md', 'modules/core/CLAUDE.md', 'modules/pixel-art/CLAUDE.md'],
    skills: ['rust-conventions'],
    files: ['modules/core/src/lib.rs', 'modules/pixel-art/src/lib.rs'],
    focus: 'Deliberate compiling stubs. Confirm they are clean (documented, no todo!/unimplemented! tripping the Stop gate, registers nothing it should not). Note the roadmap intent without flagging the stub itself as a defect.' },

  // ---- app (binary; anyhow; eframe; owns runtime + subscriber + language) ----
  { id: 'U32', group: 'app', label: 'app binary (main, diagnostics, render example)', claudeMd: ['app/CLAUDE.md'],
    skills: ['eframe', 'egui', 'egui-wgpu', 'image', 'tokio', 'tracing', 'i18n', 'rust-conventions'],
    files: ['app/src/main.rs', 'app/src/diagnostics.rs', 'app/examples/render_workspaces.rs'],
    focus: 'The binary: anyhow (not thiserror) is CORRECT here; owns the ONE tokio runtime and the ONE tracing subscriber (file appender guard lives for all of main, log->tracing bridge); sets active language at boot via sys-locale defaulting to en. eframe 0.34 App trait (the method is ui, not update). image decodes the window icon. Registers modules. unwrap/expect still denied in main (it is non-test).' },
]

// ---------------- run ----------------
phase('Audit')

const merged = await pipeline(
  UNITS,
  // Stage 1: audit
  (unit) => agent(buildAuditPrompt(unit), { label: `audit:${unit.id} ${unit.label}`, phase: 'Audit', schema: AUDIT_SCHEMA }),
  // Stage 2: adversarial verify of this unit's findings, then merge verdicts in
  async (audit, unit) => {
    const findings = (audit && audit.findings) || []
    let verdicts = []
    if (findings.length > 0) {
      const verify = await agent(buildVerifyPrompt(unit, findings), { label: `verify:${unit.id} (${findings.length})`, phase: 'Verify', schema: VERIFY_SCHEMA })
      verdicts = (verify && verify.verdicts) || []
    }
    const byId = {}
    for (const v of verdicts) byId[v.id] = v
    const mergedFindings = findings.map((f) => {
      const v = byId[f.id] || {}
      return { ...f, verdict: v.verdict || 'unverified', verdict_confidence: v.confidence || null, verdict_note: v.note || '', corrected_severity: v.corrected_severity || f.severity }
    })
    return { unit, audit: audit || { compliance_summary: 'AUDIT AGENT RETURNED NO RESULT', strengths: [] }, findings: mergedFindings }
  },
)

const results = merged.filter(Boolean)

// Group results by writer group, preserving unit order.
const groupsOrder = []
const byGroup = {}
for (const r of results) {
  const g = r.unit.group
  if (!byGroup[g]) { byGroup[g] = []; groupsOrder.push(g) }
  byGroup[g].push(r)
}

phase('Write')

const written = await parallel(groupsOrder.map((g) => async () => {
  const unitsData = byGroup[g].map((r) => ({
    unit: r.unit.label,
    files: r.unit.files,
    skills: r.unit.skills,
    compliance_summary: r.audit.compliance_summary,
    strengths: r.audit.strengths || [],
    findings: r.findings,
  }))
  const md = await agent(buildWriterPrompt(g, unitsData), { label: `write:${g}`, phase: 'Write' })
  return { group: g, markdown: md, stats: groupStats(byGroup[g]) }
}))

// ---------------- stats ----------------
function sevTally(findings) {
  const t = { critical: 0, high: 0, medium: 0, low: 0, info: 0 }
  for (const f of findings) {
    if (f.verdict === 'false_positive') continue
    const s = f.corrected_severity || f.severity
    if (t[s] !== undefined) t[s] += 1
  }
  return t
}
function groupStats(rs) {
  const all = rs.flatMap((r) => r.findings)
  const confirmed = all.filter((f) => f.verdict === 'confirmed')
  const review = all.filter((f) => f.verdict === 'needs_human_review')
  const fp = all.filter((f) => f.verdict === 'false_positive')
  return { total: all.length, confirmed: confirmed.length, needs_review: review.length, false_positive: fp.length, severity: sevTally([...confirmed, ...review]) }
}

const allFindings = results.flatMap((r) => r.findings.map((f) => ({ ...f, group: r.unit.group, unit: r.unit.label })))
const confirmedAll = allFindings.filter((f) => f.verdict === 'confirmed')
const reviewAll = allFindings.filter((f) => f.verdict === 'needs_human_review')
const fpAll = allFindings.filter((f) => f.verdict === 'false_positive')

const sevRank = { critical: 0, high: 1, medium: 2, low: 3, info: 4 }
const topFindings = [...confirmedAll, ...reviewAll]
  .filter((f) => ['critical', 'high'].includes(f.corrected_severity || f.severity))
  .sort((a, b) => sevRank[a.corrected_severity || a.severity] - sevRank[b.corrected_severity || b.severity])
  .map((f) => ({ group: f.group, file: f.file, lines: f.lines, severity: f.corrected_severity || f.severity, category: f.category, issue: f.issue, recommendation: f.recommendation, verdict: f.verdict }))

const byCategory = {}
for (const f of [...confirmedAll, ...reviewAll]) byCategory[f.category] = (byCategory[f.category] || 0) + 1

log(`Audit complete: ${results.length} units, ${allFindings.length} raw findings -> ${confirmedAll.length} confirmed, ${reviewAll.length} need review, ${fpAll.length} false positives`)

return {
  date: DATE,
  units_audited: results.length,
  groups: written.filter(Boolean),
  overall: {
    raw_findings: allFindings.length,
    confirmed: confirmedAll.length,
    needs_review: reviewAll.length,
    false_positive: fpAll.length,
    severity: sevTally([...confirmedAll, ...reviewAll]),
    by_category: byCategory,
  },
  top_findings: topFindings,
}

// ---------------- prompt builders ----------------
function buildAuditPrompt(unit) {
  return [
    `You are a meticulous Rust + egui/wgpu code auditor for Pixhaus, an MIT-licensed native (eframe + egui + wgpu) sprite/animation editor. This is a READ-ONLY audit of audit unit "${unit.label}" (${unit.id}). Do NOT modify any file.`,
    `Repo root: ${ROOT}. All paths below are relative to it.`,
    ``,
    `STEP 1 - Load the rubric. Read these skill files IN FULL (they are the rules for this unit):`,
    unit.skills.map((s) => `  - .claude/skills/pixhaus-${s}/SKILL.md`).join('\n'),
    `Read these instruction files (boundary + convention rules):`,
    `  - CLAUDE.md   (repo root - global rules)`,
    unit.claudeMd.map((c) => `  - ${c}`).join('\n'),
    `If a SKILL.md points you to a file under its references/ subdir for an exact API signature you need to judge version-correctness, read that file too.`,
    ``,
    `STEP 2 - Read EVERY one of these source files IN FULL:`,
    unit.files.map((f) => `  - ${f}`).join('\n'),
    ``,
    `STEP 3 - Audit each file against the rubric below. ${CHECK}`,
    ``,
    `UNIT-SPECIFIC FOCUS: ${unit.focus}`,
    ``,
    `STEP 4 - Report. For every REAL violation, emit a finding with: an exact file path; a line range you actually read; severity; a short category tag; the precise rule it breaks (name the skill / CLAUDE.md / bible rule); the offending code as evidence; and a concrete fix. Be grounded - cite real lines, never speculate or invent. Critically: a string that is ALREADY a key/MsgKey is NOT a hardcoded-string violation; a color pulled from a theme token is NOT a hex-literal violation; unwrap() inside #[cfg(test)] / a tests/ file is EXEMPT; anyhow in app/ is CORRECT; core/render having no tracing/i18n is CORRECT; a documented compiling stub is not a defect. Read enough to be sure before flagging. Also list genuine STRENGTHS (exemplary compliance) so the report reflects real quality, not only defects. Give an honest 2-4 sentence compliance_summary. Return ONLY the structured object.`,
  ].join('\n')
}

function buildVerifyPrompt(unit, findings) {
  return [
    `You are an ADVERSARIAL verifier checking another auditor's work on Pixhaus audit unit "${unit.label}" (${unit.id}). Repo root: ${ROOT}.`,
    `Auditors hallucinate and misapply rules. Your job is to independently confirm or reject each finding by re-reading the EXACT cited code and the EXACT cited rule. Be skeptical; do not rubber-stamp.`,
    ``,
    `Common auditor errors to catch and mark false_positive:`,
    `  - "hardcoded string" that is actually an i18n key / MsgKey, or is legitimate DATA (a file name, prompt fragment, model name, demo content) that must NOT be a key.`,
    `  - "hex color literal" that is actually inside crates/ui/src/theme/* (the token source-of-truth, where literals are allowed) or is a theme token reference.`,
    `  - "unwrap/expect/panic" that is in test code (#[cfg(test)], a tests/ file, or a #[test] fn) - those are EXEMPT.`,
    `  - "must use thiserror" applied to the app/ binary (anyhow is correct there); or "missing tracing/i18n" applied to core/render (they are correctly free of both).`,
    `  - "missing test" when a test exists in a sibling tests/ dir or another module; "missing doc" on a non-public item.`,
    `  - a rule quoted that does not actually say what the finding claims, or does not apply to this crate's layer.`,
    ``,
    `For each finding: read the file at the cited lines; if it cites a skill/CLAUDE.md rule, confirm the rule's text and that it applies to THIS file's crate/layer. Then verdict: "confirmed" (you can quote BOTH the offending code and the rule), "false_positive" (fine / rule doesn't apply / test-exempt / already compliant), or "needs_human_review" (a genuine judgment call or ambiguous). When uncertain, prefer needs_human_review over confirmed. Set corrected_severity to your own assessment.`,
    ``,
    `Findings to verify (JSON):`,
    JSON.stringify(findings),
    ``,
    `Return ONLY the structured verdicts, one per finding id.`,
  ].join('\n')
}

function buildWriterPrompt(group, unitsData) {
  return [
    `Write a thorough Markdown audit section for the "${group}" part of the Pixhaus codebase, from the VERIFIED audit data below. Output Markdown only - no preamble, no code fences around the whole thing.`,
    ``,
    `Voice: Pixhaus "Pragmatic Leader" - direct, declarative, state the issue then the fix. Sentence-case headings. Straight quotes. No emoji. Avoid LLM filler ("comprehensive", "robust", "powerful", "moreover").`,
    ``,
    `Structure exactly:`,
    `## ${group}`,
    `One short paragraph (2-4 sentences) of honest overall compliance for this group, synthesizing the per-unit summaries. State the compliance level plainly.`,
    ``,
    `### Strengths`,
    `Bulleted, deduplicated across units - the exemplary compliance worth preserving.`,
    ``,
    `### Findings`,
    `A Markdown table of all findings whose verdict is "confirmed" or "needs_human_review" (SKIP false_positive here). Columns: ID | File:Lines | Severity | Category | Issue -> Fix. Sort by severity critical -> high -> medium -> low -> info, then by file. Append " (review)" to the Severity cell for needs_human_review rows. Keep Issue and Fix specific: name the rule and the exact change. If there are none, write "No confirmed findings.".`,
    ``,
    `### Checked and cleared (false positives)`,
    `A short bulleted list of findings the verifier REJECTED, each one line: the claim and why it was rejected. This shows the audit was checked, not credulous. If none, write "None.".`,
    ``,
    `Do not invent anything beyond the data. Per-unit data (JSON):`,
    JSON.stringify(unitsData),
  ].join('\n')
}
