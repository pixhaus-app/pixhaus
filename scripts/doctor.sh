#!/usr/bin/env bash
# Doctor: read-only environment check.
# Prints PASS / WARN / FAIL per probe, grouped by section. WARNs don't fail
# the script; FAILs do. Exit 0 if zero FAILs, 1 otherwise.
#
# Never prints the value of secrets (only whether they are set).

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

pass_count=0
warn_count=0
fail_count=0

pass()    { pass_count=$((pass_count + 1)); printf 'PASS  %s\n' "$1"; }
warn()    { warn_count=$((warn_count + 1)); printf 'WARN  %s%s\n' "$1" "${2:+ ($2)}"; }
fail()    { fail_count=$((fail_count + 1)); printf 'FAIL  %s%s\n' "$1" "${2:+ ($2)}"; }
section() { printf '\n== %s ==\n' "$1"; }

# ---- Toolchain ----
section "Toolchain"

if command -v rustup >/dev/null 2>&1; then
    pass "rustup"
else
    fail "rustup" "install from https://rustup.rs"
fi

if command -v rustc >/dev/null 2>&1; then
    expected_channel=""
    if [ -f rust-toolchain.toml ]; then
        expected_channel="$(grep -E '^channel[[:space:]]*=' rust-toolchain.toml | head -n1 | sed -E 's/^channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')"
    fi
    rustc_version="$(rustc --version 2>/dev/null | awk '{print $2}')"
    if [ -z "$expected_channel" ]; then
        pass "rustc ($rustc_version)"
    elif printf '%s' "$rustc_version" | grep -qE "^${expected_channel//./\\.}(\\.|$)"; then
        pass "rust toolchain ($rustc_version, expected $expected_channel)"
    else
        fail "rust toolchain" "have $rustc_version, expected $expected_channel; run: rustup install $expected_channel"
    fi
else
    fail "rustc" "rustup install $(grep -E '^channel[[:space:]]*=' rust-toolchain.toml 2>/dev/null | sed -E 's/^channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/' || echo stable)"
fi

if command -v cargo >/dev/null 2>&1; then
    pass "cargo"
else
    fail "cargo" "install Rust via rustup"
fi

# ---- Cargo subcommands ----
section "Cargo subcommands"

for tool in cargo-nextest cargo-deny cargo-audit cargo-machete cargo-watch typos bacon cargo-tauri; do
    if command -v "$tool" >/dev/null 2>&1; then
        pass "$tool"
    else
        case "$tool" in
            typos)        warn "$tool" "cargo install typos-cli" ;;
            cargo-tauri)  warn "$tool" "cargo install tauri-cli --version ^2.0.0" ;;
            *)            warn "$tool" "cargo install $tool" ;;
        esac
    fi
done

# ---- Node ----
section "Node"

if command -v node >/dev/null 2>&1; then
    node_major="$(node --version 2>/dev/null | sed -E 's/^v([0-9]+).*/\1/')"
    if [ -n "$node_major" ] && [ "$node_major" -ge 22 ] 2>/dev/null; then
        pass "node ($(node --version))"
    else
        fail "node" "have $(node --version 2>/dev/null || echo missing), need >= 22; install from https://nodejs.org"
    fi
else
    fail "node" "install from https://nodejs.org (need >= 22)"
fi

if command -v pnpm >/dev/null 2>&1; then
    pnpm_major="$(pnpm --version 2>/dev/null | cut -d. -f1)"
    if [ -n "$pnpm_major" ] && [ "$pnpm_major" -ge 10 ] 2>/dev/null; then
        pass "pnpm ($(pnpm --version))"
    else
        fail "pnpm" "have $(pnpm --version 2>/dev/null || echo missing), need >= 10; corepack enable pnpm"
    fi
else
    fail "pnpm" "corepack enable pnpm (or: npm install -g pnpm)"
fi

# ---- Git ----
section "Git"

if command -v git >/dev/null 2>&1; then
    pass "git"
else
    fail "git" "install git"
fi

if git rev-parse --git-dir >/dev/null 2>&1; then
    pass "inside a git repo"
else
    fail "git repo" "run from a git checkout"
fi

hooks_path="$(git config --get core.hooksPath 2>/dev/null || true)"
if [ "$hooks_path" = ".githooks" ]; then
    pass "core.hooksPath = .githooks"
else
    fail "core.hooksPath" "currently '${hooks_path:-unset}'; run: pnpm bootstrap (or: node scripts/run.mjs setup-git-hooks)"
fi

if [ -f .githooks/pre-commit ]; then
    if [ "$(uname -s 2>/dev/null)" = "Darwin" ] || [ "$(uname -s 2>/dev/null)" = "Linux" ]; then
        if [ -x .githooks/pre-commit ]; then
            pass ".githooks/pre-commit executable"
        else
            warn ".githooks/pre-commit" "not executable; run: chmod +x .githooks/pre-commit"
        fi
    fi
else
    fail ".githooks/pre-commit" "missing; the repo scaffold ships it"
fi

# ---- GitHub auth ----
section "GitHub auth"

if command -v gh >/dev/null 2>&1; then
    pass "gh CLI"
    if gh auth status >/dev/null 2>&1; then
        pass "gh authenticated"
    else
        warn "gh auth" "run: gh auth login"
    fi
else
    warn "gh CLI" "install from https://cli.github.com (needed to open PRs from ralph)"
fi

# ---- Anthropic ----
section "Anthropic"

# Pixhaus uses Claude Code via subscription (Pro/Max). The claude CLI handles
# auth itself via OAuth login; ANTHROPIC_API_KEY is only needed for direct
# API mode. Don't print its value if set.
if command -v claude >/dev/null 2>&1; then
    pass "claude CLI"
else
    fail "claude CLI" "install Claude Code; see https://claude.com/claude-code"
fi

if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
    pass "ANTHROPIC_API_KEY set (API mode)"
else
    pass "ANTHROPIC_API_KEY not set (subscription mode; claude CLI handles auth)"
fi

# ---- Repo state ----
section "Repo state"

if [ -f work/queue.md ]; then
    unclaimed_count="$(grep -cE '^- \[ \] UNCLAIMED:' work/queue.md 2>/dev/null || echo 0)"
    if [ "$unclaimed_count" -gt 0 ]; then
        pass "work/queue.md ($unclaimed_count unclaimed)"
    else
        fail "work/queue.md" "no UNCLAIMED tasks; nothing to dispatch"
    fi
else
    fail "work/queue.md" "missing"
fi

# Disk space. POSIX df: -P forces standard format, -k for KB. Linux and macOS
# both support this; the GB conversion follows.
free_kb="$(df -P -k . 2>/dev/null | awk 'NR==2 {print $4}')"
if [ -n "$free_kb" ]; then
    free_gb=$((free_kb / 1024 / 1024))
    if [ "$free_gb" -ge 5 ]; then
        pass "disk space (${free_gb}G free)"
    else
        warn "disk space" "${free_gb}G free; worktrees + cargo target/ want >= 5G"
    fi
else
    warn "disk space" "could not determine free space"
fi

# ---- Summary ----
printf '\nSummary: %d PASS, %d WARN, %d FAIL\n' "$pass_count" "$warn_count" "$fail_count"

if [ "$fail_count" -gt 0 ]; then
    exit 1
fi
exit 0
