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
} from "../../lib/types";
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
} from "../../lib/commands/library";
import { open, save } from "../../lib/dialog";
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

const LibraryPanel: Component = () => {
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

  // ── data loading ────────────────────────────────────────────────────────────

  async function reload(): Promise<void> {
    setLoading(true);
    setError(null);
    try {
      setLibrary(await listComposition());
    } catch (e) {
      setError(String(e));
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
    const id = selectedStructureId();
    const row = structureRows().find((r) => r.record.id === id);
    if (!row) return;
    const copy: Structure = {
      ...row.record,
      id: generateId("project.structure"),
      name: `${row.record.name} copy`,
    };
    setEditingStructure(copy);
    setEditReadOnly(false);
  }

  async function forkStructure(): Promise<void> {
    const id = selectedStructureId();
    const row = structureRows().find((r) => r.record.id === id && r.builtin);
    if (!row) return;
    try {
      await forkBuiltin("structure" as CompositionKind, row.record.id, true);
      await reload();
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
    }
  }

  async function saveStructure(s: Structure): Promise<void> {
    try {
      await upsertStructure(s);
      setEditingStructure(null);
      await reload();
    } catch (e) {
      setError(String(e));
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
    const id = selectedStyleId();
    const row = styleRows().find((r) => r.record.id === id);
    if (!row) return;
    const copy: Style = {
      ...row.record,
      id: generateId("project.style"),
      name: `${row.record.name} copy`,
    };
    setEditingStyle(copy);
    setEditReadOnly(false);
  }

  async function forkStyle(): Promise<void> {
    const id = selectedStyleId();
    const row = styleRows().find((r) => r.record.id === id && r.builtin);
    if (!row) return;
    try {
      await forkBuiltin("style" as CompositionKind, row.record.id, true);
      await reload();
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
    }
  }

  async function saveStyle(s: Style): Promise<void> {
    try {
      await upsertStyle(s);
      setEditingStyle(null);
      await reload();
    } catch (e) {
      setError(String(e));
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
    const id = selectedPromptId();
    const row = promptRows().find((r) => r.record.id === id);
    if (!row) return;
    const copy: PromptTemplate = {
      ...row.record,
      id: generateId("project.prompt"),
      name: `${row.record.name} copy`,
    };
    setEditingPrompt(copy);
    setEditReadOnly(false);
  }

  async function forkPrompt(): Promise<void> {
    const id = selectedPromptId();
    const row = promptRows().find((r) => r.record.id === id && r.builtin);
    if (!row) return;
    try {
      await forkBuiltin("prompt" as CompositionKind, row.record.id, true);
      await reload();
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
    }
  }

  async function savePrompt(p: PromptTemplate): Promise<void> {
    try {
      await upsertPrompt(p);
      setEditingPrompt(null);
      await reload();
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
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
      setError(String(e));
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
      setError(String(e));
    }
  }

  // ── render ──────────────────────────────────────────────────────────────────

  return (
    <div class="lib-panel">
      {/* Tab strip */}
      <div class="lib-panel__tabs" role="tablist">
        <button
          class={`lib-panel__tab${tab() === "structures" ? " lib-panel__tab--active" : ""}`}
          role="tab"
          type="button"
          onClick={() => setTab("structures")}
        >
          Structures
        </button>
        <button
          class={`lib-panel__tab${tab() === "styles" ? " lib-panel__tab--active" : ""}`}
          role="tab"
          type="button"
          onClick={() => setTab("styles")}
        >
          Styles
        </button>
        <button
          class={`lib-panel__tab${tab() === "prompts" ? " lib-panel__tab--active" : ""}`}
          role="tab"
          type="button"
          onClick={() => setTab("prompts")}
        >
          Prompts
        </button>
      </div>

      {/* Error / status bar */}
      <Show when={error() !== null}>
        <div class="lib-panel__status">{error()}</div>
      </Show>

      {/* Global toolbar */}
      <div class="lib-panel__toolbar">
        <button class="lib-btn lib-btn--sm" type="button" onClick={handleImport}>
          Import .pixstyle
        </button>
        <button class="lib-btn lib-btn--sm" type="button" onClick={handleExport}>
          Export all
        </button>
        <button class="lib-btn lib-btn--sm" type="button" onClick={handleCopyFromProject}>
          Copy from project
        </button>
        <button class="lib-btn lib-btn--sm" type="button" onClick={() => reload()}>
          {loading() ? "Loading..." : "Refresh"}
        </button>
      </div>

      {/* ── Structures tab ────────────────────────────────────────────────── */}
      <Show when={tab() === "structures"}>
        <Show
          when={editingStructure() === null}
          fallback={
            <div class="lib-panel__editor">
              <div class="lib-panel__editor-toolbar">
                <Show when={!editReadOnly()}>
                  <button
                    class="lib-btn lib-btn--primary lib-btn--sm"
                    type="button"
                    onClick={() => {
                      const s = editingStructure();
                      if (s) void saveStructure(s);
                    }}
                  >
                    Save
                  </button>
                </Show>
                <button
                  class="lib-btn lib-btn--sm"
                  type="button"
                  onClick={() => setEditingStructure(null)}
                >
                  Cancel
                </button>
              </div>
              <StructureEditor
                value={editingStructure()!}
                onChange={setEditingStructure}
                readOnly={editReadOnly()}
              />
            </div>
          }
        >
          <div class="lib-panel__list-toolbar">
            <button class="lib-btn lib-btn--sm" type="button" onClick={newStructure}>
              New
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={selectedStructureId() === null}
              onClick={() => {
                const row = structureRows().find((r) => r.record.id === selectedStructureId());
                if (row) editStructure(row);
              }}
            >
              Edit
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={selectedStructureId() === null}
              onClick={duplicateStructure}
            >
              Duplicate
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={
                selectedStructureId() === null ||
                !structureRows().find((r) => r.record.id === selectedStructureId() && r.builtin)
              }
              onClick={() => void forkStructure()}
            >
              Fork built-in
            </button>
            <button
              class="lib-btn lib-btn--danger lib-btn--sm"
              type="button"
              disabled={
                selectedStructureId() === null ||
                structureRows().some((r) => r.record.id === selectedStructureId() && r.builtin)
              }
              onClick={() => void deleteSelectedStructure()}
            >
              Delete
            </button>
          </div>

          <div class="lib-panel__list" role="listbox">
            <For
              each={structureRows()}
              fallback={<div class="lib-panel__empty">No structures.</div>}
            >
              {(row) => (
                <div
                  class={`lib-panel__row${selectedStructureId() === row.record.id ? " lib-panel__row--selected" : ""}`}
                  role="option"
                  aria-selected={selectedStructureId() === row.record.id}
                  onClick={() => setSelectedStructureId(row.record.id)}
                  onDblClick={() => editStructure(row)}
                >
                  <span class="lib-panel__row-name">{row.record.name}</span>
                  <Show when={row.builtin}>
                    <span class="lib-panel__badge">built-in</span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      {/* ── Styles tab ────────────────────────────────────────────────────── */}
      <Show when={tab() === "styles"}>
        <Show
          when={editingStyle() === null}
          fallback={
            <div class="lib-panel__editor">
              <div class="lib-panel__editor-toolbar">
                <Show when={!editReadOnly()}>
                  <button
                    class="lib-btn lib-btn--primary lib-btn--sm"
                    type="button"
                    onClick={() => {
                      const s = editingStyle();
                      if (s) void saveStyle(s);
                    }}
                  >
                    Save
                  </button>
                </Show>
                <button
                  class="lib-btn lib-btn--sm"
                  type="button"
                  onClick={() => setEditingStyle(null)}
                >
                  Cancel
                </button>
              </div>
              <StyleEditor
                value={editingStyle()!}
                onChange={setEditingStyle}
                readOnly={editReadOnly()}
              />
            </div>
          }
        >
          <div class="lib-panel__list-toolbar">
            <button class="lib-btn lib-btn--sm" type="button" onClick={newStyle}>
              New
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={selectedStyleId() === null}
              onClick={() => {
                const row = styleRows().find((r) => r.record.id === selectedStyleId());
                if (row) editStyle(row);
              }}
            >
              Edit
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={selectedStyleId() === null}
              onClick={duplicateStyle}
            >
              Duplicate
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={
                selectedStyleId() === null ||
                !styleRows().find((r) => r.record.id === selectedStyleId() && r.builtin)
              }
              onClick={() => void forkStyle()}
            >
              Fork built-in
            </button>
            <button
              class="lib-btn lib-btn--danger lib-btn--sm"
              type="button"
              disabled={
                selectedStyleId() === null ||
                styleRows().some((r) => r.record.id === selectedStyleId() && r.builtin)
              }
              onClick={() => void deleteSelectedStyle()}
            >
              Delete
            </button>
          </div>

          <div class="lib-panel__list" role="listbox">
            <For each={styleRows()} fallback={<div class="lib-panel__empty">No styles.</div>}>
              {(row) => (
                <div
                  class={`lib-panel__row${selectedStyleId() === row.record.id ? " lib-panel__row--selected" : ""}`}
                  role="option"
                  aria-selected={selectedStyleId() === row.record.id}
                  onClick={() => setSelectedStyleId(row.record.id)}
                  onDblClick={() => editStyle(row)}
                >
                  <span class="lib-panel__row-name">{row.record.name}</span>
                  <Show when={row.builtin}>
                    <span class="lib-panel__badge">built-in</span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      {/* ── Prompts tab ───────────────────────────────────────────────────── */}
      <Show when={tab() === "prompts"}>
        <Show
          when={editingPrompt() === null}
          fallback={
            <div class="lib-panel__editor">
              <div class="lib-panel__editor-toolbar">
                <Show when={!editReadOnly()}>
                  <button
                    class="lib-btn lib-btn--primary lib-btn--sm"
                    type="button"
                    onClick={() => {
                      const p = editingPrompt();
                      if (p) void savePrompt(p);
                    }}
                  >
                    Save
                  </button>
                </Show>
                <button
                  class="lib-btn lib-btn--sm"
                  type="button"
                  onClick={() => setEditingPrompt(null)}
                >
                  Cancel
                </button>
              </div>
              <PromptEditor
                value={editingPrompt()!}
                onChange={setEditingPrompt}
                readOnly={editReadOnly()}
                structures={allStructures()}
                styles={allStyles()}
              />
            </div>
          }
        >
          <div class="lib-panel__list-toolbar">
            <button class="lib-btn lib-btn--sm" type="button" onClick={newPrompt}>
              New
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={selectedPromptId() === null}
              onClick={() => {
                const row = promptRows().find((r) => r.record.id === selectedPromptId());
                if (row) editPrompt(row);
              }}
            >
              Edit
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={selectedPromptId() === null}
              onClick={duplicatePrompt}
            >
              Duplicate
            </button>
            <button
              class="lib-btn lib-btn--sm"
              type="button"
              disabled={
                selectedPromptId() === null ||
                !promptRows().find((r) => r.record.id === selectedPromptId() && r.builtin)
              }
              onClick={() => void forkPrompt()}
            >
              Fork built-in
            </button>
            <button
              class="lib-btn lib-btn--danger lib-btn--sm"
              type="button"
              disabled={
                selectedPromptId() === null ||
                promptRows().some((r) => r.record.id === selectedPromptId() && r.builtin)
              }
              onClick={() => void deleteSelectedPrompt()}
            >
              Delete
            </button>
          </div>

          <div class="lib-panel__list" role="listbox">
            <For each={promptRows()} fallback={<div class="lib-panel__empty">No prompts.</div>}>
              {(row) => (
                <div
                  class={`lib-panel__row${selectedPromptId() === row.record.id ? " lib-panel__row--selected" : ""}`}
                  role="option"
                  aria-selected={selectedPromptId() === row.record.id}
                  onClick={() => setSelectedPromptId(row.record.id)}
                  onDblClick={() => editPrompt(row)}
                >
                  <span class="lib-panel__row-name">{row.record.name}</span>
                  <Show when={row.builtin}>
                    <span class="lib-panel__badge">built-in</span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
};

export default LibraryPanel;
