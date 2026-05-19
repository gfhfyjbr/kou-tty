---
name: driving-terminal
description: Drives interactive terminal sessions and TUI applications (vim, htop, less, lazygit, btop) through the kou-tty headless terminal emulator CLI. Use when an agent needs to spawn shells, send keystrokes, read screen contents, interact with curses-based or full-screen terminal programs, or run commands that require a real TTY rather than simple stdin/stdout piping.
---

# Driving Terminal

`kou-tty` is a headless terminal emulator exposed as a CLI. It spawns a PTY per terminal, drains output through a VT/ANSI parser into an in-memory grid, and exposes 17 subcommands plus a JSON-RPC stdin/stdout bridge. Use it whenever a task needs a real TTY: TUI programs, interactive prompts, paging, full-screen editors, or anything that depends on `isatty()`.

## When to use this skill

- Run interactive TUI programs (vim, less, htop, btop, lazygit, fzf, tmux, k9s).
- Drive REPLs that misbehave without a TTY (python -i, irb, mysql, psql with prompts).
- Capture coloured/curses output a normal bash subprocess can't render.
- Watch a long-running command screen by screen instead of reading megabytes of stdout.
- Inject keystrokes (arrows, F-keys, Ctrl+...) which plain `echo | program` can't deliver.

Do not use it for one-shot non-interactive commands; plain shell is faster and cheaper.

## Quick start

Run a command in a fresh terminal and read the result:

```bash
ID=$(kou-tty create --size 80x24 | jq -r .result.id)
kou-tty send-keys "$ID" '[{"text":"echo hello"},{"key":"Enter"}]'
sleep 0.2
kou-tty show "$ID"
kou-tty destroy "$ID"
```

Every subcommand prints a JSON object of the form `{"ok": bool, "result": ..., "error": ...}`. Exit code is non-zero when `ok=false`. Pipe through `jq` when scripting.

## Core workflow checklist

Copy this list into the task plan and tick boxes as work progresses:

- [ ] Step 1: `kou-tty create` (record the returned `id`)
- [ ] Step 2: send keystrokes with `send-keys` (text + named keys in one JSON array)
- [ ] Step 3: poll `status` — only read when `process_state` is `idle` or `waiting_for_input`
- [ ] Step 4: read the screen with `read --mode changes` (cheap) or `show` (plain text)
- [ ] Step 5: drive the next step, or `destroy` the terminal when done

Stop the daemon (`kou-tty shutdown`) only when no more terminals are needed; the daemon is shared across all `kou-tty` invocations on the same socket.

## Decision tree

Pick the right read command first time:

- Need plain text for summarisation/grep → `kou-tty show <id>`.
- Need to know which rows changed since last read → `kou-tty read <id> --mode changes`.
- Need the whole screen with a column/row ruler for clicking or pointing → `kou-tty read <id> --mode full`.
- Need a specific rectangle (e.g. a panel in htop) → `kou-tty region <id> --x N --y N --w N --h N`.
- Need to know if the screen changed at all before reading → `kou-tty status <id>` and check `has_new_content`.

Pick the right wait strategy:

- The shell prompt is back → `process_state == "waiting_for_input"`.
- The command finished but produced no prompt (e.g. a daemon exited cleanly) → `process_state == "exited"` with `exit_status` set.
- A TUI is steady-state → `process_state == "idle"` (no output for >500 ms) and `has_new_content == false`.

## Reading the screen efficiently

Three rules to keep token usage low:

1. Poll `status` before each `read`. Skip the read entirely if `has_new_content == false`.
2. Prefer `read --mode changes` over `read --mode full` after the first read. It returns only rows that changed since the previous read and stops at `--max-lines` (default 50, max 200).
3. Use `show` (plain text, no coordinates) for content that goes into summaries, regexes, or downstream tools. Use `read` only when coordinates matter (e.g. clicking a button at row 5, column 12).

## Sending input

`kou-tty send-keys <id> '<json-array>'` accepts a mix of text and named keys:

```json
[{"text": "vim file.txt"}, {"key": "Enter"}]
[{"key": "Escape"}, {"text": ":wq"}, {"key": "Enter"}]
[{"key": "Ctrl+c"}]
[{"key": "Alt+f"}]
```

Supported key names: `Enter`, `Tab`, `Backspace`, `Escape`, `Space`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`, `Insert`, `Delete`, `F1`..`F12`, `Ctrl+<a-z>`, `Alt+<text>`.

`kou-tty send-key <id> <name>` is a shortcut for a single named key.

## Mouse and selection

For TUI apps that listen to SGR-1006 mouse events (e.g. fzf, lazygit):

```bash
kou-tty mouse "$ID" --event click --button left --x 12 --y 5
kou-tty mouse "$ID" --event scroll --direction down --x 0 --y 0
kou-tty mouse "$ID" --event drag --from-x 0 --from-y 0 --to-x 30 --to-y 10
```

`kou-tty select <id> --from-row N --from-col N --to-row N --to-col N` returns the selected text without touching the terminal — purely a read operation.

## Sizing and resizing

Allowed presets for `--size`: `80x24` (default), `120x40`, `160x40`, `200x50`. Custom sizes accepted as `COLSxROWS`, e.g. `--size 100x30`.

Resize later with `kou-tty resize <id> <rows> <cols>`. The PTY is informed via `TIOCSWINSZ` so the running program will reflow.

## Daemon lifecycle

The first `kou-tty <subcommand>` call auto-spawns a detached daemon and waits for the socket. Subsequent calls connect to the same daemon. Override the socket path with `--socket /custom/path.sock` if multiple isolated sessions are needed (e.g. CI parallelism).

`kou-tty shutdown` stops the daemon; all terminals are killed. Use it at the very end of a workflow.

## JSON-RPC stdin/stdout bridge

For agents that prefer to talk directly over JSON instead of subcommands:

```bash
echo '{"method":"terminal_create","params":{"size":{"rows":24,"cols":80}}}' | kou-tty json
```

The bridge reads one JSON request per line and writes one JSON response per line. The method names mirror subcommand names with underscores (see `references/json-protocol.md`).

## Web viewer

`kou-tty viewer start` launches a local web UI showing every active terminal with live updates. Useful for humans watching an agent work in real time. `kou-tty viewer stop` shuts it down. The viewer never starts on its own.

## Common pitfalls

- **Reading too early.** Right after `send-keys`, the program may not have produced output yet. Either `sleep 0.1-0.3`, or poll `status` until `has_new_content` flips to `true`, before reading.
- **Forgetting `Enter`.** Text sent via `send-keys` is not auto-submitted. Append `{"key":"Enter"}` to execute it.
- **Quitting vim.** Use `[{"key":"Escape"},{"text":":q!"},{"key":"Enter"}]`. Do not send `:q` as text without first pressing Escape; vim will be in insert mode.
- **Cancelling a hung command.** Send `{"key":"Ctrl+c"}`. If the program ignores SIGINT, escalate with `kou-tty destroy <id>`.
- **Coordinate-system off-by-one.** `read --mode full` columns and rows are 0-indexed in headers. `mouse --x N --y N` expects 0-indexed too.
- **Mouse events ignored.** The target program must have enabled mouse reporting (SGR-1006). vim, htop, lazygit, fzf do; plain bash does not.

## Feedback loop pattern

For any non-trivial sequence, follow read → act → verify:

1. Send the next batch of keystrokes.
2. Poll `status` until `has_new_content == true` or `process_state` settles.
3. Read the screen (`read --mode changes` is usually enough).
4. Compare with the expected outcome.
5. If wrong, escape (`Ctrl+c`, `Escape`, `:q!`) and retry; do not "keep typing through" an unexpected state.

## References

Load these only when the current step needs them:

- `references/commands.md` — every subcommand, every flag, every example.
- `references/json-protocol.md` — JSON-RPC method names, request/response shapes, error codes.
- `references/tui-recipes.md` — ready-made sequences for vim, less, htop, btop, lazygit, fzf, tmux, k9s.
- `references/viewer.md` — web viewer details, ports, WebSocket payload.
- `references/troubleshooting.md` — daemon crashes, stuck terminals, encoding issues, sandbox/CI notes.
