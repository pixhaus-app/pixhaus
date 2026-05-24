// Animation studio: full-screen, staged AI animation surface.
//
// Four inspectable stages — reference → first frame (gpt-image-2 edit) → raw
// video (fal i2v) → extracted frames (pick → normalize → integrate). Each
// stage produces a reviewable artifact; the stage strip shows them throughout.
// See `docs/planning/work/animation-staged-pipeline.md`.

import {
  type Component,
  Match,
  Show,
  Switch,
  createMemo,
  createResource,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { listen } from "@tauri-apps/api/event";

import { aiGetProviderOverview } from "../lib/commands/ai";
import {
  type AnimationJob,
  type IntegrateArgs,
  type RgbaFrame,
  animationDeriveNeutral,
  animationIntegrate,
  animationJobClip,
  animationNormalize,
  animationRerollDependents,
} from "../lib/commands/animation";
import { activeSpriteId } from "../canvas/canvas-state";
import { entities } from "../library/library-state";
import { pushToast } from "../lib/toast/toast-state";
import { extractDetail } from "../lib/utils/errors";
import { openPreferences } from "../preferences/preferences-state";
import AnimationSet from "./AnimationSet";
import CandidateReview from "./CandidateReview";
import FirstFrameStage from "./FirstFrameStage";
import FramePicker from "./FramePicker";
import FramePreview from "./FramePreview";
import NormalizationReview from "./NormalizationReview";
import ReferenceStage from "./ReferenceStage";
import StageStrip from "./StageStrip";
import VideoStage from "./VideoStage";
import {
  type AnimType,
  type Candidate,
  type DirectionOpt,
  activeAnimationStudioEntityId,
  animType,
  closeAnimationStudio,
  direction,
  fps,
  frameSize,
  loopDirectionFor,
  nextStudioId,
  pendingClip,
  previewPlaying,
  selectedCandidate,
  setCandidates,
  setPendingClip,
  setPreviewPlaying,
  setSelectedCandidateId,
  setStage,
  setVideoJob,
  stage,
  videoJob,
} from "./animation-studio-state";

const AnimationStudio: Component = () => {
  const [accepted, setAccepted] = createSignal<Candidate | null>(null);
  // Set after a successful integrate so the extract stage shows a completion
  // panel instead of silently re-arming the picker.
  const [lastIntegrated, setLastIntegrated] = createSignal<string | null>(null);

  // Reflect backend i2v job updates into the module-level `videoJob` for the
  // whole studio session, so navigating between stages never loses an in-flight
  // or finished generation. On completion, fetch the clip for the picker.
  onMount(() => {
    const unlisten = listen<AnimationJob>("AnimationJobUpdate", (event) => {
      const job = event.payload;
      if (job.entity_id !== activeAnimationStudioEntityId()) return;
      const current = videoJob();
      if (current !== null && current.id !== job.id) return;
      setVideoJob(job);
      if (job.status === "done" && pendingClip() === null) {
        void animationJobClip(job.id)
          .then((clip) =>
            setPendingClip({
              clipBase64: clip.clip_base64,
              mime: clip.mime,
              fps: clip.fps,
              targetCount: clip.target_count,
              direction: job.direction as DirectionOpt,
              animType: (job.animation_kind ?? "custom") as AnimType,
            }),
          )
          .catch(() => undefined);
      }
    });
    onCleanup(() => {
      void unlisten.then((un) => un()).catch(() => undefined);
    });
  });

  const [providers] = createResource(aiGetProviderOverview);
  // Require a backend that is registered AND available in the runtime — not
  // merely a stored key. `state === "configured"` is `registered && available`.
  const backendReady = (): boolean => (providers() ?? []).some((p) => p.state === "configured");

  const entityName = createMemo((): string => {
    const id = activeAnimationStudioEntityId();
    const e = entities().find((x) => x.id === id);
    return e?.name ?? "Animations";
  });

  // ── extract stage: pick → normalize → candidate → integrate ──────────────────

  function addCandidate(frames: RgbaFrame[], report: Candidate["report"]): void {
    const candidate: Candidate = {
      id: nextStudioId(),
      frames,
      direction: direction(),
      animType: animType(),
      report,
      flip: direction() === "east",
    };
    setCandidates((cs) => [...cs, candidate]);
    setSelectedCandidateId(candidate.id);
  }

  /** Clears candidates so the frame picker reappears for a fresh pick. */
  function repick(): void {
    setAccepted(null);
    setCandidates([]);
    setSelectedCandidateId(null);
    setLastIntegrated(null);
  }

  async function usePickedFrames(frames: RgbaFrame[]): Promise<void> {
    try {
      const normalized = await animationNormalize({
        frames,
        canvas_size: frameSize(),
        // The first frame is rendered on magenta, so the clip frames carry it.
        chroma: "magenta",
        reference_height: null,
      });
      addCandidate(normalized.frames, normalized.report);
    } catch (err) {
      pushToast({ kind: "error", title: "Normalize failed", body: extractDetail(err) });
    }
  }

  async function integrate(candidate: Candidate): Promise<void> {
    const sprite = activeSpriteId();
    if (sprite === null) {
      pushToast({ kind: "error", title: "No active sprite." });
      return;
    }
    const args: IntegrateArgs = {
      sprite_id: sprite,
      name: `${candidate.animType}-${candidate.direction}`,
      frames: candidate.frames,
      loop_direction: loopDirectionFor(candidate.animType),
      repeat: 0,
      frame_duration_ms: Math.max(1, Math.round(1000 / fps())),
      flip_horizontal: candidate.flip,
      // Record the cascade edge so re-rolling the anchor marks this stale.
      entity_id: activeAnimationStudioEntityId(),
      animation_kind: candidate.animType === "custom" ? null : candidate.animType,
      direction: candidate.direction,
    };
    try {
      await animationIntegrate(args);
      pushToast({ kind: "success", title: `Landed ${args.name}` });
      setAccepted(null);
      setCandidates((cs) => cs.filter((c) => c.id !== candidate.id));
      setLastIntegrated(args.name);
    } catch (err) {
      pushToast({ kind: "error", title: "Integrate failed", body: extractDetail(err) });
    }
  }

  // ── neutral anchor reset ───────────────────────────────────────────────────

  const [derivingNeutral, setDerivingNeutral] = createSignal(false);

  async function deriveNeutral(): Promise<void> {
    const entityId = activeAnimationStudioEntityId();
    if (entityId === null) {
      pushToast({ kind: "error", title: "No entity selected." });
      return;
    }
    setDerivingNeutral(true);
    try {
      await animationDeriveNeutral(entityId);
      pushToast({
        kind: "success",
        title: "Neutral anchor ready",
        body: "Animations now derive from the effect-stripped neutral pose.",
      });
    } catch (err) {
      pushToast({
        kind: "error",
        title: "Strip baked-in effects failed",
        body: extractDetail(err),
      });
    } finally {
      setDerivingNeutral(false);
    }
  }

  async function rerollDependents(entityId: number): Promise<void> {
    try {
      const stale = await animationRerollDependents(entityId);
      if (stale.length === 0) {
        pushToast({ kind: "info", title: "No stale animations to re-roll." });
        return;
      }
      const list = stale.map((c) => `${c.animation_type}/${c.direction}`).join(", ");
      pushToast({ kind: "info", title: `${stale.length} stale: regenerate ${list}` });
    } catch (err) {
      pushToast({ kind: "error", title: "Re-roll check failed", body: extractDetail(err) });
    }
  }

  const current = (): Candidate | null => accepted() ?? selectedCandidate();

  return (
    <div class="animation-studio" data-testid="animation-studio">
      <header class="animation-studio__header">
        <button class="animation-studio__back" onClick={closeAnimationStudio}>
          ‹ Back
        </button>
        <span class="animation-studio__title">{entityName()} — Animations</span>
        <button
          class="animation-studio__btn animation-studio__btn--small"
          disabled={derivingNeutral() || !backendReady()}
          title="Strip baked-in effects from the canonical to make a neutral anchor"
          onClick={() => void deriveNeutral()}
          data-testid="derive-neutral"
        >
          {derivingNeutral() ? "Stripping…" : "Strip baked-in effects"}
        </button>
        <Show when={!backendReady()}>
          <span class="animation-studio__anchor-chip">no backend</span>
        </Show>
      </header>

      <StageStrip />

      <div class="animation-studio__center">
        <Switch>
          <Match when={stage() === "reference"}>
            <ReferenceStage />
          </Match>
          <Match when={stage() === "first_frame"}>
            <FirstFrameStage backendReady={backendReady()} onOpenPreferences={openPreferences} />
          </Match>
          <Match when={stage() === "video"}>
            <VideoStage backendReady={backendReady()} onOpenPreferences={openPreferences} />
          </Match>
          <Match when={stage() === "extract"}>
            <div class="animation-studio__stage" data-testid="extract-stage">
              <Show when={lastIntegrated()}>
                {(name) => (
                  <div class="animation-studio__success" data-testid="integrate-success">
                    <div class="animation-studio__success-msg">
                      Landed <strong>{name()}</strong> ✓ — it's on the sprite's timeline.
                    </div>
                    <div class="animation-studio__stage-actions">
                      <button
                        class="animation-studio__btn"
                        onClick={() => {
                          repick();
                          setStage("first_frame");
                        }}
                      >
                        Animate another
                      </button>
                      <button class="animation-studio__generate" onClick={closeAnimationStudio}>
                        Close
                      </button>
                    </div>
                  </div>
                )}
              </Show>

              <Show when={!lastIntegrated()}>
                <Show
                  when={pendingClip()}
                  fallback={<div class="animation-studio__empty">Generate a video first.</div>}
                >
                  {(clip) => (
                    <Show
                      when={current()}
                      fallback={
                        <FramePicker
                          clip={clip()}
                          onUse={(frames) => void usePickedFrames(frames)}
                          onCancel={() => setStage("video")}
                        />
                      }
                    >
                      {(candidate) => (
                        <>
                          <div class="animation-studio__preview-stage">
                            <FramePreview
                              frames={candidate().frames}
                              fps={fps()}
                              playing={previewPlaying()}
                              loopDirection={loopDirectionFor(candidate().animType)}
                              baselineFraction={0.95}
                              scale={5}
                            />
                          </div>
                          <div class="animation-studio__transport">
                            <button
                              class="animation-studio__btn"
                              onClick={() => setPreviewPlaying((p) => !p)}
                            >
                              {previewPlaying() ? "Pause" : "Play"}
                            </button>
                            <button class="animation-studio__btn" onClick={repick}>
                              Re-pick
                            </button>
                          </div>
                        </>
                      )}
                    </Show>
                  )}
                </Show>

                <CandidateReview
                  fps={fps()}
                  onAccept={(c) => setAccepted(c)}
                  onReject={(c) => {
                    setCandidates((cs) => cs.filter((x) => x.id !== c.id));
                    if (selectedCandidate()?.id === c.id) setSelectedCandidateId(null);
                  }}
                  onReroll={repick}
                />

                <Show when={accepted()}>
                  {(candidate) => (
                    <NormalizationReview
                      frames={candidate().frames}
                      report={candidate().report}
                      loopDirection={loopDirectionFor(candidate().animType)}
                      fps={fps()}
                      onReNormalize={repick}
                      onIntegrate={() => void integrate(candidate())}
                    />
                  )}
                </Show>
              </Show>
            </div>
          </Match>
        </Switch>
      </div>

      <footer class="animation-studio__footer">
        <Show when={activeAnimationStudioEntityId()}>
          {(entityId) => (
            <AnimationSet
              entityId={entityId()}
              onRerollDependents={() => void rerollDependents(entityId())}
            />
          )}
        </Show>
      </footer>
    </div>
  );
};

export default AnimationStudio;
