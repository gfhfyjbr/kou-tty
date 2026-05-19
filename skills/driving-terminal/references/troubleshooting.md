# Troubleshooting

## Contents

- "daemon did not start within timeout"
- "another kou-tty daemon is already running"
- Terminal stuck in `running` forever
- `process_state` never reaches `waiting_for_input`
- Mouse events appear ignored
- Wrong characters or `?` glyphs in `show`
- Sandbox / CI failures
- Resetting the daemon manually

## "daemon did not start within timeout"

The CLI client could not connect to the auto-spawned daemon within 2.5 seconds. Reasons:

1. The `kou-tty` binary is not in `PATH` for the spawned process. Run the daemon manually with an absolute path: `/path/to/kou-tty daemon --socket /tmp/kou-tty.sock` then re-run the client with the same `--socket`.
2. The socket directory is not writable (e.g. read-only `$TMPDIR`). Override with `--socket /writable/place.sock`.
3. The binary is panicking on startup. Run `RUST_LOG=debug kou-tty daemon --socket /tmp/kou-tty.sock` in the foreground to see the error.

## "another kou-tty daemon is already running"

A live socket exists at the target path. Either reuse it (omit `--socket` so both client and daemon agree), or stop the running daemon: `kou-tty --socket <PATH> shutdown`.

If the daemon process is gone but the socket file remains and a probe times out, delete the socket: `rm -f $(kou-tty --help | grep socket | head -1)` — the next invocation will recreate it.

## Terminal stuck in `running` forever

The child process is producing output continuously, so the idle detector never fires. This is correct for `tail -f`, `top`, `watch`. To make a real progress decision, prefer:

- Polling `terminal_poll_events` and reacting to `screen_changed` events.
- Reading with `terminal_read_screen --mode changes` and grepping for known prompts.

Do not block on `process_state == "idle"` for genuinely interactive programs.

## `process_state` never reaches `waiting_for_input`

The heuristic looks for `$ # % >` at the end of the cursor row. Custom shell prompts (powerline, oh-my-zsh symbols, multi-line prompts) may not trigger it. Fallbacks:

- Search the screen for the user-supplied prompt with `terminal_show_screen` and a regex.
- Listen for the `screen_changed` event with the cursor row in its `rows` list.

## Mouse events appear ignored

Mouse reporting must be enabled by the target program. Most TUI apps (vim, htop, lazygit, fzf, tmux) do so when they detect a real TTY. A plain shell does not. If a click "does nothing":

1. Confirm the target program is the one consuming input. After spawning a TUI, the shell is no longer reading; the TUI is.
2. Try `--button left` clicks; some apps ignore middle/right.
3. Verify coordinates with `read --mode full` first — clicking at `--y 5` means the 6th row visually.

## Wrong characters or `?` glyphs in `show`

The grid stores `char` per cell. Multi-codepoint graphemes (some emoji, combining marks) are flattened to a single visible character. For agents this is usually fine; for byte-perfect captures, prefer `terminal_show_screen` and accept the limitation, or use a screen recording tool outside kou-tty.

## Sandbox / CI failures

In containers without `$XDG_RUNTIME_DIR` the daemon falls back to `$TMPDIR`. If both are missing or read-only, pass `--socket /tmp/kou-tty.sock` explicitly. The socket inherits the process uid; cross-user usage is not supported.

For unprivileged containers, ensure `/dev/ptmx` is exposed (it is by default in Docker, podman, and most CI images).

## Resetting the daemon manually

```bash
kou-tty --socket <PATH> shutdown
pkill -f 'kou-tty daemon'
rm -f <PATH>
```

After this any new client invocation auto-spawns a fresh daemon.
