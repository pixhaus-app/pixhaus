// Dedicated AI reference-sheet editor for sprite entities.

import { open as dialogOpen } from "../lib/dialog";
import { type Component, For, Show, createEffect, createMemo, createSignal, on } from "solid-js";
import type { Entity, EntityContent, ReferenceSheet, SheetVariant } from "../lib/types";
import {
  libraryGenerateReferenceSheet,
  libraryGetEntity,
  libraryImportReferenceSheet,
  libraryRemoveReferenceSheetVariant,
  type ImageQuality,
  type ReferenceSheetTemplate,
} from "../lib/commands/library";
import { approveSheetVariantAndRefreshCorpus, refreshLibrary } from "../library/library-state";
import { Button } from "../lib/ui/Button";
import { pushToast } from "../lib/toast/toast-state";
import { reportCommandFailure } from "../lib/utils/errors";
import { useImageObjectUrl } from "../lib/utils/image-object-url";
import {
  activeSheetEditorEntityId,
  closeSheetEditor,
  selectedPanelRegion,
  setSelectedPanelRegion,
  showPanelOverlay,
  setShowPanelOverlay,
} from "./sheet-state";

type SheetEditorTemplateOption = {
  value: ReferenceSheetTemplate;
  label: string;
};

const TEMPLATES: SheetEditorTemplateOption[] = [
  { value: "character", label: "Character" },
  { value: "item", label: "Item" },
  { value: "tileset", label: "Tileset" },
  { value: "custom", label: "Custom" },
];

const QUALITIES: Array<{ value: ImageQuality; label: string }> = [
  { value: "medium", label: "Medium" },
  { value: "low", label: "Low" },
  { value: "high", label: "High" },
  { value: "auto", label: "Auto" },
];

function referenceSheet(entity: Entity | null): ReferenceSheet | null {
  if (entity === null) return null;
  const content = entity.content as EntityContent;
  if (content.type !== "Sprites") return null;
  return content.value.reference_sheet ?? null;
}

function variantLabel(variant: SheetVariant, isCanonical: boolean, index: number): string {
  if (isCanonical) return "Approved";
  const generated = new Date(variant.generated_at * 1000).toLocaleString();
  return `Draft ${index + 1} · ${generated}`;
}

const ReferenceSheetEditor: Component = () => {
  const [entity, setEntity] = createSignal<Entity | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [generating, setGenerating] = createSignal(false);
  const [prompt, setPrompt] = createSignal("");
  const [template, setTemplate] = createSignal<ReferenceSheetTemplate>("character");
  const [quality, setQuality] = createSignal<ImageQuality>("auto");
  const [candidateCount, setCandidateCount] = createSignal(2);
  const [selectedVariantId, setSelectedVariantId] = createSignal<number | null>(null);

  createEffect(
    on(activeSheetEditorEntityId, (entityId) => {
      if (entityId === null) {
        setEntity(null);
        return;
      }
      const requestId = entityId;
      setLoading(true);
      libraryGetEntity(entityId)
        .then((next) => {
          if (activeSheetEditorEntityId() !== requestId) return;
          setEntity(next);
          if (prompt().trim().length === 0) {
            setPrompt(next.name);
          }
        })
        .catch((err: unknown) => {
          if (activeSheetEditorEntityId() !== requestId) return;
          reportCommandFailure("library_get_entity", err);
        })
        .finally(() => {
          if (activeSheetEditorEntityId() !== requestId) return;
          setLoading(false);
        });
    }),
  );

  const sheet = (): ReferenceSheet | null => referenceSheet(entity());
  const canonical = (): SheetVariant | null => sheet()?.canonical ?? null;
  const drafts = (): SheetVariant[] => sheet()?.history ?? [];
  const variants = createMemo(() => {
    const rows: Array<{ variant: SheetVariant; isCanonical: boolean; index: number }> = [];
    const approved = canonical();
    if (approved !== null) rows.push({ variant: approved, isCanonical: true, index: 0 });
    drafts().forEach((variant, index) => rows.push({ variant, isCanonical: false, index }));
    return rows;
  });
  const selectedVariant = (): SheetVariant | null => {
    const selected = selectedVariantId();
    if (selected !== null) {
      const match = variants().find((row) => row.variant.id === selected);
      if (match !== undefined) return match.variant;
    }
    return canonical() ?? drafts()[0] ?? null;
  };
  const selectedIsCanonical = (): boolean => {
    const selected = selectedVariant();
    const approved = canonical();
    return selected !== null && approved !== null && selected.id === approved.id;
  };
  const displayedUrl = useImageObjectUrl(() => selectedVariant()?.image ?? null);

  const allPanels = (): Array<{ label: string; x: number; y: number; w: number; h: number }> => {
    const composition = selectedVariant()?.composition;
    if (composition == null) return [];
    return [
      ...(composition.views ?? []),
      ...(composition.expressions ?? []),
      ...(composition.callouts ?? []),
      ...(composition.outfits ?? []),
    ].map((panel) => ({
      label: panel.label,
      x: panel.region.origin.x,
      y: panel.region.origin.y,
      w: panel.region.size.width,
      h: panel.region.size.height,
    }));
  };
  const sheetWidth = (): number => Math.max(1024, ...allPanels().map((panel) => panel.x + panel.w));
  const sheetHeight = (): number =>
    Math.max(1024, ...allPanels().map((panel) => panel.y + panel.h));

  function isPanelSelected(panel: { x: number; y: number }): boolean {
    const selected = selectedPanelRegion();
    return selected !== null && selected.origin.x === panel.x && selected.origin.y === panel.y;
  }

  function handlePanelClick(panel: { x: number; y: number; w: number; h: number }): void {
    const selected = selectedPanelRegion();
    if (selected !== null && selected.origin.x === panel.x && selected.origin.y === panel.y) {
      setSelectedPanelRegion(null);
      return;
    }
    setSelectedPanelRegion({
      origin: { x: panel.x, y: panel.y },
      size: { width: panel.w, height: panel.h },
    });
  }

  function refreshEntity(next: Entity): void {
    setEntity(next);
    refreshLibrary();
  }

  function handleGenerate(): void {
    const target = entity();
    const text = prompt().trim();
    if (target === null || text.length === 0 || generating()) return;
    setGenerating(true);
    pushToast({ kind: "info", title: "Generating reference sheet…" });
    libraryGenerateReferenceSheet({
      entity_id: target.id,
      prompt: text,
      template: template(),
      quality: quality(),
      candidate_count: candidateCount(),
    })
      .then((updated) => {
        refreshEntity(updated);
        const firstDraft = referenceSheet(updated)?.history?.[0] ?? null;
        setSelectedVariantId(firstDraft?.id ?? null);
        pushToast({ kind: "success", title: "Reference sheet candidates generated." });
      })
      .catch((err: unknown) => reportCommandFailure("library_generate_reference_sheet", err))
      .finally(() => setGenerating(false));
  }

  async function handleImport(): Promise<void> {
    const target = entity();
    if (target === null) return;
    const result = await dialogOpen({
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }],
      multiple: false,
    });
    const path = typeof result === "string" ? result : null;
    if (path === null) return;

    try {
      const { convertFileSrc } = await import("@tauri-apps/api/core");
      const resp = await fetch(convertFileSrc(path));
      if (!resp.ok) throw new Error(`${resp.status} ${resp.statusText}`);
      const bytes = Array.from(new Uint8Array(await resp.arrayBuffer()));
      const ext = path.split(".").pop()?.toLowerCase() ?? "png";
      const mime = ext === "jpg" || ext === "jpeg" ? "image/jpeg" : `image/${ext}`;
      const updated = await libraryImportReferenceSheet({
        entity_id: target.id,
        bytes,
        mime,
      });
      refreshEntity(updated);
      const firstDraft = referenceSheet(updated)?.history?.[0] ?? null;
      setSelectedVariantId(firstDraft?.id ?? null);
    } catch (err) {
      reportCommandFailure("library_import_reference_sheet", err);
    }
  }

  function handleApprove(): void {
    const target = entity();
    const variant = selectedVariant();
    if (target === null || variant === null || selectedIsCanonical()) return;
    approveSheetVariantAndRefreshCorpus(target.id, variant.id)
      .then((updated) => {
        refreshEntity(updated);
        setSelectedVariantId(null);
        pushToast({ kind: "success", title: "Reference sheet approved." });
      })
      .catch((err: unknown) => reportCommandFailure("library_approve_sheet_variant", err));
  }

  function handleRemove(): void {
    const target = entity();
    const variant = selectedVariant();
    if (target === null || variant === null || selectedIsCanonical()) return;
    libraryRemoveReferenceSheetVariant(target.id, variant.id)
      .then((updated) => {
        refreshEntity(updated);
        setSelectedVariantId(null);
      })
      .catch((err: unknown) => reportCommandFailure("library_remove_reference_sheet_variant", err));
  }

  return (
    <div class="sheet-editor" data-testid="reference-sheet-editor">
      <div class="sheet-editor__topbar">
        <div>
          <div class="sheet-editor__eyebrow">Reference Sheet</div>
          <div class="sheet-editor__title">{entity()?.name ?? "Sprite"}</div>
        </div>
        <Button variant="ghost" onClick={closeSheetEditor}>
          Back
        </Button>
      </div>

      <Show when={loading()} fallback={null}>
        <div class="sheet-editor__loading">Loading…</div>
      </Show>

      <Show when={!loading()}>
        <div class="sheet-editor__workspace">
          <section class="sheet-editor__preview">
            <div class="sheet-editor__image-wrap">
              <Show
                when={displayedUrl() !== ""}
                fallback={<div class="sheet-editor__empty">No candidates</div>}
              >
                <img class="sheet-editor__image" src={displayedUrl()} alt="Reference sheet" />
              </Show>
              <Show when={showPanelOverlay() && allPanels().length > 0}>
                <svg
                  class="sheet-editor__overlay"
                  viewBox={`0 0 ${sheetWidth()} ${sheetHeight()}`}
                  preserveAspectRatio="none"
                >
                  <For each={allPanels()}>
                    {(panel) => (
                      <g
                        class="sheet-editor__panel"
                        classList={{ "sheet-editor__panel--selected": isPanelSelected(panel) }}
                        onClick={() => handlePanelClick(panel)}
                      >
                        <rect x={panel.x} y={panel.y} width={panel.w} height={panel.h} />
                        <text x={panel.x + 4} y={panel.y + 14}>
                          {panel.label}
                        </text>
                      </g>
                    )}
                  </For>
                </svg>
              </Show>
            </div>
          </section>

          <aside class="sheet-editor__controls">
            <label class="sheet-editor__field">
              <span>Prompt</span>
              <textarea
                class="sheet-editor__textarea"
                value={prompt()}
                onInput={(event) => setPrompt(event.currentTarget.value)}
              />
            </label>

            <div class="sheet-editor__grid">
              <label class="sheet-editor__field">
                <span>Template</span>
                <select
                  class="sheet-editor__select"
                  value={template()}
                  onChange={(event) =>
                    setTemplate(event.currentTarget.value as ReferenceSheetTemplate)
                  }
                >
                  <For each={TEMPLATES}>
                    {(option) => <option value={option.value}>{option.label}</option>}
                  </For>
                </select>
              </label>

              <label class="sheet-editor__field">
                <span>Quality</span>
                <select
                  class="sheet-editor__select"
                  value={quality()}
                  onChange={(event) => setQuality(event.currentTarget.value as ImageQuality)}
                >
                  <For each={QUALITIES}>
                    {(option) => <option value={option.value}>{option.label}</option>}
                  </For>
                </select>
              </label>

              <label class="sheet-editor__field">
                <span>Candidates</span>
                <select
                  class="sheet-editor__select"
                  value={candidateCount()}
                  onChange={(event) =>
                    setCandidateCount(Math.max(1, Math.min(4, Number(event.currentTarget.value))))
                  }
                >
                  <option value={1}>1</option>
                  <option value={2}>2</option>
                  <option value={3}>3</option>
                  <option value={4}>4</option>
                </select>
              </label>
            </div>

            <div class="sheet-editor__actions">
              <Button onClick={handleGenerate} disabled={generating() || prompt().trim() === ""}>
                {generating() ? "Generating…" : "Generate"}
              </Button>
              <Button variant="ghost" onClick={handleImport}>
                Import
              </Button>
              <Button
                variant="ghost"
                onClick={() => setShowPanelOverlay(!showPanelOverlay())}
                disabled={allPanels().length === 0}
              >
                Overlay
              </Button>
            </div>

            <div class="sheet-editor__variants">
              <For
                each={variants()}
                fallback={<div class="sheet-editor__variant-empty">No variants</div>}
              >
                {(row) => (
                  <button
                    class="sheet-editor__variant"
                    classList={{
                      "sheet-editor__variant--active": selectedVariant()?.id === row.variant.id,
                    }}
                    onClick={() => setSelectedVariantId(row.variant.id)}
                  >
                    {variantLabel(row.variant, row.isCanonical, row.index)}
                  </button>
                )}
              </For>
            </div>

            <div class="sheet-editor__actions">
              <Button
                onClick={handleApprove}
                disabled={selectedVariant() === null || selectedIsCanonical()}
              >
                Approve
              </Button>
              <Button
                variant="ghost"
                onClick={handleRemove}
                disabled={selectedVariant() === null || selectedIsCanonical()}
              >
                Remove
              </Button>
            </div>
          </aside>
        </div>
      </Show>
    </div>
  );
};

export default ReferenceSheetEditor;
