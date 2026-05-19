# kou-tty Command Reference

## Contents

- Global flags
- `daemon` — run the daemon
- `json` — JSON-RPC stdin/stdout bridge
- `repl` — interactive debug REPL
- `create` — spawn a terminal
- `destroy` — kill a terminal
- `list` — list active terminals
- `send-key` — send one named key
- `send-keys` — send a sequence of inputs
- `mouse` — send mouse events
- `read` — read screen with coordinate overlay
- `show` — read screen as plain text
- `rows` — read a range of rows
- `region` — read a rectangle
- `status` — process state and metadata
- `events` — drain the event queue
- `select` — extract text from a region
- `scroll` — scrollback control
- `resize` — change PTY size
- `viewer start|stop|status|open` — web viewer
- `shutdown` — stop the daemon
- Exit codes
- Output format

## Global flags

- `--socket <PATH>` — custom Unix socket. Default: `$XDG_RUNTIME_DIR/kou-tty-<uid>.sock` or `$TMPDIR/kou-tty-<uid>.sock`.

Set `RUST_LOG=info kou-tty …` to see daemon logs on stderr.

## `daemon`

Run the daemon in the foreground (no fork). Useful when supervising it with systemd, launchd, or tmux.

```bash
kou-tty daemon --socket /tmp/kou-tty.sock
```

Most workflows do not call this directly; client subcommands auto-spawn a detached daemon as needed.

## `json`

Read JSON-RPC requests from stdin (one per line) and write responses to stdout. Used by AI agents that prefer a single long-lived process over per-command subprocesses.

```bash
echo '{"method":"ping"}' | kou-tty json
```

## `repl`

Interactive line-oriented REPL. Type JSON-RPC requests, or `help` / `quit`.

```bash
kou-tty repl
```

## `create`

Spawn a new terminal.

```bash
kou-tty create [--size <SIZE>] [--shell <PATH>]
```

- `--size`: `80x24` (default) / `120x40` / `160x40` / `200x50` / custom `COLSxROWS`.
- `--shell`: shell binary. Defaults to `$SHELL`, or `/bin/bash` if unset.

Returns: `{ "id": "ab", "rows": 24, "cols": 80 }`.

## `destroy`

```bash
kou-tty destroy <ID>
```

Kills the child process and removes the terminal from the registry. The terminal `id` becomes invalid.

## `list`

```bash
kou-tty list
```

Returns all active terminals with size, `process_state`, and `has_new_content` flags.

## `send-key`

```bash
kou-tty send-key <ID> <KEY>
```

`KEY` is a named key (`Enter`, `Tab`, `Escape`, …). For literal text use `send-keys` with `{"text":"..."}`.

## `send-keys`

```bash
kou-tty send-keys <ID> '<JSON-ARRAY>'
```

Each element is `{"text":"..."}` or `{"key":"..."}`. Example:

```bash
kou-tty send-keys ab '[{"text":"vim file.txt"},{"key":"Enter"}]'
```

Supported key names:

- Whitespace: `Enter`, `Tab`, `Space`, `Backspace`
- Control: `Escape`, `Insert`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`
- Arrows: `Up`, `Down`, `Left`, `Right`
- Function: `F1`–`F12`
- Modifier combos: `Ctrl+<a-z>` (e.g. `Ctrl+c`, `Ctrl+d`), `Alt+<text>`

## `mouse`

```bash
kou-tty mouse <ID> --event click   --button left|middle|right --x <COL> --y <ROW>
kou-tty mouse <ID> --event press   --button left              --x <COL> --y <ROW>
kou-tty mouse <ID> --event release --button left              --x <COL> --y <ROW>
kou-tty mouse <ID> --event scroll  --direction up|down        --x <COL> --y <ROW>
kou-tty mouse <ID> --event drag    --from-x N --from-y N --to-x N --to-y N
```

Coordinates are 0-indexed. Mouse events are emitted as SGR-1006 escape sequences; the target program must enable mouse reporting.

## `read`

```bash
kou-tty read <ID> [--mode full|changes|plain] [--max-lines N]
```

- `full` (default): every row, with a column header and row numbers.
- `changes`: only rows that changed since the last read. Capped at `--max-lines` (default 50, max 200).
- `plain`: every row, no overlay.

Response includes `cursor: { row, col }`.

## `show`

```bash
kou-tty show <ID>
```

Plain text dump of the screen, no coordinate overlay. Trailing blank rows are preserved as empty lines.

## `rows`

```bash
kou-tty rows <ID> <FROM> <TO>
```

Read rows `[FROM..=TO]` as plain text. 0-indexed.

## `region`

```bash
kou-tty region <ID> --x <COL> --y <ROW> --w <WIDTH> --h <HEIGHT>
```

Read a rectangle, returns `lines: [string]`.

## `status`

```bash
kou-tty status <ID>
```

Fields:

- `process_state`: `running` | `idle` | `waiting_for_input` | `exited`
- `has_new_content`: `bool` (true since the last `read` / `show`)
- `exit_status`: `int | null`
- `cursor`: `{ row, col }`
- `bytes_in`: total bytes parsed from PTY
- `shell`: shell binary path

Use this *before* a `read` to skip useless work.

## `events`

```bash
kou-tty events <ID> [--max N]
```

Drains up to `N` events. Event types:

- `screen_changed { rows: [u16], timestamp_ms }`
- `process_state_changed { from, to, timestamp_ms }`
- `waiting_for_input { timestamp_ms }`
- `command_finished { exit_code, timestamp_ms }`
- `bell`

The queue is a ring buffer (capacity 1024). Drain regularly or events are dropped.

## `select`

```bash
kou-tty select <ID> --from-row R --from-col C --to-row R --to-col C
```

Returns the text between the two points. Pure read; the screen is untouched.

## `scroll`

```bash
kou-tty scroll <ID> <BY>
```

Reserved for scrollback. Positive is down. The scrollback buffer stays on the daemon side; rendering is consumer-controlled.

## `resize`

```bash
kou-tty resize <ID> <ROWS> <COLS>
```

Resize the PTY and the in-memory grid. The running program receives `SIGWINCH` and reflows.

## `viewer`

```bash
kou-tty viewer start [--port N]
kou-tty viewer stop
kou-tty viewer status
kou-tty viewer open
```

`start` binds 127.0.0.1:N (default 8039, auto-probes the next 10 ports). `open` is identical to `start`; the URL is returned for the caller to hand off to `open` / `xdg-open`.

## `shutdown`

```bash
kou-tty shutdown
```

Stops the daemon; all terminals are killed. Always returns `ok=false code=shutdown` as the daemon disconnects after acknowledging.

## Exit codes

- `0` — `ok=true` response.
- `1` — `ok=false` response, parse failure, or transport error.

## Output format

Every subcommand prints a single JSON object:

```json
{ "ok": true,  "result": { ... } }
{ "ok": false, "error":  { "code": "...", "message": "..." } }
```

Pipe through `jq` for scripting:

```bash
ID=$(kou-tty create | jq -r .result.id)
```
