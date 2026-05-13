// B10.3: contract tests for the library anchor wrappers.
//
// Asserts the IPC argument shapes — entity_id and reference_id at the
// top level for set, entity_id alone for get — so a future signature
// drift on the Rust side (which would break Tauri's snake_case argument
// routing) trips here instead of silently 404'ing at runtime.

import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

// Mock the module-level wrapper rather than @tauri-apps/api/core: the
// wrapper logs to `window.__pixhaus_ipc_log__`, which doesn't exist in
// the node-flavoured test environment configured in `vitest.config.ts`.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import {
  libraryGetAnchorPayload,
  librarySetEntityAnchor,
  type AnchorPayload,
} from "./library";
import type { EntityId } from "../types";

const ENTITY: EntityId = 42 as unknown as EntityId;
const REFERENCE: EntityId = 7 as unknown as EntityId;

describe("librarySetEntityAnchor", () => {
  it("sends entity_id and reference_id when anchoring on a reference", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await librarySetEntityAnchor(ENTITY, REFERENCE);

    expect(invokeMock).toHaveBeenCalledWith("library_set_entity_anchor", {
      entity_id: ENTITY,
      reference_id: REFERENCE,
    });
  });

  it("sends reference_id=null when clearing the anchor", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await librarySetEntityAnchor(ENTITY, null);

    expect(invokeMock).toHaveBeenCalledWith("library_set_entity_anchor", {
      entity_id: ENTITY,
      reference_id: null,
    });
  });
});

describe("libraryGetAnchorPayload", () => {
  it("sends entity_id and returns the resolved payload", async () => {
    const payload: AnchorPayload = {
      reference_entity_id: REFERENCE,
      canonical_hash: 0xdeadbeef,
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
