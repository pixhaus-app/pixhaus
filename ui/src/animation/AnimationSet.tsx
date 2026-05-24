// Animation set: the per-entity directional cascade grid.
//
// Shows coverage and staleness across type x direction. Clicking a cell
// prefills the studio controls for that direction and type. A stale banner
// appears when the anchor changed after a derived animation was made.

import { type Component, For, Show, createResource } from "solid-js";

import { type AnimationSet as AnimationSetData, animationSet } from "../lib/commands/animation";
import {
  type AnimType,
  type DirectionOpt,
  setAnimType,
  setDirection,
} from "./animation-studio-state";

type Props = {
  entityId: number;
  onRerollDependents: () => void;
};

const TYPE_ORDER: AnimType[] = ["idle", "walk", "attack"];
const DIR_ORDER: DirectionOpt[] = ["south", "west", "north", "east"];

function statusLabel(status: string): string {
  switch (status) {
    case "present":
      return "ok";
    case "stale":
      return "stale";
    default:
      return "—";
  }
}

const AnimationSet: Component<Props> = (props) => {
  const [data, { refetch }] = createResource<AnimationSetData>(() => animationSet(props.entityId));

  const cellStatus = (set: AnimationSetData, ty: string, dir: string): string =>
    set.cells.find((c) => c.animation_type === ty && c.direction === dir)?.status ?? "missing";

  return (
    <div class="animation-studio__set" data-testid="animation-set">
      <header class="animation-studio__set-head">
        Animation set
        <button
          class="animation-studio__btn animation-studio__btn--small"
          onClick={() => void refetch()}
        >
          refresh
        </button>
      </header>

      <Show when={data()} fallback={<div class="animation-studio__empty">Loading…</div>}>
        {(set) => (
          <>
            <table class="animation-studio__set-grid">
              <thead>
                <tr>
                  <th />
                  <For each={DIR_ORDER}>{(d) => <th>{d}</th>}</For>
                </tr>
              </thead>
              <tbody>
                <For each={TYPE_ORDER}>
                  {(ty) => (
                    <tr>
                      <th>{ty}</th>
                      <For each={DIR_ORDER}>
                        {(dir) => {
                          const status = (): string => cellStatus(set(), ty, dir);
                          return (
                            <td>
                              <button
                                class="animation-studio__set-cell"
                                classList={{
                                  "animation-studio__set-cell--present": status() === "present",
                                  "animation-studio__set-cell--stale": status() === "stale",
                                }}
                                onClick={() => {
                                  setAnimType(ty);
                                  setDirection(dir);
                                }}
                              >
                                {statusLabel(status())}
                              </button>
                            </td>
                          );
                        }}
                      </For>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>

            <Show when={set().has_stale}>
              <div class="animation-studio__stale-banner" data-testid="stale-banner">
                Anchor changed — derived animations may be stale.
                <button class="animation-studio__btn" onClick={props.onRerollDependents}>
                  Re-roll dependents
                </button>
              </div>
            </Show>
          </>
        )}
      </Show>
    </div>
  );
};

export default AnimationSet;
