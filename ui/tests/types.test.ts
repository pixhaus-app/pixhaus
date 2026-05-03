import { describe, expect, it } from "vitest";

import sample from "./fixtures/sample-project.json" with { type: "json" };
import type { CelData, LayerKind, Project, SelectionRegion, TilesetSource } from "../src/lib/types";

// The fixture is written by `cargo test -p pixhaus-core --test round_trip`.
// Re-running the Rust test suite refreshes the file.
//
// JSON imports widen string literals to `string` (e.g. `color_mode: "rgba"`
// becomes `string`, not the `ColorMode` union), so a direct typed binding
// fails to compile even when the shapes do match. The cast bridges that
// JSON-import limitation; the round-trip Rust test guarantees the file's
// shape, and the runtime assertions below pin every literal-typed
// discriminator (layer kinds, blend modes, region kinds, schema version)
// so a serde rename or missing field surfaces as a test failure.
const project = sample as unknown as Project;

describe("Project fixture mirrors the Rust data model", () => {
  it("exposes the current schema version", () => {
    expect(project.schema_version).toEqual({ major: 1, minor: 0 });
  });

  it("encodes feature flags as a packed bitfield", () => {
    // TILEMAPS | REFERENCES | ANIMATIONS | SLICES = 1 | 2 | 4 | 8 = 15
    expect(project.feature_flags).toBe(15);
  });

  it("carries the editor version emitted by the Rust crate", () => {
    expect(project.metadata.editor_version).toBe("0.1.0");
  });

  it("includes one sprite with all four layer kinds", () => {
    expect(project.sprites).toHaveLength(1);
    const sprite = project.sprites[0]!;
    const kinds = sprite.layers.map((l): LayerKind["kind"] => l.kind.kind);
    expect(kinds).toEqual(["raster", "group", "tilemap", "reference"]);
  });

  it("stores tilemap cels as embedded grids", () => {
    const sprite = project.sprites[0]!;
    const tilemapCel = sprite.cels.find(
      (c): c is typeof c & { data: Extract<CelData, { kind: "tilemap" }> } =>
        c.data.kind === "tilemap",
    );
    expect(tilemapCel).toBeDefined();
    expect(tilemapCel!.data.data.width).toBe(2);
    expect(tilemapCel!.data.data.cells).toHaveLength(4);
  });

  it("stores linked cels by source frame index", () => {
    const sprite = project.sprites[0]!;
    const linked = sprite.cels.find(
      (c): c is typeof c & { data: Extract<CelData, { kind: "linked" }> } =>
        c.data.kind === "linked",
    );
    expect(linked).toBeDefined();
    expect(linked!.data.source_frame).toBe(0);
  });

  it("encodes inline tilesets with a buffer handle", () => {
    const tileset = project.sprites[0]!.tilesets[0]!;
    const source = tileset.source as Extract<TilesetSource, { kind: "inline" }>;
    expect(source.kind).toBe("inline");
    expect(source.buffer).toBe(2000);
  });

  it("uses snake_case for blend modes and loop directions", () => {
    const sprite = project.sprites[0]!;
    const tilemapLayer = sprite.layers.find((l) => l.kind.kind === "tilemap")!;
    expect(tilemapLayer.blend_mode).toBe("multiply");
    expect(sprite.frame_tags[0]!.loop_direction).toBe("ping_pong");
  });

  it("represents rectangular selections with a kind tag", () => {
    const region = project.selection.region as Extract<SelectionRegion, { kind: "rect" }>;
    expect(region.kind).toBe("rect");
    expect(region.bounds).toEqual({
      origin: { x: 4, y: 4 },
      size: { width: 8, height: 8 },
    });
  });

  it("carries a slice with nine-slice and pivot metadata", () => {
    const slice = project.sprites[0]!.slices[0]!;
    expect(slice.keys[0]!.nine_slice).toEqual({
      center: { origin: { x: 4, y: 4 }, size: { width: 24, height: 24 } },
    });
    expect(slice.keys[0]!.pivot).toEqual({ offset: { x: 16, y: 16 } });
  });
});
