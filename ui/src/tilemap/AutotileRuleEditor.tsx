// Autotile rule editor.
//
// For standard rule sets (Blob47, Corner16, Minimal4) the rules are implicit
// and the editor shows a read-only description.  For custom rule sets the
// editor exposes a visual 3x3 neighbor grid per rule so the user can define
// when each tile index should be placed.
//
// Hydrates from `Tileset.autotile` when the active tilemap context arrives
// or changes; persists edits back via debounced `tilesetSetAutotile` so the
// rules survive save/reopen.

import { For, Show, createEffect, createMemo, onCleanup, type Component } from "solid-js";
import { produce } from "solid-js/store";
import type { AutotileKind, AutotileRule, TileIndex } from "../lib/types";
import {
  activeTilemapCtx,
  autotileDefaultTile,
  autotileRules,
  localAutotileKind,
  setActiveTilemapCtx,
  setAutotileDefaultTile,
  setAutotileRules,
  setLocalAutotileKind,
} from "./tilemap-state";
import { blankRule, conditionLabel, conditionTitle, nextCondition } from "./autotile-helpers";
import { activeSpriteId } from "../canvas/canvas-state";
import { tilesetSetAutotile } from "../lib/commands/tilesets";
import { reportCommandFailure } from "../lib/utils/errors";

// ── Neighbor order: NW, N, NE, W, E, SW, S, SE (matches core constants) ──────

const NEIGHBOR_LABELS = ["NW", "N", "NE", "W", "E", "SW", "S", "SE"] as const;

// 3×3 display grid positions for the 8 neighbors (the center cell is "this").
// Row-major order, center index = 4.
const GRID_POSITIONS = [0, 1, 2, 3, 5, 6, 7, 8] as const; // skip index 4

// Maps from the 8-element conditions tuple to 3×3 grid cell indices.
const CONDITION_TO_GRID: readonly number[] = GRID_POSITIONS;

// ── Standard-set description ───────────────────────────────────────────────

interface StandardInfoProps {
  kind: "blob47" | "corner16" | "minimal4";
}

const STANDARD_DESCRIPTIONS: Record<StandardInfoProps["kind"], { title: string; body: string }> = {
  blob47: {
    title: "47-tile Wang edge-blob",
    body:
      "Checks all 8 neighbors with corner trimming. " +
      "Tile indices 0–46 cover every connected-region transition. " +
      "Your tileset needs exactly 47 tiles arranged in blob order.",
  },
  corner16: {
    title: "16-tile Wang corner-blob",
    body:
      "Samples the 4 diagonal neighbors only. " +
      "Each combination of filled/empty corners maps to one of 16 tiles. " +
      "Compact format — good for small tilesets.",
  },
  minimal4: {
    title: "4-tile minimal blob",
    body:
      "Maps the number of cardinal neighbor connections (0–3+) to 4 tiles. " +
      "The simplest format: isolated, end, middle-of-run, and cross tiles.",
  },
};

const StandardInfo: Component<StandardInfoProps> = (props) => {
  const desc = createMemo(() => STANDARD_DESCRIPTIONS[props.kind]);
  return (
    <div class="are__standard-info">
      <div class="are__standard-title">{desc().title}</div>
      <p class="are__standard-body">{desc().body}</p>
      <p class="are__standard-note">
        No custom rules needed. Place the correct tile count in your tileset and Pixhaus resolves
        neighbors automatically.
      </p>
    </div>
  );
};

// ── Custom rule editor ─────────────────────────────────────────────────────

interface RuleRowProps {
  rule: AutotileRule;
  index: number;
  onConditionClick: (ruleIdx: number, neighborIdx: number) => void;
  onResultTileChange: (ruleIdx: number, value: number) => void;
  onDelete: (ruleIdx: number) => void;
}

const RuleRow: Component<RuleRowProps> = (props) => {
  // Build 3×3 grid cells: 8 neighbor cells + 1 "this" centre cell.
  const gridCells = createMemo(() => {
    const cells: Array<{ label: string; title: string; neighborIdx: number | null }> = Array.from({
      length: 9,
    });
    for (let gridPos = 0; gridPos < 9; gridPos++) {
      if (gridPos === 4) {
        // Centre cell represents "this tile".
        cells[gridPos] = { label: "T", title: "This tile", neighborIdx: null };
      } else {
        const neighborIdx = CONDITION_TO_GRID.indexOf(gridPos);
        // conditions is a fixed 8-tuple; neighborIdx is always 0–7 here.
        const cond = props.rule.conditions[neighborIdx]!;
        cells[gridPos] = {
          label: conditionLabel(cond),
          title: `${NEIGHBOR_LABELS[neighborIdx]!}: ${conditionTitle(cond)}`,
          neighborIdx,
        };
      }
    }
    return cells;
  });

  return (
    <div class="are__rule-row">
      {/* 3×3 neighbor grid */}
      <div
        class="are__rule-grid"
        role="group"
        aria-label={`Rule ${props.index + 1} neighbor conditions`}
      >
        <For each={gridCells()}>
          {(cell) => (
            <button
              class="are__rule-cell"
              classList={{
                "are__rule-cell--center": cell.neighborIdx === null,
                "are__rule-cell--filled":
                  cell.neighborIdx !== null &&
                  props.rule.conditions[cell.neighborIdx]! === "filled",
                "are__rule-cell--empty":
                  cell.neighborIdx !== null && props.rule.conditions[cell.neighborIdx]! === "empty",
              }}
              title={cell.title}
              disabled={cell.neighborIdx === null}
              onClick={() => {
                if (cell.neighborIdx !== null) {
                  props.onConditionClick(props.index, cell.neighborIdx);
                }
              }}
            >
              {cell.label}
            </button>
          )}
        </For>
      </div>

      {/* Result tile index */}
      <div class="are__rule-result">
        <label class="are__rule-result-label">Tile</label>
        <input
          class="are__rule-tile-input"
          type="number"
          min={0}
          value={props.rule.result_tile}
          onInput={(e) => {
            const v = parseInt(e.currentTarget.value, 10);
            if (!isNaN(v) && v >= 0) {
              props.onResultTileChange(props.index, v);
            }
          }}
        />
      </div>

      {/* Delete rule */}
      <button
        class="are__rule-delete"
        title="Delete rule"
        onClick={() => props.onDelete(props.index)}
      >
        ✕
      </button>
    </div>
  );
};

// ── Root component ─────────────────────────────────────────────────────────

const AutotileRuleEditor: Component = () => {
  const kind = localAutotileKind;

  // Custom rule state lives at module scope in tilemap-state so it
  // survives the editor unmounting (e.g. when the user switches
  // preferences tabs). The aliases below keep the rest of this file
  // readable.
  const rules = autotileRules;
  const setRules = setAutotileRules;
  const defaultTile = autotileDefaultTile;
  const setDefaultTile = setAutotileDefaultTile;

  // Hydrate UI state from the active tileset's persisted autotile field
  // whenever the context changes (project open, layer switch, tileset
  // switch). Without this, the editor would always show the previous
  // session's choice regardless of what's actually saved.
  let lastHydratedTilesetId: number | null = null;
  createEffect(() => {
    const ctx = activeTilemapCtx();
    if (!ctx) {
      lastHydratedTilesetId = null;
      return;
    }
    if (lastHydratedTilesetId === ctx.tilesetId) return;
    lastHydratedTilesetId = ctx.tilesetId;
    const persisted = ctx.tileset.autotile ?? null;
    setLocalAutotileKind(persisted);
    if (persisted && persisted.kind === "custom") {
      setAutotileRules(persisted.rules ?? []);
      setAutotileDefaultTile(persisted.default_tile ?? (0 as TileIndex));
    } else {
      setAutotileRules([]);
      setAutotileDefaultTile(0 as TileIndex);
    }
  });

  // Debounced persist. Edits to kind / rules / default_tile schedule
  // one IPC call after `PERSIST_DEBOUNCE_MS` of quiet, mirroring the
  // rename-on-blur cadence used elsewhere in the panel.
  const PERSIST_DEBOUNCE_MS = 200;
  let persistTimer: ReturnType<typeof setTimeout> | null = null;
  function schedulePersist() {
    const ctx = activeTilemapCtx();
    const sid = activeSpriteId();
    if (!ctx || sid === null) return;
    if (persistTimer !== null) clearTimeout(persistTimer);
    persistTimer = setTimeout(() => {
      persistTimer = null;
      const cur = activeTilemapCtx();
      const sidNow = activeSpriteId();
      if (!cur || sidNow === null) return;
      // Build the kind to persist from the current UI state. For
      // `custom`, snapshot the live rules + default_tile so the
      // persisted value reflects exactly what's on screen.
      const liveKind = localAutotileKind();
      const payload: AutotileKind | null =
        liveKind === null
          ? null
          : liveKind.kind === "custom"
            ? {
                kind: "custom",
                rules: [...autotileRules],
                default_tile: autotileDefaultTile(),
              }
            : { kind: liveKind.kind };
      tilesetSetAutotile(sidNow, cur.tilesetId, payload)
        .then((updated) => {
          const stillCurrent = activeTilemapCtx();
          if (stillCurrent && stillCurrent.tilesetId === updated.id) {
            setActiveTilemapCtx({ ...stillCurrent, tileset: updated });
          }
        })
        .catch((err) => reportCommandFailure("autotile persist", err));
    }, PERSIST_DEBOUNCE_MS);
  }
  onCleanup(() => {
    if (persistTimer !== null) clearTimeout(persistTimer);
  });

  // ── Kind picker ───────────────────────────────────────────────────────

  // Builds a fully-typed AutotileKind for a button click. The custom
  // variant is intersected with AutotileRuleSet, so a bare `{ kind:
  // "custom" }` object is invalid — pull the live rules + default_tile
  // from the hoisted state so the kind value reflects the current
  // editor contents.
  function buildKind(k: AutotileKind["kind"]): AutotileKind {
    if (k === "custom") {
      return {
        kind: "custom",
        rules: [...autotileRules],
        default_tile: autotileDefaultTile(),
      };
    }
    return { kind: k };
  }

  function selectKind(k: AutotileKind) {
    setLocalAutotileKind(k);
    if (k.kind === "custom") {
      setRules([]);
    }
    schedulePersist();
  }

  // ── Custom rule mutations ─────────────────────────────────────────────

  function addRule() {
    setRules((prev) => [...prev, blankRule()]);
    schedulePersist();
  }

  function deleteRule(idx: number) {
    setRules((prev) => prev.filter((_, i) => i !== idx));
    schedulePersist();
  }

  function cycleCondition(ruleIdx: number, neighborIdx: number) {
    setRules(
      produce((draft) => {
        const rule = draft[ruleIdx];
        if (!rule) return;
        rule.conditions[neighborIdx] = nextCondition(rule.conditions[neighborIdx]!);
      }),
    );
    schedulePersist();
  }

  function setResultTile(ruleIdx: number, value: number) {
    setRules(
      produce((draft) => {
        const rule = draft[ruleIdx];
        if (!rule) return;
        rule.result_tile = value as TileIndex;
      }),
    );
    schedulePersist();
  }

  function persistDefaultTile(value: number) {
    setDefaultTile(value as TileIndex);
    schedulePersist();
  }

  const currentKind = createMemo(() => kind());

  return (
    <div class="are">
      {/* Kind selector */}
      <div class="are__kind-row">
        <span class="are__kind-label">Rule type</span>
        <div class="are__kind-btns">
          <For each={["blob47", "corner16", "minimal4", "custom"] as const}>
            {(k) => (
              <button
                class="are__kind-btn"
                classList={{ "are__kind-btn--active": currentKind()?.kind === k }}
                onClick={() => selectKind(buildKind(k))}
              >
                {k}
              </button>
            )}
          </For>
        </div>
      </div>

      {/* Standard set description */}
      <Show when={currentKind() && currentKind()!.kind !== "custom"}>
        <StandardInfo kind={currentKind()!.kind as "blob47" | "corner16" | "minimal4"} />
      </Show>

      {/* Custom rule editor */}
      <Show when={currentKind()?.kind === "custom"}>
        <div class="are__custom">
          <div class="are__rules-header">
            <span class="are__rules-count">
              {rules.length} rule{rules.length !== 1 ? "s" : ""}
            </span>
            <button class="are__add-rule-btn" onClick={addRule}>
              + Add rule
            </button>
          </div>

          <Show
            when={rules.length > 0}
            fallback={
              <p class="are__empty-hint">
                No rules yet. Add a rule to define when each tile index should be placed.
              </p>
            }
          >
            <div class="are__rules-list">
              <For each={rules}>
                {(rule, i) => (
                  <RuleRow
                    rule={rule}
                    index={i()}
                    onConditionClick={cycleCondition}
                    onResultTileChange={setResultTile}
                    onDelete={deleteRule}
                  />
                )}
              </For>
            </div>
          </Show>

          <div class="are__default-row">
            <label class="are__default-label">Default tile (no match)</label>
            <input
              class="are__rule-tile-input"
              type="number"
              min={0}
              value={defaultTile()}
              onInput={(e) => {
                const v = parseInt(e.currentTarget.value, 10);
                if (!isNaN(v) && v >= 0) persistDefaultTile(v);
              }}
            />
          </div>
        </div>
      </Show>

      {/* Empty state — no kind selected */}
      <Show when={!currentKind()}>
        <p class="are__empty-hint">Select a rule type above to configure autotile behavior.</p>
      </Show>
    </div>
  );
};

export default AutotileRuleEditor;
