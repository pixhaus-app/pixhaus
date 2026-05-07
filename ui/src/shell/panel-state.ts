// Panel-visibility signals owned by the shell.
//
// The layer panel and timeline already own their own visibility signals
// (in layers/layer-state.ts and timeline/timeline-state.ts respectively)
// because both panels need their toggle from inside their own modules.
// The colour palette and tilemap panel didn't have one until this module
// landed, so the command palette could not toggle them.
//
// Defaults match the prior behaviour (always rendered) so existing users
// see no change unless they explicitly toggle a panel off.

import { createSignal } from "solid-js";

export const [isPalettePanelVisible, setPalettePanelVisible] = createSignal(true);
export const [isTilemapPanelVisible, setTilemapPanelVisible] = createSignal(true);
