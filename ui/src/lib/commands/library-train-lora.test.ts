// B10.5: contract test for libraryTrainEntityLora.
//
// Asserts the IPC argument shape — entity_id at the top level, options
// passed as null when omitted — so a future signature drift on the Rust
// side (which would break Tauri's snake_case argument routing) trips
// here instead of silently 404'ing at runtime.

import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

// Mock the module-level wrapper rather than @tauri-apps/api/core: the
// wrapper logs to `window.__pixhaus_ipc_log__`, which doesn't exist in
// the node-flavoured test environment configured in `vitest.config.ts`.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { libraryTrainEntityLora, type LibraryTrainEntityLoraResult } from "./library";
import type { EntityId } from "../types";

const ENTITY: EntityId = 42 as unknown as EntityId;

describe("libraryTrainEntityLora", () => {
  it("sends entity_id and null options when no overrides are supplied", async () => {
    const fakeResult: LibraryTrainEntityLoraResult = {
      entity_id: ENTITY,
      lora_path: "https://replicate.delivery/abc/hero.safetensors",
      label: "Hero",
      training_id: "tr-001",
      invocation_id: "9001",
    };
    invokeMock.mockResolvedValueOnce(fakeResult);

    const result = await libraryTrainEntityLora(ENTITY);

    expect(invokeMock).toHaveBeenCalledWith("library_train_entity_lora", {
      entity_id: ENTITY,
      options: null,
    });
    expect(result).toEqual(fakeResult);
  });

  it("forwards the options bag verbatim", async () => {
    invokeMock.mockResolvedValueOnce({
      entity_id: ENTITY,
      lora_path: "x",
      label: "Hero",
      training_id: "tr",
      invocation_id: "1",
    });

    await libraryTrainEntityLora(ENTITY, {
      lora_rank: 8,
      steps: 500,
      label: "Hero-tight",
      model: "ostris/flux-dev-lora-trainer",
    });

    expect(invokeMock).toHaveBeenCalledWith("library_train_entity_lora", {
      entity_id: ENTITY,
      options: {
        lora_rank: 8,
        steps: 500,
        label: "Hero-tight",
        model: "ostris/flux-dev-lora-trainer",
      },
    });
  });
});
