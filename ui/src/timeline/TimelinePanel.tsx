// Timeline panel — horizontal frame strip with a layer row per layer.
//
// Layout: a 2-column x 2-row CSS grid.
//   top-left  corner  | top-right  frame-head (tag bars + frame numbers)
//   bot-left  layer-col| bot-right cel-grid (primary scroll container)
//
// The cel-grid is the source of scroll truth; frame-head and layer-col are
// kept in sync via DOM ref writes inside reactive effects. 2D virtual scroll
// ensures 200-frame x 50-layer performance at 60 fps.

import {
  type Component,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
} from "solid-js";
import type { Frame } from "../lib/types";
import {
  activeFrameIndex,
  scheduleViewportSync,
  setActiveFrameIndex,
} from "../canvas/canvas-state";
import { activeSpriteId } from "../canvas/canvas-state";
import { flattenLayers, isGroupExpanded, layers, refreshLayers } from "../layers/layer-state";
import {
  addFrame,
  celPresence,
  deleteFrames,
  extendSelectionTo,
  frames,
  isLooping,
  isPlaying,
  isTimelinePanelVisible,
  onionSkin,
  onionSkinNext,
  onionSkinPrev,
  refreshTimeline,
  selectFrame,
  selectedFrames,
  setFrameDuration,
  setIsLooping,
  setOnionSkin,
  setOnionSkinNext,
  setOnionSkinPrev,
  setTimelinePanelVisible,
  stopPlayback,
  togglePlayback,
} from "./timeline-state";
import FrameTagBar, { FRAME_WIDTH } from "./FrameTagBar";
import TimelineContextMenu, { type ContextMenuTarget } from "./TimelineContextMenu";

// ── Layout constants ─────────────────────────────────────────────────────────

const ROW_HEIGHT = 24;
const LAYER_COL_WIDTH = 140;
const TAG_BAR_HEIGHT = 20;
const FRAME_HEADER_HEIGHT = 36;
const OVERSCAN = 3;

// ── Component ────────────────────────────────────────────────────────────────

const TimelinePanel: Component = () => {
  const spriteId = activeSpriteId;
  const activeFrame = activeFrameIndex;

  createEffect(() => {
    spriteId();
    refreshTimeline();
    refreshLayers();
  });

  createEffect(() => {
    if (!isTimelinePanelVisible()) stopPlayback();
  });

  // ── Scroll state ──────────────────────────────────────────────────────────

  const [scrollLeft, setScrollLeft] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  const [gridWidth, setGridWidth] = createSignal(400);
  const [gridHeight, setGridHeight] = createSignal(100);
  const [tagBarLeft, setTagBarLeft] = createSignal(0);

  let frameHeadRef: HTMLDivElement | undefined;
  let layerColRef: HTMLDivElement | undefined;

  createEffect(() => {
    const sl = scrollLeft();
    if (frameHeadRef) frameHeadRef.scrollLeft = sl;
  });

  createEffect(() => {
    const st = scrollTop();
    if (layerColRef) layerColRef.scrollTop = st;
  });

  const gridResizeObserver = new ResizeObserver((entries) => {
    const entry = entries[0];
    if (entry) {
      setGridWidth(entry.contentRect.width);
      setGridHeight(entry.contentRect.height);
    }
  });

  const celGridCallbackRef = (el: HTMLDivElement): void => {
    gridResizeObserver.observe(el);
  };

  onCleanup(() => gridResizeObserver.disconnect());

  function handleCelGridScroll(e: Event): void {
    const el = e.currentTarget as HTMLDivElement;
    setScrollLeft(el.scrollLeft);
    setScrollTop(el.scrollTop);
  }

  // ── Layer rows ────────────────────────────────────────────────────────────

  const flatEntries = createMemo(() => flattenLayers(layers(), isGroupExpanded));

  // ── Virtual scroll ────────────────────────────────────────────────────────

  const totalWidth = createMemo(() => frames().length * FRAME_WIDTH);
  const totalHeight = createMemo(() => flatEntries().length * ROW_HEIGHT);

  const visibleFrameRange = createMemo(() => {
    const sl = scrollLeft();
    const gw = gridWidth();
    const n = frames().length;
    return {
      start: Math.max(0, Math.floor(sl / FRAME_WIDTH) - OVERSCAN),
      end: Math.min(n, Math.ceil((sl + gw) / FRAME_WIDTH) + OVERSCAN),
    };
  });

  const visibleRowRange = createMemo(() => {
    const st = scrollTop();
    const gh = gridHeight();
    const n = flatEntries().length;
    return {
      start: Math.max(0, Math.floor(st / ROW_HEIGHT) - OVERSCAN),
      end: Math.min(n, Math.ceil((st + gh) / ROW_HEIGHT) + OVERSCAN),
    };
  });

  const visibleFrames = createMemo((): Array<{ frame: Frame; index: number }> => {
    const { start, end } = visibleFrameRange();
    const all = frames();
    const result: Array<{ frame: Frame; index: number }> = [];
    for (let i = start; i < end; i++) {
      const f = all[i];
      if (f !== undefined) result.push({ frame: f, index: i });
    }
    return result;
  });

  const visibleRows = createMemo(() => {
    const { start, end } = visibleRowRange();
    return flatEntries()
      .slice(start, end)
      .map((entry, i) => ({ entry, rowIndex: start + i }));
  });

  // ── Scrub head ────────────────────────────────────────────────────────────

  const scrubLeft = createMemo(() => activeFrame() * FRAME_WIDTH + FRAME_WIDTH / 2);

  // ── Duration editing ──────────────────────────────────────────────────────

  const [editingFrame, setEditingFrame] = createSignal<number | null>(null);
  const [durationInput, setDurationInput] = createSignal("");

  function beginEditDuration(index: number, currentMs: number): void {
    setEditingFrame(index);
    setDurationInput(String(currentMs));
  }

  function commitEditDuration(index: number): void {
    const id = spriteId();
    const ms = parseInt(durationInput(), 10);
    if (id !== null && !isNaN(ms) && ms >= 1) setFrameDuration(id, index, ms);
    setEditingFrame(null);
  }

  // ── Context menu ──────────────────────────────────────────────────────────

  const [contextTarget, setContextTarget] = createSignal<ContextMenuTarget | null>(null);

  function handleCelContextMenu(e: MouseEvent, frameIndex: number): void {
    e.preventDefault();
    setContextTarget({ x: e.clientX, y: e.clientY, frameIndex });
  }

  // ── Frame click ───────────────────────────────────────────────────────────

  function handleFrameClick(e: MouseEvent, index: number): void {
    if (e.shiftKey) {
      extendSelectionTo(index);
    } else {
      selectFrame(index, e.ctrlKey || e.metaKey);
    }
  }

  // ── Keyboard ──────────────────────────────────────────────────────────────

  function handleKeyDown(e: KeyboardEvent): void {
    // Bail out when focus is on a text editor (duration input, future
    // tag-rename inputs, etc.). Without this, Backspace inside the
    // duration field bubbles up and silently deletes the selected frame.
    const ae = document.activeElement;
    if (
      ae instanceof HTMLInputElement ||
      ae instanceof HTMLTextAreaElement ||
      (ae instanceof HTMLElement && ae.isContentEditable)
    ) {
      return;
    }
    const id = spriteId();
    if (id === null) return;
    const n = frames().length;
    if (e.code === "Space") {
      e.preventDefault();
      togglePlayback();
    } else if (e.code === "ArrowRight") {
      e.preventDefault();
      selectFrame(Math.min(n - 1, activeFrame() + 1), false);
    } else if (e.code === "ArrowLeft") {
      e.preventDefault();
      selectFrame(Math.max(0, activeFrame() - 1), false);
    } else if (e.code === "Delete" || e.code === "Backspace") {
      e.preventDefault();
      const sel = selectedFrames();
      if (sel.size > 0) deleteFrames(id, sel);
    }
  }

  return (
    <div class="timeline-panel" tabIndex={0} onKeyDown={handleKeyDown}>
      {/* ── Header ──────────────────────────────────────────────────────── */}
      <div class="timeline-panel__header">
        <span class="timeline-panel__title">Timeline</span>

        <div class="timeline-panel__playback">
          <button
            class="timeline-panel__pb-btn"
            classList={{ "timeline-panel__pb-btn--active": isPlaying() }}
            onClick={togglePlayback}
            title={isPlaying() ? "Pause (Space)" : "Play (Space)"}
          >
            {isPlaying() ? "Pause" : "Play"}
          </button>
          <button
            class="timeline-panel__pb-btn"
            onClick={() => {
              stopPlayback();
              setActiveFrameIndex(0);
              scheduleViewportSync();
            }}
            title="Stop"
          >
            Stop
          </button>
          <button
            class="timeline-panel__pb-btn"
            classList={{ "timeline-panel__pb-btn--active": isLooping() }}
            onClick={() => setIsLooping(!isLooping())}
            title="Toggle loop"
          >
            Loop
          </button>
        </div>

        <div class="timeline-panel__onion">
          <button
            class="timeline-panel__pb-btn"
            classList={{ "timeline-panel__pb-btn--active": onionSkin() }}
            onClick={() => setOnionSkin(!onionSkin())}
            title="Toggle onion skin"
          >
            Onion
          </button>
          <Show when={onionSkin()}>
            <label class="timeline-panel__onion-label">
              Prev
              <input
                class="timeline-panel__onion-input"
                type="number"
                min={0}
                max={8}
                value={onionSkinPrev()}
                onInput={(e) => {
                  const v = parseInt(e.currentTarget.value, 10);
                  if (!isNaN(v)) setOnionSkinPrev(v);
                }}
              />
            </label>
            <label class="timeline-panel__onion-label">
              Next
              <input
                class="timeline-panel__onion-input"
                type="number"
                min={0}
                max={8}
                value={onionSkinNext()}
                onInput={(e) => {
                  const v = parseInt(e.currentTarget.value, 10);
                  if (!isNaN(v)) setOnionSkinNext(v);
                }}
              />
            </label>
          </Show>
        </div>

        <div class="timeline-panel__header-actions">
          <button
            class="timeline-panel__icon-btn"
            onClick={() => {
              const id = spriteId();
              if (id !== null) addFrame(id);
            }}
            disabled={spriteId() === null}
            title="Add frame"
          >
            +
          </button>
          <button
            class="timeline-panel__icon-btn"
            onClick={() => setTimelinePanelVisible(false)}
            title="Close timeline"
          >
            x
          </button>
        </div>
      </div>

      {/* ── Main grid ─────────────────────────────────────────────────────── */}
      <Show
        when={spriteId() !== null}
        fallback={<div class="timeline-panel__empty">Open a project to see the timeline.</div>}
      >
        <div
          class="timeline-panel__grid"
          style={{
            "--tl-layer-col-w": `${LAYER_COL_WIDTH}px`,
            "--tl-tag-bar-h": `${TAG_BAR_HEIGHT}px`,
            "--tl-frame-head-h": `${FRAME_HEADER_HEIGHT}px`,
          }}
        >
          {/* Corner (top-left) */}
          <div class="timeline-panel__corner" />

          {/* Frame head (top-right): tag bar + frame number row */}
          <div
            class="timeline-panel__frame-head"
            ref={(el) => {
              frameHeadRef = el;
            }}
          >
            <div
              class="timeline-panel__tag-bar-container"
              ref={(el) => {
                // Capture client left after mount for tag drag coordinate conversion.
                requestAnimationFrame(() => {
                  setTagBarLeft(el.getBoundingClientRect().left);
                });
              }}
              style={{ width: `${totalWidth()}px` }}
            >
              <FrameTagBar
                spriteId={spriteId()}
                frameCount={frames().length}
                scrollLeft={scrollLeft()}
                containerLeft={tagBarLeft()}
              />
            </div>

            <div
              class="timeline-panel__frame-nums"
              style={{ width: `${totalWidth()}px`, position: "relative" }}
            >
              <div class="timeline-panel__scrub-head" style={{ left: `${scrubLeft()}px` }} />
              <For each={visibleFrames()}>
                {({ frame, index }) => (
                  <div
                    class="timeline-panel__frame-col"
                    classList={{
                      "timeline-panel__frame-col--active": index === activeFrame(),
                      "timeline-panel__frame-col--selected": selectedFrames().has(index),
                    }}
                    style={{ left: `${index * FRAME_WIDTH}px` }}
                    onClick={(e) => handleFrameClick(e, index)}
                    onContextMenu={(e) => handleCelContextMenu(e, index)}
                  >
                    <div class="timeline-panel__frame-num">{index + 1}</div>
                    <Show
                      when={editingFrame() === index}
                      fallback={
                        <div
                          class="timeline-panel__frame-dur"
                          onDblClick={() => beginEditDuration(index, frame.duration_ms)}
                        >
                          {frame.duration_ms}
                        </div>
                      }
                    >
                      <input
                        class="timeline-panel__dur-input"
                        type="number"
                        min={1}
                        value={durationInput()}
                        onInput={(e) => setDurationInput(e.currentTarget.value)}
                        onBlur={() => commitEditDuration(index)}
                        onKeyDown={(e) => {
                          // Stop bubbling so the panel-level handler doesn't
                          // see Backspace/Delete and delete the frame from
                          // under the input. Defence in depth alongside the
                          // activeElement check in handleKeyDown.
                          e.stopPropagation();
                          if (e.key === "Enter") commitEditDuration(index);
                          else if (e.key === "Escape") setEditingFrame(null);
                        }}
                        autofocus
                      />
                    </Show>
                  </div>
                )}
              </For>
            </div>
          </div>

          {/* Layer column (bottom-left) */}
          <div
            class="timeline-panel__layer-col"
            ref={(el) => {
              layerColRef = el;
            }}
          >
            <div style={{ height: `${totalHeight()}px`, position: "relative" }}>
              <For each={visibleRows()}>
                {({ entry, rowIndex }) => (
                  <div
                    class="timeline-panel__layer-name"
                    classList={{
                      "timeline-panel__layer-name--group": entry.layer.kind.kind === "group",
                    }}
                    style={{
                      position: "absolute",
                      top: `${rowIndex * ROW_HEIGHT}px`,
                      "padding-left": `${8 + entry.depth * 12}px`,
                    }}
                    title={entry.layer.name}
                  >
                    {entry.layer.name}
                  </div>
                )}
              </For>
            </div>
          </div>

          {/* Cel grid (bottom-right) — primary scroll container */}
          <div
            class="timeline-panel__cel-grid"
            ref={celGridCallbackRef}
            onScroll={handleCelGridScroll}
          >
            <div
              style={{
                width: `${totalWidth()}px`,
                height: `${totalHeight()}px`,
                position: "relative",
              }}
            >
              <div class="timeline-panel__scrub-body" style={{ left: `${scrubLeft()}px` }} />
              <For each={visibleRows()}>
                {({ entry, rowIndex }) => (
                  <For each={visibleFrames()}>
                    {({ index: frameIdx }) => (
                      <div
                        class="timeline-panel__cel"
                        classList={{
                          "timeline-panel__cel--present":
                            celPresence().get(entry.layer.id)?.has(frameIdx) === true,
                          "timeline-panel__cel--active": frameIdx === activeFrame(),
                          "timeline-panel__cel--selected": selectedFrames().has(frameIdx),
                        }}
                        style={{
                          position: "absolute",
                          top: `${rowIndex * ROW_HEIGHT}px`,
                          left: `${frameIdx * FRAME_WIDTH}px`,
                          width: `${FRAME_WIDTH}px`,
                          height: `${ROW_HEIGHT}px`,
                        }}
                        onClick={(e) => handleFrameClick(e, frameIdx)}
                        onContextMenu={(e) => handleCelContextMenu(e, frameIdx)}
                      />
                    )}
                  </For>
                )}
              </For>
            </div>
          </div>
        </div>
      </Show>

      <TimelineContextMenu
        target={contextTarget()}
        spriteId={spriteId() ?? 0}
        onClose={() => setContextTarget(null)}
      />
    </div>
  );
};

export default TimelinePanel;
