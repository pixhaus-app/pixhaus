import { afterEach, describe, expect, it } from "vitest";
import type { GroupId } from "../lib/types";
import { closeEntityCreate, entityCreate, openEntityCreate } from "./entity-create-state";

afterEach(() => {
  closeEntityCreate();
});

describe("entity-create-state", () => {
  it("starts closed", () => {
    expect(entityCreate.request).toBeNull();
  });

  it("openEntityCreate with no args opens with null initialGroupId", () => {
    openEntityCreate();
    expect(entityCreate.request).toEqual({ initialGroupId: null });
  });

  it("openEntityCreate stores initialGroupId", () => {
    openEntityCreate({ initialGroupId: 7 as GroupId });
    expect(entityCreate.request?.initialGroupId).toBe(7);
  });

  it("closeEntityCreate clears the request", () => {
    openEntityCreate();
    expect(entityCreate.request).not.toBeNull();
    closeEntityCreate();
    expect(entityCreate.request).toBeNull();
  });
});
