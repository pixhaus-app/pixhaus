// Three-tab composition library panel (Structures / Styles / Prompts).
//
// Lists built-in records (read-only, badged) and project records together.
// Provides New, Edit, Duplicate, Fork from built-in, Delete (project only),
// Import .pixstyle, Export selection, and Copy from project actions.

import { type Component, For, Show, createMemo, createSignal } from "solid-js";
import type {
  CompositionKind,
  CompositionLibrary,
  ConflictPolicy,
  PromptTemplate,
  Structure,
  StructureId,
  Style,
  StyleId,
  PromptId,
} from "../lib/types";
import {
  listComposition,
  upsertStructure,
  upsertStyle,
  upsertPrompt,
  deleteStructure,
  deleteStyle,
  deletePrompt,
  forkBuiltin,
  exportPack,
  importPack,
  copyFromProject,
} from "../lib/commands/library";
import { open, save } from "../lib/dialog";
import { extractDetail } from "../lib/utils/errors";
import { Button } from "../lib/ui/Button";
import StructureEditor from "./StructureEditor";
import StyleEditor from "./StyleEditor";
import PromptEditor from "./PromptEditor";

// ── types ─────────────────────────────────────────────────────────────────────

type Tab = "structures" | "styles" | "prompts";

// A row in the list: the record plus whether it is a built-in.
type StructureRow = { record: Structure; builtin: boolean };
type StyleRow = { record: Style; builtin: boolean };
type PromptRow = { record: PromptTemplate; builtin: boolean };

// ── helpers ───────────────────────────────────────────────────────────────────

function generateId(prefix: string): string {
  return `${prefix}.${Date.now().toString(36)}.${Math.random().toString(36).slice(2, 7)}`;
}

function emptyStructure(): Structure {
  return {
    id: generateId("project.structure"),
    name: "New structure",
    output: { paneled: { canvas: { width: 512, height: 512 }, panels: [] } },
  };
}

function emptyStyle(): Style {
  return {
    id: generateId("project.style"),
    name: "New style",
  };
}

function emptyPrompt(): PromptTemplate {
  return {
    id: generateId("project.prompt"),
    name: "New prompt",
    text: "",
  };
}

// ── component ─────────────────────────────────────────────────────────────────

const CompositionLibraryPanel: Component = () => {
  const [tab, setTab] = createSignal<Tab>("structures");
  const [library, setLibrary] = createSignal<CompositionLibrary | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const [editingStructure, setEditingStructure] = createSignal<Structure | null>(null);
  const [editingStyle, setEditingStyle] = createSignal<Style | null>(null);
  const [editingPrompt, setEditingPrompt] = createSignal<PromptTemplate | null>(null);
  const [editReadOnly, setEditReadOnly] = createSignal(false);

  const [selectedStructureId, setSelectedStructureId] = createSignal<StructureId | null>(null);
  const [selectedStyleId, setSelectedStyleId] = createSignal<StyleId | null>(null);
  const [selectedPromptId, setSelectedPromptId] = createSignal<PromptId | null>(null);

  // Merge built-ins and project records into display rows, project records first.
  const structureRows = createMemo<StructureRow[]>(() => {
    const lib = library();
    if (!lib) return [];
    const projectIds = new Set(lib.structures.map((s) => s.id));
    const builtinRows: StructureRow[] = lib.builtin_structures
      .filter((s) => !projectIds.has(s.id))
      .map((s) => ({ record: s, builtin: true }));
    return [...lib.structures.map((s) => ({ record: s, builtin: false })), ...builtinRows];
  });

  const styleRows = createMemo<StyleRow[]>(() => {
    const lib = library();
    if (!lib) return [];
    const projectIds = new Set(lib.styles.map((s) => s.id));
    const builtinRows: StyleRow[] = lib.builtin_styles
      .filter((s) => !projectIds.has(s.id))
      .map((s) => ({ record: s, builtin: true }));
    return [...lib.styles.map((s) => ({ record: s, builtin: false })), ...builtinRows];
  });

  const promptRows = createMemo<PromptRow[]>(() => {
    const lib = library();
    if (!lib) return [];
    const projectIds = new Set(lib.prompts.map((p) => p.id));
    const builtinRows: PromptRow[] = lib.builtin_prompts
      .filter((p) => !projectIds.has(p.id))
      .map((p) => ({ record: p, builtin: true }));
    return [...lib.prompts.map((p) => ({ record: p, builtin: false })), ...builtinRows];
  });

  const allStructures = createMemo<Structure[]>(() => {
    const lib = library();
    if (!lib) return [];
    return [...lib.structures, ...lib.builtin_structures];
  });

  const allStyles = createMemo<Style[]>(() => {
    const lib = library();
    if (!lib) return [];
    return [...lib.styles, ...lib.builtin_styles];
  });

  // ── helpers: selected-row lookups ────────────────────────────────────────────

  const selectedStructureRow = createMemo(() =>
    structureRows().find((r) => r.record.id === selectedStructureId()),
  );
  const selectedStyleRow = createMemo(() =>
    styleRows().find((r) => r.record.id === selectedStyleId()),
  );
  const selectedPromptRow = createMemo(() =>
    promptRows().find((r) => r.record.id === selectedPromptId()),
  );

  // ── data loading ────────────────────────────────────────────────────────────

  async function reload(): Promise<void> {
    setLoading(true);
    setError(null);
    try {
      setLibrary(await listComposition());
    } catch (e) {
      const isNoProject =
        e !== null &&
        typeof e === "object" &&
        (e as { kind?: unknown }).kind === "no_active_project";
      setError(
        isNoProject ? "Open a project to manage its prompt & style library." : extractDetail(e),
      );
    } finally {
      setLoading(false);
    }
  }

  // Load on first mount.
  reload();

  // ── structures tab actions ──────────────────────────────────────────────────

  function newStructure(): void {
    setEditingStructure(emptyStructure());
    setEditReadOnly(false);
  }

  function editStructure(row: StructureRow): void {
    setEditingStructure({ ...row.record });
    setEditReadOnly(row.builtin);
  }

  function duplicateStructure(): void {
    const row = selectedStructureRow();
    if (!row) return;
    setEditingStructure({
      ...row.record,
      id: generateId("project.structure"),
      name: `${row.record.name} copy`,
    });
    setEditReadOnly(false);
  }

  async function forkStructure(): Promise<void> {
    const row = selectedStructureRow();
    if (!row?.builtin) return;
    try {
      await forkBuiltin("structure" as CompositionKind, row.record.id, true);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function deleteSelectedStructure(): Promise<void> {
    const id = selectedStructureId();
    if (!id) return;
    try {
      await deleteStructure(id);
      setSelectedStructureId(null);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function saveStructure(s: Structure): Promise<void> {
    try {
      await upsertStructure(s);
      setEditingStructure(null);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  // ── styles tab actions ──────────────────────────────────────────────────────

  function newStyle(): void {
    setEditingStyle(emptyStyle());
    setEditReadOnly(false);
  }

  function editStyle(row: StyleRow): void {
    setEditingStyle({ ...row.record });
    setEditReadOnly(row.builtin);
  }

  function duplicateStyle(): void {
    const row = selectedStyleRow();
    if (!row) return;
    setEditingStyle({
      ...row.record,
      id: generateId("project.style"),
      name: `${row.record.name} copy`,
    });
    setEditReadOnly(false);
  }

  async function forkStyle(): Promise<void> {
    const row = selectedStyleRow();
    if (!row?.builtin) return;
    try {
      await forkBuiltin("style" as CompositionKind, row.record.id, true);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function deleteSelectedStyle(): Promise<void> {
    const id = selectedStyleId();
    if (!id) return;
    try {
      await deleteStyle(id);
      setSelectedStyleId(null);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function saveStyle(s: Style): Promise<void> {
    try {
      await upsertStyle(s);
      setEditingStyle(null);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  // ── prompts tab actions ─────────────────────────────────────────────────────

  function newPrompt(): void {
    setEditingPrompt(emptyPrompt());
    setEditReadOnly(false);
  }

  function editPrompt(row: PromptRow): void {
    setEditingPrompt({ ...row.record });
    setEditReadOnly(row.builtin);
  }

  function duplicatePrompt(): void {
    const row = selectedPromptRow();
    if (!row) return;
    setEditingPrompt({
      ...row.record,
      id: generateId("project.prompt"),
      name: `${row.record.name} copy`,
    });
    setEditReadOnly(false);
  }

  async function forkPrompt(): Promise<void> {
    const row = selectedPromptRow();
    if (!row?.builtin) return;
    try {
      await forkBuiltin("prompt" as CompositionKind, row.record.id, true);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function deleteSelectedPrompt(): Promise<void> {
    const id = selectedPromptId();
    if (!id) return;
    try {
      await deletePrompt(id);
      setSelectedPromptId(null);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function savePrompt(p: PromptTemplate): Promise<void> {
    try {
      await upsertPrompt(p);
      setEditingPrompt(null);
      await reload();
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  // ── import / export / copy ──────────────────────────────────────────────────

  async function handleImport(): Promise<void> {
    const path = await open({
      filters: [{ name: "Pixhaus style pack", extensions: ["pixstyle"] }],
    });
    if (typeof path !== "string") return;
    try {
      const report = await importPack(path, "skip" as ConflictPolicy);
      await reload();
      setError(
        `Imported ${report.imported}, skipped ${report.skipped}, overwrote ${report.overwritten}.`,
      );
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function handleExport(): Promise<void> {
    const path = await save({
      filters: [{ name: "Pixhaus style pack", extensions: ["pixstyle"] }],
    });
    if (!path) return;
    const lib = library();
    if (!lib) return;
    // Export all project-tier records.
    try {
      await exportPack(
        lib.structures.map((s) => s.id),
        lib.styles.map((s) => s.id),
        lib.prompts.map((p) => p.id),
        path,
      );
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  async function handleCopyFromProject(): Promise<void> {
    const path = await open({ filters: [{ name: "Pixhaus project", extensions: ["pixhaus"] }] });
    if (typeof path !== "string") return;
    try {
      const report = await copyFromProject(path, "skip" as ConflictPolicy);
      await reload();
      setError(
        `Copied ${report.imported}, skipped ${report.skipped}, overwrote ${report.overwritten}.`,
      );
    } catch (e) {
      setError(extractDetail(e));
    }
  }

  // ── render ──────────────────────────────────────────────────────────────────

  // Derived: is there an active editor open for the current tab?
  const isEditing = () =>
    (tab() === "structures" && editingStructure() !== null) ||
    (tab() === "styles" && editingStyle() !== null) ||
    (tab() === "prompts" && editingPrompt() !== null);

  return (
    <div class="comp-lib">
      {/* Top toolbar */}
      <div class="comp-lib__toolbar">
        <span class="comp-lib__toolbar-label">Library</span>
        <Button variant="ghost" size="sm" onClick={() => void handleImport()}>
          Import .pixstyle
        </Button>
        <Button variant="ghost" size="sm" onClick={() => void handleExport()}>
          Export all
        </Button>
        <Button variant="ghost" size="sm" onClick={() => void handleCopyFromProject()}>
          Copy from project
        </Button>
        <Button variant="ghost" size="sm" onClick={() => void reload()} disabled={loading()}>
          {loading() ? "Loading..." : "Refresh"}
        </Button>
      </div>

      {/* Status bar */}
      <Show when={error() !== null}>
        <div class="comp-lib__status">{error()}</div>
      </Show>

      {/* Two-pane body */}
      <div class="comp-lib__body">
        {/* ── Left pane: tabs + list + list-actions ── */}
        <div class="comp-lib__pane comp-lib__pane--left">
          {/* Tab strip */}
          <div class="comp-lib__tabs" role="tablist">
            <button
              class={`comp-lib__tab${tab() === "structures" ? " comp-lib__tab--active" : ""}`}
              role="tab"
              type="button"
              onClick={() => {
                setTab("structures");
                setEditingStructure(null);
              }}
            >
              Structures
            </button>
            <button
              class={`comp-lib__tab${tab() === "styles" ? " comp-lib__tab--active" : ""}`}
              role="tab"
              type="button"
              onClick={() => {
                setTab("styles");
                setEditingStyle(null);
              }}
            >
              Styles
            </button>
            <button
              class={`comp-lib__tab${tab() === "prompts" ? " comp-lib__tab--active" : ""}`}
              role="tab"
              type="button"
              onClick={() => {
                setTab("prompts");
                setEditingPrompt(null);
              }}
            >
              Prompts
            </button>
          </div>

          {/* Record list */}
          <Show when={tab() === "structures"}>
            <div class="comp-lib__list" role="listbox">
              <For
                each={structureRows()}
                fallback={<div class="comp-lib__empty">No structures.</div>}
              >
                {(row) => (
                  <div
                    class={`comp-lib__row${selectedStructureId() === row.record.id ? " comp-lib__row--selected" : ""}`}
                    role="option"
                    aria-selected={selectedStructureId() === row.record.id}
                    onClick={() => setSelectedStructureId(row.record.id)}
                    onDblClick={() => editStructure(row)}
                  >
                    <span class="comp-lib__row-name">{row.record.name}</span>
                    <Show when={row.builtin}>
                      <span class="comp-lib__badge">built-in</span>
                    </Show>
                  </div>
                )}
              </For>
            </div>
            <div class="comp-lib__list-actions">
              <Button size="sm" onClick={newStructure}>
                New
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedStructureId() === null}
                onClick={() => {
                  const row = selectedStructureRow();
                  if (row) editStructure(row);
                }}
              >
                Edit
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedStructureId() === null}
                onClick={duplicateStructure}
              >
                Duplicate
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedStructureId() === null || !selectedStructureRow()?.builtin}
                onClick={() => void forkStructure()}
              >
                Fork built-in
              </Button>
              <Button
                variant="danger"
                size="sm"
                disabled={
                  selectedStructureId() === null || (selectedStructureRow()?.builtin ?? false)
                }
                onClick={() => void deleteSelectedStructure()}
              >
                Delete
              </Button>
            </div>
          </Show>

          <Show when={tab() === "styles"}>
            <div class="comp-lib__list" role="listbox">
              <For each={styleRows()} fallback={<div class="comp-lib__empty">No styles.</div>}>
                {(row) => (
                  <div
                    class={`comp-lib__row${selectedStyleId() === row.record.id ? " comp-lib__row--selected" : ""}`}
                    role="option"
                    aria-selected={selectedStyleId() === row.record.id}
                    onClick={() => setSelectedStyleId(row.record.id)}
                    onDblClick={() => editStyle(row)}
                  >
                    <span class="comp-lib__row-name">{row.record.name}</span>
                    <Show when={row.builtin}>
                      <span class="comp-lib__badge">built-in</span>
                    </Show>
                  </div>
                )}
              </For>
            </div>
            <div class="comp-lib__list-actions">
              <Button size="sm" onClick={newStyle}>
                New
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedStyleId() === null}
                onClick={() => {
                  const row = selectedStyleRow();
                  if (row) editStyle(row);
                }}
              >
                Edit
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedStyleId() === null}
                onClick={duplicateStyle}
              >
                Duplicate
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedStyleId() === null || !selectedStyleRow()?.builtin}
                onClick={() => void forkStyle()}
              >
                Fork built-in
              </Button>
              <Button
                variant="danger"
                size="sm"
                disabled={selectedStyleId() === null || (selectedStyleRow()?.builtin ?? false)}
                onClick={() => void deleteSelectedStyle()}
              >
                Delete
              </Button>
            </div>
          </Show>

          <Show when={tab() === "prompts"}>
            <div class="comp-lib__list" role="listbox">
              <For each={promptRows()} fallback={<div class="comp-lib__empty">No prompts.</div>}>
                {(row) => (
                  <div
                    class={`comp-lib__row${selectedPromptId() === row.record.id ? " comp-lib__row--selected" : ""}`}
                    role="option"
                    aria-selected={selectedPromptId() === row.record.id}
                    onClick={() => setSelectedPromptId(row.record.id)}
                    onDblClick={() => editPrompt(row)}
                  >
                    <span class="comp-lib__row-name">{row.record.name}</span>
                    <Show when={row.builtin}>
                      <span class="comp-lib__badge">built-in</span>
                    </Show>
                  </div>
                )}
              </For>
            </div>
            <div class="comp-lib__list-actions">
              <Button size="sm" onClick={newPrompt}>
                New
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedPromptId() === null}
                onClick={() => {
                  const row = selectedPromptRow();
                  if (row) editPrompt(row);
                }}
              >
                Edit
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedPromptId() === null}
                onClick={duplicatePrompt}
              >
                Duplicate
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={selectedPromptId() === null || !selectedPromptRow()?.builtin}
                onClick={() => void forkPrompt()}
              >
                Fork built-in
              </Button>
              <Button
                variant="danger"
                size="sm"
                disabled={selectedPromptId() === null || (selectedPromptRow()?.builtin ?? false)}
                onClick={() => void deleteSelectedPrompt()}
              >
                Delete
              </Button>
            </div>
          </Show>
        </div>

        {/* ── Right pane: editor ── */}
        <div class="comp-lib__pane comp-lib__pane--right">
          <Show
            when={isEditing()}
            fallback={
              <div class="comp-lib__placeholder">Select a record and click Edit, or click New.</div>
            }
          >
            <div class="comp-lib__editor-wrap">
              <div class="comp-lib__editor-toolbar">
                <Show when={!editReadOnly()}>
                  <Button
                    size="sm"
                    onClick={() => {
                      if (tab() === "structures") {
                        const s = editingStructure();
                        if (s) void saveStructure(s);
                      } else if (tab() === "styles") {
                        const s = editingStyle();
                        if (s) void saveStyle(s);
                      } else {
                        const p = editingPrompt();
                        if (p) void savePrompt(p);
                      }
                    }}
                  >
                    Save
                  </Button>
                </Show>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setEditingStructure(null);
                    setEditingStyle(null);
                    setEditingPrompt(null);
                  }}
                >
                  Cancel
                </Button>
                <Show when={editReadOnly()}>
                  <span style={{ "font-size": "11px", color: "var(--text-secondary)" }}>
                    Read-only (built-in)
                  </span>
                </Show>
              </div>
              <div class="comp-lib__editor-body">
                <Show when={tab() === "structures" && editingStructure() !== null}>
                  <StructureEditor
                    value={editingStructure()!}
                    onChange={setEditingStructure}
                    readOnly={editReadOnly()}
                  />
                </Show>
                <Show when={tab() === "styles" && editingStyle() !== null}>
                  <StyleEditor
                    value={editingStyle()!}
                    onChange={setEditingStyle}
                    readOnly={editReadOnly()}
                  />
                </Show>
                <Show when={tab() === "prompts" && editingPrompt() !== null}>
                  <PromptEditor
                    value={editingPrompt()!}
                    onChange={setEditingPrompt}
                    readOnly={editReadOnly()}
                    structures={allStructures()}
                    styles={allStyles()}
                  />
                </Show>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
};

export default CompositionLibraryPanel;
