// Shape of the inputs the `pixhaus.builtin.animated_sprite_sheet` verb
// accepts. Kept in sync with
// `ai/src/verbs/animated_sprite_sheet/mod.rs::AnimatedSpriteSheetInputs`.

export const ANIMATED_SPRITE_SHEET_VERB_ID = "pixhaus.builtin.animated_sprite_sheet";

/** Bounds — mirror the Rust constants. */
export const MIN_GRID_SIZE = 2;
export const MAX_GRID_SIZE = 6;
export const DEFAULT_GRID_SIZE = 4;
export const MIN_CELL_SIZE_PX = 16;
export const MAX_CELL_SIZE_PX = 256;
export const DEFAULT_CELL_SIZE_PX = 64;
export const DEFAULT_FRAME_DURATION_MS = 100;

export type GenerationMode = "procedural" | "ai" | "hybrid";

/** Curated action chips. Free-form strings are also accepted. */
export const ACTION_PRESETS: readonly string[] = [
  "idle",
  "walk",
  "run",
  "attack",
  "cast",
  "jump",
  "dance",
  "death",
  "dodge",
];

export type AnimatedSpriteSheetInputs = {
  prompt: string;
  grid_size: number;
  actions?: string[];
  cell_size_px?: number;
  frame_duration_ms?: number;
  mode?: GenerationMode;
  rewritten_prompt?: string;
  system_prompt_override?: string;
  constraint_prompt_override?: string;
  style_reference?: number[];
  layer_name?: string;
  seed?: number;
};
