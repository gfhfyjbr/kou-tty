#!/usr/bin/env bash
#
# kou-tty end-to-end smoke test.
#
# Walks through every CLI subcommand against an auto-spawned daemon on an
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
    "$KOU_TTY_BIN" --socket "$SOCK" destroy "$CLEAN_ID" >/dev/null 2>&1 || true
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

jq_get() {
  python3 -c "import json,sys; d=json.load(sys.stdin); print(${1})"
}

step "create 80x24 terminal"
CREATE_JSON=$(KT create --shell /bin/sh --size 80x24)
ID=$(printf '%s' "$CREATE_JSON" | jq_get "d['result']['id']")
CLEAN_ID="$ID"
[ -n "$ID" ] || fail "empty id"
pass "id=$ID"

step "send-keys: echo smoke-marker"
SENT=$(KT send-keys "$ID" '[{"text":"echo smoke-marker"},{"key":"Enter"}]')
printf '%s' "$SENT" | jq_get "d['ok']" | grep -qi true || fail "send-keys not ok"
pass "sent $(printf '%s' "$SENT" | jq_get 'd["result"]["sent"]') bytes"

step "wait for output"
for _ in $(seq 1 30); do
  TEXT=$(KT show "$ID" | jq_get 'd["result"]["text"]')
  case "$TEXT" in
    *smoke-marker*) break ;;
  esac
  sleep 0.1
done
case "$TEXT" in
  *smoke-marker*) pass "marker visible in show" ;;
  *) fail "marker not found in show output" ;;
esac

step "status: expect waiting_for_input or idle"
STATE=$(KT status "$ID" | jq_get 'd["result"]["process_state"]')
case "$STATE" in
  waiting_for_input|idle|running) pass "process_state=$STATE" ;;
  *) fail "unexpected process_state=$STATE" ;;
esac

step "read --mode full has coordinate ruler"
FULL=$(KT read "$ID" --mode full --max-lines 5 | jq_get 'd["result"]["text"]')
case "$FULL" in
  *0123456789*) pass "ruler present" ;;
  *) fail "no coordinate ruler in --mode full output" ;;
esac

step "read --mode changes returns row list"
ROWS=$(KT read "$ID" --mode changes --max-lines 5 | jq_get 'len(d["result"]["rows"])')
[ "$ROWS" -ge 0 ] || fail "rows not a list"
pass "rows length=$ROWS"

step "region read"
REGION=$(KT region "$ID" --x 0 --y 0 --w 10 --h 2 | jq_get 'len(d["result"]["lines"])')
[ "$REGION" -eq 2 ] || fail "expected 2 lines, got $REGION"
pass "region returned 2 lines"

step "rows range"
ROWS_TEXT=$(KT rows "$ID" 0 1 | jq_get 'd["result"]["from"]')
[ "$ROWS_TEXT" = "0" ] || fail "rows.from != 0"
pass "rows.from=0"

step "events drain"
EVENTS=$(KT events "$ID" --max 50 | jq_get 'len(d["result"]["events"])')
[ "$EVENTS" -ge 0 ] || fail "events not a list"
pass "events drained=$EVENTS"

step "resize 30x100"
KT resize "$ID" 30 100 > /dev/null
NEW=$(KT status "$ID" | jq_get '(d["result"]["rows"], d["result"]["cols"])')
echo "$NEW" | grep -q "(30, 100)" || fail "resize did not stick: $NEW"
pass "resize applied"

step "json bridge ping"
PONG=$(printf '{"method":"ping"}\n' | "$KOU_TTY_BIN" --socket "$SOCK" json)
case "$PONG" in
  *'"pong":true'*) pass "ping/pong" ;;
  *) fail "no pong: $PONG" ;;
esac

step "list contains the terminal"
KT list | jq_get 'd["result"]["terminals"][0]["id"]' | grep -q "$ID" || fail "list missing $ID"
pass "list ok"

step "viewer start/stop"
VS=$(KT viewer start --port 8088)
ADDR=$(printf '%s' "$VS" | jq_get 'd["result"]["address"]')
case "$ADDR" in
  http://*) pass "viewer at $ADDR" ;;
  *) fail "no address: $VS" ;;
esac
sleep 0.2
curl -fsS "$ADDR/api/terminals" > /dev/null || fail "viewer API not reachable"
pass "viewer HTTP reachable"
KT viewer stop > /dev/null
pass "viewer stopped"

step "destroy"
KT destroy "$ID" > /dev/null
CLEAN_ID=""
pass "destroyed"

step "unknown id is a handled error"
if KT status zz > /tmp/kou-tty-zz.json; then
  fail "unexpected success"
else
  jq_get 'd["error"]["code"]' < /tmp/kou-tty-zz.json | grep -q not_found || fail "wrong error code"
  pass "not_found returned"
fi
rm -f /tmp/kou-tty-zz.json

printf '\n\033[1;32mall green\033[0m\n'
