// Controlled form editor for a PromptTemplate record.
//
// Shows the prompt text, a derived variables table (detected tokens merged
// with declared variables), and optional default Structure/Style pickers.
// No internal persistence — changes surface through `onChange`.

import { type Component, For, createMemo } from "solid-js";
import type { PromptTemplate, PromptVariable, Structure, Style } from "../lib/types";
import { detectTokens } from "./tokens";
import { StructurePicker, StylePicker } from "./pickers";

type Props = {
  value: PromptTemplate;
  onChange: (p: PromptTemplate) => void;
  readOnly?: boolean;
  structures: Structure[];
  styles: Style[];
};

const PromptEditor: Component<Props> = (props) => {
  function update(patch: Partial<PromptTemplate>): void {
    props.onChange({ ...props.value, ...patch });
  }

  // Merge declared variables with auto-detected tokens.
  // Declared variables come first (preserve their label/default); detected
  // tokens that aren't already declared are appended with label=key, default="".
  const mergedVariables = createMemo<PromptVariable[]>(() => {
    const declared = props.value.variables ?? [];
    const detected = detectTokens(props.value.text);
    const declaredKeys = new Set(declared.map((v) => v.key));
    const extra: PromptVariable[] = detected
      .filter((k) => !declaredKeys.has(k))
      .map((k) => ({ key: k, label: k, default: "" }));
    return [...declared, ...extra];
  });

  function updateVariable(index: number, patch: Partial<PromptVariable>): void {
    const next = mergedVariables().map((v, i) => (i === index ? { ...v, ...patch } : v));
    update({ variables: next });
  }

  return (
    <div class="lib-prompt-editor">
      <div class="lib-field">
        <label class="lib-field__label" for="prompt-name">
          Name
        </label>
        <input
          id="prompt-name"
          class="lib-field__input"
          type="text"
          disabled={props.readOnly ?? false}
          value={props.value.name}
          onInput={(e) => update({ name: e.currentTarget.value })}
        />
      </div>

      <div class="lib-field">
        <label class="lib-field__label" for="prompt-text">
          Prompt text
        </label>
        <textarea
          id="prompt-text"
          class="lib-field__textarea"
          disabled={props.readOnly ?? false}
          rows={6}
          placeholder="e.g. Bit, the Pixhaus mascot robot, in a {pose} pose"
          value={props.value.text}
          onInput={(e) => update({ text: e.currentTarget.value })}
        />
      </div>

      <div class="lib-variables">
        <div class="lib-variables__heading">Variables</div>
        <table class="lib-variables__table">
          <thead>
            <tr>
              <th>Key</th>
              <th>Label</th>
              <th>Default</th>
            </tr>
          </thead>
          <tbody>
            <For
              each={mergedVariables()}
              fallback={
                <tr>
                  <td colspan="3" class="lib-variables__empty">
                    No {`{token}`} variables detected.
                  </td>
                </tr>
              }
            >
              {(v, i) => (
                <tr>
                  <td class="lib-variables__key">
                    <code>{v.key}</code>
                  </td>
                  <td>
                    <input
                      class="lib-field__input lib-field__input--sm"
                      type="text"
                      disabled={props.readOnly ?? false}
                      value={v.label}
                      onInput={(e) => updateVariable(i(), { label: e.currentTarget.value })}
                    />
                  </td>
                  <td>
                    <input
                      class="lib-field__input lib-field__input--sm"
                      type="text"
                      disabled={props.readOnly ?? false}
                      value={v.default ?? ""}
                      onInput={(e) => updateVariable(i(), { default: e.currentTarget.value })}
                    />
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>

      <div class="lib-field">
        <label class="lib-field__label">Default style</label>
        <StylePicker
          styles={props.styles}
          value={props.value.default_style ?? null}
          onChange={(id) => update({ default_style: id })}
          disabled={props.readOnly ?? false}
        />
      </div>

      <div class="lib-field">
        <label class="lib-field__label">Default structure</label>
        <StructurePicker
          structures={props.structures}
          value={props.value.default_structure ?? null}
          onChange={(id) => update({ default_structure: id })}
          disabled={props.readOnly ?? false}
        />
      </div>
    </div>
  );
};

export default PromptEditor;
