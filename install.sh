#!/usr/bin/env bash
#
# kou-tty installer
#
# Installs the kou-tty binary (downloaded from a GitHub release) and the
# driving-terminal skill (extracted from a self-embedded bundle, or from
# a local ./skills/ directory when developing).
#
# Quick install (from release):
#   curl -fsSL https://github.com/gfhfyjbr/kou-tty/releases/latest/download/install.sh | bash
#
# Local / development:
#   ./install.sh bundle           # repack ./skills/ into this script
#   ./install.sh install          # install binary + skill
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
INSTALL_DIR="${KOU_TTY_INSTALL_DIR:-$HOME/.local/bin}"
DEFAULT_SKILL="driving-terminal"
ALL_TARGETS=(opencode claude-code codex pi claude-desktop openclaw)
BUNDLE_BEGIN_MARK="__KOU_TTY_BUNDLE_BEGIN__"
BUNDLE_END_MARK="__KOU_TTY_BUNDLE_END__"

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

# ── Skill bundle ─────────────────────────────────────────────────────────────

# has_embedded_bundle → 0 if the running script contains a non-empty bundle
has_embedded_bundle() {
  [ -n "$SCRIPT_PATH" ] && [ -f "$SCRIPT_PATH" ] || return 1
  awk -v b="$BUNDLE_BEGIN_MARK" -v e="$BUNDLE_END_MARK" '
    $0==b {flag=1; next}
    $0==e {exit}
    flag && NF {found=1; exit}
    END {exit found?0:1}
  ' "$SCRIPT_PATH"
}

# extract_embedded_bundle <dest-dir>
extract_embedded_bundle() {
  local dest="$1"
  has_embedded_bundle || return 1
  mkdir -p "$dest"
  awk -v b="$BUNDLE_BEGIN_MARK" -v e="$BUNDLE_END_MARK" '
    $0==b {flag=1; next}
    $0==e {exit}
    flag {print}
  ' "$SCRIPT_PATH" | base64 -d | tar xzf - -C "$dest"
}

# download_skill_bundle <dest-dir>
download_skill_bundle() {
  local dest="$1"
  local asset="skill-bundle.tar.gz"
  local url
  url="$(release_asset_url "$asset")"
  local tmp
  tmp="$(mktemp -t kou-tty-bundle.XXXXXX.tar.gz)"
  info "downloading skill bundle from ${KOU_TTY_REPO}"
  if ! download_url "$url" "$tmp"; then
    rm -f "$tmp"
    return 1
  fi
  mkdir -p "$dest"
  tar xzf "$tmp" -C "$dest"
  rm -f "$tmp"
}

# resolve_skills_source → echoes a directory containing skill folders.
# Caller is responsible for cleanup of the global SKILLS_TMP if set.
SKILLS_TMP=""
resolve_skills_source() {
  if [ -n "${KOU_TTY_SKILLS_DIR:-}" ] && [ -d "$KOU_TTY_SKILLS_DIR" ]; then
    echo "$KOU_TTY_SKILLS_DIR"
    return
  fi
  if [ -n "$SCRIPT_DIR" ] && [ -d "$SCRIPT_DIR/skills" ] && [ -d "$SCRIPT_DIR/skills/$DEFAULT_SKILL" ]; then
    echo "$SCRIPT_DIR/skills"
    return
  fi
  if has_embedded_bundle; then
    SKILLS_TMP="$(mktemp -d -t kou-tty-skills.XXXXXX)"
    extract_embedded_bundle "$SKILLS_TMP" || die "failed to extract embedded bundle"
    echo "$SKILLS_TMP"
    return
  fi
  SKILLS_TMP="$(mktemp -d -t kou-tty-skills.XXXXXX)"
  if download_skill_bundle "$SKILLS_TMP"; then
    echo "$SKILLS_TMP"
    return
  fi
  die "could not locate skills source (no local ./skills/, no embedded bundle, no release asset)"
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
  local src="$1"; shift
  if [ $# -eq 0 ]; then
    echo "$DEFAULT_SKILL"
    return
  fi
  for s in "$@"; do
    if [ "$s" = "all" ]; then
      for dir in "$src"/*/; do
        [ -f "$dir/SKILL.md" ] && basename "$dir"
      done
      return
    fi
    [ -f "$src/$s/SKILL.md" ] || die "skill '$s' not found in $src"
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

  local src_root
  src_root="$(resolve_skills_source)"

  local resolved_skills
  resolved_skills="$(resolve_skills "$src_root" ${skills[@]+"${skills[@]}"})"
  local resolved_targets
  resolved_targets="$(resolve_targets "$target")"

  if [ -z "$resolved_targets" ]; then
    warn "no targets detected. Pass --all-targets to install everywhere known."
    return
  fi

  while IFS= read -r s; do
    [ -n "$s" ] || continue
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
  info "repo:    $KOU_TTY_REPO"
  info "version: $KOU_TTY_VERSION"
  info "install: $INSTALL_DIR"
  info "os/arch: $(detect_os)/$(detect_arch)"
  if has_embedded_bundle; then
    ok "embedded skill bundle: present"
  else
    warn "embedded skill bundle: missing"
  fi
  if [ -n "$SCRIPT_DIR" ] && [ -d "$SCRIPT_DIR/skills" ]; then
    ok "local skills/: $SCRIPT_DIR/skills"
  fi
  for t in "${ALL_TARGETS[@]}"; do
    if target_detected "$t"; then
      ok "$t: detected ($(target_path "$t"))"
    else
      printf '%s   %s%s\n' "$C_DIM" "$t: not detected" "$C_RESET" >&2
    fi
  done
}

cmd_bundle() {
  local src="${1:-${SCRIPT_DIR:-$PWD}/skills}"
  [ -d "$src" ] || die "skills source not found: $src"
  [ -n "$SCRIPT_PATH" ] && [ -f "$SCRIPT_PATH" ] || die "cannot locate self for rewrite"

  info "bundling $src into $SCRIPT_PATH"

  local bundle_b64 tmp
  bundle_b64="$(tar czf - -C "$src" . | base64 | tr -d '\n')"
  tmp="${SCRIPT_PATH}.bundling.$$"

  python3 - "$SCRIPT_PATH" "$tmp" "$BUNDLE_BEGIN_MARK" "$BUNDLE_END_MARK" "$bundle_b64" <<'PY'
import sys, re
script, tmp, begin, end, payload = sys.argv[1:6]
with open(script) as f:
    content = f.read()
chunk_size = 76
chunks = [payload[i:i+chunk_size] for i in range(0, len(payload), chunk_size)]
new_block = begin + "\n" + "\n".join(chunks) + "\n" + end
pattern = re.compile(re.escape(begin) + r"\n.*?" + re.escape(end), re.DOTALL)
if not pattern.search(content):
    raise SystemExit("bundle markers not found in script")
content = pattern.sub(new_block, content)
with open(tmp, "w") as f:
    f.write(content)
PY

  mv "$tmp" "$SCRIPT_PATH"
  chmod +x "$SCRIPT_PATH"
  ok "bundled $(wc -c <<< "$bundle_b64" | tr -d ' ') chars into $SCRIPT_PATH"
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
    install.sh bundle [skills-dir]            Re-embed local skills/ into this script
    install.sh help

  Default skill: $DEFAULT_SKILL

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
    --version <tag>     Pin a specific release (default: $KOU_TTY_VERSION)
    --install-dir <path>  Override binary install directory (default: $INSTALL_DIR)

  Uninstall flags:
    --target / --all-targets / --pi-local / --dry-run (as above)
    --remove-binary     Also delete the kou-tty binary

  Environment:
    KOU_TTY_REPO         GitHub repo, default $KOU_TTY_REPO
    KOU_TTY_VERSION      Tag, default $KOU_TTY_VERSION
    KOU_TTY_INSTALL_DIR  Binary install dir, default \$HOME/.local/bin
    KOU_TTY_SKILLS_DIR   Override skill source (skip embedded bundle / download)
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
    bundle)         cmd_bundle "$@" ;;
    help|-h|--help) usage ;;
    *)              die "unknown command: $cmd (try 'help')" ;;
  esac
}

main "$@"
exit 0

# Self-embedded skill bundle (base64 tar.gz of skills/). Regenerate with:
#   ./install.sh bundle
# shellcheck disable=SC2317,SC2034,SC2287,SC2188
__KOU_TTY_BUNDLE_BEGIN__
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
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAA=
__KOU_TTY_BUNDLE_END__
