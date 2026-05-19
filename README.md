# kou-tty

A headless, in-memory terminal emulator for AI agents, exposed as a **CLI** (no MCP, no MCP framing).
Heavily inspired by [npcterm](https://github.com/alejandroqh/npcterm) — same surface, different shape: instead of an MCP server, `kou-tty` is a single binary with 17 subcommands and an optional JSON-RPC stdin/stdout bridge.

> Like SSH access. Use in sandboxed environments only.

## Install

### One-liner (binary + skill, all detected agent environments)

```sh
curl -fsSL https://github.com/paranoikcodit/kou-tty/releases/latest/download/install.sh | bash
```

This downloads the right `kou-tty` binary for your platform into `~/.local/bin`, then unpacks the `driving-terminal` skill from the embedded bundle and installs it into every detected target (`opencode`, `claude-code`, `codex`, `pi`, `claude-desktop`, `openclaw`).

Other useful invocations:

```sh
# pin a release
curl -fsSL https://github.com/paranoikcodit/kou-tty/releases/download/v0.1.0/install.sh | bash

# binary only, no skills
curl -fsSL .../install.sh | bash -s -- install --binary-only

# skill only, into one target, as a symlink to the repo
./install.sh install --skill-only --target codex --symlink

# show what would happen
./install.sh install --dry-run
```

### From source

```sh
git clone https://github.com/paranoikcodit/kou-tty.git
cd kou-tty
cargo build --release
install -m 755 target/release/kou-tty ~/.local/bin/kou-tty
./install.sh install --skill-only --symlink   # link the in-repo skill into all detected agents
```

## Quick start

```sh
ID=$(kou-tty create --size 80x24 | jq -r .result.id)
kou-tty send-keys "$ID" '[{"text":"echo hello"},{"key":"Enter"}]'
sleep 0.2
kou-tty show "$ID"
kou-tty destroy "$ID"
kou-tty shutdown
```

`kou-tty --help` lists every subcommand. The full agent-facing reference lives in [`skills/driving-terminal/`](skills/driving-terminal/SKILL.md).

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
| `./install.sh bundle` | Re-embed `./skills/` into this script (used by CI) |

Supported targets:

| Target | Path |
| --- | --- |
| `opencode` | `~/.config/opencode/skills/` |
| `claude-code` | `~/.claude/skills/` |
| `codex` | `${CODEX_HOME:-~/.codex}/skills/` |
| `pi` | `~/.pi/agent/skills/` (or `$PWD/.pi/agent/skills/` with `--pi-local`) |
| `claude-desktop` | macOS: `~/Library/Application Support/Claude/skills/` · Linux: `~/.config/Claude/skills/` |
| `openclaw` | `~/.openclaw/skills/` |

Env overrides: `KOU_TTY_REPO`, `KOU_TTY_VERSION`, `KOU_TTY_INSTALL_DIR`, `KOU_TTY_SKILLS_DIR`, `CODEX_HOME`.

## CI / Release

- `.github/workflows/ci.yml` — runs on every push/PR: `cargo build --release`, `cargo test`, `cargo +nightly fmt -- --check`, shellcheck, and a Python skill-frontmatter validator.
- `.github/workflows/release.yml` — runs on every `v*` tag: builds binaries for macOS arm64/x64, Linux x64/arm64, Windows x64, regenerates `install.sh` with a fresh skill bundle, and publishes everything as a GitHub release.

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
