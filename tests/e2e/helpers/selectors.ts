// Single source of truth for data-testid strings used across e2e specs.
//
// Specs import from this module so a typo is a TypeScript error, not a
// silent test failure. When you add a testid in the UI, mirror it here
// and reference it from the spec — never hard-code a testid string in a
// spec.

export const testid = {
  shell: "shell",
  editorLayout: "editor-layout",
  canvas: {
    container: "canvas-container",
    viewport: "canvas-viewport",
  },
  firstLaunch: {
    dialog: "first-launch-dialog",
    accept: "first-launch-dialog-accept",
    decline: "first-launch-dialog-decline",
  },
  welcome: {
    root: "welcome",
    newProject: "welcome-new-project",
    openProject: "welcome-open-project",
    samples: "welcome-samples",
    sample: (name: string) => `welcome-sample-${name}`,
    recent: "welcome-recent",
    recentItem: (index: number) => `welcome-recent-${index}`,
  },
  canvasSizeDialog: {
    width: "canvas-size-width",
    height: "canvas-size-height",
    name: "canvas-size-name",
    cancel: "canvas-size-cancel",
    create: "canvas-size-create",
    preset: (size: number) => `canvas-size-preset-${size}`,
  },
  commandPalette: {
    root: "command-palette",
    input: "command-palette-input",
    item: (commandId: string) => `command-palette-item-${commandId}`,
  },
  tool: {
    pencil: "tool-pencil",
    eraser: "tool-eraser",
    fill: "tool-fill",
    line: "tool-line",
    rect: "tool-rect",
    ellipse: "tool-ellipse",
  },
  toolOption: {
    brush: "tool-options-brush",
    fill: "tool-options-fill",
    size: "tool-option-size",
    shapePixel: "tool-option-shape-pixel",
    shapeCircle: "tool-option-shape-circle",
    shapeSquare: "tool-option-shape-square",
    pixelPerfect: "tool-option-pixel-perfect",
    tolerance: "tool-option-tolerance",
  },
  palette: {
    ioMenu: "palette-io-menu",
    ioToggle: "palette-io-toggle",
    importFormat: "palette-import-format",
    importBtn: "palette-import-btn",
    exportFormat: "palette-export-format",
    exportBtn: "palette-export-btn",
    harmonyToggle: "palette-harmony-toggle",
    harmonyType: (id: string) => `palette-harmony-type-${id}`,
    rampToggle: "palette-ramp-toggle",
    rampSteps: "palette-ramp-steps",
    rampGenerate: "palette-ramp-generate",
    swatch: (index: number) => `palette-swatch-${index}`,
    swatchDelete: "palette-swatch-delete",
  },
  timeline: {
    frameDuration: (index: number) => `timeline-frame-${index}-duration`,
    onionPrev: "timeline-onion-prev",
    onionNext: "timeline-onion-next",
    tagBar: "tl-tag-bar",
    tag: (name: string) => `tl-tag-${name}`,
    tagCtxRename: "tl-ctx-rename",
    tagCtxDelete: "tl-ctx-delete",
  },
  layer: {
    row: (index: number) => `layer-row-${index}`,
    contextMenu: {
      rename: "layer-ctx-rename",
      duplicate: "layer-ctx-duplicate",
      mergeDown: "layer-ctx-merge-down",
      mergeSelected: "layer-ctx-merge-selected",
      flattenVisible: "layer-ctx-flatten-visible",
      convertGroup: "layer-ctx-convert-group",
      convertTilemap: "layer-ctx-convert-tilemap",
      delete: "layer-ctx-delete",
    },
    renameInput: "layer-rename-input",
  },
  tilemap: {
    tilesetRow: (id: number) => `tileset-row-${id}`,
    tilesetAddBtn: "tileset-add-btn",
    tilesetAddName: "tileset-add-name",
  },
} as const;

/** CSS selector for an element with the given data-testid.
 *
 * Escapes `\` and `"` so any user-input-derived id (e.g. a tag name forwarded
 * through `testid.timeline.tag(name)`) produces a valid CSS attribute
 * selector. Anything else in the id is taken verbatim — testids are not
 * intended to carry arbitrary CSS metacharacters.
 */
export function byTestId(id: string): string {
  const escaped = id.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  return `[data-testid="${escaped}"]`;
}
