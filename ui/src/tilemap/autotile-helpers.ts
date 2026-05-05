// Pure helper functions for autotile rule editing.
//
// Extracted into their own module so unit tests can import them without
// pulling in the Solid component (which registers DOM event handlers at
// module load and fails in a node test environment).

import type { AutotileRule, NeighborCondition, TileIndex } from "../lib/types";

export function nextCondition(c: NeighborCondition): NeighborCondition {
  if (c === "any") return "filled";
  if (c === "filled") return "empty";
  return "any";
}

export function conditionLabel(c: NeighborCondition): string {
  if (c === "filled") return "F";
  if (c === "empty") return "E";
  return "·";
}

export function conditionTitle(c: NeighborCondition): string {
  if (c === "filled") return "Filled";
  if (c === "empty") return "Empty";
  return "Any";
}

export function blankRule(): AutotileRule {
  return {
    conditions: ["any", "any", "any", "any", "any", "any", "any", "any"],
    result_tile: 0 as TileIndex,
    result_flags: 0,
  };
}
