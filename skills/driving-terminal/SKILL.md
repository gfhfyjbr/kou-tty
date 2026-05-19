---
name: driving-terminal
description: Drives interactive terminal sessions and TUI applications (vim, htop, less, lazygit, btop) through the kou-tty headless terminal emulator CLI. Use when an agent needs to spawn shells, send keystrokes, read screen contents, interact with curses-based or full-screen terminal programs, or run commands that require a real TTY rather than simple stdin/stdout piping.
---

# Driving Terminal

`kou-tty` is a headless terminal emulator exposed as a noun-verb CLI: every action is `kou-tty <noun> <verb>`. The CLI runs against a long-lived daemon that holds the PTY, the VT/ANSI-parsed in-memory grid, and the event queue. Use it whenever a task needs a real TTY: TUI programs, interactive prompts, paging, full-screen editors, or anything that depends on `isatty()`.

## When to use this skill

- Run interactive TUI programs (vim, less, htop, btop, lazygit, fzf, tmux, k9s).
- Drive REPLs that misbehave without a TTY (python -i, irb, mysql, psql with prompts).
- Capture coloured or curses output a normal bash subprocess can't render.
- Watch a long-running command screen by screen instead of reading megabytes of stdout.
- Inject keystrokes (arrows, F-keys, Ctrl+...) which plain `echo | program` can't deliver.

Skip it for one-shot non-interactive commands; plain shell is faster and cheaper.

## Quick start

```bash
ID=$(kou-tty terminal create)
kou-tty terminal send-keys "$ID" '[{"text":"echo hello"},{"key":"Enter"}]'
sleep 0.2
kou-tty terminal show "$ID"
kou-tty terminal destroy "$ID" --if-exists
```

## Output modes

Default output is the **bare most-useful value** — designed for `$(...)` capture and shell pipelines. Add a flag to switch to JSON when you need it:

| Command                              | Default (bare)                          | With `--json`                                                  |
| ------------------------------------ | --------------------------------------- | -------------------------------------------------------------- |
| `kou-tty terminal create`            | `a0`                                    | `{ "ok": true, "result": { "id": "a0", "rows": 24, ... } }`    |
| `kou-tty terminal list`              | one id per line                         | `{ "result": { "terminals": [{ ... }, ...] } }`                |
| `kou-tty terminal show <id>`         | plain screen text                       | `{ "result": { "text": "..." } }`                              |
| `kou-tty terminal status <id>`       | `waiting_for_input`                     | full struct (`process_state`, `has_new_content`, `cursor`, ...)|
| `kou-tty terminal read <id> --mode full` | screen text with coordinate ruler   | `{ "result": { "text": "...", "rows": [...], "cursor": ... } }`|
| `kou-tty terminal events <id>`       | one event per line, each line is a JSON | `{ "result": { "events": [ ... ] } }`                          |
| `kou-tty terminal send-keys ...`     | empty stdout (silent ok)                | `{ "result": { "sent": N } }`                                  |
| `kou-tty viewer start`               | `http://127.0.0.1:8039`                 | `{ "result": { "address": "..." } }`                           |

Flags:

- `--json` / `-j` — print the full JSON envelope `{ ok, result | error }`. Use this for `jq` queries or when you need fields the bare value doesn't expose (cursor position, exit_status, has_new_content, ...).
- `--compact` / `-c` — single-line JSON (implies `--json`). Useful for piping into log streams or `jq -c`.

Human-readable errors (`error[<code>]: <message>` and `hint: ...`) always go to **stderr**. In bare mode, stdout stays empty on error so `$(...)` is safe. In `--json` mode, stdout has the JSON envelope and stderr still has the readable line.

## Exit codes

Branch on the exit code instead of parsing strings:

| Code | Meaning |
| --- | --- |
| `0` | success |
| `1` | general failure |
| `2` | usage error / bad request (unknown key, malformed JSON, bad size) |
| `3` | terminal id not found |
| `5` | conflict / already exists |

## Core workflow checklist

- [ ] Step 1: `kou-tty terminal create` and capture the bare `id`
- [ ] Step 2: `kou-tty terminal send-keys <id>` with a JSON array of text + named keys
- [ ] Step 3: `kou-tty terminal status <id>` until it prints `idle` or `waiting_for_input`
- [ ] Step 4: `kou-tty terminal read <id> --mode changes` (token-efficient) or `terminal show <id>`
- [ ] Step 5: repeat 2–4 until done
- [ ] Step 6: `kou-tty terminal destroy <id> --if-exists`

Run `kou-tty shutdown` only when no more terminals are needed; the daemon is shared across every `kou-tty` invocation on the same socket.

## Decision tree

**Which read command?**

- Need plain text for summarisation / grep → `kou-tty terminal show <id>` (already plain in bare mode).
- Need only the rows that changed since the last read → `kou-tty --json terminal read <id> --mode changes` and read `result.rows` + `result.text`.
- Need a column/row ruler for clicking or pointing → `kou-tty terminal read <id> --mode full`.
- Need a specific rectangle (e.g. one panel in htop) → `kou-tty --json terminal region <id> --x N --y N --w N --h N` and read `result.lines`.
- Need to know if anything changed before reading → `kou-tty --json terminal status <id> | jq .result.has_new_content`.

**Wait strategy?**

- `kou-tty terminal status <id>` prints one of `running` / `idle` / `waiting_for_input` / `exited`.
- Shell prompt is back → `waiting_for_input`.
- Command finished but produced no prompt (daemon exited, ssh closed) → `exited` (use `--json` to read `exit_status`).
- TUI is steady-state → `idle` (and `has_new_content == false` via `--json`).

## Reading the screen efficiently

Three rules to keep token usage low:

1. Poll `kou-tty --json terminal status <id>` before each `read` and skip if `result.has_new_content == false`.
2. Prefer `kou-tty terminal read <id> --mode changes` after the first read — it returns only the rows that changed since the previous read, capped at `--max-lines` (default 50, max 200).
3. Use `kou-tty terminal show <id>` (bare default = plain text) for content that will be grepped / summarised. Use `terminal read --mode full` only when coordinates matter (clicking a button at row 5, column 12).

## Colours

`show`, `read`, `rows`, `region`, and `select` accept `--color always|auto|never` (default `auto`). When colour is on, the captured SGR escape sequences are re-emitted, so piping the output into a real terminal or `less -R` shows the same colours the program drew. Use `--color never` for grep / summarisation, `--color always` for visual inspection, and the default `auto` to let stdout-is-a-TTY decide.

## Sending input

`kou-tty terminal send-keys <id> '<json-array>'` accepts a mix of text and named keys:

```json
[{"text": "vim file.txt"}, {"key": "Enter"}]
[{"key": "Escape"}, {"text": ":wq"}, {"key": "Enter"}]
[{"key": "Ctrl+c"}]
[{"key": "Alt+f"}]
```

Supported key names: `Enter`, `Tab`, `Backspace`, `Escape`, `Space`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`, `Insert`, `Delete`, `F1`..`F12`, `Ctrl+<a-z>`, `Alt+<text>`.

`kou-tty terminal send-key <id> <name>` is a shortcut for a single named key.

## Mouse and selection

For TUI apps that listen to SGR-1006 mouse events (vim, htop, lazygit, fzf):

```bash
kou-tty terminal mouse "$ID" --event click --button left --x 12 --y 5
kou-tty terminal mouse "$ID" --event scroll --direction down --x 0 --y 0
kou-tty terminal mouse "$ID" --event drag --from-x 0 --from-y 0 --to-x 30 --to-y 10
```

`kou-tty terminal select <id> --from-row N --from-col N --to-row N --to-col N` returns the selected text without touching the terminal.

## Sizing and resizing

Allowed presets for `--size`: `80x24` (default), `120x40`, `160x40`, `200x50`. Custom sizes accepted as `COLSxROWS`, e.g. `--size 100x30`.

Resize later with `kou-tty terminal resize <id> <rows> <cols>`. The PTY receives `SIGWINCH` and reflows.

## Idempotency

- `terminal destroy <id> --if-exists` succeeds silently if the terminal is already gone (exit 0, `result.missing: true`).
- `shutdown` is always exit 0 — safe to call at the end of a script even if no daemon was started.

## Daemon lifecycle

The first `kou-tty <subcommand>` call auto-spawns a detached daemon and waits for the socket. Subsequent calls connect to the same daemon. Override with `--socket /custom/path.sock` to isolate parallel sessions (CI, multi-agent setups).

`kou-tty shutdown` stops the daemon; all terminals are killed.

## JSON-RPC stdin/stdout bridge

For agents that prefer to keep a single long-lived process:

```bash
echo '{"method":"terminal_create","params":{"size":{"rows":24,"cols":80}}}' | kou-tty json
```

The bridge reads one JSON request per line and writes one JSON response per line. Method names follow the noun-verb subcommands with underscores (`terminal_create`, `terminal_send_keys`, ...). See `references/json-protocol.md`.

## Web viewer

`kou-tty viewer start` launches a local web UI showing every active terminal with live updates. `kou-tty viewer stop` shuts it down. The viewer never starts on its own.

## Common pitfalls

- **Reading too early.** Right after `send-keys`, the program may not have produced output yet. Either `sleep 0.1-0.3` or poll `terminal status` until `has_new_content` flips to `true`.
- **Forgetting `Enter`.** Text in `send-keys` is not auto-submitted. Append `{"key":"Enter"}`.
- **Quitting vim.** Use `[{"key":"Escape"},{"text":":q!"},{"key":"Enter"}]`. Vim may be in insert mode — Escape first.
- **Cancelling a hung command.** Send `{"key":"Ctrl+c"}`. If ignored, escalate with `terminal destroy <id>`.
- **Off-by-one coordinates.** `read --mode full` rows and columns are 0-indexed in the ruler. `mouse --x N --y N` is also 0-indexed.
- **Mouse events ignored.** The target program must enable mouse reporting. vim, htop, lazygit, fzf do; plain bash does not.

## Feedback loop pattern

For any non-trivial sequence, follow read → act → verify:

1. Send the next batch of keystrokes.
2. Poll `terminal status` until `has_new_content == true` or `process_state` settles.
3. Read the screen (`terminal read --mode changes` is usually enough).
4. Compare with the expected outcome.
5. If wrong, escape (`Ctrl+c`, `Escape`, `:q!`) and retry; do not "keep typing through" an unexpected state.

## References

Load these only when the current step needs them:

- `references/commands.md` — every subcommand, every flag, every example.
- `references/json-protocol.md` — JSON-RPC method names, request/response shapes, error codes.
- `references/tui-recipes.md` — ready-made sequences for vim, less, htop, btop, lazygit, fzf, tmux, k9s.
- `references/viewer.md` — web viewer details, ports, WebSocket payload.
- `references/troubleshooting.md` — daemon crashes, stuck terminals, encoding issues, sandbox / CI notes.
