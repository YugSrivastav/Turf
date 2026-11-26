# Changelog

All notable changes to **Turf** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Full-height Herdr-style cockpit UI: removed permanent bottom input bar (replaced with on-demand `F2`/`Alt+P` centered command modal and sidebar `? help «` footer).
- Unlimited horizontally scrollable top tab bar (`<`, `>`, pinned `+` `new tab` modal) and interactive tab context menu (`New tab`, `Rename`, `Close`) on active-tab left-click or tab right-click `[MD-056]`.
- Crossterm mouse capture with pane-bounded drag selection + OSC 52 / `clip.exe` clipboard copy (no sidebar selection leak) and clickable OSC 8 + wrapped URL hyperlinks (`turf-vte`, `turf-tui`).
- Strict repository directory validation (`validate_host_directory`), Windows drive-path translation (`C:\...` -> `/mnt/c/...`) for WSL hosting, and clean `TURFCODE` ASCII splash without hardcoded version slop (`crates/turf-net`).
- Windows Terminal compatibility (`ENG IN` layout `F1`–`F4` / `Alt+P` / `Alt+H` bindings, parent `cwd` inheritance on daemon split panes, opt-in `--open` Explorer launch, unborn git branch detection, and `tmux` PTY E2E suite `test/e2e-pty.sh`).
- Open-source repository structure and governance: `LICENSE` (MIT), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `docs/ARCHITECTURE.md`, `docs/ADR.md`, `.github/` issue/PR templates, `CODEOWNERS`, and GitHub Actions CI workflow.
- Live foreground process introspection and dynamic pane/tab badges (`turf-pty`, `turf-daemon`, `turf-tui`) `[MD-055]`.
- Named multi-session lifecycle management and programmatic CLI pane automation (`turf session`, `turf pane`) `[MD-054]`.
- 10,000-line historical scrollback ring buffer and decoupled viewport scrolling `[MD-053]`.
- Thin-client TUI cockpit backed by persistent background daemon with `Alt+D` detach and reconnection `[MD-052]`.
- Persistent background daemon and length-prefixed Unix domain socket / named pipe IPC engine `[MD-051]`.
- Catppuccin Mocha / Obsidian Slate theme and non-conflicting `Alt+...` multiplexer navigation `[MD-050]`.
- Native Rust P2P AES-256-GCM encrypted WebSocket tunnel (`turf-net`) `[MD-048]`.
- Native Rust spatial multiplexer cockpit (`turf-tui`), cross-platform PTY (`turf-pty`), VTE grid (`turf-vte`), and atomic lock/worktree core (`turf-core`, `turf-protocol`) `[MD-045]`–`[MD-047]`.
- Initial Node.js ESM CLI (`bin/turf.js`, `src/`) with Intent Board, atomic sequence locking, isolated git worktrees, live diff viewer, and WebSocket tunnel.

[Unreleased]: https://github.com/YugSrivastav/Turf/compare/main...HEAD
