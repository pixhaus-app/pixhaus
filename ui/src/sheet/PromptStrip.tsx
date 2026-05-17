// Prompt history strip — chronological list of prompts run against the sheet.

import { type Component, For, Show } from "solid-js";
import type { PromptEntry } from "../lib/types";

type Props = {
  prompts: PromptEntry[];
};

const PromptStrip: Component<Props> = (props) => {
  return (
    <div class="sheet-prompt-strip">
      <Show
        when={props.prompts.length > 0}
        fallback={<p class="sheet-prompt-strip__empty">No prompts yet.</p>}
      >
        <For each={[...props.prompts].reverse()}>
          {(entry) => (
            <div class="sheet-prompt-strip__entry">
              <div class="sheet-prompt-strip__text">{entry.prompt}</div>
              <div class="sheet-prompt-strip__meta">
                {new Date(entry.issued_at * 1000).toLocaleString()}
                <Show when={entry.negative_prompt != null}>
                  {" "}
                  · negative: {entry.negative_prompt}
                </Show>
              </div>
            </div>
          )}
        </For>
      </Show>
    </div>
  );
};

export default PromptStrip;
