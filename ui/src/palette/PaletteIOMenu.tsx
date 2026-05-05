// Palette import / export menu.
//
// Import uses a hidden <input type="file"> — no Tauri plugin required; the
// browser's FileReader reads the file contents and the parsed colors are
// passed back through the callback.
//
// Export builds a Blob and triggers a download via a temporary <a> element.
//
// Formats: .gpl (GIMP), .hex (Lospec), .pal (JASC-PAL text variant).

import { createSignal, Show, type Component } from "solid-js";
import type { Rgba } from "../lib/types";
import { activePalette } from "./palette-panel-state";
import {
  parseGpl,
  toGpl,
  parseHexFile,
  toHexFile,
  parseJascPal,
  toJascPal,
  type ParsedColor,
} from "./color-utils";

type Props = {
  /** Called with parsed colors when an import succeeds. */
  onImport: (colors: Rgba[]) => void;
};

type Format = "gpl" | "hex" | "pal";

const FORMAT_EXTENSIONS: Record<Format, string> = {
  gpl: ".gpl",
  hex: ".hex",
  pal: ".pal",
};

const FORMAT_ACCEPT: Record<Format, string> = {
  gpl: ".gpl",
  hex: ".hex,.txt",
  pal: ".pal",
};

const PaletteIOMenu: Component<Props> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [importFormat, setImportFormat] = createSignal<Format>("gpl");
  const [exportFormat, setExportFormat] = createSignal<Format>("gpl");
  const [error, setError] = createSignal<string | null>(null);

  // Solid binds the ref synchronously when the <input> mounts, before any
  // user click can fire handleImportClick. Use the definite-assignment
  // assertion (`!`) so we can drop the redundant truthiness check.
  let fileInput!: HTMLInputElement;

  const handleImportClick = () => {
    setError(null);
    fileInput.accept = FORMAT_ACCEPT[importFormat()];
    fileInput.click();
  };

  const handleFileChange = (e: Event) => {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const text = reader.result as string;
      let parsed: ParsedColor[] = [];
      try {
        switch (importFormat()) {
          case "gpl":
            parsed = parseGpl(text);
            break;
          case "hex":
            parsed = parseHexFile(text);
            break;
          case "pal":
            parsed = parseJascPal(text);
            break;
        }
      } catch {
        setError("Failed to parse file. Check the format.");
        return;
      }
      if (parsed.length === 0) {
        setError("No valid colors found in file.");
        return;
      }
      const colors: Rgba[] = parsed.map((c) => ({ r: c.r, g: c.g, b: c.b, a: 255 }));
      props.onImport(colors);
      setOpen(false);
    };
    reader.readAsText(file);
    // Reset so the same file can be re-selected if needed.
    input.value = "";
  };

  const handleExport = () => {
    const p = activePalette();
    if (!p) return;
    const fmt = exportFormat();
    let content = "";
    const colors = p.colors.map((e) => ({ ...e.color, name: e.name ?? null }));

    switch (fmt) {
      case "gpl":
        content = toGpl(p.name, colors);
        break;
      case "hex":
        content = toHexFile(colors);
        break;
      case "pal":
        content = toJascPal(colors);
        break;
    }

    const slug = p.name.toLowerCase().replace(/\s+/g, "-");
    triggerDownload(content, `${slug}${FORMAT_EXTENSIONS[fmt]}`, "text/plain");
    setOpen(false);
  };

  return (
    <div class="pio">
      <button class="pio__toggle" onClick={() => setOpen((v) => !v)} aria-expanded={open()}>
        Import / Export
      </button>

      <Show when={open()}>
        <div class="pio__panel">
          {/* Import */}
          <div class="pio__section">
            <p class="pio__heading">Import palette</p>
            <div class="pio__row">
              <select
                class="pio__select"
                value={importFormat()}
                onChange={(e) => setImportFormat(e.currentTarget.value as Format)}
              >
                <option value="gpl">GIMP Palette (.gpl)</option>
                <option value="hex">Lospec Hex (.hex)</option>
                <option value="pal">JASC-PAL (.pal)</option>
              </select>
              <button class="pio__btn" onClick={handleImportClick}>
                Choose file…
              </button>
            </div>
            <Show when={error() !== null}>
              <p class="pio__error">{error()}</p>
            </Show>
          </div>

          <div class="pio__divider" />

          {/* Export */}
          <div class="pio__section">
            <p class="pio__heading">Export palette</p>
            <div class="pio__row">
              <select
                class="pio__select"
                value={exportFormat()}
                onChange={(e) => setExportFormat(e.currentTarget.value as Format)}
              >
                <option value="gpl">GIMP Palette (.gpl)</option>
                <option value="hex">Lospec Hex (.hex)</option>
                <option value="pal">JASC-PAL (.pal)</option>
              </select>
              <button class="pio__btn" onClick={handleExport} disabled={activePalette() === null}>
                Export
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Hidden file input — always mounted so it can receive programmatic clicks. */}
      <input ref={fileInput} type="file" style={{ display: "none" }} onChange={handleFileChange} />
    </div>
  );
};

function triggerDownload(content: string, filename: string, mime: string): void {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export default PaletteIOMenu;
