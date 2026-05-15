// Schema-driven input form for AI verb invocation.
//
// Reads a JSON Schema (the verb's `input_schema`) and renders a field
// per top-level property:
//   - `string` → text input (or <select> if `enum` present)
//   - `number` / `integer` → number input
//   - `boolean` → checkbox
//   - array of strings with `enum` items → multi-checkbox
//   - everything else → readonly note + raw <textarea> escape hatch
//
// Required fields are validated before submit. While a verb is running
// the submit button is replaced with a cancel button wired to
// `verbCancel(invocation_id)`. The portal+backdrop styling mirrors
// `ModalInput`/`TilesetPickerDialog` so the look stays consistent with
// the rest of the app.

import { type Component, type JSX, For, Show, createEffect, createSignal } from "solid-js";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";

// JSON Schema is intentionally typed as `unknown` at the IPC boundary
// (`ui/src/lib/commands/verbs.ts::VerbInfo.input_schema`). Callers
// hand us the raw value; the parsing helpers below narrow on demand.

type FieldKind =
  | { kind: "string"; enumValues: string[] | null }
  | { kind: "number" | "integer" }
  | { kind: "boolean" }
  | { kind: "string-enum-array"; enumValues: string[] }
  | { kind: "raw" };

type Field = {
  name: string;
  description: string;
  required: boolean;
  defaultValue: unknown;
  kind: FieldKind;
};

type Props = {
  readonly open: boolean;
  /** Display name shown in the dialog header. */
  readonly title: string;
  /** The verb's `input_schema` from `verbList()`. */
  readonly schema: unknown;
  /** Invocation handle while a verb is running; null when idle. */
  readonly runningInvocationId: string | null;
  /**
   * Optional caller-supplied seed values. In field mode these are merged
   * over schema defaults; in raw-JSON mode they're serialized into the
   * textarea. Used by sheet-panel Refine/Re-run to pre-fill the form.
   */
  readonly initialValues?: Record<string, unknown> | undefined;
  /** Called with the form values once the user clicks Submit. */
  readonly onSubmit: (inputs: Record<string, unknown>) => void;
  /**
   * Called when the user clicks Cancel mid-invocation. The parent is
   * responsible for the actual `verbCancel(id)` call so this component
   * stays IPC-free.
   */
  readonly onCancelRunning: (invocationId: string) => void;
  readonly onClose: () => void;
};

const ModalForm: Component<Props> = (props) => {
  const [values, setValues] = createSignal<Record<string, unknown>>({});
  const [errors, setErrors] = createSignal<Record<string, string>>({});
  const [rawJson, setRawJson] = createSignal("");
  const [rawJsonError, setRawJsonError] = createSignal<string | null>(null);

  const fields = (): Field[] => parseFields(props.schema);
  const isRawSchema = () => fields().length === 0 && hasProperties(props.schema);

  // Reset on open. createEffect tracks `props.open` so each show pulls
  // fresh defaults from the schema rather than retaining the previous
  // session's input.
  createEffect(() => {
    if (props.open) {
      const defaults: Record<string, unknown> = {};
      for (const f of fields()) {
        if (f.defaultValue !== undefined) defaults[f.name] = f.defaultValue;
      }
      const merged = { ...defaults, ...(props.initialValues ?? {}) };
      setValues(merged);
      setErrors({});
      setRawJson(
        props.initialValues !== undefined ? JSON.stringify(props.initialValues, null, 2) : "{}",
      );
      setRawJsonError(null);
    }
  });

  function setField(name: string, value: unknown): void {
    setValues((prev) => ({ ...prev, [name]: value }));
    setErrors((prev) => {
      if (!(name in prev)) return prev;
      const next = { ...prev };
      delete next[name];
      return next;
    });
  }

  function submit(): void {
    if (isRawSchema()) {
      try {
        const parsed: unknown = JSON.parse(rawJson());
        if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
          setRawJsonError("must be a JSON object");
          return;
        }
        props.onSubmit(parsed as Record<string, unknown>);
      } catch (err) {
        setRawJsonError(err instanceof Error ? err.message : String(err));
      }
      return;
    }

    const v = values();
    const errs: Record<string, string> = {};
    for (const f of fields()) {
      if (f.required && (v[f.name] === undefined || v[f.name] === "")) {
        errs[f.name] = "Required";
      }
    }
    if (Object.keys(errs).length > 0) {
      setErrors(errs);
      return;
    }
    props.onSubmit(v);
  }

  function renderField(f: Field): JSX.Element {
    const fieldId = `mf-${f.name}`;
    return (
      <div class="modal-form__row">
        <label class="prefs__label" for={fieldId}>
          {f.name}
          <Show when={f.required}>
            <span class="modal-form__required" aria-label="required">
              {" *"}
            </span>
          </Show>
        </label>
        <Show when={f.description !== ""}>
          <p class="modal-form__hint">{f.description}</p>
        </Show>
        {renderInput(f, fieldId)}
        <Show when={errors()[f.name] !== undefined}>
          <p class="form-field__error" role="alert">
            {errors()[f.name]}
          </p>
        </Show>
      </div>
    );
  }

  function renderInput(f: Field, fieldId: string): JSX.Element {
    const v = values()[f.name];
    switch (f.kind.kind) {
      case "string": {
        if (f.kind.enumValues !== null) {
          const opts = f.kind.enumValues;
          return (
            <select
              id={fieldId}
              class="modal-input__field"
              value={typeof v === "string" ? v : ""}
              onChange={(e) => setField(f.name, e.currentTarget.value)}
            >
              <option value="" disabled>
                — select —
              </option>
              <For each={opts}>{(opt) => <option value={opt}>{opt}</option>}</For>
            </select>
          );
        }
        return (
          <input
            id={fieldId}
            type="text"
            class="modal-input__field"
            value={typeof v === "string" ? v : ""}
            onInput={(e) => setField(f.name, e.currentTarget.value)}
          />
        );
      }
      case "number":
      case "integer":
        return (
          <input
            id={fieldId}
            type="number"
            step={f.kind.kind === "integer" ? 1 : "any"}
            class="modal-input__field"
            value={typeof v === "number" ? String(v) : ""}
            onInput={(e) => {
              const raw = e.currentTarget.value;
              if (raw === "") {
                setField(f.name, undefined);
                return;
              }
              const n = f.kind.kind === "integer" ? parseInt(raw, 10) : parseFloat(raw);
              setField(f.name, Number.isNaN(n) ? undefined : n);
            }}
          />
        );
      case "boolean":
        return (
          <input
            id={fieldId}
            type="checkbox"
            checked={v === true}
            onChange={(e) => setField(f.name, e.currentTarget.checked)}
          />
        );
      case "string-enum-array": {
        const opts = f.kind.enumValues;
        const selected = Array.isArray(v)
          ? (v as unknown[]).filter((x): x is string => typeof x === "string")
          : [];
        return (
          <div class="modal-form__multi" id={fieldId}>
            <For each={opts}>
              {(opt) => (
                <label class="modal-form__multi-item">
                  <input
                    type="checkbox"
                    checked={selected.includes(opt)}
                    onChange={(e) => {
                      const next = e.currentTarget.checked
                        ? [...selected, opt]
                        : selected.filter((s) => s !== opt);
                      setField(f.name, next);
                    }}
                  />
                  {opt}
                </label>
              )}
            </For>
          </div>
        );
      }
      case "raw":
        // Unreachable in field-driven mode (raw schemas hit the
        // textarea fallback below); kept for exhaustiveness.
        return <span>(unsupported field type)</span>;
    }
  }

  return (
    <Dialog open={props.open} title={props.title} onClose={props.onClose} size="md">
      <Dialog.Body>
        <Show
          when={!isRawSchema()}
          fallback={
            <div class="modal-form__row">
              <label class="prefs__label" for="mf-raw">
                Raw JSON
              </label>
              <p class="modal-form__hint">
                Verb input schema is too complex for the auto-form. Edit JSON directly.
              </p>
              <textarea
                id="mf-raw"
                class="modal-input__field"
                rows={8}
                value={rawJson()}
                onInput={(e) => {
                  setRawJson(e.currentTarget.value);
                  setRawJsonError(null);
                }}
              />
              <Show when={rawJsonError() !== null}>
                <p class="form-field__error" role="alert">
                  {rawJsonError()}
                </p>
              </Show>
            </div>
          }
        >
          <For each={fields()}>{(f) => renderField(f)}</For>
        </Show>
      </Dialog.Body>

      <Dialog.Footer>
        <Show
          when={props.runningInvocationId === null}
          fallback={
            <Button
              variant="ghost"
              onClick={() => {
                const id = props.runningInvocationId;
                if (id !== null) props.onCancelRunning(id);
              }}
            >
              Cancel running invocation
            </Button>
          }
        >
          <Button variant="ghost" onClick={props.onClose}>
            Cancel
          </Button>
          <Button onClick={submit}>Run</Button>
        </Show>
      </Dialog.Footer>
    </Dialog>
  );
};

export default ModalForm;

// ── schema parsing ────────────────────────────────────────────────────────────

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function hasProperties(schema: unknown): boolean {
  if (!isRecord(schema)) return false;
  return isRecord(schema.properties);
}

function parseFields(schema: unknown): Field[] {
  if (!isRecord(schema)) return [];
  const props = isRecord(schema.properties) ? schema.properties : null;
  if (props === null) return [];
  const required = Array.isArray(schema.required)
    ? schema.required.filter((r): r is string => typeof r === "string")
    : [];

  const fields: Field[] = [];
  for (const [name, raw] of Object.entries(props)) {
    if (!isRecord(raw)) continue;
    const kind = parseKind(raw);
    if (kind === null) {
      // Property has nested objects, oneOf, refs, etc. Bail to raw-JSON
      // mode by returning an empty list. (See `isRawSchema` above.)
      return [];
    }
    fields.push({
      name,
      description: typeof raw.description === "string" ? raw.description : "",
      required: required.includes(name),
      defaultValue: raw.default,
      kind,
    });
  }
  return fields;
}

function parseKind(prop: Record<string, unknown>): FieldKind | null {
  const t = prop.type;
  // Skip nullable shapes: `type: ["string", "null"]` is fine if it has
  // a string base; treat the null option as "may be empty".
  let baseType: string | null = null;
  if (typeof t === "string") baseType = t;
  else if (Array.isArray(t)) {
    const nonNull = t.filter((x) => x !== "null");
    if (nonNull.length === 1 && typeof nonNull[0] === "string") baseType = nonNull[0];
  }
  if (baseType === null) return null;

  switch (baseType) {
    case "string": {
      const enumValues = Array.isArray(prop.enum)
        ? prop.enum.filter((x): x is string => typeof x === "string")
        : null;
      return { kind: "string", enumValues };
    }
    case "integer":
    case "number":
      return { kind: baseType };
    case "boolean":
      return { kind: "boolean" };
    case "array": {
      const items = isRecord(prop.items) ? prop.items : null;
      if (items === null) return null;
      if (items.type !== "string") return null;
      if (!Array.isArray(items.enum)) return null;
      const enumValues = items.enum.filter((x): x is string => typeof x === "string");
      if (enumValues.length === 0) return null;
      return { kind: "string-enum-array", enumValues };
    }
    default:
      // object, refs, oneOf, etc. — bail.
      return null;
  }
}
