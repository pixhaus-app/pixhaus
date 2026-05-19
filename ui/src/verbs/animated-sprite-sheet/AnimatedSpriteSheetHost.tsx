// Mount-once host for the animated-sprite-sheet dialog.
//
// Mirrors the pattern in `lib/ai/VerbInvokeHost.tsx`: a single host
// component subscribes to the open signal and renders the form inside a
// modal. The command-palette entry calls `openAnimatedSpriteSheetForm`.

import { type Component, Show } from "solid-js";

import AnimatedSpriteSheetForm from "./AnimatedSpriteSheetForm";
import { closeAnimatedSpriteSheetForm, isAnimatedSpriteSheetOpen } from "./state";

const AnimatedSpriteSheetHost: Component = () => {
  return (
    <Show when={isAnimatedSpriteSheetOpen()}>
      <div
        class="animated-sprite-sheet-backdrop"
        role="dialog"
        aria-modal="true"
        aria-label="Animated sprite sheet"
        onClick={(e) => {
          if (e.target === e.currentTarget) closeAnimatedSpriteSheetForm();
        }}
      >
        <div class="animated-sprite-sheet-modal">
          <AnimatedSpriteSheetForm />
        </div>
      </div>
    </Show>
  );
};

export default AnimatedSpriteSheetHost;
