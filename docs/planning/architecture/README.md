# Architecture

Three documents that lock the technical foundation:

1. **[rust-vs-electron.md](rust-vs-electron.md)** — the direct answer to "is Rust a better alternative to Electron?" Yes, with reasoning.
2. **[stack.md](stack.md)** — the full locked stack: Tauri + Rust workspace + TypeScript/Solid UI + WebGL2 viewport + MessagePack project format + Lua scripting + multi-backend AI runtime + Unity-only engine target.
3. (To come) — the IPC command catalog (`docs/ipc-commands.md`), file format spec (`docs/file-format.md`), verb plugin protocol (`docs/verb-protocol.md`). These are produced as part of the bedrock work in `../work/bedrock.md`.

If a stream in `../work/streams.md` needs an architectural decision that isn't in `stack.md`, the answer goes here as a new doc.
