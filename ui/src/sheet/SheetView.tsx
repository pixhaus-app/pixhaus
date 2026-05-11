// Sheet view panel — the main surface for reference sheet generation and management.
//
// Opens when a Reference entity is the active target. Shows the canonical sheet
// image, panel overlays, asset info, history, and prompt history. Action buttons
// fire the generate-reference-sheet (B10.1) and iterate-reference-sheet (B10.2)
// verbs via the existing VerbInvokeHost flow.

import { type Component, For, Show, createEffect, createSignal, on } from "solid-js";
import type {
  Entity,
  EntityContent,
  PromptEntry,
  ReferenceSheet,
  SheetVariant,
} from "../lib/types";
import {
  libraryApproveSheetVariant,
  libraryDeleteSheetVariant,
  libraryGetEntity,
  libraryUpdateAssetInfo,
} from "../lib/commands/library";
import { verbList } from "../lib/commands/verbs";
import { getCachedVerbList, setActiveVerb, setPendingPrefill } from "../lib/ai/verb-invoke-state";
import { pushToast } from "../lib/toast/toast-state";
import { reportCommandFailure } from "../lib/utils/errors";
import { useImageObjectUrl } from "../lib/utils/image-object-url";
import {
  activeSheetEntityId,
  closeSheetPanel,
  selectedPanelRegion,
  setSelectedPanelRegion,
  showPanelOverlay,
  setShowPanelOverlay,
} from "./sheet-state";
import AssetInfoPanel from "./AssetInfoPanel";
import HistoryStrip from "./HistoryStrip";
import PromptStrip from "./PromptStrip";

// Verb IDs for the reference sheet verbs (B10.1 and B10.2).
const GENERATE_VERB_ID = "pixhaus.builtin.generate_reference_sheet";
const ITERATE_VERB_ID = "pixhaus.builtin.iterate_reference_sheet";

type BottomTab = "history" | "prompts";

// Encodes raw image bytes to a base64 string for IPC pre-fill payloads.
// Display goes through `useImageObjectUrl` instead — see Fix D in the
// B10.4 review-fix plan.
function bytesToBase64(bytes: number[]): string {
  if (bytes.length === 0) return "";
  const u8 = new Uint8Array(bytes);
  let binary = "";
  for (let i = 0; i < u8.length; i++) {
    binary += String.fromCharCode(u8[i]!);
  }
  return btoa(binary);
}

// Extracts the ReferenceSheet from an entity's content. Returns null if the
// entity is not Reference-kind (guards against stale state after navigation).
function referenceSheet(entity: Entity): ReferenceSheet | null {
  const content = entity.content as EntityContent;
  if (content.type !== "Reference") return null;
  return content.value.sheet;
}

const SheetView: Component = () => {
  const [entity, setEntity] = createSignal<Entity | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [previewVariant, setPreviewVariant] = createSignal<SheetVariant | null>(null);
  const [activeTab, setActiveTab] = createSignal<BottomTab>("history");

  // Load (or reload) the entity whenever the active sheet ID changes.
  // Capture the requested id and ignore stale resolutions — fast back-to-back
  // selections must not let an older fetch clobber a newer one.
  createEffect(
    on(activeSheetEntityId, (entityId) => {
      if (entityId === null) {
        setEntity(null);
        return;
      }
      const requestId = entityId;
      setLoading(true);
      setPreviewVariant(null);
      libraryGetEntity(entityId)
        .then((e) => {
          if (activeSheetEntityId() !== requestId) return;
          setEntity(e);
        })
        .catch((err: unknown) => {
          if (activeSheetEntityId() !== requestId) return;
          reportCommandFailure("library_get_entity", err);
        })
        .finally(() => {
          if (activeSheetEntityId() !== requestId) return;
          setLoading(false);
        });
    }),
  );

  const sheet = (): ReferenceSheet | null => {
    const e = entity();
    return e !== null ? referenceSheet(e) : null;
  };

  // The image currently shown: the previewed history variant or the canonical.
  const displayedVariant = (): SheetVariant | null => {
    const s = sheet();
    if (s === null) return null;
    return previewVariant() ?? s.canonical;
  };

  const displayedDataUrl = useImageObjectUrl(() => displayedVariant()?.image ?? null);

  // Determine sheet image natural dimensions from the composition template
  // sizes known from B10.1 (1024×1536 for Character, 1024×1024 for others).
  // When no composition panels exist we fall back to a square viewBox.
  const sheetWidth = (): number => {
    const v = displayedVariant();
    if (v?.composition == null) return 1024;
    const allPanels = [
      ...(v.composition.views ?? []),
      ...(v.composition.expressions ?? []),
      ...(v.composition.callouts ?? []),
      ...(v.composition.outfits ?? []),
    ];
    if (allPanels.length === 0) return 1024;
    return Math.max(...allPanels.map((p) => p.region.origin.x + p.region.size.width));
  };

  const sheetHeight = (): number => {
    const v = displayedVariant();
    if (v?.composition == null) return 1024;
    const allPanels = [
      ...(v.composition.views ?? []),
      ...(v.composition.expressions ?? []),
      ...(v.composition.callouts ?? []),
      ...(v.composition.outfits ?? []),
    ];
    if (allPanels.length === 0) return 1024;
    return Math.max(...allPanels.map((p) => p.region.origin.y + p.region.size.height));
  };

  const allPanels = (): Array<{ label: string; x: number; y: number; w: number; h: number }> => {
    const v = displayedVariant();
    if (v?.composition == null) return [];
    return [
      ...(v.composition.views ?? []),
      ...(v.composition.expressions ?? []),
      ...(v.composition.callouts ?? []),
      ...(v.composition.outfits ?? []),
    ].map((p) => ({
      label: p.label,
      x: p.region.origin.x,
      y: p.region.origin.y,
      w: p.region.size.width,
      h: p.region.size.height,
    }));
  };

  function handlePanelClick(panel: {
    label: string;
    x: number;
    y: number;
    w: number;
    h: number;
  }): void {
    const current = selectedPanelRegion();
    if (current !== null && current.origin.x === panel.x && current.origin.y === panel.y) {
      setSelectedPanelRegion(null);
    } else {
      setSelectedPanelRegion({
        origin: { x: panel.x, y: panel.y },
        size: { width: panel.w, height: panel.h },
      });
    }
  }

  function isPanelSelected(panel: { x: number; y: number }): boolean {
    const sel = selectedPanelRegion();
    return sel !== null && sel.origin.x === panel.x && sel.origin.y === panel.y;
  }

  function handleApprove(variant: SheetVariant): void {
    const e = entity();
    if (e === null) return;
    libraryApproveSheetVariant(e.id, variant.id)
      .then((updated) => {
        setEntity(updated);
        setPreviewVariant(null);
        pushToast({ title: "Variant approved as canonical", kind: "info" });
      })
      .catch((err: unknown) => reportCommandFailure("library_approve_sheet_variant", err));
  }

  function handleDelete(variant: SheetVariant): void {
    const e = entity();
    if (e === null) return;
    libraryDeleteSheetVariant(e.id, variant.id)
      .then(() => {
        return libraryGetEntity(e.id).then(setEntity);
      })
      .then(() => {
        if (previewVariant()?.id === variant.id) setPreviewVariant(null);
      })
      .catch((err: unknown) => reportCommandFailure("library_delete_sheet_variant", err));
  }

  function handleSaveInfo(info: Parameters<typeof libraryUpdateAssetInfo>[1]): void {
    const e = entity();
    if (e === null) return;
    libraryUpdateAssetInfo(e.id, info)
      .then(() => libraryGetEntity(e.id))
      .then(setEntity)
      .catch((err: unknown) => reportCommandFailure("library_update_asset_info", err));
  }

  function openVerb(verbId: string, fallbackLabel: string): void {
    getCachedVerbList(verbList)
      .then((verbs) => {
        const v = verbs.find((info) => info.id === verbId);
        if (v === undefined) {
          pushToast({
            title: `${fallbackLabel}: verb "${verbId}" is not registered — check that B10.1 has landed.`,
            kind: "error",
          });
          return;
        }
        setActiveVerb(v);
      })
      .catch((err: unknown) => reportCommandFailure("verb_list", err));
  }

  function handleGenerate(): void {
    openVerb(GENERATE_VERB_ID, "Generate reference sheet");
  }

  function handleRefine(): void {
    const e = entity();
    const v = displayedVariant();
    if (e === null || v === null) {
      pushToast({ title: "No reference sheet to iterate on yet.", kind: "info" });
      return;
    }
    const region = selectedPanelRegion();
    const label =
      region === null
        ? null
        : (allPanels().find((p) => p.x === region.origin.x && p.y === region.origin.y)?.label ??
          null);
    setPendingPrefill({
      entity_id: e.id,
      source_variant_id: v.id,
      sheet_image_b64: bytesToBase64(v.image.bytes),
      composition: v.composition,
      panel_label: label,
      prompt: "",
      negative_prompt: null,
    });
    openVerb(ITERATE_VERB_ID, "Iterate reference sheet");
  }

  function handleRerun(prompt: PromptEntry): void {
    setPendingPrefill({
      prompt: prompt.prompt,
      negative_prompt: prompt.negative_prompt ?? null,
    });
    openVerb(GENERATE_VERB_ID, "Generate reference sheet");
  }

  return (
    <div class="sheet-panel">
      <div class="sheet-panel__header">
        <div class="sheet-panel__title">{entity()?.name ?? "Reference sheet"}</div>
        <div class="sheet-panel__header-actions">
          <button
            class="sheet-panel__icon-btn"
            classList={{ "sheet-panel__icon-btn--active": showPanelOverlay() }}
            onClick={() => setShowPanelOverlay(!showPanelOverlay())}
            aria-label="Toggle panel overlay"
            title="Toggle panel overlay"
          >
            ⊞
          </button>
          <button
            class="sheet-panel__icon-btn"
            onClick={closeSheetPanel}
            aria-label="Close sheet panel"
            title="Close sheet panel"
          >
            ✕
          </button>
        </div>
      </div>

      <Show when={loading()}>
        <div class="sheet-panel__loading">Loading…</div>
      </Show>

      <Show when={!loading() && sheet() === null && activeSheetEntityId() !== null}>
        <div class="sheet-panel__error">Entity not found or not a Reference kind.</div>
      </Show>

      <Show when={!loading() && sheet() !== null}>
        <div class="sheet-panel__body">
          <div class="sheet-panel__image-area">
            <div class="sheet-panel__image-wrap">
              <Show
                when={displayedDataUrl() !== ""}
                fallback={<div class="sheet-panel__image-empty">No image data</div>}
              >
                <img class="sheet-panel__image" src={displayedDataUrl()} alt="Reference sheet" />
              </Show>
              <Show when={showPanelOverlay() && allPanels().length > 0}>
                <svg
                  class="sheet-panel__overlay"
                  viewBox={`0 0 ${sheetWidth()} ${sheetHeight()}`}
                  preserveAspectRatio="none"
                >
                  <For each={allPanels()}>
                    {(panel) => (
                      <g
                        class="sheet-panel__panel-group"
                        classList={{ "sheet-panel__panel-group--selected": isPanelSelected(panel) }}
                        onClick={() => handlePanelClick(panel)}
                      >
                        <rect
                          class="sheet-panel__panel-rect"
                          x={panel.x}
                          y={panel.y}
                          width={panel.w}
                          height={panel.h}
                        />
                        <text class="sheet-panel__panel-label" x={panel.x + 4} y={panel.y + 14}>
                          {panel.label}
                        </text>
                      </g>
                    )}
                  </For>
                </svg>
              </Show>
            </div>
          </div>

          <div class="sheet-panel__info-area">
            <AssetInfoPanel
              info={sheet()?.info ?? { fields: {}, notes: [] }}
              onSave={handleSaveInfo}
            />
          </div>
        </div>

        <div class="sheet-panel__actions">
          <button class="sheet-panel__action-btn" onClick={handleGenerate}>
            Generate variant
          </button>
          <button
            class="sheet-panel__action-btn"
            onClick={handleRefine}
            title={
              selectedPanelRegion() === null
                ? "No panel selected — Refine will run on the whole sheet. Click a panel in the overlay to scope it."
                : "Refine the selected panel"
            }
          >
            Refine selection
          </button>
        </div>

        <div class="sheet-panel__bottom">
          <div class="sheet-panel__tabs">
            <button
              class="sheet-panel__tab"
              classList={{ "sheet-panel__tab--active": activeTab() === "history" }}
              onClick={() => setActiveTab("history")}
            >
              History
            </button>
            <button
              class="sheet-panel__tab"
              classList={{ "sheet-panel__tab--active": activeTab() === "prompts" }}
              onClick={() => setActiveTab("prompts")}
            >
              Prompts
            </button>
          </div>

          <Show when={activeTab() === "history"}>
            <HistoryStrip
              canonical={sheet()!.canonical}
              history={sheet()?.history ?? []}
              previewId={previewVariant()?.id ?? null}
              onPreview={(v) => setPreviewVariant(v)}
              onApprove={handleApprove}
              onDelete={handleDelete}
            />
          </Show>

          <Show when={activeTab() === "prompts"}>
            <PromptStrip prompts={sheet()?.prompts ?? []} onRerun={handleRerun} />
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default SheetView;
