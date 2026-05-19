// Pure helpers extracted from `AnimatedSpriteSheetForm.tsx` so the
// chip-multi-select constraint can be unit-tested without mounting a
// Solid tree.

import {
  type AnimatedSpriteSheetInputs,
  type GenerationMode,
  DEFAULT_CELL_SIZE_PX,
  DEFAULT_FRAME_DURATION_MS,
  DEFAULT_GRID_SIZE,
  MAX_CELL_SIZE_PX,
  MAX_GRID_SIZE,
  MIN_CELL_SIZE_PX,
  MIN_GRID_SIZE,
} from "./types";

export type FormState = {
  prompt: string;
  gridSize: number;
  actions: string[];
  cellSizePx: number;
  frameDurationMs: number;
  mode: GenerationMode;
  rewrittenPrompt: string;
  layerName: string;
  seed: string;
};

/** Returns a fresh form state populated with the upstream defaults. */
export function defaultFormState(): FormState {
  return {
    prompt: "",
    gridSize: DEFAULT_GRID_SIZE,
    actions: [],
    cellSizePx: DEFAULT_CELL_SIZE_PX,
    frameDurationMs: DEFAULT_FRAME_DURATION_MS,
    mode: "ai",
    rewrittenPrompt: "",
    layerName: "",
    seed: "",
  };
}

/**
 * Returns the next `actions` array after the user clicked a chip.
 *
 * - If the chip is already selected, it is removed.
 * - If selecting would exceed `gridSize`, the call is *silently
 *   ignored* (mirroring FalSprite's UX): the form flashes the constraint
 *   visually rather than throwing. The caller distinguishes between
 *   "removed" / "added" / "rejected" via [`ChipToggleOutcome`].
 */
export type ChipToggleOutcome = "added" | "removed" | "rejected-capacity";

export function toggleChip(
  current: readonly string[],
  chip: string,
  gridSize: number,
): { actions: string[]; outcome: ChipToggleOutcome } {
  const trimmed = chip.trim();
  if (trimmed.length === 0) {
    return { actions: [...current], outcome: "rejected-capacity" };
  }
  if (current.includes(trimmed)) {
    return {
      actions: current.filter((a) => a !== trimmed),
      outcome: "removed",
    };
  }
  if (current.length >= gridSize) {
    return { actions: [...current], outcome: "rejected-capacity" };
  }
  return {
    actions: [...current, trimmed],
    outcome: "added",
  };
}

/**
 * Trims the action list down to fit a smaller grid. The user can shrink
 * the grid below their current action count — we drop the trailing
 * actions rather than refuse the change.
 */
export function fitActionsToGrid(actions: readonly string[], gridSize: number): string[] {
  return actions.slice(0, Math.max(0, gridSize));
}

export type ValidationOutcome = { ok: true } | { ok: false; reason: string };

/** Mirrors the Rust-side validation rules so the submit button is gated. */
export function validate(state: FormState): ValidationOutcome {
  const prompt = state.prompt.trim();
  if (prompt.length === 0) {
    return { ok: false, reason: "Prompt cannot be empty." };
  }
  if (state.gridSize < MIN_GRID_SIZE || state.gridSize > MAX_GRID_SIZE) {
    return {
      ok: false,
      reason: `Grid size must be between ${MIN_GRID_SIZE} and ${MAX_GRID_SIZE}.`,
    };
  }
  if (state.actions.length > state.gridSize) {
    return {
      ok: false,
      reason: `Too many actions (${state.actions.length}) for grid size ${state.gridSize}.`,
    };
  }
  for (const a of state.actions) {
    if (a.trim().length === 0) {
      return { ok: false, reason: "Action labels cannot be empty." };
    }
  }
  if (state.cellSizePx < MIN_CELL_SIZE_PX || state.cellSizePx > MAX_CELL_SIZE_PX) {
    return {
      ok: false,
      reason: `Cell size must be between ${MIN_CELL_SIZE_PX} and ${MAX_CELL_SIZE_PX} px.`,
    };
  }
  if (state.frameDurationMs < 1) {
    return { ok: false, reason: "Frame duration must be at least 1 ms." };
  }
  if (state.seed.length > 0) {
    const n = Number(state.seed);
    if (!Number.isFinite(n) || !Number.isInteger(n) || n < 0) {
      return { ok: false, reason: "Seed must be a non-negative integer." };
    }
  }
  return { ok: true };
}

/** Converts a validated form state into the verb's input payload. */
export function toInputs(state: FormState): AnimatedSpriteSheetInputs {
  const payload: AnimatedSpriteSheetInputs = {
    prompt: state.prompt.trim(),
    grid_size: state.gridSize,
    cell_size_px: state.cellSizePx,
    frame_duration_ms: state.frameDurationMs,
    mode: state.mode,
  };
  if (state.actions.length > 0) {
    payload.actions = state.actions.map((a) => a.trim());
  }
  const rewritten = state.rewrittenPrompt.trim();
  if (rewritten.length > 0) {
    payload.rewritten_prompt = rewritten;
  }
  const layer = state.layerName.trim();
  if (layer.length > 0) {
    payload.layer_name = layer;
  }
  if (state.seed.length > 0) {
    const n = Number(state.seed);
    if (Number.isFinite(n) && Number.isInteger(n) && n >= 0) {
      payload.seed = n;
    }
  }
  return payload;
}
