# Product Vision & Roadmap

TurfCode CLI is building a **multiplayer, AI-native terminal cockpit**: a lightweight binary where developers host a repository, invite collaborators or AI coding agents over encrypted tunnels, run shells and agent CLIs side by side in persistent split panes, and coordinate edits safely via atomic file locks and isolated git worktrees.

---

## What Works Today

- **Interactive Launcher & Hosting (`turf` / `turfcode`)**: Validates local repository paths (including Windows paths pasted into WSL like `"C:\Users\you\repo"`), detects git branches cleanly (including unborn branches), and boots the TUI cockpit.
- **Persistent Daemon (`turf-daemon`)**: Keeps background PTY shells and child processes alive across `F4` / `Alt+D` detaches, terminal closures, and `turf attach <session>`. Supports programmatic automation (`turf session ...`, `turf pane read|send-keys|split`).
- **Herdr-Grade Multiplexer (`turf-tui` + `turf-vte`)**:
  - Full-height borderless BSP split panes (`Alt+V` / `Alt+H`), 60fps VTE blitting, 10k-line scrollback, and native terminal background transparency (`Color::Reset`).
  - Horizontally scrollable tab strip (`<` / `>`, pinned `+` modal) with click/right-click context menu (`New tab`, `Rename`, `Close`).
  - Pane-bounded mouse selection copy (`OSC 52` + `clip.exe`), mouse wheel scrolling, and clickable OSC 8 / wrapped HTTP(S) links.
  - Real-world AI CLIs (`codex`, `claude`, `agy`) and full-screen TUIs (`vim`, `less`) run directly inside multiplexer panes.
- **Multiplayer Sync & Safety (`turf-core` + `turf-net`)**: AES-256-GCM encrypted WebSocket tunnel (`TURF-XXXX`), live Intent Board (`TaskCard`), atomic fail-fast multi-file locking (`LockManager`), and isolated git worktree creation + squash merges (`WorktreeManager`).

---

## Near-Term Roadmap (Open Opportunities)

1. **Port Native Agent Runner & BYOK Support to Rust (`crates/`)**
   - Currently [`src/agent-runner.js`](./src/agent-runner.js) implements subprocess agent execution (`/agent`, `GEMINI_API_KEY` / `AGY_AUTH_TOKEN` memory-only credential passthrough) in Node.js only.
   - Next: wire native Rust agent task runners in `turf-daemon` / `turf-tui` supporting both **Bring-Your-Own-API-Key** providers and installed company CLIs (`codex`, `claude`, `agy`, `gemini`) per Intent Board task worktree.
2. **Remote Relay & NAT Traversal**
   - Today `TunnelServer` listens on local TCP/WebSocket (`ws://127.0.0.1:7873` or LAN).
   - Next: add optional encrypted relay / peer discovery so hosts and collaborators on different networks pair with just `turf join <TURF-XXXX>`.
3. **Interactive Sidebar & Diff / Merge Review in Rust**
   - Wire mouse/keyboard task assignment, lock conflict prompts, and an interactive git diff viewer (mirroring [`src/diff-viewer.js`](./src/diff-viewer.js)) directly into `turf-tui` for reviewing worktree diffs before squash-merging.
4. **Cross-Platform Terminal & Clipboard Polish**
   - Test and harden native Linux (`wl-copy` / `xclip` / `xdg-open`), macOS (`pbcopy` / `open`), and native Windows ConPTY workflows alongside WSL2 + Windows Terminal.
5. **Isolated Daemon Test Fixture**
   - Isolate CLI unit tests (`test_session_cli_list_when_stopped`, `test_daemon_cli_*`) with unique temp socket paths so `cargo test --workspace` never requires stopping a local user daemon or passing `--test-threads=1`.
