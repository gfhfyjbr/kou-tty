#!/usr/bin/env bash
#
# kou-tty installer
#
# Installs the kou-tty binary (downloaded from a GitHub release) and the
# driving-terminal skill (fetched from raw.githubusercontent.com, or from
# a local ./skills/ directory when developing).
#
# Quick install:
#   curl -fsSL https://raw.githubusercontent.com/gfhfyjbr/kou-tty/main/install.sh | bash
#
# Pinned to a tag:
#   curl -fsSL https://raw.githubusercontent.com/gfhfyjbr/kou-tty/v0.1.0/install.sh | bash
#
# Local / development:
#   ./install.sh install                         # install binary + skill
#   ./install.sh install --no-binary --symlink   # only the skill, symlinked
#
# Targets (auto-detected):
#   opencode       ~/.config/opencode/skills/
#   claude-code    ~/.claude/skills/
#   codex          ${CODEX_HOME:-~/.codex}/skills/
#   pi             ~/.pi/agent/skills/         (or $PWD/.pi/agent/skills/ with --pi-local)
#   claude-desktop macOS/Linux Claude Desktop skills dir
#   openclaw       ~/.openclaw/skills/
#
set -euo pipefail

# ── Configuration ───────────────────────────────────────────────────────────

KOU_TTY_REPO="${KOU_TTY_REPO:-gfhfyjbr/kou-tty}"
KOU_TTY_VERSION="${KOU_TTY_VERSION:-latest}"
KOU_TTY_SKILL_REF="${KOU_TTY_SKILL_REF:-}"
INSTALL_DIR="${KOU_TTY_INSTALL_DIR:-$HOME/.local/bin}"
DEFAULT_SKILL="driving-terminal"
ALL_SKILLS=(driving-terminal)
ALL_TARGETS=(opencode claude-code codex pi claude-desktop openclaw)

# Files for each registered skill. Update when adding new skill files.
# Returns the file list for the given skill on stdout, one path per line.
skill_files() {
  case "$1" in
    driving-terminal)
      cat <<'EOF'
SKILL.md
references/commands.md
references/json-protocol.md
references/tui-recipes.md
references/viewer.md
references/troubleshooting.md
EOF
      ;;
    *)
      die "no file list registered for skill '$1'"
      ;;
  esac
}

SCRIPT_PATH="${BASH_SOURCE[0]:-$0}"
if [ "$SCRIPT_PATH" = "bash" ] || [ "$SCRIPT_PATH" = "sh" ] || [ -z "$SCRIPT_PATH" ]; then
  SCRIPT_PATH=""
fi
if [ -n "$SCRIPT_PATH" ] && [ -f "$SCRIPT_PATH" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
else
  SCRIPT_DIR=""
fi

# ── Output helpers ───────────────────────────────────────────────────────────

if [ -t 2 ]; then
  C_INFO=$'\033[1;34m'
  C_OK=$'\033[1;32m'
  C_WARN=$'\033[1;33m'
  C_ERR=$'\033[1;31m'
  C_DIM=$'\033[1;30m'
  C_RESET=$'\033[0m'
else
  C_INFO=''; C_OK=''; C_WARN=''; C_ERR=''; C_DIM=''; C_RESET=''
fi

info() { printf '%s=>%s %s\n' "$C_INFO" "$C_RESET" "$*" >&2; }
ok()   { printf '%s=>%s %s\n' "$C_OK"   "$C_RESET" "$*" >&2; }
warn() { printf '%s=>%s %s\n' "$C_WARN" "$C_RESET" "$*" >&2; }
die()  { printf '%serror:%s %s\n' "$C_ERR" "$C_RESET" "$*" >&2; exit 1; }

# ── Platform detection ───────────────────────────────────────────────────────

detect_os() {
  case "$(uname -s)" in
    Darwin)               echo "macos" ;;
    Linux)                echo "linux" ;;
    MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
    *) die "unsupported OS: $(uname -s)" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    arm64|aarch64)        echo "arm64" ;;
    x86_64|amd64)         echo "x64" ;;
    *) die "unsupported architecture: $(uname -m)" ;;
  esac
}

bin_filename() {
  local os
  os="$(detect_os)"
  if [ "$os" = "windows" ]; then
    echo "kou-tty.exe"
  else
    echo "kou-tty"
  fi
}

bin_asset_name() {
  local os arch
  os="$(detect_os)"
  arch="$(detect_arch)"
  if [ "$os" = "windows" ]; then
    echo "kou-tty-${os}-${arch}.exe"
  else
    echo "kou-tty-${os}-${arch}"
  fi
}

bin_install_path() {
  echo "$INSTALL_DIR/$(bin_filename)"
}

# ── Binary download ──────────────────────────────────────────────────────────

release_asset_url() {
  local asset="$1"
  if [ "$KOU_TTY_VERSION" = "latest" ]; then
    echo "https://github.com/${KOU_TTY_REPO}/releases/latest/download/${asset}"
  else
    echo "https://github.com/${KOU_TTY_REPO}/releases/download/${KOU_TTY_VERSION}/${asset}"
  fi
}

download_url() {
  local url="$1" dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fSL --progress-bar -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q --show-progress -O "$dest" "$url"
  else
    die "neither curl nor wget found"
  fi
}

install_binary() {
  local asset dest tmp
  asset="$(bin_asset_name)"
  dest="$(bin_install_path)"

  if [ "${DRY_RUN:-0}" = "1" ]; then
    info "[dry-run] would download $(release_asset_url "$asset") → $dest"
    return
  fi

  mkdir -p "$INSTALL_DIR"
  tmp="${dest}.dl"
  info "downloading $(bin_asset_name) (${KOU_TTY_VERSION}) from ${KOU_TTY_REPO}"
  if ! download_url "$(release_asset_url "$asset")" "$tmp"; then
    rm -f "$tmp"
    die "failed to download $asset"
  fi
  mv "$tmp" "$dest"
  chmod +x "$dest"
  ok "installed binary: $dest"

  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) warn "$INSTALL_DIR is not in PATH — add it to your shell rc:"
       warn "    export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
  esac
}

uninstall_binary() {
  local dest
  dest="$(bin_install_path)"
  if [ ! -f "$dest" ]; then
    return
  fi
  if [ "${DRY_RUN:-0}" = "1" ]; then
    info "[dry-run] would remove $dest"
    return
  fi
  rm -f "$dest"
  ok "removed binary: $dest"
}

# ── Skill source resolution ─────────────────────────────────────────────────

# skill_ref → echoes the git ref used for raw.githubusercontent fetches
skill_ref() {
  if [ -n "$KOU_TTY_SKILL_REF" ]; then
    echo "$KOU_TTY_SKILL_REF"
  elif [ "$KOU_TTY_VERSION" = "latest" ]; then
    echo "main"
  else
    echo "$KOU_TTY_VERSION"
  fi
}

# fetch_skill_to_tmp <skill> → echoes a tmp dir containing <skill>/...
fetch_skill_to_tmp() {
  local skill="$1"
  local ref tmp base
  ref="$(skill_ref)"
  base="https://raw.githubusercontent.com/${KOU_TTY_REPO}/${ref}/skills/${skill}"
  tmp="$(mktemp -d -t "kou-tty-skill.XXXXXX")"
  SKILLS_TMP="$tmp"
  mkdir -p "$tmp/$skill"
  info "fetching skill '$skill' from ${KOU_TTY_REPO}@${ref}"
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    local dest_file="$tmp/$skill/$f"
    mkdir -p "$(dirname "$dest_file")"
    if ! download_url "$base/$f" "$dest_file"; then
      rm -rf "$tmp"
      die "failed to fetch skills/$skill/$f from ${KOU_TTY_REPO}@${ref}"
    fi
  done < <(skill_files "$skill")
  echo "$tmp"
}

# resolve_skills_source <skill> → echoes a directory containing the skill
# folder. Caller can reuse the path as $src/$skill/.
SKILLS_TMP=""
resolve_skills_source() {
  local skill="$1"
  if [ -n "${KOU_TTY_SKILLS_DIR:-}" ] && [ -d "$KOU_TTY_SKILLS_DIR/$skill" ]; then
    echo "$KOU_TTY_SKILLS_DIR"
    return
  fi
  if [ -n "$SCRIPT_DIR" ] && [ -d "$SCRIPT_DIR/skills/$skill" ]; then
    echo "$SCRIPT_DIR/skills"
    return
  fi
  fetch_skill_to_tmp "$skill"
}

# shellcheck disable=SC2329
cleanup_skills_tmp() {
  if [ -n "${SKILLS_TMP:-}" ] && [ -d "$SKILLS_TMP" ]; then
    rm -rf "$SKILLS_TMP"
  fi
}
trap cleanup_skills_tmp EXIT

# ── Target path resolution ───────────────────────────────────────────────────

target_path() {
  case "$1" in
    opencode)       echo "$HOME/.config/opencode/skills" ;;
    claude-code)    echo "$HOME/.claude/skills" ;;
    codex)          echo "${CODEX_HOME:-$HOME/.codex}/skills" ;;
    pi)
      if [ "${PI_SCOPE:-global}" = "local" ]; then
        echo "$PWD/.pi/agent/skills"
      else
        echo "$HOME/.pi/agent/skills"
      fi
      ;;
    claude-desktop)
      case "$(uname -s)" in
        Darwin) echo "$HOME/Library/Application Support/Claude/skills" ;;
        Linux)  echo "$HOME/.config/Claude/skills" ;;
        *)      echo "" ;;
      esac
      ;;
    openclaw)       echo "$HOME/.openclaw/skills" ;;
    *)              echo "" ;;
  esac
}

target_detected() {
  local p
  p="$(target_path "$1")"
  [ -n "$p" ] || return 1
  local parent
  parent="$(dirname "$p")"
  [ -d "$parent" ] || [ -d "$p" ]
}

resolve_targets() {
  case "$1" in
    all)
      for t in "${ALL_TARGETS[@]}"; do
        if target_detected "$t"; then
          echo "$t"
        fi
      done
      ;;
    all-known)
      for t in "${ALL_TARGETS[@]}"; do
        echo "$t"
      done
      ;;
    *)
      local found=0
      for t in "${ALL_TARGETS[@]}"; do
        if [ "$t" = "$1" ]; then
          found=1
          break
        fi
      done
      [ $found -eq 1 ] || die "unknown target: $1. valid: ${ALL_TARGETS[*]}"
      echo "$1"
      ;;
  esac
  return 0
}

# ── Skill installation primitives ───────────────────────────────────────────

resolve_skills() {
  if [ $# -eq 0 ]; then
    echo "$DEFAULT_SKILL"
    return
  fi
  for s in "$@"; do
    if [ "$s" = "all" ]; then
      printf '%s\n' "${ALL_SKILLS[@]}"
      return
    fi
    local found=0
    for k in "${ALL_SKILLS[@]}"; do
      [ "$k" = "$s" ] && found=1 && break
    done
    [ $found -eq 1 ] || die "unknown skill '$s'. valid: ${ALL_SKILLS[*]}"
    echo "$s"
  done
}

install_skill_one() {
  local src_root="$1" skill="$2" target="$3"
  local src="$src_root/$skill"
  local dest_root
  dest_root="$(target_path "$target")"
  [ -n "$dest_root" ] || { warn "$target: no path on this platform — skipping"; return; }

  local dest="$dest_root/$skill"

  if [ "${DRY_RUN:-0}" = "1" ]; then
    info "[dry-run] would install $skill → $dest ($METHOD)"
    return
  fi

  mkdir -p "$dest_root"

  if [ -e "$dest" ] || [ -L "$dest" ]; then
    if [ -L "$dest" ] && [ "$(readlink "$dest")" = "$src" ]; then
      ok "$target: $skill already linked"
      return
    fi
    local bak="$dest_root/.kou-tty-bak-$(date +%Y%m%d%H%M%S)"
    mkdir -p "$bak"
    mv "$dest" "$bak/$skill"
    warn "$target: backed up existing $skill → $bak/$skill"
  fi

  case "$METHOD" in
    symlink)
      ln -s "$src" "$dest"
      ok "$target: linked $skill → $dest"
      ;;
    copy|*)
      cp -R "$src" "$dest"
      ok "$target: copied $skill → $dest"
      ;;
  esac
}

uninstall_skill_one() {
  local skill="$1" target="$2"
  local dest_root
  dest_root="$(target_path "$target")"
  [ -n "$dest_root" ] || return

  local dest="$dest_root/$skill"
  if [ ! -e "$dest" ] && [ ! -L "$dest" ]; then
    return
  fi

  if [ "${DRY_RUN:-0}" = "1" ]; then
    info "[dry-run] would remove $dest"
    return
  fi

  rm -rf "$dest"
  ok "$target: removed $skill"
}

# ── Commands ─────────────────────────────────────────────────────────────────

cmd_install() {
  local target="all" skills=() do_binary=1 do_skill=1
  while [ $# -gt 0 ]; do
    case "$1" in
      --target)        target="$2"; shift 2 ;;
      --all-targets)   target="all-known"; shift ;;
      --method)        METHOD="$2"; shift 2 ;;
      --copy)          METHOD="copy"; shift ;;
      --symlink)       METHOD="symlink"; shift ;;
      --pi-local)      PI_SCOPE="local"; shift ;;
      --dry-run)       DRY_RUN=1; shift ;;
      --no-binary)     do_binary=0; shift ;;
      --skill-only)    do_binary=0; shift ;;
      --binary-only)   do_skill=0; shift ;;
      --version)       KOU_TTY_VERSION="$2"; shift 2 ;;
      --install-dir)   INSTALL_DIR="$2"; shift 2 ;;
      -h|--help)       usage; exit 0 ;;
      --)              shift; break ;;
      -*)              die "unknown flag: $1" ;;
      *)               skills+=("$1"); shift ;;
    esac
  done

  if [ "$do_binary" = "1" ]; then
    install_binary
  fi

  if [ "$do_skill" = "0" ]; then
    return
  fi

  local resolved_skills
  resolved_skills="$(resolve_skills ${skills[@]+"${skills[@]}"})"
  local resolved_targets
  resolved_targets="$(resolve_targets "$target")"

  if [ -z "$resolved_targets" ]; then
    warn "no targets detected. Pass --all-targets to install everywhere known."
    return
  fi

  while IFS= read -r s; do
    [ -n "$s" ] || continue
    local src_root
    src_root="$(resolve_skills_source "$s")"
    while IFS= read -r t; do
      [ -n "$t" ] || continue
      install_skill_one "$src_root" "$s" "$t"
    done <<< "$resolved_targets"
  done <<< "$resolved_skills"
}

cmd_uninstall() {
  local target="all" skills=() remove_binary=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --target)        target="$2"; shift 2 ;;
      --all-targets)   target="all-known"; shift ;;
      --pi-local)      PI_SCOPE="local"; shift ;;
      --dry-run)       DRY_RUN=1; shift ;;
      --remove-binary) remove_binary=1; shift ;;
      -h|--help)       usage; exit 0 ;;
      --)              shift; break ;;
      -*)              die "unknown flag: $1" ;;
      *)               skills+=("$1"); shift ;;
    esac
  done

  local src_root=""
  if [ ${#skills[@]} -eq 0 ]; then
    skills=("$DEFAULT_SKILL")
  fi
  local resolved_targets
  resolved_targets="$(resolve_targets "$target")"

  for s in "${skills[@]}"; do
    while IFS= read -r t; do
      [ -n "$t" ] || continue
      uninstall_skill_one "$s" "$t"
    done <<< "$resolved_targets"
  done

  if [ "$remove_binary" = "1" ]; then
    uninstall_binary
  fi
}

cmd_list() {
  printf '\n%-16s %-12s %s\n' "TARGET" "STATUS" "PATH"
  printf '%-16s %-12s %s\n' "------" "------" "----"
  for t in "${ALL_TARGETS[@]}"; do
    local p
    p="$(target_path "$t")"
    local status="not detected"
    if target_detected "$t"; then
      if [ -d "$p/$DEFAULT_SKILL" ]; then
        if [ -L "$p/$DEFAULT_SKILL" ]; then
          status="linked"
        else
          status="copied"
        fi
      else
        status="empty"
      fi
    fi
    printf '%-16s %-12s %s\n' "$t" "$status" "${p:-(unsupported)}"
  done
  echo
  local bp
  bp="$(bin_install_path)"
  if [ -f "$bp" ]; then
    printf 'binary: %s (installed)\n' "$bp"
  else
    printf 'binary: %s (missing)\n' "$bp"
  fi
  echo
}

cmd_doctor() {
  info "kou-tty installer doctor"
  info "repo:      $KOU_TTY_REPO"
  info "version:   $KOU_TTY_VERSION"
  info "skill ref: $(skill_ref)"
  info "install:   $INSTALL_DIR"
  info "os/arch:   $(detect_os)/$(detect_arch)"
  if [ -n "$SCRIPT_DIR" ] && [ -d "$SCRIPT_DIR/skills" ]; then
    ok "local skills/: $SCRIPT_DIR/skills"
  else
    info "local skills/: missing (will fetch from raw.githubusercontent.com)"
  fi
  for t in "${ALL_TARGETS[@]}"; do
    if target_detected "$t"; then
      ok "$t: detected ($(target_path "$t"))"
    else
      printf '%s   %s%s\n' "$C_DIM" "$t: not detected" "$C_RESET" >&2
    fi
  done
}

cmd_menu() {
  while true; do
    printf '\n  %skou-tty%s installer (%s)\n\n' "$C_INFO" "$C_RESET" "$KOU_TTY_REPO"
    printf '  Detected targets:\n'
    for t in "${ALL_TARGETS[@]}"; do
      if target_detected "$t"; then
        printf '    %s●%s %s\n' "$C_OK" "$C_RESET" "$t"
      else
        printf '    %s○%s %s\n' "$C_DIM" "$C_RESET" "$t"
      fi
    done
    printf '\n  Actions:\n'
    printf '    1  Install binary + %s into all detected targets (copy)\n' "$DEFAULT_SKILL"
    printf '    2  Install binary + %s (symlink skill)\n' "$DEFAULT_SKILL"
    printf '    3  Skill only\n'
    printf '    4  Binary only\n'
    printf '    5  Uninstall skill from all targets\n'
    printf '    6  Show status (list)\n'
    printf '    7  Doctor\n'
    printf '    q  Quit\n\n'
    printf '  > '
    local answer
    read -r answer
    case "$answer" in
      1) METHOD=copy cmd_install ;;
      2) METHOD=symlink cmd_install ;;
      3) cmd_install --no-binary ;;
      4) cmd_install --binary-only ;;
      5) cmd_uninstall ;;
      6) cmd_list ;;
      7) cmd_doctor ;;
      q|Q|"") return 0 ;;
      *) warn "unknown choice: $answer" ;;
    esac
  done
}

usage() {
  cat >&2 <<EOF

  kou-tty installer

  Usage:
    install.sh                                Interactive menu
    install.sh install   [skill...] [flags]   Install binary + skill(s)
    install.sh uninstall [skill...] [flags]
    install.sh list                           Show what is installed where
    install.sh doctor                         Diagnose detection
    install.sh help

  Default skill: $DEFAULT_SKILL (registered: ${ALL_SKILLS[*]})

  Install flags:
    --target <name>     Single target: ${ALL_TARGETS[*]} | all
    --all-targets       Install into every known target (even if not detected)
    --method copy|symlink
    --copy / --symlink  Shortcut for --method
    --pi-local          For target pi: install into \$PWD/.pi/agent/skills/
    --dry-run           Show what would be done, change nothing
    --no-binary         Skip binary download (skill only)
    --skill-only        Same as --no-binary
    --binary-only       Skip skill installation
    --version <tag>     Pin a specific binary release (default: $KOU_TTY_VERSION)
    --install-dir <path>  Override binary install directory (default: $INSTALL_DIR)

  Uninstall flags:
    --target / --all-targets / --pi-local / --dry-run (as above)
    --remove-binary     Also delete the kou-tty binary

  Environment:
    KOU_TTY_REPO         GitHub repo, default $KOU_TTY_REPO
    KOU_TTY_VERSION      Binary release tag, default $KOU_TTY_VERSION
    KOU_TTY_SKILL_REF    Git ref for skill files (default: matches version, or 'main')
    KOU_TTY_INSTALL_DIR  Binary install dir, default \$HOME/.local/bin
    KOU_TTY_SKILLS_DIR   Override skill source (skip raw.githubusercontent fetch)
    CODEX_HOME           Used by the codex target
EOF
}

# ── Main ─────────────────────────────────────────────────────────────────────

METHOD="${METHOD:-copy}"
DRY_RUN="${DRY_RUN:-0}"
PI_SCOPE="${PI_SCOPE:-global}"

main() {
  if [ $# -eq 0 ]; then
    if [ -t 0 ] && [ -t 1 ]; then
      cmd_menu
    else
      cmd_install
    fi
    return
  fi
  local cmd="$1"; shift
  case "$cmd" in
    install)        cmd_install "$@" ;;
    uninstall)      cmd_uninstall "$@" ;;
    list)           cmd_list ;;
    doctor)         cmd_doctor ;;
    help|-h|--help) usage ;;
    *)              die "unknown command: $cmd (try 'help')" ;;
  esac
}

main "$@"
