// Contract tests for the library anchor payload wrapper.
//
// Asserts the IPC argument shape — entity_id at the top level — so a
// future signature drift on the Rust side (which would break Tauri's
// snake_case argument routing) trips here instead of silently 404'ing
// at runtime.

import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

// Mock the module-level wrapper rather than @tauri-apps/api/core: the
// wrapper logs to `window.__pixhaus_ipc_log__`, which doesn't exist in
// the node-flavoured test environment configured in `vitest.config.ts`.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { libraryGetAnchorPayload, type AnchorPayload } from "./library";
import type { EntityId } from "../types";

const ENTITY: EntityId = 42 as unknown as EntityId;
const SPRITE_ENTITY: EntityId = 7 as unknown as EntityId;

describe("libraryGetAnchorPayload", () => {
  it("sends entity_id and returns the resolved payload", async () => {
    const payload: AnchorPayload = {
      reference_entity_id: SPRITE_ENTITY,
      // canonical_hash is intentionally not declared on AnchorPayload; the
      // Rust u64 doesn't survive JS-number precision. See the type comment
      // in library.ts.
      mime: "image/png",
      image_bytes: [0, 1, 2, 3],
      image_b64: "AAECAw==",
      palette: [],
      composition: {},
      lora_path: null,
      strength: 0.7,
    };
    invokeMock.mockResolvedValueOnce(payload);

    const result = await libraryGetAnchorPayload(ENTITY);

    expect(invokeMock).toHaveBeenCalledWith("library_get_anchor_payload", {
      entity_id: ENTITY,
    });
    expect(result).toEqual(payload);
  });

  it("propagates a null result when the entity carries no anchor", async () => {
    invokeMock.mockResolvedValueOnce(null);

    const result = await libraryGetAnchorPayload(ENTITY);

    expect(result).toBeNull();
  });
});
