// Lospec palette browser.
//
// Two modes:
//   1. Import by URL/slug: user pastes a Lospec palette URL or slug and we
//      fetch https://lospec.com/palette-list/<slug>.json.
//   2. Search: queries the Lospec palette-list endpoint and shows results.
//
// Both paths ultimately call props.onImport with the resolved RGBA colors.
//
// CSP note: tauri.conf.json has csp: null so external fetches are allowed.

import { createSignal, For, Show, type Component } from "solid-js";
import type { Rgba } from "../lib/types";
import { parseHexFile } from "./color-utils";

type LospecPalette = {
  name: string;
  author: string;
  colors: string[]; // bare 6-digit hex strings
};

type SearchResult = {
  name: string;
  slug: string;
  author: string;
  colors: string[];
};

type Props = {
  onImport: (name: string, colors: Rgba[]) => void;
};

const LOSPEC_BASE = "https://lospec.com/palette-list";

// Lospec's palette-list/load endpoint returns a JSON object with a `palettes`
// array when queried with the right parameters.
const LOSPEC_SEARCH = `${LOSPEC_BASE}/load`;

const LospecBrowser: Component<Props> = (props) => {
  const [tab, setTab] = createSignal<"search" | "url">("search");
  const [query, setQuery] = createSignal("");
  const [urlInput, setUrlInput] = createSignal("");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const clearError = () => setError(null);

  // ── URL import ─────────────────────────────────────────────────────────────

  const slugFromInput = (raw: string): string => {
    // Accept full URLs like https://lospec.com/palette-list/endesga-32
    // or bare slugs like "endesga-32".
    const match = raw.match(/palette-list\/([^/?#]+)/);
    if (match?.[1]) return match[1];
    return raw.trim().replace(/\s+/g, "-").toLowerCase();
  };

  const handleUrlImport = async () => {
    const slug = slugFromInput(urlInput());
    if (!slug) return;
    clearError();
    setLoading(true);
    try {
      const palette = await fetchPalette(slug);
      const colors = hexArrayToRgba(palette.colors);
      props.onImport(palette.name, colors);
      setUrlInput("");
    } catch (err: unknown) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  // ── Search ─────────────────────────────────────────────────────────────────

  const handleSearch = async () => {
    const q = query().trim();
    if (!q) return;
    clearError();
    setLoading(true);
    setResults([]);
    try {
      const url = new URL(LOSPEC_SEARCH);
      url.searchParams.set("colorNumber", "all");
      url.searchParams.set("sortingType", "default");
      url.searchParams.set("tag", q);
      url.searchParams.set("page", "0");

      const res = await fetch(url.toString());
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as { palettes?: SearchResult[] };
      setResults(data.palettes ?? []);
      if ((data.palettes ?? []).length === 0) {
        setError(`No palettes found for "${q}".`);
      }
    } catch (err: unknown) {
      setError(`Search failed: ${errorMessage(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleResultImport = async (result: SearchResult) => {
    clearError();
    setLoading(true);
    try {
      const palette = await fetchPalette(result.slug);
      props.onImport(palette.name, hexArrayToRgba(palette.colors));
    } catch (err: unknown) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div class="lospec">
      {/* Tab switcher */}
      <div class="lospec__tabs" role="tablist">
        <button
          role="tab"
          aria-selected={tab() === "search"}
          class="lospec__tab"
          onClick={() => {
            setTab("search");
            clearError();
          }}
        >
          Search
        </button>
        <button
          role="tab"
          aria-selected={tab() === "url"}
          class="lospec__tab"
          onClick={() => {
            setTab("url");
            clearError();
          }}
        >
          Import URL
        </button>
      </div>

      {/* Search tab */}
      <Show when={tab() === "search"}>
        <div class="lospec__search-row">
          <input
            class="lospec__input"
            type="search"
            placeholder="Search palettes…"
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleSearch();
            }}
            disabled={loading()}
          />
          <button class="lospec__btn" onClick={() => void handleSearch()} disabled={loading()}>
            {loading() ? "…" : "Search"}
          </button>
        </div>

        <Show when={results().length > 0}>
          <div class="lospec__results" role="list">
            <For each={results()}>
              {(result) => (
                <div class="lospec__result" role="listitem">
                  <div class="lospec__result-info">
                    <span class="lospec__result-name">{result.name}</span>
                    <span class="lospec__result-author">by {result.author}</span>
                  </div>
                  <div class="lospec__result-preview" aria-hidden="true">
                    <For each={result.colors.slice(0, 16)}>
                      {(hex) => (
                        <div class="lospec__preview-dot" style={{ background: `#${hex}` }} />
                      )}
                    </For>
                  </div>
                  <button
                    class="lospec__btn lospec__btn--small"
                    onClick={() => void handleResultImport(result)}
                    disabled={loading()}
                  >
                    Import
                  </button>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      {/* URL import tab */}
      <Show when={tab() === "url"}>
        <div class="lospec__url-section">
          <input
            class="lospec__input"
            type="url"
            placeholder="https://lospec.com/palette-list/endesga-32"
            value={urlInput()}
            onInput={(e) => setUrlInput(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleUrlImport();
            }}
            disabled={loading()}
          />
          <button
            class="lospec__btn"
            onClick={() => void handleUrlImport()}
            disabled={loading() || !urlInput()}
          >
            {loading() ? "Fetching…" : "Import"}
          </button>
          <p class="lospec__hint">Paste a Lospec URL or palette slug (e.g. "endesga-32").</p>
        </div>
      </Show>

      <Show when={error() !== null}>
        <p class="lospec__error" role="alert">
          {error()}
        </p>
      </Show>
    </div>
  );
};

// ── Helpers ───────────────────────────────────────────────────────────────────

async function fetchPalette(slug: string): Promise<LospecPalette> {
  const res = await fetch(`${LOSPEC_BASE}/${slug}.json`);
  if (!res.ok) throw new Error(`HTTP ${res.status} fetching "${slug}"`);
  return res.json() as Promise<LospecPalette>;
}

function hexArrayToRgba(hexes: string[]): Rgba[] {
  // Lospec colors are bare 6-digit hex strings without a leading '#'.
  return parseHexFile(hexes.join("\n")).map((c) => ({ r: c.r, g: c.g, b: c.b, a: 255 }));
}

function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}

export default LospecBrowser;
