// Persistent pipeline stage strip.
//
// Shows the reference, first-frame, and video artifacts as thumbnails across
// the top of the studio at all times, with the active stage highlighted.
// Clicking a stage navigates to it (only stages whose prerequisites are met).

import { type Component, For, Show } from "solid-js";

import { type Stage, studio, setStage } from "./animation-studio-state";

type StageDef = {
  key: Stage;
  label: string;
  /** Data-URI thumbnail for the stage's artifact, or null when not produced. */
  thumb: () => string | null;
  /** Whether the stage can be opened yet. */
  ready: () => boolean;
};

const pngUri = (b64: string): string => `data:image/png;base64,${b64}`;

const StageStrip: Component = () => {
  const stages = (): StageDef[] => [
    {
      key: "reference",
      label: "Reference",
      thumb: () => {
        const r = studio.referenceImage;
        return r ? pngUri(r.png_base64) : null;
      },
      ready: () => true,
    },
    {
      key: "first_frame",
      label: "First frame",
      thumb: () => {
        const f = studio.approvedFirstFrame;
        return f ? pngUri(f.png_base64) : null;
      },
      ready: () => studio.referenceImage !== null,
    },
    {
      key: "video",
      label: "Video",
      // The clip is a video, not an image; show the approved first frame as a
      // stand-in poster (it is the clip's first frame).
      thumb: () => {
        const f = studio.approvedFirstFrame;
        return studio.pendingClip && f ? pngUri(f.png_base64) : null;
      },
      ready: () => studio.approvedFirstFrame !== null,
    },
    {
      key: "extract",
      label: "Frames",
      thumb: () => null,
      ready: () => studio.pendingClip !== null,
    },
  ];

  return (
    <div class="animation-studio__stage-strip" data-testid="stage-strip">
      <For each={stages()}>
        {(s, i) => (
          <>
            <Show when={i() > 0}>
              <span class="animation-studio__stage-arrow">›</span>
            </Show>
            <button
              class="animation-studio__stage-chip"
              classList={{
                "animation-studio__stage-chip--active": studio.stage === s.key,
                "animation-studio__stage-chip--locked": !s.ready(),
              }}
              disabled={!s.ready()}
              onClick={() => setStage(s.key)}
              data-testid={`stage-${s.key}`}
            >
              <span class="animation-studio__stage-thumb">
                <Show
                  when={s.thumb()}
                  fallback={<span class="animation-studio__stage-num">{i() + 1}</span>}
                >
                  {(src) => <img src={src()} alt={s.label} />}
                </Show>
              </span>
              <span class="animation-studio__stage-label">{s.label}</span>
            </button>
          </>
        )}
      </For>
    </div>
  );
};

export default StageStrip;
