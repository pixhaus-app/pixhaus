// Controlled form editor for a Style record.
//
// No internal persistence — all changes are surfaced through `onChange`.
// The parent decides when (and whether) to call upsertStyle.

import { type Component, For } from "solid-js";
import type { ModelId, Quality, Style } from "../lib/types";
import { MODEL_OPTIONS, QUALITY_OPTIONS } from "./options";

type Props = {
  value: Style;
  onChange: (s: Style) => void;
  readOnly?: boolean;
};

const StyleEditor: Component<Props> = (props) => {
  function update(patch: Partial<Style>): void {
    props.onChange({ ...props.value, ...patch });
  }

  // Updates an optional string field: removes the key when value is empty so
  // exactOptionalPropertyTypes is satisfied (absent !== undefined).
  function updateOptStr(key: "modifiers" | "look_negatives", value: string): void {
    const next: Style = { ...props.value };
    if (value === "") {
      delete next[key];
    } else {
      next[key] = value;
    }
    props.onChange(next);
  }

  return (
    <div class="lib-style-editor">
      <div class="lib-field">
        <label class="lib-field__label" for="style-name">
          Name
        </label>
        <input
          id="style-name"
          class="lib-field__input"
          type="text"
          disabled={props.readOnly}
          value={props.value.name}
          onInput={(e) => update({ name: e.currentTarget.value })}
        />
      </div>

      <div class="lib-field">
        <label class="lib-field__label" for="style-modifiers">
          Modifiers
        </label>
        <textarea
          id="style-modifiers"
          class="lib-field__textarea"
          disabled={props.readOnly}
          rows={4}
          value={props.value.modifiers ?? ""}
          onInput={(e) => updateOptStr("modifiers", e.currentTarget.value)}
        />
      </div>

      <div class="lib-field">
        <label class="lib-field__label" for="style-look-negatives">
          Look negatives
        </label>
        <textarea
          id="style-look-negatives"
          class="lib-field__textarea"
          disabled={props.readOnly}
          rows={3}
          value={props.value.look_negatives ?? ""}
          onInput={(e) => updateOptStr("look_negatives", e.currentTarget.value)}
        />
      </div>

      <div class="lib-field">
        <label class="lib-field__label" for="style-model">
          Model preference
        </label>
        <select
          id="style-model"
          class="lib-field__select"
          disabled={props.readOnly}
          value={props.value.model_pref ?? ""}
          onChange={(e) => {
            const v = e.currentTarget.value;
            update({ model_pref: v === "" ? null : (v as ModelId) });
          }}
        >
          <option value="">— inherit —</option>
          <For each={MODEL_OPTIONS}>{(opt) => <option value={opt.value}>{opt.label}</option>}</For>
        </select>
      </div>

      <div class="lib-field">
        <label class="lib-field__label" for="style-quality">
          Quality preference
        </label>
        <select
          id="style-quality"
          class="lib-field__select"
          disabled={props.readOnly}
          value={props.value.quality ?? ""}
          onChange={(e) => {
            const v = e.currentTarget.value;
            update({ quality: v === "" ? null : (v as Quality) });
          }}
        >
          <option value="">— inherit —</option>
          <For each={QUALITY_OPTIONS}>
            {(opt) => <option value={opt.value}>{opt.label}</option>}
          </For>
        </select>
      </div>
    </div>
  );
};

export default StyleEditor;
