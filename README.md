# TurfCode CLI

> Real-time multiplayer AI coding and terminal multiplexing in your shell.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](./Cargo.toml)
[![Node.js](https://img.shields.io/badge/Node.js-%3E%3D20-339933.svg)](./package.json)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](./CONTRIBUTING.md)

**TurfCode CLI** turns the native terminal into a real-time multiplayer workspace. Developers can host a local repository, share an unambiguous session code (`TURF-XXXX`), collaborate over an encrypted WebSocket tunnel, coordinate work on a shared Intent Board, and prevent concurrent file edits with atomic multi-file locking and isolated git worktrees.

---

## Features

- **Instant Session Pairing**: Host any local repository (`turf host [dir]`) and invite collaborators via a generated room code (`turf join <TURF-XXXX>`).
- **Persistent Background Daemon (`turf-daemon`)**: Keeps PTY shells and running processes alive across client disconnects, terminal restarts, and `Alt+D` detaches over Unix domain sockets / Windows named pipes.
- **Native Rust Spatial Multiplexer (`turf-tui`)**:
  - Borderless, full-height split panes (horizontal & vertical) with 60fps `turf-vte` cell blitting and a Catppuccin Mocha / Obsidian Slate theme (`Color::Reset` native terminal transparency).
  - Herdr-style unlimited horizontally scrollable tab strip (`<`, `>`, pinned `+` modal, and click/right-click `New tab` / `Rename` / `Close` context menu).
  - Pane-bounded mouse drag selection + clipboard copy (OSC 52 / `clip.exe`), mouse wheel viewport scrollback, and clickable OSC 8 & wrapped URL hyperlinks.
  - 10,000-line scrollback ring buffer with live-view snap-back on input.
  - Live foreground process introspection badges on split panes and non-conflicting `Alt+...` / `F1`–`F4` navigation so `Tab`, `Ctrl+C`, and readline shortcuts pass straight through to your shell.
- **Atomic Multi-File Sequence Locking (`turf-core`)**: Acquires upfront locks across all declared target files simultaneously—failing fast on conflicts instead of leaving partial writes.
- **Isolated Git Worktrees**: Runs tasks in isolated `.turf/worktrees/<task-id>` checkouts and squash-merges back with standard `Co-authored-by` trailers.
- **Encrypted P2P State Sync (`turf-net`)**: Derives a 256-bit key from the room code and encrypts peer handshakes and state deltas with AES-256-GCM.
- **Programmatic Session & Pane Automation**: Scriptable `turf session` and `turf pane` subcommands (`read`, `send-keys`, `split`) for headless automation and AI agent harnesses.

---

## Architecture Overview

TurfCode contains a modular **Rust Cargo workspace** (the native multiplexer, daemon, crypto tunnel, VTE, and core concurrency engine) alongside a **Node.js ESM CLI/prototype layer** (`bin/`, `src/`, `test/`).

```text
turfcode-cli/
├── crates/
│   ├── turf-pty/           # Cross-platform PTY allocation (ConPTY / POSIX) & process introspection
│   ├── turf-vte/           # 2D terminal cell grid, ANSI/TrueColor parser, 10k-line scrollback
│   ├── turf-protocol/      # Shared Intent Board, Lock, Worktree & length-prefixed IPC contracts
│   ├── turf-core/          # Atomic LockManager, thread-safe IntentBoard, git WorktreeManager
│   ├── turf-daemon/        # Persistent background daemon server, socket IPC & auto-spawner
│   ├── turf-tui/           # Ratatui spatial cockpit, split pane tree & 60fps VTE blitter
│   └── turf-net/           # Encrypted AES-256-GCM WebSocket tunnel & `turf` / `turfcode` binaries
├── bin/
│   └── turf.js             # Node.js CLI entry point
├── src/                    # Node.js ESM CLI, cockpit, lock manager, worktree & tunnel modules
├── test/                   # Node.js built-in test runner suites
├── docs/
│   ├── ARCHITECTURE.md     # System design & component architecture
│   └── ADR.md              # Architectural Decision Records (MD-001 through MD-055)
└── .github/                # CI workflows, issue forms, PR template & CODEOWNERS
```

See [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) for the full technical design and [docs/ADR.md](./docs/ADR.md) for all recorded architectural invariants.

---

## Getting Started

### Prerequisites

- **Rust** (stable toolchain, 2021 edition) for building the native binaries and running workspace tests.
- **Node.js** (`>= 20.0.0`) for running the Node.js CLI and test suite.
- **Git** (`>= 2.20` recommended for worktree support).

### Build & Run (Rust Workspace)

```bash
# Build all workspace crates
cargo build --workspace

# Run the native CLI interactive launcher
cargo run -p turf-net --bin turf

# Or install the native `turf` and `turfcode` binaries locally
cargo install --path crates/turf-net
```

### Run (Node.js CLI)

```bash
# Install dependencies
npm install

# Launch interactive CLI
npm start
```

---

## CLI Usage

```text
turfcode                         Open interactive launcher menu (Create, Join, or Attach)
turf                             Alias for turfcode
turf host [dir]                  Host a shared Turf session in the given directory (default: .)
turf join <id> [url]             Join an active Turf session using a room code (e.g. TURF-7A9B)
turf list                        List active daemon sessions
turf attach [name]               Attach interactive TUI client to a named session
turf session list                List active sessions and pane counts
turf session new <name>          Spawn a detached named session
turf session attach <name>       Attach to a named session
turf session kill <name>         Terminate a named session and reap its child PTYs
turf pane read <id> [--lines N]  Read captured screen buffer text from a pane
turf pane send-keys <id> <keys>  Inject keystrokes or text into a pane PTY
turf pane split <id> [-d h|v]    Programmatically split a pane horizontally or vertically
turf daemon start|stop|status    Manage the persistent background daemon
```

### Cockpit Keybindings

| Shortcut | Action |
| :--- | :--- |
| `Alt+T` | Open a new tab |
| `Alt+W` | Close active tab |
| `Alt+1` .. `Alt+9` | Switch to tab `1..9` |
| `Alt+V` / `Alt+H` *(or `Alt+S`)* | Split pane vertically / horizontally |
| `F3` / `Alt+O` / `Alt+X` | Cycle split panes / Close active pane |
| `Alt+B` | Toggle Intent Board sidebar |
| `F2` / `Alt+P` *(or `Alt+Enter`)* | Open centered command modal (`/task`, `/lock`, `/split-h`, `/tab`) |
| `Alt+Up` / `Alt+Down` | Scroll viewport history up / down |
| `Shift+PageUp` / `Shift+PageDown` | Page through scrollback history |
| `F1` / `Alt+?` | Show shortcuts & slash commands overlay |
| `F4` / `Alt+D` | Detach from daemon session (keeps processes running) |
| `Ctrl+Q` / `Alt+Q` | Quit cockpit |
| Mouse Click / Drag / Scroll | Switch/scroll tabs, `+` new tab modal, tab menu (`New tab`/`Rename`/`Close`), pane-bounded copy, clickable OSC 8 links |

---

## Running Tests

```bash
# Rust workspace tests
cargo test --workspace

# Real-user interactive PTY E2E suite (requires tmux on Linux/macOS/WSL)
bash test/e2e-pty.sh

# Node.js tests
npm test
```

---

## Contributing

Contributions are welcome! Because `main` is protected, all changes—including fixes, features, and documentation improvements—must be submitted via a **Pull Request** for maintainer review.

Please read:
- [CONTRIBUTING.md](./CONTRIBUTING.md) for fork/PR workflow, testing requirements, and coding conventions.
- [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) for community expectations.
- [docs/ADR.md](./docs/ADR.md) for architectural invariants you must preserve.

---

## Security & License

- To report a security vulnerability, please follow [SECURITY.md](./SECURITY.md).
- Released under the [MIT License](./LICENSE).
