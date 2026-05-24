// Walk-cycle frame picker (i2v).
//
// When a walk is generated via i2v the backend returns a clip; this picker
// decodes it host-side (animation_pick_frames), auto-detects loop markers at
// recurring neutral poses, and previews the picked even loop and its seam.
// "Use these" feeds the picked frames into candidate review and normalization.

import { type Component, Show, createSignal, onMount } from "solid-js";

import {
  type PickFramesResult,
  type RgbaFrame,
  animationPickFrames,
} from "../lib/commands/animation";
import { type PendingClip } from "./animation-studio-state";
import FramePreview from "./FramePreview";

type Props = {
  clip: PendingClip;
  onUse: (frames: RgbaFrame[]) => void;
  onCancel: () => void;
};

const FramePicker: Component<Props> = (props) => {
  const [result, setResult] = createSignal<PickFramesResult | null>(null);
  // Initial value read once; the picker is re-created per clip via <Show>.
  // eslint-disable-next-line solid/reactivity
  const [count, setCount] = createSignal(props.clip.targetCount);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [markerA, setMarkerA] = createSignal<number | null>(null);
  const [markerB, setMarkerB] = createSignal<number | null>(null);

  async function pick(useManualMarkers: boolean): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const a = markerA();
      const b = markerB();
      const res = await animationPickFrames({
        clip_base64: props.clip.clipBase64,
        mime: props.clip.mime,
        fps: props.clip.fps,
        target_count: count(),
        markers: useManualMarkers && a !== null && b !== null ? [a, b] : null,
      });
      setResult(res);
      setMarkerA(res.markers[0]);
      setMarkerB(res.markers[1]);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  onMount(() => {
    void pick(false);
  });

  const seamLabel = (): string => {
    const r = result();
    if (r === null) return "—";
    if (r.seam_similarity > 0.9) return "close";
    if (r.seam_similarity > 0.7) return "ok";
    return "loose";
  };

  return (
    <div class="animation-studio__picker" data-testid="frame-picker">
      <header class="animation-studio__picker-head">Pick walk loop — {props.clip.direction}</header>

      <Show when={error()}>{(msg) => <div class="animation-studio__error">{msg()}</div>}</Show>

      <Show
        when={result()}
        fallback={<div class="animation-studio__empty">{busy() ? "Decoding clip…" : "—"}</div>}
      >
        {(res) => (
          <>
            <div class="animation-studio__picker-preview">
              <FramePreview
                frames={res().frames}
                fps={props.clip.fps}
                playing={true}
                loopDirection="forward"
                fit={true}
              />
            </div>
            <div class="animation-studio__picker-meta">
              picked: {res().frames.length} of {res().total_frames} frames · seam: {seamLabel()}
            </div>
            <div class="animation-studio__picker-markers">
              <label class="animation-studio__field">
                marker A
                <input
                  type="number"
                  min={0}
                  max={res().total_frames - 1}
                  value={markerA() ?? res().markers[0]}
                  onInput={(e) => setMarkerA(Number(e.currentTarget.value))}
                />
              </label>
              <label class="animation-studio__field">
                marker B
                <input
                  type="number"
                  min={0}
                  max={res().total_frames - 1}
                  value={markerB() ?? res().markers[1]}
                  onInput={(e) => setMarkerB(Number(e.currentTarget.value))}
                />
              </label>
              <label class="animation-studio__field">
                frames
                <input
                  type="number"
                  min={2}
                  max={24}
                  value={count()}
                  onInput={(e) => setCount(Number(e.currentTarget.value))}
                />
              </label>
            </div>
            <div class="animation-studio__picker-actions">
              <button
                class="animation-studio__btn"
                disabled={busy()}
                onClick={() => void pick(false)}
              >
                Auto-detect
              </button>
              <button
                class="animation-studio__btn"
                disabled={busy()}
                onClick={() => void pick(true)}
              >
                Re-pick
              </button>
              <button class="animation-studio__btn" onClick={props.onCancel}>
                Cancel
              </button>
              <button
                class="animation-studio__btn animation-studio__btn--accept"
                disabled={busy()}
                onClick={() => props.onUse(res().frames)}
                data-testid="frame-picker-use"
              >
                Use these
              </button>
            </div>
          </>
        )}
      </Show>
    </div>
  );
};

export default FramePicker;
