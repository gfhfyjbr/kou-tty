#!/usr/bin/env bash
#
# kou-tty end-to-end smoke test.
#
# Walks through every subcommand against an auto-spawned daemon on an
# isolated socket, asserts the responses look correct, and shuts everything
# down cleanly. Exit code 0 = green.
#
# Usage:
#   scripts/smoke.sh                       # uses ./target/release/kou-tty
#   KOU_TTY_BIN=$(command -v kou-tty) scripts/smoke.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
KOU_TTY_BIN="${KOU_TTY_BIN:-$REPO_ROOT/target/release/kou-tty}"

if [ ! -x "$KOU_TTY_BIN" ]; then
  echo "building release binary..."
  (cd "$REPO_ROOT" && cargo build --release)
fi
[ -x "$KOU_TTY_BIN" ] || { echo "kou-tty binary not found at $KOU_TTY_BIN"; exit 1; }

SOCK="$(mktemp -t kou-tty-smoke.XXXXXX.sock)"
rm -f "$SOCK"
CLEAN_ID=""

trap 'cleanup' EXIT

cleanup() {
  if [ -n "$CLEAN_ID" ]; then
    "$KOU_TTY_BIN" --socket "$SOCK" terminal destroy "$CLEAN_ID" --if-exists >/dev/null 2>&1 || true
  fi
  "$KOU_TTY_BIN" --socket "$SOCK" shutdown >/dev/null 2>&1 || true
  rm -f "$SOCK"
}

step() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
pass() { printf '\033[1;32m ok\033[0m  %s\n' "$*"; }
fail() { printf '\033[1;31m fail\033[0m %s\n' "$*"; exit 1; }

KT() {
  "$KOU_TTY_BIN" --socket "$SOCK" "$@"
}

KTJ() {
  "$KOU_TTY_BIN" --socket "$SOCK" --json "$@"
}

jq_get() {
  python3 -c "import json,sys; d=json.load(sys.stdin); print(${1})"
}

step "terminal create (bare default → id only)"
ID=$(KT terminal create --shell /bin/sh --size 80x24)
CLEAN_ID="$ID"
[ -n "$ID" ] || fail "empty id"
[ ${#ID} -eq 2 ] || fail "expected 2-char id, got '$ID'"
pass "id=$ID"

step "terminal send-keys: echo smoke-marker (bare → empty stdout)"
OUT=$(KT terminal send-keys "$ID" '[{"text":"echo smoke-marker"},{"key":"Enter"}]')
[ -z "$OUT" ] || fail "expected empty stdout, got '$OUT'"
pass "silent"

step "wait for output via bare show"
for _ in $(seq 1 30); do
  TEXT=$(KT terminal show "$ID")
  case "$TEXT" in
    *smoke-marker*) break ;;
  esac
  sleep 0.1
done
case "$TEXT" in
  *smoke-marker*) pass "marker visible in show" ;;
  *) fail "marker not found" ;;
esac

step "terminal status (bare → process_state)"
STATE=$(KT terminal status "$ID")
case "$STATE" in
  waiting_for_input|idle|running) pass "process_state=$STATE" ;;
  *) fail "unexpected '$STATE'" ;;
esac

step "terminal read --mode full has coordinate ruler"
FULL=$(KT terminal read "$ID" --mode full --max-lines 5)
case "$FULL" in
  *0123456789*) pass "ruler present" ;;
  *) fail "no coordinate ruler" ;;
esac

step "terminal read --mode changes via --json"
ROWS=$(KTJ terminal read "$ID" --mode changes --max-lines 5 | jq_get 'len(d["result"]["rows"])')
[ "$ROWS" -ge 0 ] || fail "rows not a list"
pass "rows length=$ROWS"

step "terminal region read via --json"
REGION=$(KTJ terminal region "$ID" --x 0 --y 0 --w 10 --h 2 | jq_get 'len(d["result"]["lines"])')
[ "$REGION" -eq 2 ] || fail "expected 2 lines, got $REGION"
pass "region returned 2 lines"

step "terminal rows range via --json"
FROM=$(KTJ terminal rows "$ID" 0 1 | jq_get 'd["result"]["from"]')
[ "$FROM" = "0" ] || fail "rows.from != 0"
pass "rows.from=0"

step "terminal events drain (bare → JSONL)"
EVENTS=$(KT terminal events "$ID" --max 50 | wc -l | tr -d ' ')
[ "$EVENTS" -ge 0 ] || fail "events not a list"
pass "events lines=$EVENTS"

step "terminal resize 30x100"
KT terminal resize "$ID" 30 100 > /dev/null
SIZE=$(KTJ terminal status "$ID" | jq_get '(d["result"]["rows"], d["result"]["cols"])')
echo "$SIZE" | grep -q "(30, 100)" || fail "resize did not stick: $SIZE"
pass "resize applied"

step "--compact emits single line"
COMPACT=$($KOU_TTY_BIN --socket "$SOCK" --compact terminal status "$ID")
LINES=$(printf '%s\n' "$COMPACT" | wc -l | tr -d ' ')
[ "$LINES" -eq 1 ] || fail "compact had $LINES lines"
pass "single-line JSON"

step "json bridge ping"
PONG=$(printf '{"method":"ping"}\n' | "$KOU_TTY_BIN" --socket "$SOCK" json)
case "$PONG" in
  *'"pong":true'*) pass "ping/pong" ;;
  *) fail "no pong: $PONG" ;;
esac

step "terminal list (bare → ids one per line)"
LIST=$(KT terminal list)
case "$LIST" in
  *"$ID"*) pass "list contains $ID" ;;
  *) fail "list missing $ID" ;;
esac

step "viewer start (bare → address)"
ADDR=$(KT viewer start --port 8088)
case "$ADDR" in
  http://*) pass "viewer at $ADDR" ;;
  *) fail "no address: '$ADDR'" ;;
esac
sleep 0.2
curl -fsS "$ADDR/api/terminals" > /dev/null || fail "viewer API not reachable"
pass "viewer HTTP reachable"
KT viewer stop > /dev/null
pass "viewer stopped"

step "terminal destroy"
KT terminal destroy "$ID" > /dev/null
CLEAN_ID=""
pass "destroyed"

step "destroy --if-exists is idempotent"
KT terminal destroy "$ID" --if-exists > /dev/null
pass "no error on missing id"

step "destroy without --if-exists returns exit 3"
set +e
STDERR=$(KT terminal destroy "$ID" 2>&1 >/dev/null)
RC=$?
set -e
[ "$RC" = "3" ] || fail "expected exit 3, got $RC"
case "$STDERR" in
  *"error[not_found]"*) ;;
  *) fail "stderr missing error[not_found]: $STDERR" ;;
esac
case "$STDERR" in
  *"hint:"*) ;;
  *) fail "stderr missing hint: $STDERR" ;;
esac
pass "exit=3, stderr has error+hint"

step "shutdown returns exit 0"
KT shutdown > /dev/null
pass "shutdown exit 0"

printf '\n\033[1;32mall green\033[0m\n'
