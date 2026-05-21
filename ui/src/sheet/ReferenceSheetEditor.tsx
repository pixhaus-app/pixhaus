// Dedicated AI reference-sheet editor for sprite entities.

import { open as dialogOpen } from "../lib/dialog";
import {
  type Component,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  on,
  onCleanup,
  onMount,
} from "solid-js";
import { listen } from "@tauri-apps/api/event";
import type {
  Entity,
  ModelId,
  Quality,
  ReferenceAsset,
  ReferenceImage,
  ReferenceRole,
  ReferenceSheet,
  ReferenceSheetTemplateDefinition,
  ReferenceSheetTemplateId,
  ReferenceSlot,
  RefinementKind,
  RegionDefinition,
  SheetVariant,
} from "../lib/types";
import {
  libraryGenerateReferenceSheet,
  libraryGetEntity,
  libraryImportReferenceSheet,
  libraryBrowseAssets,
  libraryCancelReferenceSheetRequest,
  libraryExportVariantAsVector,
  libraryListReferenceSheetTemplates,
  libraryPromoteVariantToFinal,
  libraryRefineReferenceSheetVariant,
  libraryRemoveReferenceSheetVariant,
  librarySaveReferenceToLibrary,
  librarySaveVariantAsCharacterCard,
  librarySaveVariantAsStyleSwatch,
  libraryStartCrossModelGrid,
  librarySubmitChatTurn,
  libraryTrainLora,
  type ModelQualityPair,
  type RequestId,
  type ReferenceSheetTemplate,
} from "../lib/commands/library";
import { approveSheetVariantAndRefreshCorpus, refreshLibrary } from "../library/library-state";
import HistoryStrip from "./HistoryStrip";
import { zoomToCursor } from "./preview-zoom";
import { Button } from "../lib/ui/Button";
import { pushToast } from "../lib/toast/toast-state";
import { extractDetail, reportCommandFailure } from "../lib/utils/errors";
import { useImageObjectUrl } from "../lib/utils/image-object-url";
import {
  activeSheetEditorEntityId,
  closeSheetEditor,
  selectedPanelRegion,
  setSelectedPanelRegion,
  showPanelOverlay,
  setShowPanelOverlay,
} from "./sheet-state";
import {
  DEFAULT_SHEET_DIMENSION,
  type FlatPanel,
  flatPanels,
  sheetHeight as computeSheetHeight,
  sheetWidth as computeSheetWidth,
} from "./sheet-panels";
import {
  createSheetEditorState,
  TEMPLATES,
  QUALITIES,
  VIEWS,
  MODEL_OPTIONS,
  QUALITY_OPTIONS,
  ROLE_OPTIONS,
  referenceSheet,
  rgbaToHex,
  hexToRgba,
  dataUri,
  variantLabel,
  type RefineMode,
  type SlotTarget,
  type RegionDraft,
  type SheetRequestCompletePayload,
  type SheetRequestProgressPayload,
  type SheetRequestCancelledPayload,
  type SheetRequestErrorPayload,
} from "./sheet-editor-state";

const ReferenceSheetEditor: Component = () => {
  const {
    entity,
    setEntity,
    loading,
    setLoading,
    generating,
    setGenerating,
    prompt,
    setPrompt,
    template,
    setTemplate,
    candidateCount,
    setCandidateCount,
    selectedVariantId,
    setSelectedVariantId,
    activeView,
    setActiveView,
    templates,
    setTemplates,
    templateId,
    setTemplateId,
    dimensionValue,
    setDimensionValue,
    chromaHex,
    setChromaHex,
    model,
    setModel,
    v1Quality,
    setV1Quality,
    refineMode,
    setRefineMode,
    refinePrompt,
    setRefinePrompt,
    chatMessage,
    setChatMessage,
    comparePrompt,
    setComparePrompt,
    busy,
    setBusy,
    assets,
    setAssets,
    assetName,
    setAssetName,
    referenceSlots,
    setReferenceSlots,
    additionalReferenceSlots,
    setAdditionalReferenceSlots,
    regionReferenceSlots,
    setRegionReferenceSlots,
    activeRequests,
    setActiveRequests,
    assetTab,
    setAssetTab,
    regionDrafts,
    setRegionDrafts,
    loraName,
    setLoraName,
    loraTrigger,
    setLoraTrigger,
    selectedLoraId,
    setSelectedLoraId,
    loraWeight,
    setLoraWeight,
    realWorldGrounding,
    setRealWorldGrounding,
    brushSize,
    setBrushSize,
    maskFeather,
    setMaskFeather,
    maskExpand,
    setMaskExpand,
    maskMode,
    setMaskMode,
  } = createSheetEditorState();
  let maskCanvas: HTMLCanvasElement | undefined;
  let maskDrawing = false;

  // Preview pan/zoom. The transform lives on `.sheet-editor__image-wrap`,
  // which is absolutely anchored to the non-transformed viewport, so the
  // cursor measured against the viewport rect shares the pan coordinate
  // space that `zoomToCursor` expects.
  const [scale, setScale] = createSignal(1);
  const [panX, setPanX] = createSignal(0);
  const [panY, setPanY] = createSignal(0);
  const [spaceHeld, setSpaceHeld] = createSignal(false);
  const [panning, setPanning] = createSignal(false);
  let viewportEl: HTMLDivElement | undefined;
  let panStart: { px: number; py: number; ox: number; oy: number } | null = null;

  function resetView(): void {
    setScale(1);
    setPanX(0);
    setPanY(0);
  }

  // Space toggles pan mode, but it must keep working as the activation key
  // for focused controls (the editor's tabs and actions are <button>s) and
  // as text input in fields, so skip pan mode when a control is focused.
  function isInteractiveTarget(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (el === null) return false;
    return (
      el.tagName === "INPUT" ||
      el.tagName === "TEXTAREA" ||
      el.tagName === "SELECT" ||
      el.tagName === "BUTTON" ||
      el.isContentEditable
    );
  }

  function handleWheel(event: WheelEvent): void {
    if (viewportEl === undefined) return;
    event.preventDefault();
    const rect = viewportEl.getBoundingClientRect();
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    const next = zoomToCursor(
      { scale: scale(), panX: panX(), panY: panY() },
      event.clientX - rect.left,
      event.clientY - rect.top,
      factor,
    );
    setScale(next.scale);
    setPanX(next.panX);
    setPanY(next.panY);
  }

  // Pan only on middle-mouse or space+left so left-drag stays free for
  // mask painting and panel selection, matching the main canvas.
  function handlePreviewMouseDown(event: MouseEvent): void {
    if (event.button === 1 || (event.button === 0 && spaceHeld())) {
      event.preventDefault();
      panStart = { px: event.clientX, py: event.clientY, ox: panX(), oy: panY() };
      setPanning(true);
    }
  }

  function handleWindowMouseMove(event: MouseEvent): void {
    if (panStart === null) return;
    setPanX(panStart.ox + (event.clientX - panStart.px));
    setPanY(panStart.oy + (event.clientY - panStart.py));
  }

  function endPan(): void {
    panStart = null;
    setPanning(false);
  }

  function handleSpaceDown(event: KeyboardEvent): void {
    if (event.code !== "Space" || isInteractiveTarget(event.target)) return;
    // Only entered when no control is focused, so suppressing the default
    // (page scroll) is safe and keeps the pan gesture clean.
    event.preventDefault();
    setSpaceHeld(true);
  }

  function handleSpaceUp(event: KeyboardEvent): void {
    if (event.code !== "Space") return;
    if (spaceHeld()) event.preventDefault();
    setSpaceHeld(false);
  }

  onMount(() => {
    const el = viewportEl;
    el?.addEventListener("wheel", handleWheel, { passive: false });
    window.addEventListener("mousemove", handleWindowMouseMove);
    window.addEventListener("mouseup", endPan);
    window.addEventListener("keydown", handleSpaceDown);
    window.addEventListener("keyup", handleSpaceUp);
    onCleanup(() => {
      el?.removeEventListener("wheel", handleWheel);
      window.removeEventListener("mousemove", handleWindowMouseMove);
      window.removeEventListener("mouseup", endPan);
      window.removeEventListener("keydown", handleSpaceDown);
      window.removeEventListener("keyup", handleSpaceUp);
    });
  });

  onMount(() => {
    libraryListReferenceSheetTemplates()
      .then((defs) => {
        setTemplates(defs);
        const first = defs[0];
        if (first !== undefined) {
          setTemplateId(first.id);
          setDimensionValue(`${first.default_dimensions.width}x${first.default_dimensions.height}`);
          setChromaHex(rgbaToHex(first.default_chroma));
        }
      })
      .catch((err: unknown) => reportCommandFailure("library_list_reference_sheet_templates", err));

    libraryBrowseAssets()
      .then(setAssets)
      .catch((err: unknown) => reportCommandFailure("library_browse_assets", err));

    const progressListener = listen<SheetRequestProgressPayload>(
      "SheetRequestProgress",
      (event) => {
        setActiveRequests((rows) =>
          rows.map((row) =>
            row.id === event.payload.request_id
              ? {
                  ...row,
                  status: "running",
                  streamIndex: event.payload.stream_index,
                  candidateIndex: event.payload.candidate_index,
                  partialIndex: event.payload.partial_index,
                  partialImage: event.payload.partial_image ?? row.partialImage,
                  elapsedMs: event.payload.elapsed_ms,
                }
              : row,
          ),
        );
      },
    );
    const completeListener = listen<SheetRequestCompletePayload>(
      "SheetRequestComplete",
      (event) => {
        const activeId = activeSheetEditorEntityId();
        if (activeId === null || event.payload.entity_id !== activeId) return;
        refreshEntity(event.payload.sprite);
        setGenerating(false);
        setBusy(false);
        setActiveRequests((rows) =>
          rows.map((row) =>
            row.id === event.payload.request_id ? { ...row, status: "complete" } : row,
          ),
        );
        const firstDraft = referenceSheet(event.payload.sprite)?.variants?.[0] ?? null;
        setSelectedVariantId(firstDraft?.id ?? selectedVariantId());
        pushToast({ kind: "success", title: "Reference sheet request complete." });
      },
    );
    const errorListener = listen<SheetRequestErrorPayload>("SheetRequestError", (event) => {
      setGenerating(false);
      setBusy(false);
      const message = extractDetail(event.payload.error);
      setActiveRequests((rows) =>
        rows.map((row) =>
          row.id === event.payload.request_id ? { ...row, status: "error", error: message } : row,
        ),
      );
      pushToast({ kind: "error", title: message });
    });
    const cancelListener = listen<SheetRequestCancelledPayload>(
      "SheetRequestCancelled",
      (event) => {
        setGenerating(false);
        setBusy(false);
        setActiveRequests((rows) =>
          rows.map((row) =>
            row.id === event.payload.request_id ? { ...row, status: "cancelled" } : row,
          ),
        );
        pushToast({ kind: "info", title: "Reference sheet request cancelled." });
      },
    );
    onCleanup(() => {
      void progressListener.then((unlisten) => unlisten());
      void completeListener.then((unlisten) => unlisten());
      void errorListener.then((unlisten) => unlisten());
      void cancelListener.then((unlisten) => unlisten());
    });
  });

  createEffect(
    on(activeSheetEditorEntityId, (entityId) => {
      resetView();
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
          if (comparePrompt().trim().length === 0) setComparePrompt(next.name);
          if (assetName().trim().length === 0) setAssetName(next.name);
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
  const drafts = (): SheetVariant[] => sheet()?.variants ?? [];
  const templateDefinition = (): ReferenceSheetTemplateDefinition | null =>
    templates().find((def) => def.id === templateId()) ?? null;
  const dimensionOptions = () => templateDefinition()?.allowed_dimensions ?? [];
  const selectedDimensions = (): { width: number; height: number } => {
    const parts = dimensionValue()
      .split("x")
      .map((value) => Number(value));
    const width = parts[0];
    const height = parts[1];
    return {
      width: typeof width === "number" && Number.isFinite(width) ? width : 2048,
      height: typeof height === "number" && Number.isFinite(height) ? height : 1024,
    };
  };
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

  createEffect(
    on(templateId, () => {
      const def = templateDefinition();
      if (def === null) return;
      setDimensionValue(`${def.default_dimensions.width}x${def.default_dimensions.height}`);
      setChromaHex(rgbaToHex(def.default_chroma));
    }),
  );

  const allPanels = (): FlatPanel[] => flatPanels(selectedVariant()?.composition);
  const sheetWidth = (): number =>
    computeSheetWidth(selectedVariant()?.composition, {
      floor: DEFAULT_SHEET_DIMENSION,
      fallback: DEFAULT_SHEET_DIMENSION,
    });
  const sheetHeight = (): number =>
    computeSheetHeight(selectedVariant()?.composition, {
      floor: DEFAULT_SHEET_DIMENSION,
      fallback: DEFAULT_SHEET_DIMENSION,
    });

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

  async function refreshActiveEntity(): Promise<void> {
    const target = entity();
    if (target === null) return;
    const updated = await libraryGetEntity(target.id);
    refreshEntity(updated);
  }

  function trackRequest(id: RequestId, label: string): void {
    setActiveRequests((rows) => [
      {
        id,
        label,
        streamIndex: 0,
        candidateIndex: 0,
        partialIndex: 0,
        elapsedMs: 0,
        partialImage: null,
        status: "queued",
      },
      ...rows.filter((row) => row.id !== id).slice(0, 7),
    ]);
  }

  async function queueSheetRequest(label: string, action: () => Promise<RequestId>): Promise<void> {
    if (busy()) return;
    setBusy(true);
    try {
      const id = await action();
      trackRequest(id, label);
      pushToast({ kind: "info", title: `${label} queued.` });
    } catch (err) {
      reportCommandFailure(label, err);
    } finally {
      setBusy(false);
      setGenerating(false);
    }
  }

  function slotSetter(target: SlotTarget) {
    if (target === "refine") return setAdditionalReferenceSlots;
    if (target === "region") return setRegionReferenceSlots;
    return setReferenceSlots;
  }

  function addReferenceSlot(slot: ReferenceSlot, target: SlotTarget = "generate"): void {
    slotSetter(target)((rows) => [...rows, slot].slice(0, 16));
  }

  function updateSlot(
    index: number,
    patch: Partial<Pick<ReferenceSlot, "role" | "weight">>,
    target: SlotTarget = "generate",
  ): void {
    slotSetter(target)((rows) =>
      rows.map((slot, i) => (i === index ? { ...slot, ...patch } : slot)),
    );
  }

  function removeSlot(index: number, target: SlotTarget = "generate"): void {
    slotSetter(target)((rows) => rows.filter((_, i) => i !== index));
  }

  function moveSlot(index: number, direction: -1 | 1, target: SlotTarget = "generate"): void {
    slotSetter(target)((rows) => {
      const next = [...rows];
      const swap = index + direction;
      if (swap < 0 || swap >= next.length) return rows;
      const current = next[index];
      const other = next[swap];
      if (current === undefined || other === undefined) return rows;
      next[index] = other;
      next[swap] = current;
      return next;
    });
  }

  async function handleReferenceDrop(
    event: DragEvent,
    target: SlotTarget = "generate",
  ): Promise<void> {
    event.preventDefault();
    const assetPayload = event.dataTransfer?.getData("application/x-pixhaus-reference");
    if (assetPayload) {
      let asset: ReferenceAsset;
      try {
        asset = JSON.parse(assetPayload) as ReferenceAsset;
      } catch {
        pushToast({ kind: "error", title: "Invalid reference payload." });
        return;
      }
      addReferenceSlot(
        {
          image: asset.image,
          role: asset.default_role,
          weight: 1,
          source_asset_id: asset.id,
        },
        target,
      );
      return;
    }
    const files = Array.from(event.dataTransfer?.files ?? []);
    for (const file of files) {
      if (!file.type.startsWith("image/")) continue;
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      addReferenceSlot(
        {
          image: { bytes, mime: file.type || "image/png" },
          role: "generic",
          weight: 1,
          source_asset_id: null,
        },
        target,
      );
    }
  }

  function clearMask(): void {
    if (!maskCanvas) return;
    const ctx = maskCanvas.getContext("2d");
    ctx?.clearRect(0, 0, maskCanvas.width, maskCanvas.height);
  }

  function fillMask(): void {
    if (!maskCanvas) return;
    const ctx = maskCanvas.getContext("2d");
    if (ctx === null) return;
    ctx.fillStyle = "rgba(255,255,255,1)";
    ctx.fillRect(0, 0, maskCanvas.width, maskCanvas.height);
  }

  function drawMaskAt(event: PointerEvent): void {
    if (!maskCanvas) return;
    const rect = maskCanvas.getBoundingClientRect();
    const ctx = maskCanvas.getContext("2d");
    if (ctx === null) return;
    const x = ((event.clientX - rect.left) / rect.width) * maskCanvas.width;
    const y = ((event.clientY - rect.top) / rect.height) * maskCanvas.height;
    ctx.globalCompositeOperation = maskMode() === "erase" ? "destination-out" : "source-over";
    ctx.fillStyle = "rgba(255,255,255,0.75)";
    ctx.beginPath();
    ctx.arc(x, y, brushSize() / 2, 0, Math.PI * 2);
    ctx.fill();
  }

  function maskImage(): ReferenceImage {
    const source = selectedVariant();
    if (!maskCanvas || source === null) return { bytes: [], mime: "image/png" };
    const base = document.createElement("canvas");
    base.width = source.width;
    base.height = source.height;
    const baseCtx = base.getContext("2d");
    if (baseCtx !== null) {
      baseCtx.drawImage(maskCanvas, 0, 0, source.width, source.height);
    }

    let output = base;
    const expand = maskExpand();
    if (expand > 0) {
      const expanded = document.createElement("canvas");
      expanded.width = source.width;
      expanded.height = source.height;
      const ctx = expanded.getContext("2d");
      if (ctx !== null) {
        ctx.globalCompositeOperation = "source-over";
        ctx.drawImage(base, 0, 0);
        const step = Math.max(1, Math.floor(expand / 8));
        for (let dx = -expand; dx <= expand; dx += step) {
          for (let dy = -expand; dy <= expand; dy += step) {
            if (dx * dx + dy * dy <= expand * expand) ctx.drawImage(base, dx, dy);
          }
        }
      }
      output = expanded;
    }

    const feather = maskFeather();
    if (feather > 0) {
      const feathered = document.createElement("canvas");
      feathered.width = source.width;
      feathered.height = source.height;
      const ctx = feathered.getContext("2d");
      if (ctx !== null) {
        ctx.filter = `blur(${feather}px)`;
        ctx.drawImage(output, 0, 0);
      }
      output = feathered;
    }

    const dataUrl = output.toDataURL("image/png");
    const raw = atob(dataUrl.split(",")[1] ?? "");
    return {
      bytes: Array.from(raw, (char) => char.charCodeAt(0)),
      mime: "image/png",
    };
  }

  function addRegionFromSelection(): void {
    const selected = selectedPanelRegion();
    if (selected === null) return;
    const polygon = [
      selected.origin,
      { x: selected.origin.x + selected.size.width, y: selected.origin.y },
      { x: selected.origin.x + selected.size.width, y: selected.origin.y + selected.size.height },
      { x: selected.origin.x, y: selected.origin.y + selected.size.height },
    ];
    const next: RegionDraft = {
      id: Date.now(),
      polygon,
      prompt: refinePrompt(),
      region_references: regionReferenceSlots(),
    };
    if (regionOverlaps(next, regionDrafts())) {
      pushToast({ kind: "error", title: "Regional refinement areas cannot overlap." });
      return;
    }
    setRegionDrafts((rows) => [...rows, next]);
  }

  function regionOverlaps(region: RegionDraft, regions: RegionDraft[]): boolean {
    const bounds = regionBounds(region);
    return regions.some((other) => rectsOverlap(bounds, regionBounds(other)));
  }

  function regionBounds(region: RegionDefinition): [number, number, number, number] {
    const polygon = region.polygon ?? [];
    const xs = polygon.map((point) => point.x);
    const ys = polygon.map((point) => point.y);
    return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  }

  function rectsOverlap(
    a: [number, number, number, number],
    b: [number, number, number, number],
  ): boolean {
    return a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1];
  }

  function handleGenerate(): void {
    const target = entity();
    const text = prompt().trim();
    if (target === null || text.length === 0 || generating()) return;
    setGenerating(true);
    void queueSheetRequest("Reference sheet generation", () =>
      libraryGenerateReferenceSheet({
        entity_id: target.id,
        prompt: text,
        template: templateId(),
        width: selectedDimensions().width,
        height: selectedDimensions().height,
        chroma_color: hexToRgba(chromaHex()),
        quality: v1Quality(),
        candidate_count: candidateCount(),
        model: model(),
        references: referenceSlots(),
        real_world_grounding: realWorldGrounding(),
        applied_lora: selectedLoraId(),
        lora_weight: loraWeight(),
      }),
    );
  }

  function currentRefinement(): RefinementKind {
    if (refineMode() === "masked") {
      return { kind: "masked", value: { mask_png: maskImage() } };
    }
    if (refineMode() === "regional") {
      return {
        kind: "regional",
        value: { regions: regionDrafts().map(({ id: _id, ...region }) => region) },
      };
    }
    return { kind: "prompt_only" };
  }

  async function runRequest(label: string, action: () => Promise<unknown>): Promise<void> {
    if (busy()) return;
    setBusy(true);
    try {
      await action();
      await refreshActiveEntity();
      await libraryBrowseAssets()
        .then(setAssets)
        .catch(() => undefined);
      pushToast({ kind: "success", title: label });
    } catch (err) {
      reportCommandFailure(label, err);
    } finally {
      setBusy(false);
    }
  }

  function handleRefine(): void {
    const target = entity();
    const variant = selectedVariant();
    const text = refinePrompt().trim();
    if (target === null || variant === null || text.length === 0) return;
    if (refineMode() === "regional" && regionDrafts().length === 0) {
      pushToast({ kind: "error", title: "Add at least one regional refinement area." });
      return;
    }
    void queueSheetRequest("Reference refinement", () =>
      libraryRefineReferenceSheetVariant({
        entity_id: target.id,
        parent_variant_id: variant.id,
        refinement: currentRefinement(),
        prompt: text,
        quality: v1Quality(),
        candidate_count: candidateCount(),
        model: model(),
        additional_references: additionalReferenceSlots(),
      }),
    );
  }

  function handleChatTurn(): void {
    const target = entity();
    const variant = selectedVariant();
    const message = chatMessage().trim();
    if (target === null || variant === null || message.length === 0) return;
    void queueSheetRequest("Chat turn", async () => {
      const id = await librarySubmitChatTurn({
        entity_id: target.id,
        variant_id: variant.id,
        user_message: message,
        mask: refineMode() === "masked" ? maskImage() : null,
        model: model(),
      });
      setChatMessage("");
      return id;
    });
  }

  function handlePromote(): void {
    const target = entity();
    const variant = selectedVariant();
    if (target === null || variant === null) return;
    void queueSheetRequest("Promotion draft", () =>
      libraryPromoteVariantToFinal({
        entity_id: target.id,
        source_variant_id: variant.id,
        target_quality: "high",
        target_model: model(),
      }),
    );
  }

  function handleCompare(): void {
    const target = entity();
    const text = comparePrompt().trim();
    if (target === null || text.length === 0) return;
    const dims = selectedDimensions();
    const combinations: ModelQualityPair[] = [
      { model: "open_ai_gpt_image2", quality: "high" },
      { model: "google_nano_banana_pro", quality: v1Quality() },
    ];
    void queueSheetRequest("Cross-model grid", () =>
      libraryStartCrossModelGrid({
        entity_id: target.id,
        prompt: text,
        template: templateId(),
        width: dims.width,
        height: dims.height,
        chroma_color: hexToRgba(chromaHex()),
        references: referenceSlots(),
        combinations,
        candidate_count_per_combo: 1,
      }),
    );
  }

  function handleVectorExport(): void {
    const target = entity();
    const variant = selectedVariant();
    if (target === null || variant === null) return;
    void queueSheetRequest("Vector export", () =>
      libraryExportVariantAsVector(target.id, variant.id),
    );
  }

  function handleCancelRequest(id: RequestId): void {
    libraryCancelReferenceSheetRequest(id).catch((err: unknown) =>
      reportCommandFailure("library_cancel_reference_sheet_request", err),
    );
  }

  function handleSaveReference(): void {
    const variant = selectedVariant();
    if (variant === null) return;
    void runRequest("Reference saved to asset library", () =>
      librarySaveReferenceToLibrary({
        image: variant.image,
        role: "subject",
        tags: [entity()?.name ?? "reference"].filter((tag) => tag.trim().length > 0),
      }),
    );
  }

  function handleSaveCard(kind: "character" | "style"): void {
    const target = entity();
    const variant = selectedVariant();
    const name = assetName().trim() || target?.name || "Untitled";
    if (target === null || variant === null) return;
    void runRequest(kind === "character" ? "Character card saved" : "Style swatch saved", () =>
      kind === "character"
        ? librarySaveVariantAsCharacterCard({
            entity_id: target.id,
            variant_id: variant.id,
            name,
            style_notes: prompt(),
          })
        : librarySaveVariantAsStyleSwatch({
            entity_id: target.id,
            variant_id: variant.id,
            name,
            style_notes: prompt(),
          }),
    );
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
      const firstDraft = referenceSheet(updated)?.variants?.[0] ?? null;
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

  // Adapters for the candidate thumbnail strip. HistoryStrip uses
  // `previewId === null` to mean "show canonical"; the editor's
  // `selectedVariantId` falls back to canonical, so translate both ways.
  const stripPreviewId = (): number | null => {
    const sel = selectedVariant();
    if (sel === null) return null;
    return selectedIsCanonical() ? null : sel.id;
  };

  function handleStripPreview(variant: SheetVariant | null): void {
    setSelectedVariantId(variant?.id ?? canonical()?.id ?? null);
  }

  function approveVariant(variant: SheetVariant): void {
    const target = entity();
    if (target === null) return;
    approveSheetVariantAndRefreshCorpus(target.id, variant.id)
      .then((updated) => {
        refreshEntity(updated);
        setSelectedVariantId(null);
        pushToast({ kind: "success", title: "Reference sheet approved." });
      })
      .catch((err: unknown) => reportCommandFailure("library_approve_sheet_variant", err));
  }

  function removeVariant(variant: SheetVariant): void {
    const target = entity();
    if (target === null) return;
    libraryRemoveReferenceSheetVariant(target.id, variant.id)
      .then((updated) => {
        refreshEntity(updated);
        setSelectedVariantId(null);
      })
      .catch((err: unknown) => reportCommandFailure("library_remove_reference_sheet_variant", err));
  }

  function ReferenceSlotRow(props: {
    slot: ReferenceSlot;
    index: number;
    target: SlotTarget;
    compact?: boolean;
  }) {
    return (
      <div class="sheet-editor__ref-slot">
        <img src={dataUri(props.slot.image)} alt="" />
        <select
          value={props.slot.role}
          onChange={(event) =>
            updateSlot(
              props.index,
              { role: event.currentTarget.value as ReferenceRole },
              props.target,
            )
          }
        >
          <For each={ROLE_OPTIONS}>
            {(option) => <option value={option.value}>{option.label}</option>}
          </For>
        </select>
        <Show when={!props.compact}>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={props.slot.weight}
            onInput={(event) =>
              updateSlot(props.index, { weight: Number(event.currentTarget.value) }, props.target)
            }
          />
          <Button variant="ghost" size="sm" onClick={() => moveSlot(props.index, -1, props.target)}>
            Up
          </Button>
          <Button variant="ghost" size="sm" onClick={() => moveSlot(props.index, 1, props.target)}>
            Down
          </Button>
        </Show>
        <Button variant="ghost" size="sm" onClick={() => removeSlot(props.index, props.target)}>
          Remove
        </Button>
      </div>
    );
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
        <Show when={activeRequests().length > 0}>
          <div class="sheet-editor__request-strip">
            <For each={activeRequests()}>
              {(request) => (
                <div class="sheet-editor__request">
                  <div class="sheet-editor__request-main">
                    <span>{request.label}</span>
                    <span>{request.status}</span>
                    <span>{Math.round(request.elapsedMs / 1000)}s</span>
                    <span>
                      stream {request.streamIndex + 1} · candidate {request.candidateIndex + 1}
                    </span>
                  </div>
                  <Show when={request.partialImage !== null}>
                    <img
                      class="sheet-editor__request-preview"
                      src={dataUri(request.partialImage as ReferenceImage)}
                      alt=""
                    />
                  </Show>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleCancelRequest(request.id)}
                    disabled={!["queued", "running"].includes(request.status)}
                  >
                    Cancel
                  </Button>
                </div>
              )}
            </For>
          </div>
        </Show>
        <div class="sheet-editor__workspace">
          <section class="sheet-editor__preview">
            <div
              class="sheet-editor__viewport"
              classList={{
                "sheet-editor__viewport--grab": spaceHeld() && !panning(),
                "sheet-editor__viewport--grabbing": panning(),
              }}
              ref={(el) => (viewportEl = el)}
              onMouseDown={handlePreviewMouseDown}
              onDblClick={resetView}
            >
              <div
                class="sheet-editor__image-wrap"
                style={{ transform: `translate(${panX()}px, ${panY()}px) scale(${scale()})` }}
              >
                <Show
                  when={displayedUrl() !== ""}
                  fallback={<div class="sheet-editor__empty">No candidates</div>}
                >
                  <img class="sheet-editor__image" src={displayedUrl()} alt="Reference sheet" />
                </Show>
                <Show
                  when={
                    activeView() === "refine" &&
                    refineMode() === "masked" &&
                    selectedVariant() !== null
                  }
                >
                  <canvas
                    ref={maskCanvas}
                    class="sheet-editor__mask-canvas"
                    width={selectedVariant()?.width ?? 1024}
                    height={selectedVariant()?.height ?? 1024}
                    onPointerDown={(event) => {
                      if (spaceHeld() || event.button !== 0) return;
                      maskDrawing = true;
                      drawMaskAt(event);
                    }}
                    onPointerMove={(event) => {
                      if (maskDrawing && !spaceHeld() && !panning()) drawMaskAt(event);
                    }}
                    onPointerUp={() => {
                      maskDrawing = false;
                    }}
                    onPointerLeave={() => {
                      maskDrawing = false;
                    }}
                  />
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
                <Show
                  when={
                    activeView() === "refine" &&
                    refineMode() === "regional" &&
                    regionDrafts().length > 0
                  }
                >
                  <svg
                    class="sheet-editor__overlay sheet-editor__overlay--regions"
                    viewBox={`0 0 ${sheetWidth()} ${sheetHeight()}`}
                    preserveAspectRatio="none"
                  >
                    <For each={regionDrafts()}>
                      {(region) => {
                        const [x0, y0, x1, y1] = regionBounds(region);
                        return <rect x={x0} y={y0} width={x1 - x0} height={y1 - y0} />;
                      }}
                    </For>
                  </svg>
                </Show>
              </div>
            </div>
            <Show when={variants().length > 1}>
              <HistoryStrip
                canonical={canonical()}
                history={drafts()}
                previewId={stripPreviewId()}
                onPreview={handleStripPreview}
                onApprove={approveVariant}
                onDelete={removeVariant}
              />
            </Show>
          </section>

          <aside class="sheet-editor__controls">
            <div class="sheet-editor__tabs">
              <For each={VIEWS}>
                {(view) => (
                  <button
                    class="sheet-editor__tab"
                    classList={{ "sheet-editor__tab--active": activeView() === view.value }}
                    onClick={() => setActiveView(view.value)}
                  >
                    {view.label}
                  </button>
                )}
              </For>
            </div>

            <Show when={activeView() === "generate"}>
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
                  <span>Legacy template</span>
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
                  <span>V1 template</span>
                  <select
                    class="sheet-editor__select"
                    value={templateId()}
                    onChange={(event) =>
                      setTemplateId(event.currentTarget.value as ReferenceSheetTemplateId)
                    }
                  >
                    <For each={templates()}>
                      {(option) => <option value={option.id}>{option.label}</option>}
                    </For>
                  </select>
                </label>

                <label class="sheet-editor__field">
                  <span>Dimensions</span>
                  <select
                    class="sheet-editor__select"
                    value={dimensionValue()}
                    onChange={(event) => setDimensionValue(event.currentTarget.value)}
                  >
                    <For each={dimensionOptions()}>
                      {(dims) => (
                        <option value={`${dims.width}x${dims.height}`}>
                          {dims.width}x{dims.height}
                        </option>
                      )}
                    </For>
                  </select>
                </label>

                <label class="sheet-editor__field">
                  <span>Chroma</span>
                  <input
                    class="sheet-editor__input"
                    type="color"
                    value={chromaHex()}
                    onInput={(event) => setChromaHex(event.currentTarget.value)}
                  />
                </label>

                <label class="sheet-editor__field">
                  <span>Model</span>
                  <select
                    class="sheet-editor__select"
                    value={model()}
                    onChange={(event) => setModel(event.currentTarget.value as ModelId)}
                  >
                    <For each={MODEL_OPTIONS}>
                      {(option) => <option value={option.value}>{option.label}</option>}
                    </For>
                  </select>
                </label>

                <label class="sheet-editor__field">
                  <span>Quality</span>
                  <select
                    class="sheet-editor__select"
                    value={v1Quality()}
                    onChange={(event) => setV1Quality(event.currentTarget.value as Quality)}
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

              <label class="sheet-editor__field sheet-editor__toggle">
                <input
                  type="checkbox"
                  checked={realWorldGrounding()}
                  disabled={
                    !["google_nano_banana_pro", "google_gemini_flash_image"].includes(model())
                  }
                  onChange={(event) => setRealWorldGrounding(event.currentTarget.checked)}
                />
                <span>Real-world grounding</span>
              </label>

              <Show when={model().startsWith("fal_")}>
                <div class="sheet-editor__grid">
                  <label class="sheet-editor__field">
                    <span>LoRA</span>
                    <select
                      class="sheet-editor__select"
                      value={selectedLoraId() ?? ""}
                      onChange={(event) =>
                        setSelectedLoraId(
                          event.currentTarget.value === ""
                            ? null
                            : Number(event.currentTarget.value),
                        )
                      }
                    >
                      <option value="">None</option>
                      <For each={assets()?.loras ?? []}>
                        {(asset) => <option value={asset.id}>{asset.name}</option>}
                      </For>
                    </select>
                  </label>
                  <label class="sheet-editor__field">
                    <span>LoRA weight {loraWeight().toFixed(1)}</span>
                    <input
                      type="range"
                      min="0"
                      max="2"
                      step="0.1"
                      value={loraWeight()}
                      onInput={(event) => setLoraWeight(Number(event.currentTarget.value))}
                    />
                  </label>
                </div>
              </Show>

              <div
                class="sheet-editor__dropzone"
                onDragOver={(event) => event.preventDefault()}
                onDrop={(event) => void handleReferenceDrop(event)}
              >
                <span>References</span>
                <For each={referenceSlots()}>
                  {(slot, index) => (
                    <ReferenceSlotRow slot={slot} index={index()} target="generate" />
                  )}
                </For>
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
            </Show>

            <Show when={activeView() === "browse"}>
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
                  onClick={handlePromote}
                  disabled={selectedVariant() === null || busy()}
                >
                  Promote
                </Button>
                <Button
                  variant="ghost"
                  onClick={handleRemove}
                  disabled={selectedVariant() === null || selectedIsCanonical()}
                >
                  Remove
                </Button>
              </div>
            </Show>

            <Show when={activeView() === "refine"}>
              <div class="sheet-editor__segmented">
                <For each={["masked", "prompt_only", "regional"] as RefineMode[]}>
                  {(mode) => (
                    <button
                      class="sheet-editor__segment"
                      classList={{ "sheet-editor__segment--active": refineMode() === mode }}
                      onClick={() => setRefineMode(mode)}
                    >
                      {mode === "prompt_only"
                        ? "Prompt"
                        : mode === "masked"
                          ? "Masked"
                          : "Regional"}
                    </button>
                  )}
                </For>
              </div>
              <label class="sheet-editor__field">
                <span>Refinement prompt</span>
                <textarea
                  class="sheet-editor__textarea"
                  value={refinePrompt()}
                  onInput={(event) => setRefinePrompt(event.currentTarget.value)}
                />
              </label>
              <div class="sheet-editor__grid">
                <label class="sheet-editor__field">
                  <span>Model</span>
                  <select
                    class="sheet-editor__select"
                    value={model()}
                    onChange={(event) => setModel(event.currentTarget.value as ModelId)}
                  >
                    <For each={MODEL_OPTIONS}>
                      {(option) => <option value={option.value}>{option.label}</option>}
                    </For>
                  </select>
                </label>
                <label class="sheet-editor__field">
                  <span>Quality</span>
                  <select
                    class="sheet-editor__select"
                    value={v1Quality()}
                    onChange={(event) => setV1Quality(event.currentTarget.value as Quality)}
                  >
                    <For each={QUALITY_OPTIONS}>
                      {(option) => <option value={option.value}>{option.label}</option>}
                    </For>
                  </select>
                </label>
              </div>
              <Show when={refineMode() === "masked"}>
                <div class="sheet-editor__tool-row">
                  <Button
                    variant={maskMode() === "brush" ? "primary" : "ghost"}
                    size="sm"
                    onClick={() => setMaskMode("brush")}
                  >
                    Brush
                  </Button>
                  <Button
                    variant={maskMode() === "erase" ? "primary" : "ghost"}
                    size="sm"
                    onClick={() => setMaskMode("erase")}
                  >
                    Eraser
                  </Button>
                  <Button variant="ghost" size="sm" onClick={fillMask}>
                    Fill
                  </Button>
                  <Button variant="ghost" size="sm" onClick={clearMask}>
                    Clear
                  </Button>
                </div>
                <div class="sheet-editor__grid">
                  <label class="sheet-editor__field">
                    <span>Brush {brushSize()}px</span>
                    <input
                      type="range"
                      min="4"
                      max="160"
                      value={brushSize()}
                      onInput={(event) => setBrushSize(Number(event.currentTarget.value))}
                    />
                  </label>
                  <label class="sheet-editor__field">
                    <span>Feather {maskFeather()}px</span>
                    <input
                      type="range"
                      min="0"
                      max="24"
                      value={maskFeather()}
                      onInput={(event) => setMaskFeather(Number(event.currentTarget.value))}
                    />
                  </label>
                  <label class="sheet-editor__field">
                    <span>Expand {maskExpand()}px</span>
                    <input
                      type="range"
                      min="0"
                      max="64"
                      value={maskExpand()}
                      onInput={(event) => setMaskExpand(Number(event.currentTarget.value))}
                    />
                  </label>
                </div>
              </Show>
              <Show when={refineMode() === "regional"}>
                <div class="sheet-editor__tool-row">
                  <Button variant="ghost" size="sm" onClick={addRegionFromSelection}>
                    Add selected panel
                  </Button>
                  <Button variant="ghost" size="sm" onClick={() => setRegionDrafts([])}>
                    Clear regions
                  </Button>
                </div>
                <div
                  class="sheet-editor__dropzone"
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) => void handleReferenceDrop(event, "region")}
                >
                  <span>Region references</span>
                  <For each={regionReferenceSlots()}>
                    {(slot, index) => (
                      <ReferenceSlotRow slot={slot} index={index()} target="region" compact />
                    )}
                  </For>
                </div>
                <div class="sheet-editor__meta">Regions: {regionDrafts().length}</div>
              </Show>
              <div
                class="sheet-editor__dropzone"
                onDragOver={(event) => event.preventDefault()}
                onDrop={(event) => void handleReferenceDrop(event, "refine")}
              >
                <span>Additional references</span>
                <For each={additionalReferenceSlots()}>
                  {(slot, index) => (
                    <ReferenceSlotRow slot={slot} index={index()} target="refine" compact />
                  )}
                </For>
              </div>
              <Button
                onClick={handleRefine}
                disabled={selectedVariant() === null || refinePrompt().trim() === "" || busy()}
              >
                Refine
              </Button>
            </Show>

            <Show when={activeView() === "chat"}>
              <div class="sheet-editor__meta">
                Turns: {selectedVariant()?.chat_transcript?.turns?.length ?? 0}
              </div>
              <div class="sheet-editor__asset-list">
                <For each={selectedVariant()?.chat_transcript?.turns ?? []}>
                  {(turn, index) => (
                    <button
                      class="sheet-editor__asset-row"
                      onClick={() => {
                        setChatMessage(turn.user_message);
                        setActiveView("chat");
                      }}
                    >
                      {index() + 1}. {turn.user_message}
                    </button>
                  )}
                </For>
              </div>
              <label class="sheet-editor__field">
                <span>Message</span>
                <textarea
                  class="sheet-editor__textarea"
                  value={chatMessage()}
                  onInput={(event) => setChatMessage(event.currentTarget.value)}
                />
              </label>
              <Button
                onClick={handleChatTurn}
                disabled={selectedVariant() === null || chatMessage().trim() === "" || busy()}
              >
                Send turn
              </Button>
            </Show>

            <Show when={activeView() === "compare"}>
              <label class="sheet-editor__field">
                <span>Compare prompt</span>
                <textarea
                  class="sheet-editor__textarea"
                  value={comparePrompt()}
                  onInput={(event) => setComparePrompt(event.currentTarget.value)}
                />
              </label>
              <Button onClick={handleCompare} disabled={comparePrompt().trim() === "" || busy()}>
                Compare across models
              </Button>
            </Show>

            <Show when={activeView() === "assets"}>
              <div class="sheet-editor__segmented">
                <For each={["references", "cards", "styles", "loras", "notes"] as const}>
                  {(tab) => (
                    <button
                      class="sheet-editor__segment"
                      classList={{ "sheet-editor__segment--active": assetTab() === tab }}
                      onClick={() => setAssetTab(tab)}
                    >
                      {tab}
                    </button>
                  )}
                </For>
              </div>
              <label class="sheet-editor__field">
                <span>Asset name</span>
                <input
                  class="sheet-editor__input"
                  value={assetName()}
                  onInput={(event) => setAssetName(event.currentTarget.value)}
                />
              </label>
              <div class="sheet-editor__actions">
                <Button
                  variant="ghost"
                  onClick={handleSaveReference}
                  disabled={selectedVariant() === null || busy()}
                >
                  Save ref
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => handleSaveCard("character")}
                  disabled={selectedVariant() === null || busy()}
                >
                  Character card
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => handleSaveCard("style")}
                  disabled={selectedVariant() === null || busy()}
                >
                  Style swatch
                </Button>
              </div>
              <Show when={assetTab() === "references"}>
                <div class="sheet-editor__asset-grid">
                  <For each={assets()?.references ?? []}>
                    {(asset) => (
                      <button
                        class="sheet-editor__asset-card"
                        draggable
                        onDragStart={(event) =>
                          event.dataTransfer?.setData(
                            "application/x-pixhaus-reference",
                            JSON.stringify(asset),
                          )
                        }
                        onClick={() =>
                          addReferenceSlot({
                            image: asset.image,
                            role: asset.default_role,
                            weight: 1,
                            source_asset_id: asset.id,
                          })
                        }
                      >
                        <img src={dataUri(asset.image)} alt="" />
                        <span>{asset.default_role}</span>
                      </button>
                    )}
                  </For>
                </div>
              </Show>
              <Show when={assetTab() === "cards"}>
                <div class="sheet-editor__asset-list">
                  <For each={assets()?.character_cards ?? []}>
                    {(card) => <div class="sheet-editor__asset-row">{card.name}</div>}
                  </For>
                </div>
              </Show>
              <Show when={assetTab() === "styles"}>
                <div class="sheet-editor__asset-list">
                  <For each={assets()?.style_swatches ?? []}>
                    {(swatch) => <div class="sheet-editor__asset-row">{swatch.name}</div>}
                  </For>
                </div>
              </Show>
              <Show when={assetTab() === "loras"}>
                <div class="sheet-editor__grid">
                  <label class="sheet-editor__field">
                    <span>LoRA name</span>
                    <input
                      class="sheet-editor__input"
                      value={loraName()}
                      onInput={(event) => setLoraName(event.currentTarget.value)}
                    />
                  </label>
                  <label class="sheet-editor__field">
                    <span>Trigger word</span>
                    <input
                      class="sheet-editor__input"
                      value={loraTrigger()}
                      onInput={(event) => setLoraTrigger(event.currentTarget.value)}
                    />
                  </label>
                </div>
                <Button
                  variant="ghost"
                  disabled={(assets()?.references?.length ?? 0) < 10 || loraName().trim() === ""}
                  onClick={() =>
                    void runRequest("LoRA training queued", () =>
                      libraryTrainLora({
                        name: loraName().trim(),
                        kind: "character",
                        target_model: "fal_flux_dev",
                        trigger_word:
                          loraTrigger().trim() ||
                          loraName().trim().toLowerCase().replace(/\s+/g, "_"),
                        training_data: (assets()?.references ?? [])
                          .slice(0, 20)
                          .map((asset) => asset.id),
                      }),
                    )
                  }
                >
                  Train LoRA
                </Button>
                <div class="sheet-editor__asset-list">
                  <For each={assets()?.loras ?? []}>
                    {(lora) => (
                      <button
                        class="sheet-editor__asset-row"
                        onClick={() => setSelectedLoraId(lora.id)}
                      >
                        {lora.name} · {lora.trigger_word}
                      </button>
                    )}
                  </For>
                </div>
              </Show>
              <Show when={assetTab() === "notes"}>
                <div class="sheet-editor__meta">
                  Project style notes are edited in Preferences &gt; AI.
                </div>
              </Show>
            </Show>

            <Show when={activeView() === "provenance"}>
              <div class="sheet-editor__provenance">
                <div>Variant: {selectedVariant()?.id ?? "None"}</div>
                <div>Origin: {selectedVariant()?.origin ?? "n/a"}</div>
                <div>Model: {selectedVariant()?.model ?? "n/a"}</div>
                <div>Quality: {selectedVariant()?.quality ?? "n/a"}</div>
                <div>Template: {selectedVariant()?.template ?? "n/a"}</div>
                <div>
                  Size: {selectedVariant()?.width ?? 0}x{selectedVariant()?.height ?? 0}
                </div>
                <div>References: {selectedVariant()?.references?.length ?? 0}</div>
                <div>Cost: ${selectedVariant()?.cost_usd?.toFixed(4) ?? "0.0000"}</div>
                <div>Grounding: {selectedVariant()?.real_world_grounding ? "on" : "off"}</div>
                <div>LoRA: {selectedVariant()?.applied_lora ?? "none"}</div>
                <div>Parent: {selectedVariant()?.parent_variant_id ?? "none"}</div>
                <div>Promotion: {selectedVariant()?.promotion ? "yes" : "no"}</div>
                <div>Vector: {selectedVariant()?.vector_image != null ? "available" : "none"}</div>
                <div>Chat turns: {selectedVariant()?.chat_transcript?.turns?.length ?? 0}</div>
                <div class="sheet-editor__prompt">{selectedVariant()?.composed_prompt ?? ""}</div>
                <div class="sheet-editor__asset-list">
                  <For each={selectedVariant()?.references ?? []}>
                    {(slot) => (
                      <div class="sheet-editor__ref-slot">
                        <img src={dataUri(slot.image)} alt="" />
                        <span>{slot.role}</span>
                        <span>{slot.weight.toFixed(2)}</span>
                      </div>
                    )}
                  </For>
                </div>
              </div>
              <div class="sheet-editor__actions">
                <Button
                  variant="ghost"
                  onClick={handleVectorExport}
                  disabled={selectedVariant() === null || busy()}
                >
                  Vector export
                </Button>
                <Button
                  variant="ghost"
                  onClick={handleSaveReference}
                  disabled={selectedVariant() === null || busy()}
                >
                  Use as reference
                </Button>
              </div>
            </Show>
          </aside>
        </div>
      </Show>
    </div>
  );
};

export default ReferenceSheetEditor;
