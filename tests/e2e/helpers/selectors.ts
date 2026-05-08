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
  commandPalette: {
    root: "command-palette",
    input: "command-palette-input",
    item: (commandId: string) => `command-palette-item-${commandId}`,
  },
} as const;

/** CSS selector for an element with the given data-testid. */
export function byTestId(id: string): string {
  return `[data-testid="${id}"]`;
}
