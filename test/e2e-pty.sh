#!/usr/bin/env bash
# Real-user interactive PTY E2E test suite for Turf using tmux.
# Verifies interactive hosting, working directory binding, multi-session isolation,
# split pane navigation/closing, shortcuts overlay, lock deduplication, and pane automation.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TURF_BIN="${TURF_BIN:-$ROOT_DIR/target/debug/turf}"
if [[ ! -x "$TURF_BIN" ]]; then
  echo "Building turf and turf-daemon..."
  cargo build -p turf-net --bin turf -p turf-daemon --bin turf-daemon
fi

if ! command -v tmux >/dev/null 2>&1; then
  echo "ERROR: tmux is required to run E2E PTY tests." >&2
  exit 1
fi

SOCK="/tmp/turf-e2e-$$.sock"
WORKDIR="/tmp/turf-e2e-work-$$"
TMUX_SESS="turf_e2e_$$"
mkdir -p "$WORKDIR"

cleanup() {
  tmux kill-session -t "$TMUX_SESS" 2>/dev/null || true
  TURF_SOCKET_PATH="$SOCK" "$TURF_BIN" daemon stop >/dev/null 2>&1 || true
  rm -rf "$SOCK" "$WORKDIR"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $1" >&2
  echo "--- LAST CAPTURED SCREEN ---" >&2
  tmux capture-pane -t "$TMUX_SESS" -p 2>/dev/null || true
  exit 1
}

wait_for_text() {
  local needle="$1"
  local timeout_iters="${2:-40}"
  for _ in $(seq 1 "$timeout_iters"); do
    local screen
    screen="$(tmux capture-pane -t "$TMUX_SESS" -p)"
    if grep -qF -- "$needle" <<<"$screen"; then
      return 0
    fi
    sleep 0.15
  done
  fail "Timed out waiting for text: '$needle'"
}

echo "[1/5] Starting bash inside tmux (100x30) & creating isolated session 'alpha'..."
tmux new-session -d -s "$TMUX_SESS" -x 100 -y 30 "env TURF_SOCKET_PATH=$SOCK TERM=xterm-256color bash"
wait_for_text "$" 20

tmux send-keys -t "$TMUX_SESS" "$TURF_BIN session new alpha" Enter
wait_for_text "Created detached session 'alpha'" 40

echo "[2/5] Hosting WORKDIR and verifying CWD + UI + split navigation + help overlay + dedup locks..."
tmux send-keys -t "$TMUX_SESS" "$TURF_BIN host $WORKDIR" Enter
wait_for_text "mode: host-pairing" 40

tmux send-keys -t "$TMUX_SESS" "pwd" Enter
wait_for_text "$WORKDIR" 30

# Vertical split -> cycle back with Alt+O -> close with Alt+X
tmux send-keys -t "$TMUX_SESS" M-v
sleep 0.6
tmux send-keys -t "$TMUX_SESS" M-o
sleep 0.3
tmux send-keys -t "$TMUX_SESS" M-x
sleep 0.4

# Open shortcuts help overlay via Alt+Enter then '?'
tmux send-keys -t "$TMUX_SESS" M-Enter
sleep 0.2
tmux send-keys -t "$TMUX_SESS" "?"
wait_for_text "Shortcuts & Commands" 20
tmux send-keys -t "$TMUX_SESS" Escape
sleep 0.2

# Test /task and duplicate /lock deduplication via centered command modal
tmux send-keys -t "$TMUX_SESS" "/task Fix E2E" Enter
sleep 0.2
tmux send-keys -t "$TMUX_SESS" M-Enter
sleep 0.15
tmux send-keys -t "$TMUX_SESS" "/lock src/main.rs" Enter
sleep 0.2
tmux send-keys -t "$TMUX_SESS" M-Enter
sleep 0.15
tmux send-keys -t "$TMUX_SESS" "/lock src/main.rs" Enter
wait_for_text "active locks (1)" 20

# Detach cleanly via Alt+D
tmux send-keys -t "$TMUX_SESS" M-d
wait_for_text "Shutting down Turf session" 30

echo "[3/5] Verifying lowercase session attach isolation ('alpha' does not see host CWD output)..."
tmux send-keys -t "$TMUX_SESS" "$TURF_BIN attach alpha" Enter
wait_for_text "alpha • CLIENT" 30

ALPHA_SCREEN="$(tmux capture-pane -t "$TMUX_SESS" -p)"
if grep -qF -- "$WORKDIR" <<<"$ALPHA_SCREEN"; then
  fail "Session 'alpha' leaked host session's pane output!"
fi
tmux send-keys -t "$TMUX_SESS" C-q
sleep 0.4

echo "[4/5] Verifying programmatic 'turf pane send-keys' with Enter token..."
tmux send-keys -t "$TMUX_SESS" "$TURF_BIN pane send-keys 1 'echo PANE_AUTO_VERIFIED' Enter" Enter
sleep 0.6
PANE_OUT="$(TURF_SOCKET_PATH="$SOCK" "$TURF_BIN" pane read 1 --lines 10)"
if ! grep -qF -- "PANE_AUTO_VERIFIED" <<<"$PANE_OUT"; then
  echo "$PANE_OUT"
  fail "Expected PANE_AUTO_VERIFIED in pane 1 output"
fi

echo "[5/5] Verifying session list & kill..."
SESS_LIST="$(TURF_SOCKET_PATH="$SOCK" "$TURF_BIN" list)"
grep -qF "alpha" <<<"$SESS_LIST" || fail "Expected 'alpha' in session list"
TURF_SOCKET_PATH="$SOCK" "$TURF_BIN" session kill alpha >/dev/null

echo "ALL E2E PTY TESTS PASSED!"
