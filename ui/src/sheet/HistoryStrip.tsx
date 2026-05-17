// History strip — thumbnail carousel of past sheet variants.
//
// Shows the canonical variant (marked with a badge) followed by history
// entries newest-first. Clicking a thumbnail previews it in place.
// Right-click context menu offers: Approve as canonical / Delete.

import { type Component, For, Show, createSignal, onCleanup } from "solid-js";
import type { SheetVariant } from "../lib/types";
import { useImageObjectUrl } from "../lib/utils/image-object-url";

type ContextMenuState = {
  x: number;
  y: number;
  variant: SheetVariant;
  isCanonical: boolean;
};

type Props = {
  canonical: SheetVariant | null;
  history: SheetVariant[];
  /** Currently previewed variant id — null means the canonical is shown. */
  previewId: number | null;
  onPreview: (variant: SheetVariant | null) => void;
  onApprove: (variant: SheetVariant) => void;
  onDelete: (variant: SheetVariant) => void;
};

const HistoryStrip: Component<Props> = (props) => {
  const [menu, setMenu] = createSignal<ContextMenuState | null>(null);

  function openMenu(e: MouseEvent, variant: SheetVariant, isCanonical: boolean): void {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, variant, isCanonical });
  }

  function closeMenu(): void {
    setMenu(null);
  }

  // Close the context menu when clicking anywhere outside it.
  const handleOutsideClick = (e: MouseEvent): void => {
    const target = e.target as Element | null;
    if (!target?.closest(".sheet-history-menu")) closeMenu();
  };
  document.addEventListener("click", handleOutsideClick);
  onCleanup(() => document.removeEventListener("click", handleOutsideClick));

  const allVariants = (): Array<{ variant: SheetVariant; isCanonical: boolean }> => {
    const variants = props.history.map((v) => ({ variant: v, isCanonical: false }));
    return props.canonical === null
      ? variants
      : [{ variant: props.canonical, isCanonical: true }, ...variants];
  };

  return (
    <div class="sheet-history-strip">
      <Show
        when={allVariants().length > 0}
        fallback={<p class="sheet-prompt-strip__empty">No sheet candidates yet.</p>}
      >
        <For each={allVariants()}>
          {({ variant, isCanonical }) => {
            const isActive = (): boolean =>
              props.previewId === null ? isCanonical : props.previewId === variant.id;
            const thumbUrl = useImageObjectUrl(() => variant.image);

            return (
              <div
                class="sheet-history-thumb"
                classList={{ "sheet-history-thumb--active": isActive() }}
                onClick={() => props.onPreview(isCanonical ? null : variant)}
                onContextMenu={(e) => openMenu(e, variant, isCanonical)}
                title={
                  isCanonical
                    ? "Canonical — currently approved"
                    : new Date(variant.created_at * 1000).toLocaleString()
                }
              >
                <Show
                  when={thumbUrl() !== ""}
                  fallback={<div class="sheet-history-thumb__empty" />}
                >
                  <img class="sheet-history-thumb__img" src={thumbUrl()} alt="" />
                </Show>
                <Show when={isCanonical}>
                  <div class="sheet-history-thumb__badge">approved</div>
                </Show>
              </div>
            );
          }}
        </For>
      </Show>

      <Show when={menu()}>
        {(m) => (
          <div class="sheet-history-menu" style={{ left: `${m().x}px`, top: `${m().y}px` }}>
            <Show when={!m().isCanonical}>
              <button
                class="sheet-history-menu__item"
                onClick={() => {
                  props.onApprove(m().variant);
                  closeMenu();
                }}
              >
                Approve as canonical
              </button>
            </Show>
            <Show when={props.canonical !== null}>
              <button
                class="sheet-history-menu__item"
                onClick={() => {
                  props.onPreview(null);
                  closeMenu();
                }}
              >
                Preview canonical
              </button>
            </Show>
            <Show when={!m().isCanonical}>
              <button
                class="sheet-history-menu__item sheet-history-menu__item--danger"
                onClick={() => {
                  props.onDelete(m().variant);
                  closeMenu();
                }}
              >
                Delete
              </button>
            </Show>
          </div>
        )}
      </Show>
    </div>
  );
};

export default HistoryStrip;
