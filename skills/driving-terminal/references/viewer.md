# Web Viewer

The viewer is an opt-in, locally-bound web UI that shows every active terminal with live updates. It is useful for humans watching an agent work; it has no effect on the agent's normal flow.

## Starting

```bash
kou-tty viewer start [--port 8039]
```

`viewer start` is idempotent: if the viewer is already running it returns the existing address.

The viewer always binds `127.0.0.1`. If the requested port is occupied it tries the next 10 ports.

## Stopping

```bash
kou-tty viewer stop
```

Stops the HTTP server. Terminals are unaffected.

## Status

```bash
kou-tty viewer status
```

Returns `{ "running": bool, "address": "http://127.0.0.1:8039" | null }`.

## URL handoff

`viewer open` is identical to `viewer start` but is meant for piping into `open` / `xdg-open`:

```bash
URL=$(kou-tty viewer open | jq -r .result.address)
open "$URL"
```

## HTTP API

- `GET /` — embedded HTML UI.
- `GET /api/terminals` — list of terminals (`id`, `rows`, `cols`, `process_state`).
- `GET /api/terminals/<id>` — single terminal with current plain-text screen.
- `WS  /ws/terminals/<id>` — pushes a JSON snapshot every ~150 ms when the screen sequence number changes:

```json
{ "id": "ab",
  "rows": 24, "cols": 80,
  "cursor": { "row": 5, "col": 10 },
  "process_state": "running",
  "text": "...",
  "seq": 42 }
```

## When not to use the viewer

The viewer keeps a WebSocket open and re-renders the screen on every change. For agent-only workflows it adds CPU and memory load with no benefit. Start it only when a human asks to watch.
