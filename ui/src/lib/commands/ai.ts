// AI backend settings commands.

import { invoke } from "../ipc";

export type OpenAiStatus = {
  configured: boolean;
  registered: boolean;
  model: string;
};

export type ProviderStatus = {
  provider: "openai" | "google_ai" | "fal";
  label: string;
  configured: boolean;
  registered: boolean;
  state: "not_configured" | "configured" | "invalid";
  models: string[];
};

export function aiGetOpenAiStatus(): Promise<OpenAiStatus> {
  return invoke<OpenAiStatus>("ai_get_openai_status");
}

export function aiSetOpenAiApiKey(api_key: string): Promise<OpenAiStatus> {
  return invoke<OpenAiStatus>("ai_set_openai_api_key", { api_key });
}

export function aiClearOpenAiApiKey(): Promise<OpenAiStatus> {
  return invoke<OpenAiStatus>("ai_clear_openai_api_key");
}

export function aiGetProviderOverview(): Promise<ProviderStatus[]> {
  return invoke<ProviderStatus[]>("ai_get_provider_overview");
}

export function aiGetGoogleAiStatus(): Promise<ProviderStatus> {
  return invoke<ProviderStatus>("ai_get_google_ai_status");
}

export function aiSetGoogleAiApiKey(api_key: string): Promise<ProviderStatus> {
  return invoke<ProviderStatus>("ai_set_google_ai_api_key", { api_key });
}

export function aiClearGoogleAiApiKey(): Promise<ProviderStatus> {
  return invoke<ProviderStatus>("ai_clear_google_ai_api_key");
}

export function aiGetFalStatus(): Promise<ProviderStatus> {
  return invoke<ProviderStatus>("ai_get_fal_status");
}

export function aiSetFalApiKey(api_key: string): Promise<ProviderStatus> {
  return invoke<ProviderStatus>("ai_set_fal_api_key", { api_key });
}

export function aiClearFalApiKey(): Promise<ProviderStatus> {
  return invoke<ProviderStatus>("ai_clear_fal_api_key");
}
