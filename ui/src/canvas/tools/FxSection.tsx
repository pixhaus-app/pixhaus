// FX section placeholder.
//
// Reserves a slot in the right rail for non-destructive layer effects.
// Real controls land with the FX engine work. Closed by default.

import { type Component } from "solid-js";

const FxSection: Component = () => (
  <div class="tool-options-group" data-testid="tool-options-fx">
    <p class="rail-section__placeholder">FX controls will live here.</p>
  </div>
);

export default FxSection;
