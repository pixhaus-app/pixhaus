// Sheet view panel — the main surface for embedded reference sheet management.
//
// Opens for sprite entities that own a reference sheet. Shows the canonical
// sheet image, panel overlays, asset info, history, and prompt history.

import { type Component, For, Show, createEffect, createSignal, on } from "solid-js";
import type {
  AssetInfo,
  Entity,
  EntityContent,
  EntityId,
  PromptEntry,
  ReferenceSheet,
  SheetVariant,
} from "../lib/types";
import {
  libraryDeleteSheetVariant,
  libraryGetEntity,
  libraryTrainEntityLora,
  libraryUpdateAssetInfo,
} from "../lib/commands/library";
import { approveSheetVariantAndRefreshCorpus } from "../library/library-state";
import { pushToast } from "../lib/toast/toast-state";
import { reportCommandFailure } from "../lib/utils/errors";
import { useImageObjectUrl } from "../lib/utils/image-object-url";
import {
  sheetState,
  clearSheetEntity,
  setSelectedPanelRegion,
  setShowPanelOverlay,
} from "./sheet-state";
import { closeSection } from "../shell/rail-state";
import {
  flatPanels,
  sheetHeight as computeSheetHeight,
  sheetWidth as computeSheetWidth,
} from "./sheet-panels";
import AssetInfoPanel from "./AssetInfoPanel";
import HistoryStrip from "./HistoryStrip";
import PromptStrip from "./PromptStrip";

type BottomTab = "history" | "prompts";

type LegacySheetExtras = {
  info?: AssetInfo;
  prompts?: PromptEntry[];
  history?: SheetVariant[];
};

// Extracts the ReferenceSheet from an entity's sprite content. Returns null
// when the entity has no embedded sheet.
function referenceSheet(entity: Entity): ReferenceSheet | null {
  const content = entity.content as EntityContent;
  if (content.type !== "Sprites") return null;
  return content.value.reference_sheet ?? null;
}

type Props = {
  /** When true, render without the outer panel header — the right-rail
   *  accordion provides its own chrome. */
  inRail?: boolean;
};

const SheetView: Component<Props> = (props) => {
  const [entity, setEntity] = createSignal<Entity | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [previewVariant, setPreviewVariant] = createSignal<SheetVariant | null>(null);
  const [activeTab, setActiveTab] = createSignal<BottomTab>("history");
  // B10.5: per-entity LoRA training is a long (15-30 min) Replicate call.
  // Track the entity currently training (not just a boolean) so navigating
  // to a different sheet mid-run doesn't disable that sheet's Train button.
  // Cleared in the .finally() of the originating call.
  const [trainingLoraEntity, setTrainingLoraEntity] = createSignal<EntityId | null>(null);
  const isTrainingThisLora = (): boolean => {
    const e = entity();
    return e !== null && trainingLoraEntity() === e.id;
  };

  // Load (or reload) the entity whenever the active sheet ID changes.
  // Capture the requested id and ignore stale resolutions — fast back-to-back
  // selections must not let an older fetch clobber a newer one.
  createEffect(
    on(
      () => sheetState.activeSheetEntityId,
      (entityId) => {
        if (entityId === null) {
          setEntity(null);
          return;
        }
        const requestId = entityId;
        setLoading(true);
        setPreviewVariant(null);
        libraryGetEntity(entityId)
          .then((e) => {
            if (sheetState.activeSheetEntityId !== requestId) return;
            setEntity(e);
          })
          .catch((err: unknown) => {
            if (sheetState.activeSheetEntityId !== requestId) return;
            reportCommandFailure("library_get_entity", err);
          })
          .finally(() => {
            if (sheetState.activeSheetEntityId !== requestId) return;
            setLoading(false);
          });
      },
    ),
  );

  const sheet = (): ReferenceSheet | null => {
    const e = entity();
    return e !== null ? referenceSheet(e) : null;
  };
  const variants = (): SheetVariant[] => {
    const s = sheet();
    if (s === null) return [];
    return s.variants ?? (s as ReferenceSheet & LegacySheetExtras).history ?? [];
  };
  const legacyInfo = (): AssetInfo => {
    const s = sheet() as (ReferenceSheet & LegacySheetExtras) | null;
    return s?.info ?? { fields: {}, notes: [] };
  };
  const legacyPrompts = (): PromptEntry[] => {
    const s = sheet() as (ReferenceSheet & LegacySheetExtras) | null;
    return s?.prompts ?? [];
  };

  // The image currently shown: the previewed history variant, canonical,
  // or first draft candidate when no canonical has been approved yet.
  const displayedVariant = (): SheetVariant | null => {
    const s = sheet();
    if (s === null) return null;
    return previewVariant() ?? s.canonical ?? variants()[0] ?? null;
  };

  const displayedDataUrl = useImageObjectUrl(() => displayedVariant()?.image ?? null);

  const sheetWidth = (): number => computeSheetWidth(displayedVariant()?.composition);
  const sheetHeight = (): number => computeSheetHeight(displayedVariant()?.composition);
  const allPanels = () => flatPanels(displayedVariant()?.composition);

  function handlePanelClick(panel: {
    label: string;
    x: number;
    y: number;
    w: number;
    h: number;
  }): void {
    const current = sheetState.selectedPanelRegion;
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
    const sel = sheetState.selectedPanelRegion;
    return sel !== null && sel.origin.x === panel.x && sel.origin.y === panel.y;
  }

  function handleApprove(variant: SheetVariant): void {
    const e = entity();
    if (e === null) return;
    approveSheetVariantAndRefreshCorpus(e.id, variant.id)
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

  // B10.5: kicks off the long Replicate train-entity-lora flow. Returns
  // synchronously after the IPC promise is queued so the caller doesn't
  // wait; the toast surface reports completion or cancellation.
  function handleTrainLora(): void {
    const e = entity();
    if (e === null) return;
    if (trainingLoraEntity() === e.id) {
      pushToast({ title: "Training already in progress for this sheet.", kind: "info" });
      return;
    }
    setTrainingLoraEntity(e.id);
    pushToast({
      title: "Training consistency LoRA — this takes 15-30 minutes.",
      kind: "info",
    });
    libraryTrainEntityLora(e.id)
      .then((result) => {
        pushToast({
          title: `Trained consistency LoRA "${result.label}"`,
          kind: "info",
        });
        return libraryGetEntity(e.id);
      })
      .then((refreshed) => {
        // Only update if the active sheet hasn't changed since the
        // request was issued — long training runs may outlive the
        // user's navigation.
        if (sheetState.activeSheetEntityId === e.id) {
          setEntity(refreshed);
        }
      })
      .catch((err: unknown) => reportCommandFailure("library_train_entity_lora", err))
      .finally(() => {
        // Only clear if this entity is still the one we marked busy.
        // Guards against a stale settle from an earlier run wiping the
        // state of a different in-flight training on the same entity.
        if (trainingLoraEntity() === e.id) {
          setTrainingLoraEntity(null);
        }
      });
  }

  // True when the entity carries a non-empty `lora_path`. Drives the
  // training-status pill so users can tell at a glance whether the
  // anchor will ship a per-entity LoRA.
  const loraPath = (): string | null => {
    const e = entity();
    if (e === null) return null;
    const path = e.ai?.lora_path;
    return typeof path === "string" && path.length > 0 ? path : null;
  };

  const hasTrainedLora = (): boolean => loraPath() !== null;

  return (
    <div class="sheet-panel" classList={{ "sheet-panel--in-rail": !!props.inRail }}>
      <Show when={!props.inRail}>
        <div class="sheet-panel__header">
          <div class="sheet-panel__title">{entity()?.name ?? "Reference sheet"}</div>
          <div class="sheet-panel__header-actions">
            <button
              class="sheet-panel__icon-btn"
              classList={{ "sheet-panel__icon-btn--active": sheetState.showPanelOverlay }}
              onClick={() => setShowPanelOverlay(!sheetState.showPanelOverlay)}
              aria-label="Toggle panel overlay"
              title="Toggle panel overlay"
            >
              ⊞
            </button>
            <button
              class="sheet-panel__icon-btn"
              onClick={() => {
                clearSheetEntity();
                closeSection("reference");
              }}
              aria-label="Close sheet panel"
              title="Close sheet panel"
            >
              ✕
            </button>
          </div>
        </div>
      </Show>
      <Show when={props.inRail}>
        <div class="sheet-panel__rail-actions">
          <button
            class="sheet-panel__icon-btn"
            classList={{ "sheet-panel__icon-btn--active": sheetState.showPanelOverlay }}
            onClick={() => setShowPanelOverlay(!sheetState.showPanelOverlay)}
            aria-label="Toggle panel overlay"
            title="Toggle panel overlay"
          >
            ⊞ Overlay
          </button>
        </div>
      </Show>

      <Show when={loading()}>
        <div class="sheet-panel__loading">Loading…</div>
      </Show>

      <Show when={!loading() && sheet() === null && sheetState.activeSheetEntityId !== null}>
        <div class="sheet-panel__error">Entity not found or has no reference sheet.</div>
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
              <Show when={sheetState.showPanelOverlay && allPanels().length > 0}>
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
            <AssetInfoPanel info={legacyInfo()} onSave={handleSaveInfo} />
          </div>
        </div>

        <div class="sheet-panel__actions">
          <button
            class="sheet-panel__action-btn"
            classList={{ "sheet-panel__action-btn--busy": isTrainingThisLora() }}
            onClick={handleTrainLora}
            disabled={isTrainingThisLora()}
            title={
              isTrainingThisLora()
                ? "Training is in progress (15-30 min)."
                : hasTrainedLora()
                  ? "Retrain the consistency LoRA from this sheet. The new weights replace the existing per-entity LoRA on completion."
                  : "Train a consistency LoRA from this sheet. Takes 15-30 min on Replicate; subsequent generations against this entity use the resulting per-entity weights."
            }
          >
            {isTrainingThisLora() ? "Training…" : hasTrainedLora() ? "Retrain LoRA" : "Train LoRA"}
          </button>
          <Show when={hasTrainedLora()}>
            <span
              class="sheet-panel__lora-status"
              title={`Per-entity LoRA active: ${loraPath() ?? ""}`}
            >
              LoRA trained
            </span>
          </Show>
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
              history={variants()}
              previewId={previewVariant()?.id ?? null}
              onPreview={(v) => setPreviewVariant(v)}
              onApprove={handleApprove}
              onDelete={handleDelete}
            />
          </Show>

          <Show when={activeTab() === "prompts"}>
            <PromptStrip prompts={legacyPrompts()} />
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default SheetView;
