import { afterEach, describe, expect, it } from "vitest";
import type { GroupId } from "../lib/types";
import { closeEntityCreate, entityCreateRequest, openEntityCreate } from "./entity-create-state";

afterEach(() => {
  closeEntityCreate();
});

describe("entity-create-state", () => {
  it("starts closed", () => {
    expect(entityCreateRequest()).toBeNull();
  });

  it("openEntityCreate with no args opens with null initialGroupId", () => {
    openEntityCreate();
    expect(entityCreateRequest()).toEqual({ initialGroupId: null });
  });

  it("openEntityCreate stores initialGroupId", () => {
    openEntityCreate({ initialGroupId: 7 as GroupId });
    expect(entityCreateRequest()?.initialGroupId).toBe(7);
  });

  it("closeEntityCreate clears the request", () => {
    openEntityCreate();
    expect(entityCreateRequest()).not.toBeNull();
    closeEntityCreate();
    expect(entityCreateRequest()).toBeNull();
  });
});
