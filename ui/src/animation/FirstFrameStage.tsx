// Stage 1: first frame (gpt-image-2 edit).
//
// Edits the reference sheet into a single isolated pose on magenta. The user
// rerolls/picks candidates and can refine the picked one with a mask-based
// inpaint edit before approving it for the video stage.

import { type Component, For, Show, createSignal } from "solid-js";

import {
  type AnimType,
  type DirectionOpt,
  studio,
  approveFirstFrame,
  setAnimType,
  setDirection,
  setFirstFrameCandidates,
  setSelectedFirstFrame,
  setFirstFramePrompt,
  setStage,
} from "./animation-studio-state";
import { type FirstFrameImage, animationGenerateFirstFrame } from "../lib/commands/animation";
import { extractDetail } from "../lib/utils/errors";
import MaskCanvas from "./MaskCanvas";

type Props = {
  backendReady: boolean;
  onOpenPreferences: () => void;
};

const TYPES: AnimType[] = ["idle", "walk", "attack", "custom"];
const DIRECTIONS: DirectionOpt[] = ["south", "west", "north", "east"];

const FirstFrameStage: Component<Props> = (props) => {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [inpainting, setInpainting] = createSignal(false);
  const [fixPrompt, setFixPrompt] = createSignal("");
  let exportMask: (() => string | null) | null = null;

  async function generate(base?: FirstFrameImage, mask?: string, prompt?: string): Promise<void> {
    const entityId = studio.activeAnimationStudioEntityId;
    if (entityId === null) {
      setError("No entity selected.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const res = await animationGenerateFirstFrame({
        entity_id: entityId,
        direction: studio.direction,
        animation_kind: studio.animType,
        prompt: prompt ?? (studio.firstFramePrompt.trim() || null),
        base_image_base64: base?.png_base64 ?? null,
        mask_base64: mask ?? null,
        num_images: 2,
      });
      setFirstFrameCandidates(res.images);
      setSelectedFirstFrame(res.images[0] ?? null);
      setInpainting(false);
    } catch (err) {
      setError(extractDetail(err));
    } finally {
      setBusy(false);
    }
  }

  function applyInpaint(): void {
    const base = studio.selectedFirstFrame;
    if (base === null || exportMask === null) return;
    const mask = exportMask();
    if (mask === null) {
      setError("Paint the region to edit first.");
      return;
    }
    void generate(base, mask, fixPrompt().trim() || undefined);
  }

  function approve(): void {
    const frame = studio.selectedFirstFrame;
    if (frame === null) return;
    approveFirstFrame(frame);
    setStage("video");
  }

  return (
    <div class="animation-studio__stage" data-testid="first-frame-stage">
      <div class="animation-studio__stage-controls">
        <div class="animation-studio__radio-row">
          <For each={TYPES}>
            {(t) => (
              <button
                class="animation-studio__chip"
                classList={{ "animation-studio__chip--active": studio.animType === t }}
                onClick={() => setAnimType(t)}
              >
                {t}
              </button>
            )}
          </For>
        </div>
        <div class="animation-studio__radio-row">
          <For each={DIRECTIONS}>
            {(d) => (
              <button
                class="animation-studio__chip"
                classList={{ "animation-studio__chip--active": studio.direction === d }}
                onClick={() => setDirection(d)}
              >
                {d}
              </button>
            )}
          </For>
        </div>
        <textarea
          class="animation-studio__prompt"
          rows={2}
          placeholder="Pose detail (optional) — e.g. Bit waving, antenna blinking, arms raised"
          value={studio.firstFramePrompt}
          onInput={(e) => setFirstFramePrompt(e.currentTarget.value)}
        />
        <Show
          when={props.backendReady}
          fallback={
            <button class="animation-studio__generate" onClick={() => props.onOpenPreferences()}>
              Configure AI backend…
            </button>
          }
        >
          <button
            class="animation-studio__generate"
            disabled={busy()}
            onClick={() => void generate()}
            data-testid="first-frame-generate"
          >
            {busy() ? "Generating…" : "Generate first frame"}
          </button>
        </Show>
      </div>

      <Show when={error()}>{(msg) => <div class="animation-studio__error">{msg()}</div>}</Show>

      <Show
        when={inpainting()}
        fallback={
          <FirstFrameCandidates
            selected={studio.selectedFirstFrame}
            onSelect={setSelectedFirstFrame}
          />
        }
      >
        <Show when={studio.selectedFirstFrame}>
          {(frame) => (
            <div class="animation-studio__stage">
              <MaskCanvas
                src={frame().png_base64}
                width={frame().width}
                height={frame().height}
                onReady={(fn) => {
                  exportMask = fn;
                }}
              />
              <textarea
                class="animation-studio__prompt"
                rows={2}
                placeholder="Describe the fix for the painted region — e.g. recolor the screen face cyan"
                value={fixPrompt()}
                onInput={(e) => setFixPrompt(e.currentTarget.value)}
              />
              <div class="animation-studio__stage-actions">
                <button class="animation-studio__btn" onClick={() => setInpainting(false)}>
                  Cancel
                </button>
                <button
                  class="animation-studio__btn animation-studio__btn--accept"
                  disabled={busy()}
                  onClick={applyInpaint}
                >
                  {busy() ? "Applying…" : "Apply edit"}
                </button>
              </div>
            </div>
          )}
        </Show>
      </Show>

      <Show when={studio.selectedFirstFrame && !inpainting()}>
        <div class="animation-studio__stage-actions">
          <button class="animation-studio__btn" onClick={() => setInpainting(true)}>
            Refine (inpaint)
          </button>
          <button
            class="animation-studio__generate"
            onClick={approve}
            data-testid="first-frame-approve"
          >
            Approve →
          </button>
        </div>
      </Show>
    </div>
  );
};

/** The first-frame candidate strip. */
const FirstFrameCandidates: Component<{
  selected: FirstFrameImage | null;
  onSelect: (f: FirstFrameImage) => void;
}> = (props) => (
  <Show
    when={studio.firstFrameCandidates.length > 0}
    fallback={
      <div class="animation-studio__empty">Generate to preview first-frame candidates.</div>
    }
  >
    <div class="animation-studio__candidate-strip">
      <For each={studio.firstFrameCandidates}>
        {(img) => (
          <button
            class="animation-studio__candidate-thumb"
            classList={{
              "animation-studio__candidate-thumb--active":
                props.selected?.png_base64 === img.png_base64,
            }}
            onClick={() => props.onSelect(img)}
          >
            <img src={`data:image/png;base64,${img.png_base64}`} alt="first-frame candidate" />
          </button>
        )}
      </For>
    </div>
  </Show>
);

export default FirstFrameStage;
