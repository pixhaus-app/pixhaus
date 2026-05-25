// Candidate review: animated candidate strip + accept / reject / re-roll.
//
// Reuses the HistoryStrip shape (a thumbnail row) but each thumbnail is an
// animated loop. Per-candidate quality flags (drift / scale / seam) read from
// the normalization measurements; accept is allowed even with warnings, but
// warnings are explicit.

import { type Component, For, Show } from "solid-js";

import type { NormalizeReport } from "../lib/types/NormalizeReport";
import {
  type Candidate,
  studio,
  loopDirectionFor,
  selectedCandidate,
  setSelectedCandidateId,
} from "./animation-studio-state";
import FramePreview from "./FramePreview";

type Props = {
  fps: number;
  onAccept: (candidate: Candidate) => void;
  onReject: (candidate: Candidate) => void;
  onReroll: (candidate: Candidate) => void;
};

/** A drift/scale/seam quality chip from the normalization report. */
function flagClass(ok: boolean): string {
  return ok ? "animation-studio__flag--ok" : "animation-studio__flag--warn";
}

const QualityFlags: Component<{ report: NormalizeReport | null }> = (p) => (
  <Show when={p.report} fallback={<span class="animation-studio__flags">no measurements</span>}>
    {(report) => (
      <div class="animation-studio__flags" data-testid="candidate-quality-flags">
        <span class={`animation-studio__flag ${flagClass(report().baseline_drift_px === 0)}`}>
          drift: {report().baseline_drift_px}px
        </span>
        <span class={`animation-studio__flag ${flagClass(report().scale_match_pct >= 90)}`}>
          scale: {report().scale_match_pct}%
        </span>
        <span class={`animation-studio__flag ${flagClass(report().seam === "ok")}`}>
          seam: {report().seam}
        </span>
      </div>
    )}
  </Show>
);

const CandidateReview: Component<Props> = (props) => {
  return (
    <div class="animation-studio__candidates" data-testid="candidate-review">
      <Show
        when={studio.candidates.length > 0}
        fallback={<div class="animation-studio__empty">No candidates yet — generate to start.</div>}
      >
        <div class="animation-studio__candidate-strip">
          <For each={studio.candidates}>
            {(candidate, i) => (
              <button
                class="animation-studio__candidate-thumb"
                classList={{
                  "animation-studio__candidate-thumb--active":
                    studio.selectedCandidateId === candidate.id,
                }}
                aria-pressed={studio.selectedCandidateId === candidate.id}
                onClick={() => setSelectedCandidateId(candidate.id)}
                data-testid={`candidate-thumb-${i()}`}
              >
                <FramePreview
                  frames={candidate.frames}
                  fps={props.fps}
                  playing={studio.selectedCandidateId === candidate.id}
                  loopDirection={loopDirectionFor(candidate.animType)}
                  scale={2}
                />
                <span class="animation-studio__candidate-index">#{i() + 1}</span>
              </button>
            )}
          </For>
        </div>

        <Show when={selectedCandidate()}>
          {(candidate) => (
            <div class="animation-studio__candidate-detail">
              <QualityFlags report={candidate().report} />
              <div class="animation-studio__candidate-actions">
                <button
                  class="animation-studio__btn animation-studio__btn--accept"
                  onClick={() => props.onAccept(candidate())}
                  data-testid="candidate-accept"
                >
                  Accept
                </button>
                <button
                  class="animation-studio__btn"
                  onClick={() => props.onReroll(candidate())}
                  data-testid="candidate-reroll"
                >
                  Re-roll
                </button>
                <button
                  class="animation-studio__btn animation-studio__btn--reject"
                  onClick={() => props.onReject(candidate())}
                  data-testid="candidate-reject"
                >
                  Reject
                </button>
              </div>
            </div>
          )}
        </Show>
      </Show>
    </div>
  );
};

export default CandidateReview;
