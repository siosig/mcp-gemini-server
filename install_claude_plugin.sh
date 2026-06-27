#!/usr/bin/env bash
# Install the mcp-gemini-server Claude plugin from its GitHub marketplace.
#
# Distribution model:
#   - The marketplace source is the GitHub repo (siosig/mcp-gemini-server).
#   - `claude plugin install` clones the repo into Claude's plugin cache and
#     reads the committed plugin (skills / agents / hooks / MCP server).
#   - The MCP server is launched by the committed, self-contained bundle at
#     plugins/mcp-gemini-server/dist/index.js via `bun ${CLAUDE_PLUGIN_ROOT}/...`
#     (see that plugin's .mcp.json). bun runs it in a single ~75MB process — no
#     `npx` (npx forked a 2nd Node process and pushed total RSS to ~219MB).
#
# Requirements on the target machine:
#   - claude  (Claude Code CLI)
#   - bun     (on PATH; runs the MCP server)
#   - GEMINI_API_KEY in the env, or ~/.gemini-mcp.json
#
# Maintainers: after changing src/, regenerate and commit the bundle BEFORE the
# GitHub install can pick it up:
#   pnpm build:plugin && git add plugins/mcp-gemini-server/dist && git commit && git push
#
# Env overrides:
#   PLUGIN_SCOPE        user | project | local        (default: user)
#   MARKETPLACE_SOURCE  GitHub owner/repo, URL, or a   (default: siosig/mcp-gemini-server)
#                       local directory path for dev
#
# Usage:
#   ./install_claude_plugin.sh

set -euo pipefail

MARKETPLACE_NAME="mcp-gemini-server"
PLUGIN_NAME="mcp-gemini-server"
PLUGIN_REF="${PLUGIN_NAME}@${MARKETPLACE_NAME}"
SCOPE="${PLUGIN_SCOPE:-user}"
MARKETPLACE_SOURCE="${MARKETPLACE_SOURCE:-siosig/mcp-gemini-server}"

# ── 1. Required commands ─────────────────────────────────────────────────────
_require() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: '$1' not found. $2" >&2
    exit 1
  fi
  echo "✓ $1: $(command -v "$1")"
}

_require claude "Install Claude Code: https://claude.ai/code"
_require bun    "Install bun (runs the MCP server): https://bun.sh  or  npm i -g bun"

# ── 2. GEMINI_API_KEY notice ─────────────────────────────────────────────────
if [[ -z "${GEMINI_API_KEY:-}" && ! -f "${HOME}/.gemini-mcp.json" ]]; then
  echo ""
  echo "NOTE: GEMINI_API_KEY is not set. Configure it with one of:"
  echo "  export GEMINI_API_KEY=<your-key>"
  echo "  echo '{\"GEMINI_API_KEY\":\"<your-key>\"}' > ~/.gemini-mcp.json"
  echo ""
fi

# ── 3. Clean any prior install/marketplace (idempotent) ──────────────────────
# Uninstall the plugin BEFORE removing its marketplace (a marketplace cannot be
# removed while a plugin from it is installed). A clean remove+add forces Claude
# to re-clone the latest committed bundle (a same-version reinstall otherwise
# keeps the stale cache).
if claude plugin list 2>/dev/null | grep -q "${PLUGIN_REF}"; then
  echo "→ uninstalling existing ${PLUGIN_REF}"
  claude plugin uninstall "${PLUGIN_REF}" --scope "${SCOPE}"
fi
if claude plugin marketplace list 2>/dev/null | grep -q "^[[:space:]]*❯[[:space:]]*${MARKETPLACE_NAME}\b"; then
  echo "→ removing existing marketplace '${MARKETPLACE_NAME}'"
  claude plugin marketplace remove "${MARKETPLACE_NAME}"
fi

# ── 4. Add the GitHub marketplace and install ────────────────────────────────
echo "→ adding marketplace: ${MARKETPLACE_SOURCE}"
claude plugin marketplace add "${MARKETPLACE_SOURCE}"

echo "→ installing ${PLUGIN_REF} (scope=${SCOPE})"
claude plugin install "${PLUGIN_REF}" --scope "${SCOPE}"

# ── 5. Verify ────────────────────────────────────────────────────────────────
echo "→ verifying"
claude plugin details "${PLUGIN_REF}" || true

cat <<EOF

------------------------------------------------------------------
Installation complete. Restart Claude Code to activate the plugin.

  marketplace : ${MARKETPLACE_SOURCE}
  runtime     : bun (from PATH) → \${CLAUDE_PLUGIN_ROOT}/dist/index.js
  scope       : ${SCOPE}

Verify the launch with:  claude mcp list   (expect: gemini … bun … ✔ Connected)
------------------------------------------------------------------
EOF
