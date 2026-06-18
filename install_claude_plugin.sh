#!/usr/bin/env bash
# Install or update the mcp-gemini-server Claude plugin.
#
# Usage:
#   ./install_claude_plugin.sh
#
# When run from inside the already-cloned repository (dist/ present), the local
# tree is used directly and no network access is required.
# Otherwise the repository is cloned from GitHub to INSTALL_DIR and built.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd 2>/dev/null || pwd)"
INSTALL_DIR="${MCP_GEMINI_INSTALL_DIR:-${HOME}/.local/share/mcp-gemini-server}"
REPO_URL="https://github.com/siosig/mcp-gemini-server.git"
MARKETPLACE_NAME="mcp-gemini-server"
PLUGIN_NAME="mcp-gemini-server"

# ── 1. Check required commands ───────────────────────────────────────────────
_require() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: '$1' not found. Please install it and try again." >&2
    [[ -n "${2:-}" ]] && echo "       $2" >&2
    exit 1
  fi
  echo "✓ $1: $(command -v "$1")"
}

_require git    "https://git-scm.com/downloads"
_require node   "https://nodejs.org/"
_require claude "https://claude.ai/code"

# ── 2. GEMINI_API_KEY notice ──────────────────────────────────────────────────
if [[ -z "${GEMINI_API_KEY:-}" && ! -f "${HOME}/.gemini-mcp.json" ]]; then
  echo ""
  echo "NOTE: GEMINI_API_KEY is not set. Configure it with one of:"
  echo "  export GEMINI_API_KEY=<your-key>"
  echo "  echo '{\"GEMINI_API_KEY\":\"<your-key>\"}' > ~/.gemini-mcp.json"
  echo ""
fi

# ── 3. Acquire source and build ───────────────────────────────────────────────
# If running inside an already-built local repository, use it directly
if [[ -f "${SCRIPT_DIR}/package.json" && -d "${SCRIPT_DIR}/dist" ]]; then
  PLUGIN_DIR="${SCRIPT_DIR}"
  echo "✓ source: local repository (${PLUGIN_DIR})"
else
  if [[ -d "${INSTALL_DIR}/.git" ]]; then
    echo "→ updating repository: ${INSTALL_DIR}"
    git -C "${INSTALL_DIR}" pull --ff-only
  else
    echo "→ cloning repository: ${REPO_URL}"
    git clone "${REPO_URL}" "${INSTALL_DIR}"
  fi
  echo "→ installing dependencies"
  (cd "${INSTALL_DIR}" && npm install)
  echo "→ building"
  (cd "${INSTALL_DIR}" && npm run build)
  PLUGIN_DIR="${INSTALL_DIR}"
  echo "✓ build complete: ${PLUGIN_DIR}"
fi

# ── 4. Register marketplace (add if not registered) ──────────────────────────
if claude plugin marketplace list 2>/dev/null | grep -q "^  ❯ ${MARKETPLACE_NAME}"; then
  echo "✓ marketplace '${MARKETPLACE_NAME}': already registered"
else
  echo "→ registering marketplace '${MARKETPLACE_NAME}': ${PLUGIN_DIR}"
  claude plugin marketplace add "${PLUGIN_DIR}"
  echo "✓ marketplace '${MARKETPLACE_NAME}': registered"
fi

# ── 5. Remove existing installation ──────────────────────────────────────────
if claude plugin list 2>/dev/null | grep -q "❯ ${PLUGIN_NAME}@"; then
  echo "→ uninstalling existing '${PLUGIN_NAME}' plugin"
  claude plugin uninstall "${PLUGIN_NAME}" --yes
  echo "✓ '${PLUGIN_NAME}': uninstalled"
else
  echo "✓ '${PLUGIN_NAME}': not installed (skip uninstall)"
fi

# ── 6. Install plugin ─────────────────────────────────────────────────────────
echo "→ installing '${PLUGIN_NAME}@${MARKETPLACE_NAME}'"
claude plugin install "${PLUGIN_NAME}@${MARKETPLACE_NAME}"
echo "✓ '${PLUGIN_NAME}@${MARKETPLACE_NAME}': installed"

echo ""
echo "Installation complete. Restart Claude Code to activate the plugin."
