import { afterEach, describe, expect, it } from "vitest";
import type { EntityId } from "../lib/types";
import { anchorPickerRequest, closeAnchorPicker, openAnchorPicker } from "./anchor-picker-state";

afterEach(() => {
  closeAnchorPicker();
});

describe("anchor-picker-state", () => {
  it("starts closed", () => {
    expect(anchorPickerRequest()).toBeNull();
  });

  it("openAnchorPicker stores the request", () => {
    let received: EntityId | null = null;
    openAnchorPicker({
      entityId: 1 as EntityId,
      references: [{ id: 2 as EntityId, name: "Hero" }],
      onConfirm: (id) => {
        received = id;
      },
    });
    const req = anchorPickerRequest();
    expect(req).not.toBeNull();
    expect(req?.entityId).toBe(1);
    expect(req?.references).toEqual([{ id: 2, name: "Hero" }]);
    req?.onConfirm(2 as EntityId);
    expect(received).toBe(2);
  });

  it("closeAnchorPicker clears the request", () => {
    openAnchorPicker({
      entityId: 1 as EntityId,
      references: [],
      onConfirm: () => undefined,
    });
    expect(anchorPickerRequest()).not.toBeNull();
    closeAnchorPicker();
    expect(anchorPickerRequest()).toBeNull();
  });

  it("openAnchorPicker overwrites a prior request", () => {
    openAnchorPicker({
      entityId: 1 as EntityId,
      references: [{ id: 2 as EntityId, name: "Hero" }],
      onConfirm: () => undefined,
    });
    openAnchorPicker({
      entityId: 9 as EntityId,
      references: [{ id: 10 as EntityId, name: "Other" }],
      onConfirm: () => undefined,
    });
    expect(anchorPickerRequest()?.entityId).toBe(9);
  });
});
