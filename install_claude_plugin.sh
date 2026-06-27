#!/usr/bin/env bash
# Install (or uninstall with -d) the mcp-gemini-server Claude Code plugin (Rust edition).
#
# Distribution model:
#   - The plugin (skills / agents / hooks / MCP server config) is registered as a
#     Claude Code marketplace. By default the marketplace SOURCE is this local repo
#     checkout, so the currently-checked-out plugin is installed as-is (no push or
#     GitHub Release required). Point MARKETPLACE_SOURCE at the GitHub repo once a
#     release is published to install the remote version instead.
#   - The MCP server is a native Rust binary, installed to
#     ${HOME}/.local/share/mcp-gemini-server/mcp-gemini-server, which the plugin's
#     .mcp.json launches directly (no Node.js / bun runtime needed).
#
# Binary acquisition order (install mode):
#   1. Download a pre-built binary from GitHub Releases for this OS/arch.
#   2. Fall back to `cargo build --release` from this repo checkout.
#   3. If neither is possible, print an actionable error and exit 1.
#
# Requirements:
#   - claude  (Claude Code CLI)            — always
#   - curl + tar (Releases download) OR cargo (local build) — install mode only
#   - GEMINI_API_KEY in the env, or ~/.gemini-mcp.json
#
# Env overrides:
#   MARKETPLACE_SOURCE  local dir (default: this repo) | GitHub owner/repo | URL
#   GEMINI_INSTALL_DIR  binary install directory (default: ~/.local/share/mcp-gemini-server)
#
# Usage:
#   ./install_claude_plugin.sh              # install / re-install (default)
#   ./install_claude_plugin.sh -d           # uninstall everything this script created

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO="siosig/mcp-gemini-server"
BINARY="mcp-gemini-server"
MARKETPLACE_NAME="mcp-gemini-server"
PLUGIN_NAME="mcp-gemini-server"
PLUGIN_REF="${PLUGIN_NAME}@${MARKETPLACE_NAME}"
MARKETPLACE_SOURCE="${MARKETPLACE_SOURCE:-${SCRIPT_DIR}}"
INSTALL_DIR="${GEMINI_INSTALL_DIR:-${HOME}/.local/share/mcp-gemini-server}"
RULES_FILE="${HOME}/.claude/rules/mcp-gemini-server.md"

usage() {
  cat >&2 <<EOF
Usage: ${0##*/} [-d|--uninstall] [-h|--help]

  (no flag)        Install / re-install the '${PLUGIN_NAME}' plugin (default).
  -d, --uninstall  Remove everything this installer created:
                     • uninstall the '${PLUGIN_NAME}' plugin
                     • remove the '${MARKETPLACE_NAME}' marketplace entry
                     • delete the installed binary at ${INSTALL_DIR}
                     • delete the delegation rules at ${RULES_FILE}
  -h, --help       Show this help.
EOF
}

MODE="install"
while [[ $# -gt 0 ]]; do
  case "$1" in
    -d|--uninstall|--delete) MODE="uninstall" ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

# ── Shared: required-command check ───────────────────────────────────────────
_require() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: '$1' not found. ${2:-Please install it and try again.}" >&2
    exit 1
  fi
  echo "✓ $1: $(command -v "$1")"
}

# ── Uninstall: reverse everything the installer created ──────────────────────
# Only the `claude` CLI is needed to tear down. Every step is idempotent and
# fail-soft: a component that is already absent is reported and skipped.
_uninstall() {
  echo "→ uninstalling ${PLUGIN_NAME} (-d)"
  _require claude "Install Claude Code: https://claude.ai/code"

  # 1. Uninstall the plugin.
  if claude plugin list 2>/dev/null | grep -q "❯ ${PLUGIN_NAME}@"; then
    claude plugin uninstall "${PLUGIN_NAME}" --yes
    echo "✓ plugin '${PLUGIN_NAME}': uninstalled"
  else
    echo "✓ plugin '${PLUGIN_NAME}': not installed (skip)"
  fi

  # 2. Remove the marketplace entry.
  if claude plugin marketplace list 2>/dev/null | grep -q "${MARKETPLACE_NAME}"; then
    claude plugin marketplace remove "${MARKETPLACE_NAME}" || true
    echo "✓ marketplace '${MARKETPLACE_NAME}': removed"
  else
    echo "✓ marketplace '${MARKETPLACE_NAME}': not registered (skip)"
  fi

  # 3. Remove the installed binary — only the installer-created directory under
  #    ~/.local/share, never the local working repository.
  if [[ -e "${INSTALL_DIR}/${BINARY}" && "${INSTALL_DIR}" != "${SCRIPT_DIR}" ]]; then
    rm -rf "${INSTALL_DIR}"
    echo "✓ binary ${INSTALL_DIR}: removed"
  else
    echo "✓ binary ${INSTALL_DIR}: not present (skip)"
  fi

  # 4. Remove the delegation rules file.
  if [[ -f "${RULES_FILE}" ]]; then
    rm -f "${RULES_FILE}"
    echo "✓ rules ${RULES_FILE}: removed"
  else
    echo "✓ rules ${RULES_FILE}: not present (skip)"
  fi

  echo ""
  echo "Uninstall complete. Restart Claude Code to drop the plugin from the session."
}

# Tear-down path: reverse the install and exit before any install-only checks.
if [[ "${MODE}" == "uninstall" ]]; then
  _uninstall
  exit 0
fi

# ── 1. Required commands ─────────────────────────────────────────────────────
_require claude "Install Claude Code: https://claude.ai/code"

# ── 2. GEMINI_API_KEY notice ─────────────────────────────────────────────────
if [[ -z "${GEMINI_API_KEY:-}" && ! -f "${HOME}/.gemini-mcp.json" ]]; then
  echo ""
  echo "NOTE: GEMINI_API_KEY is not set. Configure it with one of:"
  echo "  export GEMINI_API_KEY=<your-key>"
  echo "  echo '{\"GEMINI_API_KEY\":\"<your-key>\"}' > ~/.gemini-mcp.json"
  echo ""
fi

# ── 3. Acquire the Rust binary ───────────────────────────────────────────────
case "$(uname -s)" in
  Linux*)  OS="linux" ;;
  Darwin*) OS="darwin" ;;
  *) OS="" ;;
esac
# Arch normalization: macOS returns 'arm64', Linux 'aarch64' → normalize to aarch64.
case "$(uname -m)" in
  x86_64|amd64)  ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) ARCH="" ;;
esac

download_release() {
  [[ -n "$OS" && -n "$ARCH" ]] || return 1
  command -v curl &>/dev/null || return 1
  command -v tar  &>/dev/null || return 1

  local asset="${BINARY}-${OS}-${ARCH}.tar.gz"
  local latest
  # Resolve the latest tag without jq (rate limit 60/hr is safe for an installer).
  latest="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
    | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')" || return 1
  [[ -n "$latest" ]] || return 1

  local url="https://github.com/${REPO}/releases/download/${latest}/${asset}"
  local tmp
  tmp="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '$tmp'" RETURN

  echo "→ downloading ${asset} (${latest})"
  curl -fsSL --retry 3 "$url" -o "${tmp}/${asset}" || return 1
  tar xzf "${tmp}/${asset}" -C "$tmp" || return 1
  mkdir -p "$INSTALL_DIR"
  install -m 755 "${tmp}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
}

build_from_source() {
  [[ -f "${SCRIPT_DIR}/Cargo.toml" ]] || return 1
  # rustup installs cargo to ~/.cargo/bin but does not always export it onto PATH
  # (e.g. non-login shells that never source ~/.cargo/env). Locate it ourselves.
  if ! command -v cargo &>/dev/null; then
    if [[ -r "${CARGO_HOME:-${HOME}/.cargo}/env" ]]; then
      # shellcheck disable=SC1091
      . "${CARGO_HOME:-${HOME}/.cargo}/env"
    fi
    case ":${PATH}:" in
      *":${HOME}/.cargo/bin:"*) ;;
      *) PATH="${HOME}/.cargo/bin:${PATH}" ;;
    esac
  fi
  command -v cargo &>/dev/null || return 1
  echo "✓ cargo: $(command -v cargo)"
  echo "→ building ${BINARY} from source (cargo build --release)"
  ( cd "$SCRIPT_DIR" && cargo build --release ) || return 1
  mkdir -p "$INSTALL_DIR"
  install -m 755 "${SCRIPT_DIR}/target/release/${BINARY}" "${INSTALL_DIR}/${BINARY}"
}

echo "→ acquiring the ${BINARY} binary"
if download_release; then
  echo "✓ installed from GitHub Releases → ${INSTALL_DIR}/${BINARY}"
elif build_from_source; then
  echo "✓ built from source → ${INSTALL_DIR}/${BINARY}"
else
  echo "ERROR: could not obtain ${BINARY}." >&2
  echo "  No matching GitHub Release asset, and no cargo toolchain + Cargo.toml here." >&2
  echo "  Install Rust and re-run: https://rustup.rs" >&2
  exit 1
fi
"${INSTALL_DIR}/${BINARY}" --version 2>/dev/null || true

# ── 4. Register the marketplace (idempotent) ─────────────────────────────────
if claude plugin marketplace list 2>/dev/null | grep -q "${MARKETPLACE_NAME}"; then
  echo "→ refreshing marketplace '${MARKETPLACE_NAME}'"
  claude plugin marketplace remove "${MARKETPLACE_NAME}" || true
fi
echo "→ adding marketplace: ${MARKETPLACE_SOURCE}"
claude plugin marketplace add "${MARKETPLACE_SOURCE}"

# ── 5. Remove any existing install, then install ─────────────────────────────
if claude plugin list 2>/dev/null | grep -q "❯ ${PLUGIN_NAME}@"; then
  echo "→ uninstalling existing '${PLUGIN_NAME}'"
  claude plugin uninstall "${PLUGIN_NAME}" --yes
fi
echo "→ installing ${PLUGIN_REF}"
claude plugin install "${PLUGIN_REF}"

# ── 6. Install the delegation rules file ─────────────────────────────────────
# The delegation policy is delivered as an always-loaded rule (not a per-prompt
# UserPromptSubmit hook), so nothing is injected into Claude's input stream.
mkdir -p "$(dirname "${RULES_FILE}")"
cat > "${RULES_FILE}" <<'RULES_EOF'
# mcp-gemini-server — delegation policy

Guidance for using the `gemini-delegate` subagent and `gemini-team` skill shipped
by the mcp-gemini-server plugin. Delivered as an always-loaded rule (the plugin no
longer registers a UserPromptSubmit hook, to keep the prompt input stream clean).

## DEFAULT-ON: delegate read-only work to Gemini

For self-contained, read-only work over existing local code/docs — comprehension,
summarization, investigation, review — whose full context fits in one prompt,
delegate to the `gemini-delegate` subagent by default and consume only the
distilled conclusion.

## Do NOT delegate (exclusions)

1. Final decisions, file edits, Git, and orchestration stay with the main Claude.
2. Latest library/SDK/API specs — do not let Gemini adjudicate them; verify via
   context7 / Claude.
3. Tasks needing fine-grained sequential control (debugging isolation, staged
   refactors).

Always verify delegated results before using them.

## Multi-agent work

For multi-perspective or iterative-refinement tasks, use the `gemini-team` skill
(`mul` / `it` / `mulit` modes) instead of a single delegation.
RULES_EOF
echo "✓ rules ${RULES_FILE}: created"

# ── 7. Verify ────────────────────────────────────────────────────────────────
echo "→ verifying"
claude plugin details "${PLUGIN_REF}" || true

cat <<EOF

------------------------------------------------------------------
Installation complete. Restart Claude Code to activate the plugin.

  marketplace : ${MARKETPLACE_SOURCE}
  binary      : ${INSTALL_DIR}/${BINARY}
  rules       : ${RULES_FILE}

Verify the launch with:  claude mcp list   (expect: gemini … ✔ Connected)
Uninstall with:          ${0##*/} -d
------------------------------------------------------------------
EOF
