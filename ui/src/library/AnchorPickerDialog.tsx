// "Set as anchor" picker dialog. Lets the user pick a Reference entity
// to anchor a Custom entity to. Mounted at Shell level; opened via
// openAnchorPicker(...) from LibraryPanel's context menu.

import { For, createEffect, createSignal, type Component } from "solid-js";
import type { EntityId } from "../lib/types";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";
import { anchorPickerRequest, closeAnchorPicker } from "./anchor-picker-state";

const AnchorPickerDialog: Component = () => {
  const [selected, setSelected] = createSignal<EntityId | null>(null);

  createEffect(() => {
    const req = anchorPickerRequest();
    if (req === null) {
      setSelected(null);
      return;
    }
    setSelected(req.references[0]?.id ?? null);
  });

  function onConfirm(): void {
    const req = anchorPickerRequest();
    const refId = selected();
    if (req === null || refId === null) return;
    closeAnchorPicker();
    req.onConfirm(refId);
  }

  return (
    <Dialog
      open={anchorPickerRequest() !== null}
      title="Set as anchor"
      onClose={closeAnchorPicker}
      size="sm"
    >
      <Dialog.Body>
        <div class="prefs__row">
          <div>
            <div class="prefs__label">Reference entity</div>
            <div class="prefs__sublabel">The canonical sheet this entity will be anchored to.</div>
          </div>
          <select
            class="prefs__select"
            data-testid="anchor-picker-select"
            value={selected() ?? ""}
            onChange={(e) => {
              const v = e.currentTarget.value;
              setSelected(v === "" ? null : (parseInt(v, 10) as EntityId));
            }}
          >
            <For each={anchorPickerRequest()?.references ?? []}>
              {(r) => <option value={r.id}>{r.name}</option>}
            </For>
          </select>
        </div>
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="ghost" onClick={closeAnchorPicker}>
          Cancel
        </Button>
        <Button
          onClick={onConfirm}
          disabled={selected() === null}
          data-testid="anchor-picker-confirm"
        >
          Set as anchor
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
};

export default AnchorPickerDialog;
