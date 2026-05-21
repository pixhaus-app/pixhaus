// Plain <select>-based pickers for composition library records.
//
// Thin controlled wrappers — no internal state, no IPC. The parent provides
// the list of available records and handles onChange.

import { type Component, For } from "solid-js";
import type {
  Structure,
  StructureId,
  Style,
  StyleId,
  PromptTemplate,
  PromptId,
} from "../lib/types";

// ── StructurePicker ───────────────────────────────────────────────────────────

type StructurePickerProps = {
  structures: Structure[];
  value: StructureId | null;
  onChange: (id: StructureId | null) => void;
  disabled?: boolean;
};

export const StructurePicker: Component<StructurePickerProps> = (props) => {
  return (
    <select
      class="lib-picker lib-picker--structure"
      disabled={props.disabled}
      value={props.value ?? ""}
      onChange={(e) => {
        const v = e.currentTarget.value;
        props.onChange(v === "" ? null : v);
      }}
    >
      <option value="">— none —</option>
      <For each={props.structures}>{(s) => <option value={s.id}>{s.name}</option>}</For>
    </select>
  );
};

// ── StylePicker ───────────────────────────────────────────────────────────────

type StylePickerProps = {
  styles: Style[];
  value: StyleId | null;
  onChange: (id: StyleId | null) => void;
  disabled?: boolean;
};

export const StylePicker: Component<StylePickerProps> = (props) => {
  return (
    <select
      class="lib-picker lib-picker--style"
      disabled={props.disabled}
      value={props.value ?? ""}
      onChange={(e) => {
        const v = e.currentTarget.value;
        props.onChange(v === "" ? null : v);
      }}
    >
      <option value="">— none —</option>
      <For each={props.styles}>{(s) => <option value={s.id}>{s.name}</option>}</For>
    </select>
  );
};

// ── PromptPicker ──────────────────────────────────────────────────────────────

type PromptPickerProps = {
  prompts: PromptTemplate[];
  value: PromptId | null;
  onChange: (id: PromptId | null) => void;
  disabled?: boolean;
};

export const PromptPicker: Component<PromptPickerProps> = (props) => {
  return (
    <select
      class="lib-picker lib-picker--prompt"
      disabled={props.disabled}
      value={props.value ?? ""}
      onChange={(e) => {
        const v = e.currentTarget.value;
        props.onChange(v === "" ? null : v);
      }}
    >
      <option value="">— none —</option>
      <For each={props.prompts}>{(p) => <option value={p.id}>{p.name}</option>}</For>
    </select>
  );
};
