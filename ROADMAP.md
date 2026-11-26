# Product Vision & Roadmap

TurfCode CLI is building a multiplayer, AI-native terminal cockpit: a single binary where developers host a repository, pair with teammates or AI coding agents over encrypted tunnels, run shells and external CLIs side by side in persistent split panes, and coordinate edits safely using atomic locks and isolated git worktrees.

## Working Today

- [turf-net](./crates/turf-net) : Interactive launcher validating local repository paths (including Windows drive paths pasted into WSL like `"C:\Users\you\repo"`), unborn git branch detection, room-code pairing (`TURF-XXXX`), and AES-256-GCM WebSocket sync.
- [turf-daemon](./crates/turf-daemon) : Background session server preserving shells across `F4` / `Alt+D` detaches, splits, and `turf attach <session>`, plus headless automation (`turf session ...`, `turf pane read|send-keys|split`).
- [turf-tui](./crates/turf-tui) & [turf-vte](./crates/turf-vte) : Full-height borderless split panes, 10k-line scrollback, native transparency (`Color::Reset`), horizontally scrollable tabs (`<` / `>`, pinned `+` modal, and `New tab` / `Rename` / `Close` context menu), pane-bounded clipboard copy (`OSC 52` + `clip.exe`), clickable OSC 8 / wrapped URLs, and running `codex`, `claude`, `agy`, and `vim` directly inside panes.
- [turf-core](./crates/turf-core) : Shared Intent Board (`IntentBoard`), atomic fail-fast multi-file locking (`LockManager`), and isolated `.turf/worktrees/*` checkouts with `Co-authored-by` squash merges (`WorktreeManager`).

## Open Roadmap

1. **Port AgentRunner & BYOK Support to Rust** — Currently [src/agent-runner.js](./src/agent-runner.js) implements subprocess agent execution (`/agent`, memory-only `GEMINI_API_KEY` / `AGY_AUTH_TOKEN` passthrough) in Node.js only. Next is wiring native Rust task runners inside [turf-daemon](./crates/turf-daemon) and [turf-tui](./crates/turf-tui) for both BYOK providers and installed CLIs (`codex`, `claude`, `agy`, `gemini`) per task worktree.
2. **Remote Relay & NAT Traversal** — [turf-net](./crates/turf-net) currently binds `ws://127.0.0.1:7873` or LAN TCP. Adding an optional encrypted relay lets hosts and collaborators on separate networks pair with `turf join <TURF-XXXX>`.
3. **Interactive Sidebar & Diff Review in Rust** — Bring task assignment interactions, lock conflict prompts, and an inline diff viewer (mirroring [src/diff-viewer.js](./src/diff-viewer.js)) into [turf-tui](./crates/turf-tui) before squash-merging task worktrees.
4. **Cross-Platform Clipboard & Opener Polish** — Test and harden native Linux (`wl-copy` / `xclip` / `xdg-open`), macOS (`pbcopy` / `open`), and native Windows ConPTY flows alongside WSL2 + Windows Terminal.
5. **Isolated Daemon Test Fixtures** — Isolate CLI unit tests (`test_session_cli_list_when_stopped`, `test_daemon_cli_*`) onto dedicated temp sockets so `cargo test --workspace` never requires stopping a running user daemon or passing `--test-threads=1`.
