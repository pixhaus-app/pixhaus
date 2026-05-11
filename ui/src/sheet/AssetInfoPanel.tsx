// Side panel for editing a Reference entity's asset info.
//
// Renders a key-value field list (name, age, species, era, faction) and
// a personality-notes text area. Changes are committed on blur so each
// keystroke doesn't fire an IPC round-trip.

import { type Component, For, Show, createEffect, createSignal } from "solid-js";
import type { AssetInfo } from "../lib/types";

// Fields surfaced as structured rows. The Reference-entity data model
// supports arbitrary string keys; this list pre-labels the common ones.
// Adding a free-form key from the UI is a B10.4 follow-up.
const KNOWN_FIELDS: ReadonlyArray<{ key: string; label: string }> = [
  { key: "name", label: "Name" },
  { key: "age", label: "Age" },
  { key: "species", label: "Species" },
  { key: "era", label: "Era" },
  { key: "faction", label: "Faction" },
];

type Props = {
  info: AssetInfo;
  onSave: (info: AssetInfo) => void;
};

const AssetInfoPanel: Component<Props> = (props) => {
  // Local draft so we only call onSave on blur, not every keystroke.
  const [fields, setFields] = createSignal<Record<string, string>>({ ...props.info.fields });
  const [notes, setNotes] = createSignal<string>((props.info.notes ?? []).join("\n"));

  // Sync inbound changes (e.g. after an approval that rewrites the entity).
  createEffect(() => {
    setFields({ ...props.info.fields });
    setNotes((props.info.notes ?? []).join("\n"));
  });

  function commitField(key: string, value: string): void {
    const next = { ...fields() };
    if (value === "") {
      delete next[key];
    } else {
      next[key] = value;
    }
    setFields(next);
    props.onSave({
      fields: next,
      notes: notesArray(),
    });
  }

  function notesArray(): string[] {
    return notes()
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
  }

  function commitNotes(): void {
    props.onSave({
      fields: fields(),
      notes: notesArray(),
    });
  }

  return (
    <div class="sheet-info-panel">
      <div class="sheet-info-panel__fields">
        <For each={KNOWN_FIELDS}>
          {({ key, label }) => (
            <div class="sheet-info-panel__row">
              <label class="sheet-info-panel__label" for={`info-field-${key}`}>
                {label}
              </label>
              <input
                id={`info-field-${key}`}
                class="sheet-info-panel__input"
                type="text"
                value={fields()[key] ?? ""}
                onInput={(e) => {
                  const next = { ...fields(), [key]: e.currentTarget.value };
                  setFields(next);
                }}
                onBlur={(e) => commitField(key, e.currentTarget.value)}
              />
            </div>
          )}
        </For>
      </div>
      <div class="sheet-info-panel__notes-section">
        <div class="sheet-info-panel__notes-label">Personality / behaviour notes</div>
        <textarea
          class="sheet-info-panel__notes"
          placeholder="One bullet per line"
          value={notes()}
          onInput={(e) => setNotes(e.currentTarget.value)}
          onBlur={commitNotes}
          rows={5}
        />
      </div>
      <Show when={Object.keys(fields()).some((k) => !KNOWN_FIELDS.some((f) => f.key === k))}>
        <div class="sheet-info-panel__extra-fields">
          <div class="sheet-info-panel__extra-label">Additional fields</div>
          <For
            each={Object.entries(fields()).filter(([k]) => !KNOWN_FIELDS.some((f) => f.key === k))}
          >
            {([key, value]) => (
              <div class="sheet-info-panel__row">
                <span class="sheet-info-panel__label">{key}</span>
                <input
                  class="sheet-info-panel__input"
                  type="text"
                  value={value}
                  onInput={(e) => {
                    const next = { ...fields(), [key]: e.currentTarget.value };
                    setFields(next);
                  }}
                  onBlur={(e) => commitField(key, e.currentTarget.value)}
                />
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

export default AssetInfoPanel;
