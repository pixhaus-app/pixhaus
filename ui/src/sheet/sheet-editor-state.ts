// State, option tables, and pure helpers for the reference-sheet editor.
//
// The reactive state is created per editor instance by
// `createSheetEditorState()` (the editor destructures it into local
// signals), keeping the large `ReferenceSheetEditor` component focused on
// behavior and JSX. The option constants and pure helpers below carry no
// reactivity.

import { createSignal } from "solid-js";
import type {
  AssetLibrary,
  Entity,
  EntityContent,
  ModelId,
  PromptId,
  Quality,
  ReferenceImage,
  ReferenceRole,
  ReferenceSheet,
  ReferenceSheetTemplateDefinition,
  ReferenceSheetTemplateId,
  ReferenceSlot,
  RegionDefinition,
  Rgba,
  SheetVariant,
  StructureId,
  StyleId,
} from "../lib/types";
import type { RequestId, ReferenceSheetTemplate } from "../lib/commands/library";

export type SheetEditorTemplateOption = {
  value: ReferenceSheetTemplate;
  label: string;
};

export type EditorView =
  | "browse"
  | "generate"
  | "refine"
  | "chat"
  | "compare"
  | "assets"
  | "provenance"
  | "library";
export type RefineMode = "masked" | "prompt_only" | "regional";
export type SlotTarget = "generate" | "refine" | "region";

export type SheetRequestCompletePayload = {
  request_id: number;
  entity_id: number;
  sprite: Entity;
  total_cost_usd: number;
};

export type SheetRequestProgressPayload = {
  request_id: number;
  stream_index: number;
  candidate_index: number;
  partial_index: number;
  partial_image?: ReferenceImage | null;
  elapsed_ms: number;
};

export type SheetRequestCancelledPayload = {
  request_id: number;
};

export type SheetRequestErrorPayload = {
  request_id: number;
  error: { kind?: string; message?: unknown };
};

export type ActiveSheetRequest = {
  id: RequestId;
  label: string;
  streamIndex: number;
  candidateIndex: number;
  partialIndex: number;
  elapsedMs: number;
  partialImage: ReferenceImage | null;
  status: "queued" | "running" | "complete" | "cancelled" | "error";
  error?: string;
};

export type RegionDraft = RegionDefinition & { id: number };

export const TEMPLATES: SheetEditorTemplateOption[] = [
  { value: "character", label: "Character" },
  { value: "item", label: "Item" },
  { value: "tileset", label: "Tileset" },
  { value: "custom", label: "Custom" },
];

export const QUALITIES: Array<{ value: Quality; label: string }> = [
  { value: "medium", label: "Medium" },
  { value: "low", label: "Low" },
  { value: "high", label: "High" },
  { value: "auto", label: "Auto" },
];

export const VIEWS: Array<{ value: EditorView; label: string }> = [
  { value: "browse", label: "Browse" },
  { value: "generate", label: "Generate" },
  { value: "refine", label: "Refine" },
  { value: "chat", label: "Chat" },
  { value: "compare", label: "Compare" },
  { value: "assets", label: "Assets" },
  { value: "provenance", label: "Provenance" },
  { value: "library", label: "Library" },
];

export const MODEL_OPTIONS: Array<{ value: ModelId; label: string }> = [
  { value: "auto", label: "Auto" },
  { value: "open_ai_gpt_image2", label: "OpenAI gpt-image-2" },
  { value: "google_nano_banana_pro", label: "Nano Banana Pro" },
  { value: "google_gemini_flash_image", label: "Gemini Flash Image" },
  { value: "fal_flux_kontext", label: "fal Flux Kontext" },
  { value: "fal_flux_dev", label: "fal Flux.1 dev" },
];

export const QUALITY_OPTIONS: Array<{ value: Quality; label: string }> = [
  { value: "auto", label: "Auto" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

export const ROLE_OPTIONS: Array<{ value: ReferenceRole; label: string }> = [
  { value: "subject", label: "Subject" },
  { value: "style", label: "Style" },
  { value: "pose", label: "Pose" },
  { value: "outfit", label: "Outfit" },
  { value: "context", label: "Context" },
  { value: "generic", label: "Generic" },
];

export const DEFAULT_CHROMA: Rgba = { r: 255, g: 0, b: 255, a: 255 };

/** Returns the embedded reference sheet of a sprite entity, or `null`. */
export function referenceSheet(entity: Entity | null): ReferenceSheet | null {
  if (entity === null) return null;
  const content = entity.content as EntityContent;
  if (content.type !== "Sprites") return null;
  return content.value.reference_sheet ?? null;
}

/** Formats an RGBA color as an uppercase `#RRGGBB` hex string. */
export function rgbaToHex(color: Rgba): string {
  const channel = (value: number) => value.toString(16).padStart(2, "0");
  return `#${channel(color.r)}${channel(color.g)}${channel(color.b)}`.toUpperCase();
}

/** Parses a `#RRGGBB` hex string into RGBA, falling back to magenta. */
export function hexToRgba(hex: string): Rgba {
  const clean = hex.replace(/^#/, "");
  if (!/^[0-9a-fA-F]{6}$/.test(clean)) return DEFAULT_CHROMA;
  return {
    r: Number.parseInt(clean.slice(0, 2), 16),
    g: Number.parseInt(clean.slice(2, 4), 16),
    b: Number.parseInt(clean.slice(4, 6), 16),
    a: 255,
  };
}

/** Encodes a reference image as a base64 `data:` URI. */
export function dataUri(image: ReferenceImage): string {
  const bytes = new Uint8Array(image.bytes);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return `data:${image.mime};base64,${btoa(binary)}`;
}

/** Human label for a variant in the history strip. */
export function variantLabel(variant: SheetVariant, isCanonical: boolean, index: number): string {
  if (isCanonical) return "Approved";
  const generated = new Date(variant.created_at * 1000).toLocaleString();
  return `Draft ${index + 1} · ${generated}`;
}

/**
 * Creates the reactive state for one reference-sheet editor instance.
 *
 * Called once during component setup; the editor destructures the
 * returned signals into local bindings, so call sites keep their
 * original names. Per-instance creation preserves the previous
 * (component-local) reactivity semantics exactly.
 */
export function createSheetEditorState() {
  const [entity, setEntity] = createSignal<Entity | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [generating, setGenerating] = createSignal(false);
  const [prompt, setPrompt] = createSignal("");
  const [template, setTemplate] = createSignal<ReferenceSheetTemplate>("character");
  const [candidateCount, setCandidateCount] = createSignal(2);
  const [selectedVariantId, setSelectedVariantId] = createSignal<number | null>(null);
  const [activeView, setActiveView] = createSignal<EditorView>("generate");
  const [templates, setTemplates] = createSignal<ReferenceSheetTemplateDefinition[]>([]);
  const [templateId, setTemplateId] = createSignal<ReferenceSheetTemplateId>("turnaround4_view");
  const [dimensionValue, setDimensionValue] = createSignal("2048x1024");
  const [chromaHex, setChromaHex] = createSignal("#FF00FF");
  const [model, setModel] = createSignal<ModelId>("auto");
  const [v1Quality, setV1Quality] = createSignal<Quality>("medium");
  const [refineMode, setRefineMode] = createSignal<RefineMode>("prompt_only");
  const [refinePrompt, setRefinePrompt] = createSignal("");
  const [chatMessage, setChatMessage] = createSignal("");
  const [comparePrompt, setComparePrompt] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [assets, setAssets] = createSignal<AssetLibrary | null>(null);
  const [assetName, setAssetName] = createSignal("");
  const [referenceSlots, setReferenceSlots] = createSignal<ReferenceSlot[]>([]);
  const [additionalReferenceSlots, setAdditionalReferenceSlots] = createSignal<ReferenceSlot[]>([]);
  const [regionReferenceSlots, setRegionReferenceSlots] = createSignal<ReferenceSlot[]>([]);
  const [activeRequests, setActiveRequests] = createSignal<ActiveSheetRequest[]>([]);
  const [assetTab, setAssetTab] = createSignal<
    "references" | "cards" | "styles" | "loras" | "notes"
  >("references");
  const [regionDrafts, setRegionDrafts] = createSignal<RegionDraft[]>([]);
  const [loraName, setLoraName] = createSignal("");
  const [loraTrigger, setLoraTrigger] = createSignal("");
  const [selectedLoraId, setSelectedLoraId] = createSignal<number | null>(null);
  const [loraWeight, setLoraWeight] = createSignal(1);
  const [realWorldGrounding, setRealWorldGrounding] = createSignal(false);
  const [brushSize, setBrushSize] = createSignal(50);
  const [maskFeather, setMaskFeather] = createSignal(3);
  const [maskExpand, setMaskExpand] = createSignal(12);
  const [maskMode, setMaskMode] = createSignal<"brush" | "erase">("brush");

  // ── composition library selection signals ─────────────────────────────────
  // Drive the Library panel (spec sections 15-17) without touching the
  // existing template/templateId generation path.
  const [selectedStructureId, setSelectedStructureId] = createSignal<StructureId | null>(null);
  const [selectedStyleId, setSelectedStyleId] = createSignal<StyleId | null>(null);
  const [selectedPromptId, setSelectedPromptId] = createSignal<PromptId | null>(null);
  const [variableValues, setVariableValues] = createSignal<Record<string, string>>({});
  const [inlineText, setInlineText] = createSignal<string>("");
  const [inlineNegatives, setInlineNegatives] = createSignal<string>("");

  return {
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
    selectedStructureId,
    setSelectedStructureId,
    selectedStyleId,
    setSelectedStyleId,
    selectedPromptId,
    setSelectedPromptId,
    variableValues,
    setVariableValues,
    inlineText,
    setInlineText,
    inlineNegatives,
    setInlineNegatives,
  };
}
