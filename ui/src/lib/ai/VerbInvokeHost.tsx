// Single mount point for the verb-invocation modal.
//
// The command-palette handlers open the modal by writing to
// `verb-invoke-state.ts`; this component subscribes and renders the
// schema-driven `ModalForm`. It owns the IPC plumbing — fetching the
// verb list, calling `verbInvoke`, surfacing toasts, threading the
// invocation id through cancel.

import { type Component, Show } from "solid-js";
import ModalForm from "../../components/ModalForm";
import { pushToast } from "../toast/toast-state";
import { reportCommandFailure } from "../utils/errors";
import { verbCancel, verbInvoke } from "../commands/verbs";
import {
  activeInvocationId,
  activeVerb,
  pendingPrefill,
  setActiveInvocationId,
  setActiveVerb,
  setPendingPrefill,
} from "./verb-invoke-state";

const VerbInvokeHost: Component = () => {
  function close(): void {
    setActiveVerb(null);
    setActiveInvocationId(null);
    setPendingPrefill(null);
  }

  function onSubmit(inputs: Record<string, unknown>): void {
    const verb = activeVerb();
    if (verb === null) return;
    const verbId = verb.id;
    const label = verb.display_name;
    pushToast({ title: `${label}: invoking…`, kind: "info" });

    verbInvoke({ verb_id: verbId, inputs })
      .then((result) => {
        // Track the invocation for cancellation up-front. The promise
        // resolves only when the verb finishes, so by the time we get
        // here `invocation_id` is no longer cancellable. Setting it
        // briefly is harmless — the cancel button is gated on the same
        // signal we clear below.
        setActiveInvocationId(result.invocation_id);
        pushToast({ title: `${label}: completed`, kind: "info" });
        close();
      })
      .catch((err: unknown) => {
        reportCommandFailure(`verb_invoke ${verbId}`, err);
        setActiveInvocationId(null);
      });
  }

  function onCancelRunning(invocationId: string): void {
    verbCancel(invocationId)
      .then(() => {
        pushToast({ title: "Cancellation sent", kind: "info" });
        // Don't close the modal — the verbInvoke promise will resolve
        // (rejected with VerbError::Cancelled) and run the catch above.
      })
      .catch((err: unknown) => reportCommandFailure("verb_cancel", err));
  }

  return (
    <Show when={activeVerb() !== null}>
      <ModalForm
        open={true}
        title={activeVerb()?.display_name ?? "Verb"}
        schema={activeVerb()?.input_schema ?? {}}
        runningInvocationId={activeInvocationId()}
        initialValues={pendingPrefill() ?? undefined}
        onSubmit={onSubmit}
        onCancelRunning={onCancelRunning}
        onClose={close}
      />
    </Show>
  );
};

export default VerbInvokeHost;
