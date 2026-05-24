// Stage 2: video (image-to-video).
//
// Animates the approved first frame into a raw clip via fal (Seedance default,
// Wan selectable) and plays it back unmodified so the generation can be judged
// before extraction. Generation runs as a durable backend job; this stage only
// reflects the module-level `videoJob` + `pendingClip`, so leaving and
// returning keeps the in-flight status, elapsed timer, and finished clip.

import { type Component, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";

import {
  type AnimationJob,
  animationCancelClipJob,
  animationGenerateClip,
  animationJobClip,
  animationJobGet,
  animationJobsList,
} from "../lib/commands/animation";
import { extractDetail } from "../lib/utils/errors";
import {
  type AnimType,
  type DirectionOpt,
  type VideoModel,
  activeAnimationStudioEntityId,
  animType,
  approvedFirstFrame,
  choreography,
  direction,
  fps,
  frameCount,
  pendingClip,
  seed,
  setChoreography,
  setPendingClip,
  setStage,
  setVideoJob,
  setVideoModel,
  videoJob,
  videoModel,
} from "./animation-studio-state";

type Props = {
  backendReady: boolean;
  onOpenPreferences: () => void;
};

const VideoStage: Component<Props> = (props) => {
  const [error, setError] = createSignal<string | null>(null);
  const [clipUrl, setClipUrl] = createSignal<string | null>(null);
  const [nowMs, setNowMs] = createSignal(Date.now());

  const generating = (): boolean => videoJob()?.status === "running";

  // Tick a clock while a job runs so the elapsed time advances; elapsed is
  // derived from the job's start so it stays correct across stage navigation.
  createEffect(() => {
    if (!generating()) return;
    const timer = setInterval(() => setNowMs(Date.now()), 1000);
    onCleanup(() => clearInterval(timer));
  });
  const elapsedS = (): number => {
    const job = videoJob();
    if (job === null) return 0;
    return Math.max(0, Math.round((nowMs() - job.created_at_ms) / 1000));
  };

  // Fetches a finished job's clip into `pendingClip` (once).
  async function fetchClipFor(job: AnimationJob): Promise<void> {
    if (pendingClip() !== null) return;
    try {
      const clip = await animationJobClip(job.id);
      setPendingClip({
        clipBase64: clip.clip_base64,
        mime: clip.mime,
        fps: clip.fps,
        targetCount: clip.target_count,
        direction: job.direction as DirectionOpt,
        animType: (job.animation_kind ?? "custom") as AnimType,
      });
    } catch {
      // Clip not ready or unreadable; a later poll retries.
    }
  }

  // Poll the backend job while it runs — authoritative for completion and
  // cancellation, so the UI reflects them even if the live event is missed.
  createEffect(() => {
    const job = videoJob();
    if (job === null || job.status !== "running") return;
    const jobId = job.id;
    const timer = setInterval(() => {
      void animationJobGet(jobId)
        .then((updated) => {
          if (updated === null) return;
          setVideoJob(updated);
          if (updated.status === "done") void fetchClipFor(updated);
        })
        .catch(() => undefined);
    }, 2000);
    onCleanup(() => clearInterval(timer));
  });

  // Materialize the base64 clip into a blob URL for the <video>, revoking the
  // previous one when the clip changes or the stage unmounts.
  createEffect(() => {
    const c = pendingClip();
    if (c === null) {
      setClipUrl(null);
      return;
    }
    const bytes = Uint8Array.from(atob(c.clipBase64), (ch) => ch.charCodeAt(0));
    const url = URL.createObjectURL(new Blob([bytes], { type: c.mime }));
    setClipUrl(url);
    onCleanup(() => URL.revokeObjectURL(url));
  });

  // On (re)mount, adopt the entity's latest job if we aren't already tracking
  // one — covers reopening the studio (or app restart) with a finished or
  // interrupted job on disk.
  onMount(() => {
    const entityId = activeAnimationStudioEntityId();
    if (entityId === null || videoJob() !== null) return;
    void animationJobsList(entityId)
      .then((jobs) => {
        const latest = jobs[0];
        if (latest === undefined) return;
        setVideoJob(latest);
        if (latest.status === "done") void fetchClipFor(latest);
      })
      .catch(() => undefined);
  });

  async function generate(): Promise<void> {
    const frame = approvedFirstFrame();
    const entityId = activeAnimationStudioEntityId();
    if (frame === null || entityId === null) {
      setError("Approve a first frame first.");
      return;
    }
    setError(null);
    setPendingClip(null);
    try {
      const jobId = await animationGenerateClip({
        entity_id: entityId,
        first_frame_base64: frame.png_base64,
        direction: direction(),
        animation_kind: animType() === "custom" ? null : animType(),
        choreography: choreography().trim() || null,
        model: videoModel(),
        num_frames: frameCount(),
        fps: fps(),
        seed: seed(),
      });
      // Optimistic running state; the AnimationJobUpdate listener refines it.
      setVideoJob({
        id: jobId,
        entity_id: entityId,
        direction: direction(),
        animation_kind: animType(),
        model: videoModel(),
        status: "running",
        created_at_ms: Date.now(),
        target_count: frameCount(),
        fps: fps(),
        mime: "video/mp4",
        error: null,
      });
    } catch (err) {
      setError(extractDetail(err));
    }
  }

  function cancel(): void {
    const job = videoJob();
    if (job !== null) void animationCancelClipJob(job.id).catch(() => undefined);
  }

  const jobError = (): string | null => {
    const job = videoJob();
    if (job?.status === "error") return job.error ?? "generation failed";
    if (job?.status === "interrupted") return "this generation was interrupted — generate again";
    if (job?.status === "cancelled") return "generation cancelled";
    return null;
  };

  return (
    <div class="animation-studio__stage" data-testid="video-stage">
      <div class="animation-studio__stage-controls">
        <label class="animation-studio__field">
          model
          <select
            value={videoModel()}
            onChange={(e) => setVideoModel(e.currentTarget.value as VideoModel)}
            data-testid="video-model"
          >
            <option value="seedance">Seedance 2.0</option>
            <option value="wan">Wan 2.1</option>
          </select>
        </label>
        <textarea
          class="animation-studio__prompt"
          rows={2}
          placeholder="Motion detail (optional) — e.g. screen flickers, antenna light pulses"
          value={choreography()}
          onInput={(e) => setChoreography(e.currentTarget.value)}
        />
        <Show
          when={props.backendReady}
          fallback={
            <button class="animation-studio__generate" onClick={() => props.onOpenPreferences()}>
              Configure AI backend…
            </button>
          }
        >
          <div class="animation-studio__stage-actions">
            <button
              class="animation-studio__generate"
              disabled={generating()}
              onClick={() => void generate()}
              data-testid="video-generate"
            >
              {generating() ? `Generating… ${elapsedS()}s` : "Generate video"}
            </button>
            <Show when={generating()}>
              <button class="animation-studio__btn" onClick={cancel}>
                Cancel
              </button>
            </Show>
          </div>
        </Show>
      </div>

      <Show when={error() ?? jobError()}>
        {(msg) => <div class="animation-studio__error">{msg()}</div>}
      </Show>

      <div class="animation-studio__preview-stage">
        <Show
          when={clipUrl()}
          fallback={
            <div class="animation-studio__empty">
              {generating() ? "Animating the first frame…" : "Generate to preview the raw video."}
            </div>
          }
        >
          {(url) => (
            <video class="animation-studio__fit-image" src={url()} autoplay loop muted controls />
          )}
        </Show>
      </div>

      <Show when={pendingClip()}>
        <div class="animation-studio__stage-actions">
          <button
            class="animation-studio__generate"
            onClick={() => setStage("extract")}
            data-testid="video-extract"
          >
            Extract frames →
          </button>
        </div>
      </Show>
    </div>
  );
};

export default VideoStage;
