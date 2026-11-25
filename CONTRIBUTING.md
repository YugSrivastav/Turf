# Contributing to TurfCode CLI

Thank you for your interest in contributing to **TurfCode CLI**! We welcome bug reports, feature proposals, documentation updates, and pull requests from developers of all backgrounds.

By participating in this project, you agree to abide by our [Code of Conduct](./CODE_OF_CONDUCT.md).

---

## Repository Workflow (Pull Requests Only)

The `main` branch is **protected**. Direct pushes to `main` are disabled—all changes must go through a **Pull Request (PR)** and receive maintainer review before merging.

### 1. Fork & Clone

1. Fork [YugSrivastav/turfcode-cli](https://github.com/YugSrivastav/turfcode-cli) on GitHub.
2. Clone your fork locally and add `upstream`:

```bash
git clone https://github.com/<your-username>/turfcode-cli.git
cd turfcode-cli
git remote add upstream https://github.com/YugSrivastav/turfcode-cli.git
```

### 2. Create a Feature Branch

Always branch from an up-to-date `main`:

```bash
git checkout main
git pull upstream main
git checkout -b feat/your-short-description
```

Use clear branch prefixes such as `feat/`, `fix/`, `docs/`, `refactor/`, or `test/`.

### 3. Make & Verify Your Changes

#### Prerequisites
- **Rust**: Stable toolchain (`rustup toolchain install stable`)
- **Node.js**: Version 20 or newer
- **Git**: 2.20+

#### Running Checks Locally

```bash
# Rust workspace checks & tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace

# Real-user interactive PTY E2E suite (Linux/macOS/WSL with tmux)
bash test/e2e-pty.sh

# Node.js dependencies & tests
npm ci
npm test
```

### 4. Commit Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat(tui): add pane search highlight`
- `fix(daemon): handle stale Unix socket cleanup`
- `docs: clarify lock manager invariants`
- `test(core): add concurrent lock conflict case`

If your change implements or updates a recorded architectural decision, reference its ID (for example, `[MD-055]`) and update [docs/ADR.md](./docs/ADR.md) if introducing a new architectural invariant.

### 5. Open a Pull Request

1. Push your branch to **your fork**:
   ```bash
   git push origin feat/your-short-description
   ```
2. Open a Pull Request against `YugSrivastav/turfcode-cli:main`.
3. Fill out the [Pull Request template](./.github/PULL_REQUEST_TEMPLATE.md) clearly, link any related issues, and ensure all CI checks pass.
4. Maintainers will review your PR and either request changes, approve and merge it, or close it with feedback.

---

## Architectural Guidelines & Invariants

Before modifying core subsystems, review:
- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — system topology and crate responsibilities.
- [docs/ADR.md](./docs/ADR.md) — architectural decision records (`MD-001` through `MD-055`).

Key invariants to keep in mind:
- **Never block the TUI event loop**: Keep PTY I/O and IPC non-blocking to preserve 60fps rendering.
- **Never acquire partial file locks**: Multi-file lock requests must succeed atomically or fail fast.
- **Never persist sensitive credentials**: Tokens and auth secrets stay in memory only and must never be written to `.turf/` or logs.
- **Preserve non-conflicting PTY input**: Do not intercept raw `Tab` or shell `Ctrl+...` bindings when terminal panes are focused.

---

## Reporting Issues

- Use the [Bug Report](./.github/ISSUE_TEMPLATE/bug_report.yml) or [Feature Request](./.github/ISSUE_TEMPLATE/feature_request.yml) issue templates.
- For security vulnerabilities, **do not open a public issue**—follow [SECURITY.md](./SECURITY.md).
