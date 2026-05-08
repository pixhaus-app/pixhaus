import { createMemo, createSignal, createEffect, For, type Component } from "solid-js";
import { closeCommandPalette } from "../palette-state";
import { getAllCommands, dispatchCommand, type Command } from "./command-registry";

type CommandWithKeybind = Command & { keybind?: string };

type Segment = { text: string; highlighted: boolean };

// Returns true when every character of query appears in order in text.
function fuzzyMatch(query: string, text: string): boolean {
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  let qi = 0;
  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (q[qi] === t[ti]) qi++;
  }
  return qi === q.length;
}

// Lower score = better match.
function fuzzyScore(query: string, text: string): number {
  if (!query) return 0;
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  const exactIdx = t.indexOf(q);
  if (exactIdx === 0) return 0;
  if (exactIdx > 0) return 1;
  return 2;
}

function matchesQuery(query: string, cmd: CommandWithKeybind): boolean {
  if (!query) return true;
  return [cmd.label, cmd.category, ...(cmd.keywords ?? [])].some((f) => fuzzyMatch(query, f));
}

function scoreCommand(query: string, cmd: CommandWithKeybind): number {
  if (!query) return 0;
  return Math.min(fuzzyScore(query, cmd.label), fuzzyScore(query, cmd.category));
}

// Splits text into highlighted/plain segments matching the query subsequence.
function buildSegments(query: string, text: string): Segment[] {
  if (!query) return [{ text, highlighted: false }];
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  const segments: Segment[] = [];
  let qi = 0;
  let segStart = 0;
  let inHighlight = false;

  for (let ti = 0; ti < text.length; ti++) {
    if (qi < q.length && t[ti] === q[qi]) {
      if (!inHighlight) {
        if (ti > segStart) {
          segments.push({ text: text.slice(segStart, ti), highlighted: false });
        }
        segStart = ti;
        inHighlight = true;
      }
      qi++;
    } else if (inHighlight) {
      segments.push({ text: text.slice(segStart, ti), highlighted: true });
      segStart = ti;
      inHighlight = false;
    }
  }
  if (segStart < text.length) {
    segments.push({ text: text.slice(segStart), highlighted: inHighlight });
  }
  return segments;
}

// Renders label text with fuzzy-matched characters wrapped in <mark>.
function HighlightedLabel(props: { query: string; text: string }) {
  const segments = createMemo(() => buildSegments(props.query, props.text));
  return (
    <For each={segments()}>
      {(seg) => (seg.highlighted ? <mark>{seg.text}</mark> : <span>{seg.text}</span>)}
    </For>
  );
}

const CommandPalette: Component = () => {
  const [query, setQuery] = createSignal("");
  const [selectedIndex, setSelectedIndex] = createSignal(0);

  let inputRef: HTMLInputElement | undefined;

  // Focus input when palette mounts
  createEffect(() => {
    inputRef?.focus();
  });

  // Reset selection when query changes
  createEffect(() => {
    query(); // track
    setSelectedIndex(0);
  });

  const filteredCommands = createMemo<CommandWithKeybind[]>(() => {
    const q = query().trim();
    const all = getAllCommands();
    const matched = all.filter((cmd) => matchesQuery(q, cmd));
    if (!q) return matched;
    return matched.sort((a, b) => scoreCommand(q, a) - scoreCommand(q, b));
  });

  function closeAndReset(): void {
    setQuery("");
    setSelectedIndex(0);
    closeCommandPalette();
  }

  function runSelected(): void {
    const cmds = filteredCommands();
    const idx = selectedIndex();
    const cmd = cmds[idx];
    if (cmd) {
      closeAndReset();
      dispatchCommand(cmd.id);
    }
  }

  function onKeyDown(e: KeyboardEvent): void {
    const cmds = filteredCommands();
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        closeAndReset();
        break;
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, cmds.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        runSelected();
        break;
    }
  }

  return (
    <div
      class="palette-backdrop"
      onClick={(e) => {
        if (e.target === e.currentTarget) closeAndReset();
      }}
    >
      <div class="palette" role="dialog" aria-label="Command Palette" data-testid="command-palette">
        <div class="palette__input-row">
          <input
            ref={inputRef}
            class="palette__input"
            data-testid="command-palette-input"
            type="text"
            placeholder="Type a command..."
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
            onKeyDown={onKeyDown}
            aria-autocomplete="list"
            autocomplete="off"
            spellcheck={false}
          />
        </div>
        <div class="palette__results" role="listbox" aria-label="Commands">
          <For each={filteredCommands()}>
            {(cmd, index) => (
              <>
                {(index() === 0 || filteredCommands()[index() - 1]?.category !== cmd.category) && (
                  <div class="palette__category">{cmd.category}</div>
                )}
                <div
                  class="palette__item"
                  data-testid={`command-palette-item-${cmd.id}`}
                  role="option"
                  aria-selected={index() === selectedIndex()}
                  onClick={() => {
                    closeAndReset();
                    dispatchCommand(cmd.id);
                  }}
                  onMouseEnter={() => setSelectedIndex(index())}
                >
                  <span class="palette__item__label">
                    <HighlightedLabel query={query().trim()} text={cmd.label} />
                  </span>
                  <span class="palette__item__category">{cmd.category}</span>
                  {cmd.keybind !== undefined && <kbd class="palette__item__kbd">{cmd.keybind}</kbd>}
                </div>
              </>
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

export default CommandPalette;
