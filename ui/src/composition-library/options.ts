// Shared option tables for composition-library editors.
//
// These were previously defined in sheet-editor-state; they now live here so
// the composition-library surface can import them without pulling in sheet
// internals. sheet-editor-state re-exports them for backward compatibility.

import type { ModelId, Quality } from "../lib/types";

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
