# Invert Colors Verb — Pixhaus WASM plugin example

A minimal Pixhaus plugin written in Rust and compiled to WebAssembly. It registers one AI verb: **Invert Colors**, which inverts the RGB channels of every pixel on the active layer.

## What it demonstrates

- `pixhaus_describe` — how to return a verb descriptor the host reads at load time
- `pixhaus_verb_run` — how to receive pixel bytes, transform them, and return the result
- Preview-then-commit — the host shows a before/after diff; the user accepts or cancels

## Build

```sh
rustup target add wasm32-wasip1
./build.sh
```

The script compiles the Rust crate and copies `plugin.wasm` into this folder.

## Install

Copy this folder to `~/.pixhaus/plugins/`:

```sh
cp -r . ~/.pixhaus/plugins/invert-colors-verb/
```

The editor hot-reloads when `plugin.wasm` changes, so you can build and test without restarting.

## How it works

The host loads `plugin.wasm` and calls `pixhaus_describe` once. The returned JSON tells the host this is a verb with ID `com.pixhaus.examples.invert-colors`.

When the user picks "Invert Colors" from the AI menu, the host:

1. Gathers the active layer's pixel bytes for the active frame.
2. Calls `pixhaus_verb_run` with a JSON payload `{ pixels, width, height }`.
3. Receives the modified pixel bytes in the response.
4. Shows a preview panel — before on the left, after on the right.
5. On "Accept": commits the change as a single undo entry.
6. On "Cancel": discards the result.

## See also

- [WASM plugins guide](https://docs.pixhaus.app/plugins/wasm-plugins/)
- [AI verb authoring guide](https://docs.pixhaus.app/plugins/ai-verb-authoring/)
