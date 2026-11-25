# Security Policy

## Supported Versions

TurfCode CLI is currently in active pre-1.0 development (`0.1.x`). Security fixes are applied to the `main` branch and included in upcoming releases.

| Version | Supported |
| :--- | :--- |
| `main` / `0.1.x` | Yes |

---

## Reporting a Vulnerability

If you discover a security issue (such as credential leaks, tunnel encryption flaws, command injection in PTY/worktree handling, or path traversal outside the hosted repository root):

1. **Do not open a public GitHub issue.**
2. Report it privately using [GitHub Security Advisories](https://github.com/YugSrivastav/turfcode-cli/security/advisories/new) (preferred) or by emailing `yug.srivastav99@gmail.com` with:
   - A clear description of the vulnerability and potential impact.
   - Steps to reproduce or a minimal proof of concept.
   - Affected crate(s) or module(s), if known.
3. You should receive an acknowledgment within 72 hours, followed by coordinated remediation and disclosure once a fix is ready.

---

## Security Design Expectations

Contributors modifying TurfCode CLI should preserve these core security boundaries:

- **No Shell Interpolation**: Never pass untrusted user input or prompts through `bash -c`, `sh -c`, or `powershell -Command`. Use explicit argument arrays.
- **In-Memory Credentials Only**: Peer tokens and session credentials transmitted during encrypted handshakes must never be written to `.turf/session.json`, logs, or stdout.
- **Workspace Containment**: Peer operations and worktree paths must remain scoped strictly within the hosted repository root.
