# AGENTS.md — Contributor Guide for AI Coding Agents

This file gives AI coding agents (Codex, Claude Code, Antigravity `agy`, Cursor, Gemini CLI) the architectural map, codebase truth, UI taste rules, and verification commands for **TurfCode CLI**.

Read [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md), [ROADMAP.md](./ROADMAP.md), and [docs/ADR.md](./docs/ADR.md) before modifying core subsystems.

---

## 1. Product Vision

TurfCode CLI turns a developer's terminal into a **multiplayer engineering workspace** where humans and AI coding agents work on the same repository in parallel without overwriting each other:

1. **Instant Pairing**: Host a local repository (`turf host <dir>`) and share a room code (`turf join <TURF-XXXX>`) over an AES-256-GCM encrypted WebSocket tunnel.
2. **Real Terminal Multiplexing**: Run shells, editors (`vim`), and company AI CLIs (`codex`, `claude`, `agy`, `gemini`) inside persistent background PTY panes (`turf-daemon`) rendered at 60fps in a minimal Herdr-inspired cockpit (`turf-tui`).
3. **Collision-Free Parallelism**: Tasks declare target files on a shared Intent Board, acquire atomic all-or-nothing multi-file locks (`turf-core`), and execute inside isolated git worktrees (`.turf/worktrees/<task-id>`) merged cleanly with `Co-authored-by` trailers.

---

## 2. Critical Codebase Truth (Read First)

This repository contains **two** implementations:

| Layer | Path | Status | What lives here |
| :--- | :--- | :--- | :--- |
| **Native Rust Workspace** | [`crates/`](./crates) | **Active shipping CLI (`turf`, `turfcode`, `turf-daemon`)** | Everything users run via `turf` / `turfcode`: launcher, tunnel, daemon, PTY, VTE, TUI cockpit, locks, worktrees. |
| **Node.js ESM Layer** | [`src/`](./src), [`bin/`](./bin) | **Legacy prototype & test harness** | Original JS prototype (`src/cli.js`, `src/cockpit.js`) plus JS-only features not yet ported to Rust—notably [`src/agent-runner.js`](./src/agent-runner.js) (`AgentRunner` `/agent` credential passthrough) and [`src/diff-viewer.js`](./src/diff-viewer.js). |

**Rule:** Always edit [`crates/`](./crates) for fixes or features in the `turf` / `turfcode` CLI. Only touch `src/` if explicitly updating the Node fallback/prototype or porting features from it into Rust.

---

## 3. Crate Map

- [`crates/turf-pty`](./crates/turf-pty) — Cross-platform PTY allocation (`portable-pty`) and foreground process introspection (`/proc` + `tcgetpgrp`).
- [`crates/turf-vte`](./crates/turf-vte) — 2D `ScreenBuffer`, ANSI/TrueColor state machine, OSC 8 hyperlink tracking, wide-glyph continuation cells, alt-screen isolation, 10,000-line scrollback ring buffer.
- [`crates/turf-protocol`](./crates/turf-protocol) — Shared data contracts (`TaskCard`, `FileLock`, `CoAuthor`, length-prefixed JSON-RPC daemon IPC messages).
- [`crates/turf-core`](./crates/turf-core) — Atomic fail-fast `LockManager` (with heartbeat TTL & PID liveness), `IntentBoard` atomic disk persistence, `WorktreeManager` (isolated worktrees + squash merge).
- [`crates/turf-daemon`](./crates/turf-daemon) — Persistent background daemon (`turf-daemon`) over Unix domain socket / Windows named pipe; preserves running panes across detach/reconnect and passes parent `cwd` on split.
- [`crates/turf-tui`](./crates/turf-tui) — Ratatui + Crossterm cockpit: full-height split panes, Catppuccin Mocha theme (`Color::Reset` transparency), horizontally scrollable tab bar with pinned `+` and tab context menu (`New tab`, `Rename`, `Close`), pane-bounded mouse copy (OSC 52 + `clip.exe`), clickable OSC 8 / wrapped URLs, centered modals (`F1`, `F2`, `+`).
- [`crates/turf-net`](./crates/turf-net) — AES-256-GCM WebSocket tunnel (`TunnelServer` / `TunnelClient`) and the `turf` / `turfcode` CLI entrypoint (`cli.rs`: ASCII splash, strict local directory validation, Windows->WSL path translation).

---

## 4. UI Taste & Platform Guardrails

Preserve these hard-won UX rules (see `[MD-050]`–`[MD-056]` in [`docs/ADR.md`](./docs/ADR.md)):

- **Keep UI clean & minimal (Herdr aesthetic)**:
  - Keep the 77-column `TURFCODE` ASCII box splash clean—do **not** add `ELECTRIC LIME COCKPIT` or `v0.1.0` subtitle clutter.
  - Keep terminal panes **full height** (no permanent bottom input bar; `/task`, `/lock`, `/split-h` live in the centered `F2` / `Alt+P` modal).
  - Keep top tab pills numbered/minimal (` 1 `, ` 2 `) without injecting noisy shell names like `: zsh`.
  - Keep the `+` tab button permanently pinned using `<` / `>` horizontal scroll overflow (no hard 16-tab ceiling).
- **Windows Terminal & WSL Compatibility**:
  - Windows `ENG-IN` keyboard layout turns `Alt+S` and `Alt+D` into accented characters (`ś`, `ḍ`), and Windows Terminal intercepts `Alt+Enter`. Always preserve safe alternatives (`F1` help, `F2`/`Alt+P` command modal, `F3` next pane, `F4` detach, `Alt+H` horizontal split).
  - Never pass raw pasted Windows paths (`"C:\Users\..."`) straight to `create_dir_all` inside WSL; use `validate_host_directory` + `translate_windows_path_for_unix`.
  - Never bypass WSL `Tab` or shell `Ctrl+...` keybindings when `FocusArea::Terminal` is active.
- **Worker Invariants**:
  - Keep PTY polling and daemon IPC strictly non-blocking inside the 60fps UI loop.
  - Keep mouse selection clamped inside the clicked pane's `Rect` so drag-copy never bleeds across the sidebar divider.
  - Never write credentials or OAuth/API tokens to disk (`.turf/` or logs).

---

## 5. Build, Install & Verification

All Rust crates compile and run tests inside **Linux/WSL** (or macOS/Windows native toolchains):

```bash
# 1. Stop any running default daemon first so socket CLI tests don't race
turf daemon stop || true

# 2. Run all Rust unit & integration tests (0 warnings expected)
cargo test --workspace -- --test-threads=1

# 3. Run real-user interactive PTY E2E suite (requires tmux)
bash test/e2e-pty.sh

# 4. Install updated release binaries locally (offline-safe if crates are cached)
cargo install --path crates/turf-net --locked --force
cargo install --path crates/turf-daemon --locked --force
```

For the legacy Node.js layer:
```bash
npm ci
npm test
```

## 6. Contribution Workflow

- `main` is protected. Always work on a scoped branch (`feat/...`, `fix/...`, `docs/...`) and open a Pull Request.
- Record new architectural decisions and worker invariants in [`docs/ADR.md`](./docs/ADR.md) (`[MD-057]` onward) and update [`CHANGELOG.md`](./CHANGELOG.md) under `[Unreleased]`.
