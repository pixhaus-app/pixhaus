import { describe, expect, it } from "vitest";
import { detectTokens } from "./tokens";

describe("detectTokens", () => {
  it("returns a single token, deduped", () => {
    expect(detectTokens("a {x} {{y}} {x}")).toEqual(["x"]);
  });

  it("returns tokens in first-seen order, deduped", () => {
    expect(detectTokens("a {species} {age} {species}")).toEqual(["species", "age"]);
  });

  it("returns no tokens for double-brace escapes", () => {
    expect(detectTokens("{{not a token}}")).toEqual([]);
  });

  it("returns empty array for plain text", () => {
    expect(detectTokens("no tokens here")).toEqual([]);
  });

  it("handles multiple distinct tokens", () => {
    expect(detectTokens("{a} {b} {c}")).toEqual(["a", "b", "c"]);
  });

  it("skips empty keys", () => {
    expect(detectTokens("{} {x} {}")).toEqual(["x"]);
  });

  it("handles adjacent tokens", () => {
    expect(detectTokens("{foo}{bar}")).toEqual(["foo", "bar"]);
  });

  it("handles mixed escapes and tokens", () => {
    expect(detectTokens("{{literal}} {real} {{another}}")).toEqual(["real"]);
  });

  it("ignores an unterminated opening brace", () => {
    expect(detectTokens("a {name")).toEqual([]);
    expect(detectTokens("{a} {b")).toEqual(["a"]);
  });
});
