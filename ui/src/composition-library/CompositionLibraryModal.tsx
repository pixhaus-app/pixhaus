import { type Component } from "solid-js";
import { Dialog } from "../lib/ui/Dialog";
import { closeCompositionLibrary } from "./composition-library-state";
import CompositionLibraryPanel from "./CompositionLibraryPanel";
import "./composition-library.css";

const CompositionLibraryModal: Component = () => {
  return (
    <Dialog
      open={true}
      title="Prompt & Style Library"
      onClose={closeCompositionLibrary}
      size="full"
      initialFocus="none"
    >
      <Dialog.Body class="comp-lib__dialog-body">
        <CompositionLibraryPanel />
      </Dialog.Body>
    </Dialog>
  );
};

export default CompositionLibraryModal;
