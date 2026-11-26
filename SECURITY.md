# Security Policy

## Supported Versions

Turf is in active pre-1.0 development (`0.1.x`). Security fixes are applied to `main` and shipped in subsequent builds.

| Version | Supported |
| :--- | :--- |
| `main` / `0.1.x` | Yes |

## Reporting a Vulnerability

If you find a vulnerability—such as credential leaks, tunnel encryption flaws in [turf-net](./crates/turf-net), shell/command injection in [turf-pty](./crates/turf-pty), or path traversal outside the hosted directory in [turf-core](./crates/turf-core):

1. Do not open a public GitHub issue.
2. Submit a private report through [GitHub Security Advisories](https://github.com/YugSrivastav/Turf/security/advisories/new) (preferred) or email `yug.srivastav99@gmail.com` with a clear description, reproduction steps, and affected crates or files.
3. Expect an initial response within 72 hours, followed by coordinated remediation and disclosure once a fix lands on `main`.

## Core Security Boundaries

Changes touching subprocesses, tunnels, or filesystem state must maintain three boundaries documented in [docs/ADR.md](./docs/ADR.md):

- Spawn child processes and system helpers (`rundll32.exe`, `xdg-open`, `open`, shells) using explicit argument arrays—never pass untrusted user input or URLs through `bash -c`, `sh -c`, or `cmd /c`.
- Keep peer auth tokens, OAuth tokens, and API credentials in memory only; never write them to `.turf/`, daemon logs, or stdout.
- Scope all peer operations, lock paths, and `.turf/worktrees/*` checkouts strictly inside the validated hosted repository root.
