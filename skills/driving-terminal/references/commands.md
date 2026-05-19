# kou-tty Command Reference

## Contents

- Output flags
- Exit codes
- Global flags
- `daemon` — run the daemon
- `json` — JSON-RPC stdin/stdout bridge
- `repl` — interactive debug REPL
- `shutdown` — stop the daemon (idempotent)
- `terminal create`
- `terminal destroy`
- `terminal list`
- `terminal send-key`
- `terminal send-keys`
- `terminal mouse`
- `terminal read`
- `terminal show`
- `terminal rows`
- `terminal region`
- `terminal status`
- `terminal events`
- `terminal select`
- `terminal scroll`
- `terminal resize`
- `viewer start | stop | status | open`

## Output flags

Every subcommand writes JSON to stdout. The two global flags control the format:

- (default) — pretty multi-line JSON.
- `--compact` / `-c` — single-line JSON, ideal for piping into `jq -c` or grep.
- `--quiet`  / `-q` — bare value of the most useful field only (no JSON envelope).

Human-readable error lines (`error[<code>]: ...` and `hint: ...`) always go to **stderr**.

## Exit codes

- `0` — success
- `1` — general failure (PTY spawn failed, write failed, internal panic captured)
- `2` — usage / bad request (unknown key name, malformed JSON, bad size string)
- `3` — terminal id not found
- `5` — conflict / already exists

`shutdown` always exits 0, even if no daemon is running.

## Global flags

- `--socket <PATH>` — custom Unix socket. Default: `$XDG_RUNTIME_DIR/kou-tty-<uid>.sock` or `$TMPDIR/kou-tty-<uid>.sock`.
- `--quiet` / `-q`, `--compact` / `-c` — see above.

Set `RUST_LOG=info kou-tty ...` to see daemon logs on stderr.

## `daemon`

Run the daemon in the foreground (no fork). Useful when supervising with systemd, launchd, or tmux. Most workflows do not call this directly; client subcommands auto-spawn a detached daemon.

```bash
kou-tty daemon --socket /tmp/kou-tty.sock
```

## `json`

Read JSON-RPC requests from stdin (one per line) and write responses to stdout (one per line).

```bash
echo '{"method":"ping"}' | kou-tty json
```

## `repl`

Interactive line-oriented REPL. Type JSON-RPC requests, or `help` / `quit`.

```bash
kou-tty repl
```

## `shutdown`

Idempotent. Always exit 0.

```bash
kou-tty shutdown
```

## `terminal create`

```bash
kou-tty terminal create [--size <SIZE>] [--shell <PATH>]
kou-tty --quiet terminal create        # prints just the id
```

- `--size`: `80x24` (default) / `120x40` / `160x40` / `200x50` / custom `COLSxROWS`.
- `--shell`: shell binary. Defaults to `$SHELL`, or `/bin/bash` if unset.

Result: `{ "id": "ab", "rows": 24, "cols": 80 }`.

## `terminal destroy`

```bash
kou-tty terminal destroy <ID>
kou-tty terminal destroy <ID> --if-exists   # idempotent, never fails on missing
```

Kills the child process and removes the terminal from the registry.

## `terminal list`

```bash
kou-tty terminal list
kou-tty --quiet terminal list   # one id per line
```

Returns all active terminals with size, `process_state`, and `has_new_content`.

## `terminal send-key`

```bash
kou-tty terminal send-key <ID> <KEY>
```

`KEY` is a named key (`Enter`, `Tab`, `Escape`, `Ctrl+c`, ...). For literal text use `terminal send-keys`.

## `terminal send-keys`

```bash
kou-tty terminal send-keys <ID> '<JSON-ARRAY>'
```

Each element is `{"text":"..."}` or `{"key":"..."}`. Example:

```bash
kou-tty terminal send-keys ab '[{"text":"vim file.txt"},{"key":"Enter"}]'
```

Supported key names:

- Whitespace: `Enter`, `Tab`, `Space`, `Backspace`
- Control: `Escape`, `Insert`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`
- Arrows: `Up`, `Down`, `Left`, `Right`
- Function: `F1`–`F12`
- Modifier combos: `Ctrl+<a-z>` (`Ctrl+c`, `Ctrl+d`), `Alt+<text>`

## `terminal mouse`

```bash
kou-tty terminal mouse <ID> --event click   --button left|middle|right --x <COL> --y <ROW>
kou-tty terminal mouse <ID> --event press   --button left              --x <COL> --y <ROW>
kou-tty terminal mouse <ID> --event release --button left              --x <COL> --y <ROW>
kou-tty terminal mouse <ID> --event scroll  --direction up|down        --x <COL> --y <ROW>
kou-tty terminal mouse <ID> --event drag    --from-x N --from-y N --to-x N --to-y N
```

Coordinates are 0-indexed. SGR-1006 encoding is emitted; the target program must enable mouse reporting.

## `terminal read`

```bash
kou-tty terminal read <ID> [--mode full|changes|plain] [--max-lines N]
kou-tty --quiet terminal read <ID> --mode full   # bare text (still has the ruler)
```

- `full` (default): every row, with a column ruler header and row numbers.
- `changes`: only rows that changed since the last read. Capped at `--max-lines` (default 50, max 200).
- `plain`: every row, no overlay.

## `terminal show`

```bash
kou-tty terminal show <ID>
kou-tty --quiet terminal show <ID>   # bare plain text
```

Plain text dump of the screen, no coordinate overlay. Trailing blank rows are preserved as empty lines.

## `terminal rows`

```bash
kou-tty terminal rows <ID> <FROM> <TO>
```

Read rows `[FROM..=TO]` as plain text. 0-indexed.

## `terminal region`

```bash
kou-tty terminal region <ID> --x <COL> --y <ROW> --w <WIDTH> --h <HEIGHT>
```

Returns `lines: [string]`.

## `terminal status`

```bash
kou-tty terminal status <ID>
kou-tty --quiet terminal status <ID>   # just the process_state string
```

Fields: `process_state` (`running|idle|waiting_for_input|exited`), `has_new_content`, `exit_status`, `cursor`, `bytes_in`, `shell`.

## `terminal events`

```bash
kou-tty terminal events <ID> [--max N]
```

Drains up to `N` events. Types: `screen_changed`, `process_state_changed`, `waiting_for_input`, `command_finished`, `bell`. The ring buffer holds 1024 events.

## `terminal select`

```bash
kou-tty terminal select <ID> --from-row R --from-col C --to-row R --to-col C
```

Returns the text between the two points. Pure read.

## `terminal scroll`

```bash
kou-tty terminal scroll <ID> <BY>
```

Reserved for scrollback control. Positive is down.

## `terminal resize`

```bash
kou-tty terminal resize <ID> <ROWS> <COLS>
```

Resize the PTY and the in-memory grid. The running program receives `SIGWINCH`.

## `viewer`

```bash
kou-tty viewer start [--port N]
kou-tty viewer stop
kou-tty viewer status
kou-tty viewer open    # same as start; prints the URL for `open` / `xdg-open`
```

Binds 127.0.0.1:N (default 8039, auto-probes the next 10 ports).
