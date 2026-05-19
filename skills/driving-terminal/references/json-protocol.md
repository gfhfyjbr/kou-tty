# kou-tty JSON-RPC Protocol

## Contents

- Transport
- Request envelope
- Response envelope
- Method index
- Method details
- Error codes
- Event payloads

## Transport

Two ways to speak JSON-RPC to the daemon:

1. `kou-tty json` — long-lived process. Write one JSON request per line to stdin, read one response per line from stdout.
2. Direct Unix socket — connect to `$XDG_RUNTIME_DIR/kou-tty-<uid>.sock`, write a single JSON line, read the response line, close. Suitable for thin language bindings.

Both transports share the same envelopes.

## Request envelope

```json
{ "method": "<name>", "params": { ... } }
```

Methods without parameters may omit `params`:

```json
{ "method": "ping" }
```

## Response envelope

```json
{ "ok": true,  "result": { ... } }
{ "ok": false, "error":  { "code": "<code>", "message": "<text>" } }
```

## Method index

| Method | Description |
| --- | --- |
| `ping` | Liveness probe |
| `terminal_create` | Spawn a new terminal |
| `terminal_destroy` | Kill a terminal |
| `terminal_list` | List active terminals |
| `terminal_send_key` | Send one named key |
| `terminal_send_keys` | Send a sequence of inputs |
| `terminal_mouse` | Send a mouse event |
| `terminal_read_screen` | Read screen with coordinate overlay |
| `terminal_show_screen` | Read screen as plain text |
| `terminal_read_rows` | Read a range of rows |
| `terminal_read_region` | Read a rectangle |
| `terminal_status` | Process state and metadata |
| `terminal_poll_events` | Drain the event ring buffer |
| `terminal_select` | Extract text from a region |
| `terminal_scroll` | Scrollback control |
| `terminal_resize` | Change PTY size |
| `viewer_start` | Start the web viewer |
| `viewer_stop` | Stop the web viewer |
| `viewer_status` | Web viewer status |
| `shutdown` | Stop the daemon |

## Method details

### `terminal_create`

```json
{ "method": "terminal_create",
  "params": { "size": "120x40", "shell": "/bin/zsh" } }
```

`size` is either a preset name (`80x24`, `120x40`, `160x40`, `200x50`) or `{ "rows": R, "cols": C }`. Both `size` and `shell` are optional.

Result:

```json
{ "id": "ab", "rows": 40, "cols": 120 }
```

### `terminal_send_keys`

```json
{ "method": "terminal_send_keys",
  "params": { "id": "ab",
              "input": [ { "text": "ls" }, { "key": "Enter" } ] } }
```

Each element is `{"text":"..."}` (literal characters) or `{"key":"..."}` (named key, see commands.md).

Result: `{ "sent": <bytes> }`.

### `terminal_mouse`

```json
{ "method": "terminal_mouse",
  "params": { "id": "ab",
              "event": "click", "button": "left", "x": 12, "y": 5 } }
```

`event` is one of: `click`, `press`, `release`, `scroll`, `drag`.

- `scroll` requires `direction: "up" | "down"`.
- `drag` requires `from_x`, `from_y`, `to_x`, `to_y` instead of `x`/`y`.

Result: `{ "events": <n> }`.

### `terminal_read_screen`

```json
{ "method": "terminal_read_screen",
  "params": { "id": "ab", "mode": "changes", "max_lines": 50 } }
```

`mode`: `full` (default) | `changes` | `plain`. `max_lines` is honoured only for `changes` (default 50, max 200).

Result:

```json
{ "text": "...", "rows": [u16], "cursor": { "row": R, "col": C } }
```

### `terminal_status`

```json
{ "method": "terminal_status", "params": { "id": "ab" } }
```

Result:

```json
{ "id": "ab",
  "rows": 24, "cols": 80,
  "process_state": "waiting_for_input",
  "has_new_content": false,
  "exit_status": null,
  "cursor": { "row": 5, "col": 14 },
  "bytes_in": 1024,
  "shell": "/bin/zsh" }
```

### `terminal_poll_events`

```json
{ "method": "terminal_poll_events", "params": { "id": "ab", "max": 100 } }
```

Result: `{ "events": [ ... ] }` — drains up to `max` events from the ring buffer. See *Event payloads*.

### Remaining methods

`terminal_destroy`, `terminal_list`, `terminal_send_key`, `terminal_show_screen`, `terminal_read_rows`, `terminal_read_region`, `terminal_select`, `terminal_scroll`, `terminal_resize`, `viewer_start`, `viewer_stop`, `viewer_status`, `shutdown` mirror the subcommands of the same name in `commands.md`; the request shapes follow the CLI flag names converted to snake_case.

## Error codes

| Code | Meaning |
| --- | --- |
| `bad_request` | Malformed JSON or wrong shape |
| `bad_size` | Unrecognised size string |
| `bad_key` | Unknown key name in `send-key` / `send-keys` |
| `not_found` | Terminal `id` does not exist |
| `create_failed` | PTY spawn or shell exec failed |
| `write_failed` | PTY write failed (terminal dead?) |
| `resize_failed` | PTY ioctl failed |
| `viewer_failed` | Bind / start failed |
| `shutdown` | Daemon acknowledging shutdown (this is returned then connection closes) |
| `internal` | Unexpected panic captured in `spawn_blocking` |

## Event payloads

```json
{ "type": "screen_changed",
  "rows": [u16], "timestamp_ms": 1731000000000 }

{ "type": "process_state_changed",
  "from": "running", "to": "waiting_for_input",
  "timestamp_ms": 1731000000123 }

{ "type": "waiting_for_input", "timestamp_ms": 1731000000123 }

{ "type": "command_finished",
  "exit_code": 0, "timestamp_ms": 1731000000456 }

{ "type": "bell" }
```

Drain regularly with `terminal_poll_events`; the ring buffer holds 1024 events and silently drops the oldest when full.
