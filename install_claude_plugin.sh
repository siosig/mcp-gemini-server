#!/usr/bin/env bash
# Install or update the mcp-gemini-server Claude plugin from GitHub.
#
# Usage:
#   ./install_claude_plugin.sh           # user scope (default)
#   ./install_claude_plugin.sh --scope project

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MARKETPLACE_JSON="${SCRIPT_DIR}/.claude-plugin/marketplace.json"

# Parse --scope argument
SCOPE="user"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --scope) SCOPE="$2"; shift 2 ;;
        *) echo "[ERROR] Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# --- Parse marketplace.json ---
if [[ ! -f "${MARKETPLACE_JSON}" ]]; then
    echo "[ERROR] .claude-plugin/marketplace.json not found" >&2
    exit 1
fi
MARKETPLACE_NAME=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(d['name'])" "${MARKETPLACE_JSON}")
PLUGIN_NAME=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(d['plugins'][0]['name'])" "${MARKETPLACE_JSON}")

# --- Derive public HTTPS URL from git remote origin ---
# Normalizes any GitHub URL (SSH, custom host alias, HTTPS) to https://github.com/<slug>
ORIGIN=$(git -C "${SCRIPT_DIR}" remote get-url origin 2>/dev/null || true)
if [[ -z "${ORIGIN}" ]]; then
    echo "[ERROR] No git remote 'origin' found" >&2
    exit 1
fi

# Extract GitHub slug from any URL form:
#   git@github.com:user/repo.git
#   git@github-personal:user/repo.git  (custom SSH host alias for github.com)
#   https://github.com/user/repo.git
if [[ "${ORIGIN}" =~ ^git@[^:]+:([^/]+/.+?)(.git)?$ ]]; then
    REPO_URL="https://github.com/${BASH_REMATCH[1]}"
elif [[ "${ORIGIN}" =~ ^https://github\.com/([^/]+/.+?)(.git)?$ ]]; then
    REPO_URL="https://github.com/${BASH_REMATCH[1]}"
else
    REPO_URL="${ORIGIN}"
fi

echo "=== Claude Plugin Installer ==="
echo "  Marketplace : ${MARKETPLACE_NAME}"
echo "  Plugin      : ${PLUGIN_NAME}"
echo "  URL         : ${REPO_URL}"
echo "  Scope       : ${SCOPE}"
echo ""

# GIT_CONFIG_GLOBAL=/dev/null prevents local git insteadOf rewrites
# (e.g. url."git@github.com:".insteadOf = https://github.com/) from
# converting the HTTPS URL back to SSH inside claude's internal git clone.
export GIT_CONFIG_GLOBAL=/dev/null

# --- Marketplace: add or update ---
MARKETPLACE_LIST=$(claude plugin marketplace list 2>/dev/null || true)
if echo "${MARKETPLACE_LIST}" | grep -qF "❯ ${MARKETPLACE_NAME}"; then
    echo "[1/2] Marketplace '${MARKETPLACE_NAME}' already registered — updating..."
    claude plugin marketplace update "${MARKETPLACE_NAME}"
    echo "      Done."
else
    echo "[1/2] Adding marketplace '${MARKETPLACE_NAME}' from ${REPO_URL}..."
    claude plugin marketplace add "${REPO_URL}" --scope "${SCOPE}"
    echo "      Done."
fi

# --- Plugin: install or update ---
PLUGIN_LIST=$(claude plugin list 2>/dev/null || true)
if echo "${PLUGIN_LIST}" | grep -qF "❯ ${PLUGIN_NAME}@${MARKETPLACE_NAME}"; then
    echo "[2/2] Plugin '${PLUGIN_NAME}' already installed — updating..."
    claude plugin update "${PLUGIN_NAME}" --scope "${SCOPE}"
    echo "      Done."
else
    echo "[2/2] Installing plugin '${PLUGIN_NAME}'..."
    claude plugin install "${PLUGIN_NAME}" --scope "${SCOPE}"
    echo "      Done."
fi

echo ""
echo "✓ Complete. Restart Claude Code to apply changes."
