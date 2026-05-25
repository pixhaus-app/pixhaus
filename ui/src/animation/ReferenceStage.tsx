// Stage 0: reference.
//
// Loads and shows the entity's resolved directional reference anchor — the
// image the first frame is derived from. "Use as base" advances to the
// first-frame stage.

import { type Component, Show, createResource } from "solid-js";

import { animationGetAnchor } from "../lib/commands/animation";
import { extractDetail } from "../lib/utils/errors";
import { studio, setReferenceImage, setStage } from "./animation-studio-state";

const ReferenceStage: Component = () => {
  // Re-fetch whenever the entity or direction changes; cache the result into
  // the shared signal so the stage strip can show it too.
  const [anchor] = createResource(
    () => ({ id: studio.activeAnimationStudioEntityId, dir: studio.direction }),
    async ({ id, dir }) => {
      // Clear the shared image so a stale anchor isn't shown for the wrong
      // entity or after a failed fetch.
      setReferenceImage(null);
      if (id === null) return null;
      const img = await animationGetAnchor(id, dir);
      setReferenceImage(img);
      return img;
    },
  );

  return (
    <div class="animation-studio__stage" data-testid="reference-stage">
      <Show
        when={!anchor.error}
        fallback={<div class="animation-studio__error">{extractDetail(anchor.error)}</div>}
      >
        <Show
          when={anchor()}
          fallback={<div class="animation-studio__empty">Loading reference…</div>}
        >
          {(img) => (
            <>
              <div class="animation-studio__preview-stage">
                <img
                  class="animation-studio__fit-image"
                  src={`data:image/png;base64,${img().png_base64}`}
                  alt="reference anchor"
                />
              </div>
              <div class="animation-studio__stage-actions">
                <span class="animation-studio__anchor-chip">
                  anchor: {studio.direction} · {img().width}×{img().height}
                </span>
                <button
                  class="animation-studio__generate"
                  onClick={() => setStage("first_frame")}
                  data-testid="reference-use"
                >
                  Use as base →
                </button>
              </div>
            </>
          )}
        </Show>
      </Show>
    </div>
  );
};

export default ReferenceStage;
