# AGENTS.md — Contributor Guide for AI Coding Agents

Read [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md), [ROADMAP.md](./ROADMAP.md), and [docs/ADR.md](./docs/ADR.md) before modifying core subsystems.

## Product Vision

Turf turns your terminal into a shared engineering workspace where humans and AI coding CLIs (`codex`, `claude`, `agy`, `gemini`) work on the same repository in parallel without overwriting each other:

1. Host a local repository with `turf host <dir>` and pair collaborators with `turf join <TURF-XXXX>` over an AES-256-GCM encrypted WebSocket tunnel.
2. Run shells, editors (`vim`), and company AI CLIs inside persistent background PTY panes ([turf-daemon](./crates/turf-daemon)) rendered at 60fps in a minimal Herdr-inspired cockpit ([turf-tui](./crates/turf-tui)).
3. Coordinate parallel edits by declaring target files on the Intent Board, acquiring all-or-nothing multi-file locks ([turf-core](./crates/turf-core)), and merging isolated `.turf/worktrees/<task-id>` branches back with `Co-authored-by` trailers.

## Codebase Truth (Read First)

This repository holds two implementations:

| Layer | Path | Status | Contents |
| :--- | :--- | :--- | :--- |
| Native Rust Workspace | [crates/](./crates) | Active shipping CLI (`turf`, `turf-daemon`) | Everything users run: launcher, tunnel, daemon, PTY, VTE, TUI cockpit, locks, worktrees. |
| Node.js ESM Layer | [src/](./src), [bin/](./bin) | Legacy prototype & test harness | Original JS prototype ([src/cli.js](./src/cli.js), [src/cockpit.js](./src/cockpit.js)) plus JS-only modules not yet ported to Rust—notably [src/agent-runner.js](./src/agent-runner.js) (`AgentRunner` `/agent` credential passthrough) and [src/diff-viewer.js](./src/diff-viewer.js). |

Always edit [crates/](./crates) for fixes or features in `turf`. Only edit `src/` when explicitly updating the legacy Node fallback or porting behavior from it into Rust.

## Crate Map

- [turf-pty](./crates/turf-pty) : Cross-platform PTY allocation (`portable-pty`) and foreground process introspection (`/proc` + `tcgetpgrp`).
- [turf-vte](./crates/turf-vte) : 2D `ScreenBuffer`, ANSI/TrueColor state machine, OSC 8 hyperlink tracking, wide-glyph continuation cells, alt-screen isolation, and a 10,000-line scrollback buffer.
- [turf-protocol](./crates/turf-protocol) : Shared contracts (`TaskCard`, `FileLock`, `CoAuthor`) and length-prefixed JSON-RPC daemon IPC messages.
- [turf-core](./crates/turf-core) : Atomic fail-fast `LockManager` (heartbeat TTL & PID liveness), atomic `IntentBoard` disk persistence, and `WorktreeManager`.
- [turf-daemon](./crates/turf-daemon) : Persistent background daemon (`turf-daemon`) over Unix domain socket / Windows named pipe; preserves panes across detaches and passes parent `cwd` on split.
- [turf-tui](./crates/turf-tui) : Ratatui + Crossterm cockpit with full-height split panes, Catppuccin Mocha transparency (`Color::Reset`), scrollable tabs with pinned `+` and tab context menu (`New tab`, `Rename`, `Close`), pane-bounded mouse copy (OSC 52 + `clip.exe`), clickable OSC 8 / wrapped URLs, and centered modals (`F1`, `F2`, `+`).
- [turf-net](./crates/turf-net) : AES-256-GCM WebSocket tunnel (`TunnelServer` / `TunnelClient`) and `turf` CLI entrypoint ([cli.rs](./crates/turf-net/src/cli.rs)).

## UI Taste & Platform Guardrails

Preserve these UX rules from `[MD-050]`–`[MD-056]` in [docs/ADR.md](./docs/ADR.md):

- Keep the `TURF` ASCII box splash clean—never add `ELECTRIC LIME COCKPIT` or `v0.1.0` clutter inside the launcher header.
- Keep terminal panes full height down to the bottom edge (no permanent bottom input bar; `/task`, `/lock`, `/split-h` open inside the `F2` / `Alt+P` modal).
- Keep top tab pills numbered and clean (` 1 `, ` 2 `)—never inject noisy shell names like `: zsh`, and keep `+` permanently pinned with `<` / `>` horizontal overflow so tabs are never capped at 16.
- Preserve Windows `ENG-IN` and Windows Terminal-safe shortcuts (`F1` help, `F2`/`Alt+P` command modal, `F3` next pane, `F4` detach, `Alt+H` horizontal split) because `Alt+S`, `Alt+D`, and `Alt+Enter` conflict on Windows.
- Validate and translate pasted Windows paths (`"C:\Users\..."`) before hosting inside WSL (`validate_host_directory` + `translate_windows_path_for_unix`); never call `create_dir_all` blindly inside `System32`.
- Keep PTY polling and daemon IPC non-blocking in the 60fps loop, keep mouse drag selection clamped inside the clicked pane's `Rect`, never intercept raw `Tab` or shell `Ctrl+...` keys when a terminal pane has focus, and never persist credentials or tokens to `.turf/` or logs.

## Build, Install & Verification

All Rust crates compile and run tests inside Linux/WSL (or macOS/Windows native toolchains):

```bash
# Stop any running background daemon so socket CLI tests do not race
turf daemon stop || true

# Run all Rust unit & integration tests (0 warnings expected)
cargo test --workspace -- --test-threads=1

# Run real-user interactive PTY E2E suite (requires tmux)
bash test/e2e-pty.sh

# Install updated release binaries locally
cargo install --path crates/turf-net --locked --force
cargo install --path crates/turf-daemon --locked --force
```

For the legacy Node.js layer: `npm ci && npm test`.

## Contribution Workflow

`main` is protected. Always work on a scoped branch (`feat/...`, `fix/...`, `docs/...`), record new architectural decisions in [docs/ADR.md](./docs/ADR.md) (`[MD-057]` onward), update [CHANGELOG.md](./CHANGELOG.md) under `[Unreleased]`, and open a Pull Request.
