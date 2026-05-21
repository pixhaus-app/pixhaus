import { type Component } from "solid-js";
import { Button } from "../lib/ui/Button";
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
      size="lg"
      initialFocus="none"
    >
      <Dialog.Body class="comp-lib__dialog-body">
        <CompositionLibraryPanel />
      </Dialog.Body>

      <Dialog.Footer>
        <Button onClick={closeCompositionLibrary}>Close</Button>
      </Dialog.Footer>
    </Dialog>
  );
};

export default CompositionLibraryModal;
