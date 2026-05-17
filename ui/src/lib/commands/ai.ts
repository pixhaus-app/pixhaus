// AI backend settings commands.

import { invoke } from "../ipc";

export type OpenAiStatus = {
  configured: boolean;
  registered: boolean;
  model: string;
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
