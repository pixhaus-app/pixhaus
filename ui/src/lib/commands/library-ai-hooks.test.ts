// B9.4: contract tests for the library AI-hook IPC wrappers.
//
// Asserts the IPC names and argument shapes for libraryAutoTagEntity,
// libraryAcceptSuggestedTag, libraryRejectSuggestedTag, and
// libraryUpdateCorpus. A future signature drift on the Rust side
// (Tauri routes args by snake_case key) trips here instead of
// silently 404'ing at runtime.

import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

// Mock the module-level wrapper rather than @tauri-apps/api/core: the
// wrapper logs to `window.__pixhaus_ipc_log__`, which doesn't exist in
// the node-flavoured test environment configured in `vitest.config.ts`.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import {
  libraryAcceptSuggestedTag,
  libraryAutoTagEntity,
  libraryRejectSuggestedTag,
  libraryUpdateCorpus,
} from "./library";
import type { EntityId, TagDefinition, TagId } from "../types";

const ENTITY: EntityId = 42 as unknown as EntityId;
const OTHER_ENTITY: EntityId = 99 as unknown as EntityId;
const TAG: TagId = 7 as unknown as TagId;

describe("libraryAutoTagEntity", () => {
  it("sends entity_id at the top level and returns suggested tags", async () => {
    const fakeResult: TagDefinition[] = [{ id: TAG, name: "knight", auto_generated: true }];
    invokeMock.mockResolvedValueOnce(fakeResult);

    const result = await libraryAutoTagEntity(ENTITY);

    expect(invokeMock).toHaveBeenCalledWith("library_auto_tag_entity", {
      entity_id: ENTITY,
    });
    expect(result).toEqual(fakeResult);
  });
});

describe("libraryAcceptSuggestedTag", () => {
  it("sends entity_id and tag_id at the top level", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await libraryAcceptSuggestedTag(ENTITY, TAG);

    expect(invokeMock).toHaveBeenCalledWith("library_accept_suggested_tag", {
      entity_id: ENTITY,
      tag_id: TAG,
    });
  });
});

describe("libraryRejectSuggestedTag", () => {
  it("sends entity_id and tag_id at the top level", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await libraryRejectSuggestedTag(ENTITY, TAG);

    expect(invokeMock).toHaveBeenCalledWith("library_reject_suggested_tag", {
      entity_id: ENTITY,
      tag_id: TAG,
    });
  });
});

describe("libraryUpdateCorpus", () => {
  it("wraps the entity ids in an entity_ids array", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await libraryUpdateCorpus([ENTITY, OTHER_ENTITY]);

    expect(invokeMock).toHaveBeenCalledWith("library_update_corpus", {
      entity_ids: [ENTITY, OTHER_ENTITY],
    });
  });

  it("accepts an empty array", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await libraryUpdateCorpus([]);

    expect(invokeMock).toHaveBeenCalledWith("library_update_corpus", {
      entity_ids: [],
    });
  });
});
