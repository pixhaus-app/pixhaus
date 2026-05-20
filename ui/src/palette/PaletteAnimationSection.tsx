// Palette animation keyframe controls (S54).
//
// Lets the user keyframe a single palette entry's color over the timeline:
// pick an entry, set a keyframe at the current frame from the entry's static
// color, list and delete keyframes, and watch the "resolved at frame N" chip
// update as the timeline scrubs (step semantics). This surface is
// self-contained — it does not recolor the canvas; the visible, testable
// effect is the resolved chip changing as the active frame changes.

import { For, Show, createMemo, createResource, createSignal, type Component } from "solid-js";
import { activePalette, activePaletteId } from "./palette-panel-state";
import {
  paletteAnimationGet,
  paletteAnimationRemoveKeyframe,
  paletteAnimationResolved,
  paletteAnimationSetKeyframe,
} from "../lib/commands/palette";
import { rgbaToCss } from "./color-utils";
import { activeFrameIndex } from "../canvas/canvas-state";
import { reportCommandFailure } from "../lib/utils/errors";
import type { Rgba, SpriteId } from "../lib/types";

type Props = {
  /** The sprite owning the palette. Null when no project is open. */
  spriteId: SpriteId | null;
};

/** A single keyframe row, flattened from the per-entry animation table. */
type Keyframe = {
  frame: number;
  color: Rgba;
};

const PaletteAnimationSection: Component<Props> = (props) => {
  // Index of the entry being keyframed. Clamped to the palette length on use.
  const [entryIndex, setEntryIndex] = createSignal(0);
  // Bumped after each mutation to re-run the keyframe-list resource.
  const [refreshTick, setRefreshTick] = createSignal(0);

  const colorCount = (): number => activePalette()?.colors.length ?? 0;

  // Source tuple for the keyframe-list resource: re-fetch when the sprite,
  // palette, selected entry, or a mutation changes.
  const listSource = createMemo(() => {
    const sid = props.spriteId;
    const pid = activePaletteId();
    if (sid === null || pid === null) return null;
    return { sid, pid, entry: entryIndex(), tick: refreshTick() };
  });

  const [keyframes] = createResource(listSource, async (src): Promise<Keyframe[]> => {
    const animation = await paletteAnimationGet(src.sid, src.pid);
    const perEntry = animation?.keyframes?.[src.entry];
    if (!perEntry) return [];
    return Object.entries(perEntry)
      .map(([frame, color]) => ({ frame: Number(frame), color: color as Rgba }))
      .sort((a, b) => a.frame - b.frame);
  });

  // Source tuple for the resolved chip: also tracks the active frame so the
  // chip recomputes as the user scrubs the timeline.
  const resolvedSource = createMemo(() => {
    const sid = props.spriteId;
    const pid = activePaletteId();
    if (sid === null || pid === null) return null;
    return { sid, pid, entry: entryIndex(), frame: activeFrameIndex(), tick: refreshTick() };
  });

  const [resolved] = createResource(resolvedSource, async (src): Promise<Rgba | null> => {
    const colors = await paletteAnimationResolved(src.sid, src.pid, src.frame);
    return colors[src.entry] ?? null;
  });

  const onEntryInput = (value: string): void => {
    const n = Number.parseInt(value, 10);
    if (Number.isNaN(n)) return;
    const max = Math.max(0, colorCount() - 1);
    setEntryIndex(Math.min(Math.max(n, 0), max));
  };

  const handleSetKeyframe = async (): Promise<void> => {
    const sid = props.spriteId;
    const pid = activePaletteId();
    const palette = activePalette();
    const idx = entryIndex();
    const color = palette?.colors[idx]?.color;
    if (sid === null || pid === null || color === undefined) return;
    try {
      await paletteAnimationSetKeyframe(sid, pid, idx, activeFrameIndex(), color);
      setRefreshTick((t) => t + 1);
    } catch (err: unknown) {
      reportCommandFailure("palette_animation_set_keyframe", err);
    }
  };

  const handleRemoveKeyframe = async (frame: number): Promise<void> => {
    const sid = props.spriteId;
    const pid = activePaletteId();
    if (sid === null || pid === null) return;
    try {
      await paletteAnimationRemoveKeyframe(sid, pid, entryIndex(), frame);
      setRefreshTick((t) => t + 1);
    } catch (err: unknown) {
      reportCommandFailure("palette_animation_remove_keyframe", err);
    }
  };

  return (
    <div class="pp__anim" data-testid="palette-animation-section">
      <div class="pp__anim-header">
        <span class="pp__anim-label">Animation</span>
        <button
          type="button"
          class="pp__action-btn"
          onClick={() => void handleSetKeyframe()}
          disabled={activePaletteId() === null || colorCount() === 0}
          data-testid="palette-anim-set-keyframe"
        >
          Set keyframe @ {activeFrameIndex()}
        </button>
      </div>

      <div class="pp__anim-entry-row">
        <label class="pp__anim-entry-label" for="pp-anim-entry">
          Entry
        </label>
        <input
          id="pp-anim-entry"
          type="number"
          class="pp__anim-entry-input"
          min={0}
          max={Math.max(0, colorCount() - 1)}
          value={entryIndex()}
          onInput={(e) => onEntryInput(e.currentTarget.value)}
          disabled={activePaletteId() === null || colorCount() === 0}
          data-testid="palette-anim-entry-input"
        />
        <span class="pp__anim-resolved">
          <span class="pp__anim-resolved-label">Resolved @ {activeFrameIndex()}</span>
          <Show when={resolved()} fallback={<span class="pp__anim-chip pp__anim-chip--empty" />}>
            {(color) => (
              <span
                class="pp__anim-chip"
                style={{ background: rgbaToCss(color()) }}
                data-testid="palette-anim-resolved-chip"
              />
            )}
          </Show>
        </span>
      </div>

      <ul class="pp__anim-list" data-testid="palette-anim-keyframes">
        <For
          each={keyframes() ?? []}
          fallback={
            <li class="pp__anim-empty" data-testid="palette-anim-empty">
              No keyframes for entry {entryIndex()}
            </li>
          }
        >
          {(kf) => (
            <li class="pp__anim-row" data-testid={`palette-anim-keyframe-${kf.frame}`}>
              <span class="pp__anim-row-frame">Frame {kf.frame}</span>
              <span class="pp__anim-chip" style={{ background: rgbaToCss(kf.color) }} />
              <button
                type="button"
                class="pp__icon-btn"
                onClick={() => void handleRemoveKeyframe(kf.frame)}
                title={`Remove keyframe at frame ${kf.frame}`}
                aria-label={`Remove keyframe at frame ${kf.frame}`}
                data-testid={`palette-anim-remove-${kf.frame}`}
              >
                &#x2715;
              </button>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
};

export default PaletteAnimationSection;
