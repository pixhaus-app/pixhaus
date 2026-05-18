// Dithering pattern picker placeholder.
//
// The section exists so the rail has a fixed home for dithering controls;
// the real pattern picker lands with the dithering brush work. Closed by
// default — the user opens it explicitly until that lands.

import { type Component } from "solid-js";

const DitheringSection: Component = () => (
  <div class="tool-options-group" data-testid="tool-options-dithering">
    <p class="rail-section__placeholder">Dithering patterns are coming with the dithering brush.</p>
  </div>
);

export default DitheringSection;
