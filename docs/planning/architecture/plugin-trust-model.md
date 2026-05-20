# Plugin and script trust model

Pixhaus runs two kinds of extension code: Lua scripts via `mlua` and WASM
plugins via `extism`. This document states what each is allowed to do, why
the current limits exist, and the one hard rule that gates everything:
**Pixhaus does not load untrusted, third-party plugins yet.** Extensions are
developer-authored and shipped in-repo (`plugins/`, `examples/plugins/`).
Until the WASM runtime is patched (see below), that is a security boundary,
not just a convention.

## Why this matters

Extension code runs in the editor process. A script or plugin that escapes
its sandbox can read the user's files, exfiltrate data, or run arbitrary
code. The mitigations below assume the author is trusted. They are
defense-in-depth, not a license to load arbitrary code from the internet.

## Lua scripts

The Lua VM (`scripting/src/runtime.rs`) loads a restricted standard library.

- `LuaRuntime::new()` — the sandboxed default. Loads only `coroutine`,
  `table`, `string`, `math`, and `utf8`. It does **not** load `os`, `io`,
  `debug`, `package`, or `ffi`. A sandboxed script cannot touch the
  filesystem, spawn processes, read wall-clock time, load native code, or
  use the `debug` library to reach outside the VM. All pixel and project
  access is mediated by the host `app` and `Color` globals.
- `LuaRuntime::new_trusted()` — widens the sandbox with `os` and `io` for
  vetted, in-repo scripts that genuinely need them (the `palette-export`
  sample writes a file via `io`). `debug`, `package`, and `ffi` stay off
  even here. Only the host may choose this constructor, and only for code
  it has reviewed.

The split is deliberate: capability is opt-in per script, and the
unrestricted libraries are never reachable by a script the host has not
explicitly elevated.

## WASM plugins

The extism host (`plugins/src/wasm/mod.rs`) loads each plugin with a
deny-all capability posture:

- No `allowed_paths` — no preopened directories, so no filesystem access.
- No `allowed_hosts` — no network access.
- WASI is enabled only so that guest toolchains targeting `wasm32-wasip1`
  link; the host exposes nothing through it.

Adding any allowed path or host is a trust-model change and must be
reviewed against this document.

## The wasmtime advisory gate

`cargo-deny` (`.cargo/deny.toml`) flags a cluster of security advisories in
`wasmtime 41`, pulled in transitively by `extism 1.21` (the latest release,
which pins wasmtime 41). They range from denial-of-service panics to
**sandbox-escaping memory accesses** (RUSTSEC-2026-0085..0096, 0114). No
semver-compatible fix exists yet — the upstream fixes land in
wasmtime >= 43.0.2.

These advisories are ignored in `deny.toml` **only** because the WASM host
is reachable solely by developer-authored plugins under this trust model. A
sandbox escape requires a hostile plugin, and Pixhaus does not load those.

Before Pixhaus ever loads untrusted or third-party WASM plugins, both must
be true:

1. `extism` ships a release on a patched wasmtime (>= 43.0.2), and the
   `RUSTSEC-2026-00xx` ignores are removed from `.cargo/deny.toml` so the
   advisory check passes without exceptions.
2. The deny-all extism capability posture is re-reviewed for the
   untrusted-input threat model (resource limits, timeouts, memory caps).

Until then, treat "load a third-party plugin" as a feature that is not yet
safe to build.

## Summary

| Surface | Default capability | Elevated path |
| --- | --- | --- |
| Lua script | `coroutine`/`table`/`string`/`math`/`utf8` only | `new_trusted()` adds `os`/`io` for vetted scripts |
| WASM plugin | no filesystem, no network, WASI for linking only | none — a capability change is a trust-model change |
| Untrusted/third-party plugins | not loadable | blocked until extism ships a patched wasmtime |
