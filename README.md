# kou-tty

A headless, in-memory terminal emulator for AI agents, exposed as a **CLI** (no MCP, no MCP framing).
Heavily inspired by [npcterm](https://github.com/alejandroqh/npcterm) — same surface, different shape: instead of an MCP server, `kou-tty` is a single binary with 17 subcommands and an optional JSON-RPC stdin/stdout bridge.

> Like SSH access. Use in sandboxed environments only.

## Install

### One-liner (binary + skill, all detected agent environments)

```sh
curl -fsSL https://raw.githubusercontent.com/gfhfyjbr/kou-tty/main/install.sh | bash
```

The script downloads the right `kou-tty` binary for your platform from the latest GitHub release into `~/.local/bin`, then fetches the `driving-terminal` skill files from `raw.githubusercontent.com` and installs them into every detected target (`opencode`, `claude-code`, `codex`, `pi`, `claude-desktop`, `openclaw`).

Other useful invocations:

```sh
# pin to a tag (binary release + matching skill ref)
curl -fsSL https://raw.githubusercontent.com/gfhfyjbr/kou-tty/v0.1.0/install.sh \
  | KOU_TTY_VERSION=v0.1.0 bash

# binary only, no skills
curl -fsSL https://raw.githubusercontent.com/gfhfyjbr/kou-tty/main/install.sh \
  | bash -s -- install --binary-only

# skill only, into one target, as a symlink to the repo
./install.sh install --skill-only --target codex --symlink

# show what would happen
./install.sh install --dry-run
```

### From source

```sh
git clone https://github.com/gfhfyjbr/kou-tty.git
cd kou-tty
cargo build --release
install -m 755 target/release/kou-tty ~/.local/bin/kou-tty
./install.sh install --skill-only --symlink   # link the in-repo skill into all detected agents
```

## Quick start

```sh
ID=$(kou-tty terminal create)
kou-tty terminal send-keys "$ID" '[{"text":"echo hello"},{"key":"Enter"}]'
sleep 0.2
kou-tty terminal show "$ID"
kou-tty terminal destroy "$ID" --if-exists
kou-tty shutdown
```

`kou-tty --help` lists every subcommand. Output is the **bare most-useful value** by default (id, plain text, process state), ready for `$(...)` and pipelines. Add `--json` / `-j` for the full envelope, `--compact` / `-c` for single-line JSON. Errors go to stderr as `error[<code>]: <message>` plus a `hint: ...` line.

Exit codes: `0` success · `2` usage / bad request · `3` not found · `5` conflict · `1` general failure. `shutdown` is idempotent (always `0`).

The full agent-facing reference lives in [`skills/driving-terminal/`](skills/driving-terminal/SKILL.md).

## Web viewer

```sh
kou-tty viewer start            # binds 127.0.0.1:8039, auto-probes +1..+10 if busy
open http://127.0.0.1:8039      # macOS; xdg-open on Linux
kou-tty viewer stop
```

## Installer cheatsheet

| Command | What it does |
| --- | --- |
| `./install.sh` | Interactive menu (or `cmd_install` if stdin is piped) |
| `./install.sh install` | Download binary + install skill into all detected targets |
| `./install.sh install --skill-only --symlink` | Symlink the local skill (dev workflow) |
| `./install.sh install --target codex` | One specific target |
| `./install.sh install --all-targets` | Even ones that aren't detected |
| `./install.sh install --version v0.1.0` | Pin a release |
| `./install.sh uninstall` | Remove the skill from every target |
| `./install.sh uninstall --remove-binary` | …and delete the binary |
| `./install.sh list` | Show what's installed where |
| `./install.sh doctor` | Diagnose detection / bundle / paths |


Supported targets:

| Target | Path |
| --- | --- |
| `opencode` | `~/.config/opencode/skills/` |
| `claude-code` | `~/.claude/skills/` |
| `codex` | `${CODEX_HOME:-~/.codex}/skills/` |
| `pi` | `~/.pi/agent/skills/` (or `$PWD/.pi/agent/skills/` with `--pi-local`) |
| `claude-desktop` | macOS: `~/Library/Application Support/Claude/skills/` · Linux: `~/.config/Claude/skills/` |
| `openclaw` | `~/.openclaw/skills/` |

Env overrides: `KOU_TTY_REPO`, `KOU_TTY_VERSION`, `KOU_TTY_SKILL_REF`, `KOU_TTY_INSTALL_DIR`, `KOU_TTY_SKILLS_DIR`, `CODEX_HOME`.

## Testing

| Layer | Command | What it covers |
| --- | --- | --- |
| Unit + lib integration | `cargo test --release` | `Grid` ops, ANSI/VT parser via real `vte`, key encoding, real PTY spawn (`Emulator::spawn` → echo a marker → assert in `grid.plain_text()`) |
| CLI integration | `cargo test --release --test cli` | Spawns the actual `target/release/kou-tty` binary, talks to the auto-spawned daemon over an isolated socket, asserts JSON responses |
| End-to-end shell | `scripts/smoke.sh` | 18 sequential CLI steps: create / send-keys / show / status / read / region / events / resize / json bridge / list / viewer HTTP / destroy / unknown-id error |

Examples:

```sh
# everything: 26 tests in 4 binaries
cargo test --release

# only the CLI integration suite
cargo test --release --test cli

# black-box shell test against the built binary
cargo build --release
scripts/smoke.sh
```

Manual one-liners while developing:

```sh
ID=$(./target/release/kou-tty terminal create)
./target/release/kou-tty terminal send-keys "$ID" '[{"text":"echo hi"},{"key":"Enter"}]'
sleep 0.2
./target/release/kou-tty terminal show "$ID"
./target/release/kou-tty terminal destroy "$ID" --if-exists
./target/release/kou-tty shutdown
```

JSON-RPC bridge (no subcommands, one request per line):

```sh
printf '{"method":"ping"}\n{"method":"terminal_list"}\n' \
  | ./target/release/kou-tty json
```

## CI / Release

- `.github/workflows/ci.yml` — runs on every push/PR: `cargo build --release`, `cargo test`, `cargo +nightly fmt -- --check`, shellcheck, and a Python skill-frontmatter validator.
- `.github/workflows/release.yml` — runs on every `v*` tag: builds binaries for macOS arm64/x64 and Linux x64/arm64 and publishes them as a GitHub release. `install.sh` and the skill itself live in the repo and are served via `raw.githubusercontent.com`; they are *not* uploaded as release assets.

## How `kou-tty` compares to `npcterm`

| | npcterm | kou-tty |
| --- | --- | --- |
| Transport | MCP (JSON-RPC over stdio, schema-aware) | CLI subcommands + raw JSON-RPC over Unix socket / stdin |
| Daemon | spawned per MCP client | one shared daemon per UID, auto-spawned on first call |
| Tools | 17 MCP tools | 17 subcommands (same surface) |
| Viewer | embedded HTML, axum | embedded HTML, axum |
| Audience | MCP-aware clients (Claude Code, Desktop, Codex, OpenCode, OpenClaw) | anything that can `exec("kou-tty …")` or write JSON to a pipe |

If your agent already speaks MCP, npcterm is the right tool. If it doesn't, or you want to compose terminal driving into normal shell pipelines, this is the one.

## License

Apache-2.0.
