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

# ── 1. 必須コマンドの確認 ────────────────────────────────────────────────────
_require() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: '$1' が見つかりません。インストールしてから再試行してください。" >&2
    [[ -n "${2:-}" ]] && echo "       $2" >&2
    exit 1
  fi
  echo "✓ $1: $(command -v "$1")"
}

_require git    "https://git-scm.com/downloads"
_require node   "https://nodejs.org/"
_require claude "https://claude.ai/code"

# ── 2. GEMINI_API_KEY の案内 ──────────────────────────────────────────────────
if [[ -z "${GEMINI_API_KEY:-}" && ! -f "${HOME}/.gemini-mcp.json" ]]; then
  echo ""
  echo "NOTE: GEMINI_API_KEY が未設定です。以下のいずれかで設定してください:"
  echo "  export GEMINI_API_KEY=<your-key>"
  echo "  echo '{\"GEMINI_API_KEY\":\"<your-key>\"}' > ~/.gemini-mcp.json"
  echo ""
fi

# ── 3. ソース取得・ビルド ─────────────────────────────────────────────────────
# ビルド済みのローカルリポジトリ内から実行されている場合はそのまま使う
if [[ -f "${SCRIPT_DIR}/package.json" && -d "${SCRIPT_DIR}/dist" ]]; then
  PLUGIN_DIR="${SCRIPT_DIR}"
  echo "✓ ソース: ローカルリポジトリ (${PLUGIN_DIR})"
else
  if [[ -d "${INSTALL_DIR}/.git" ]]; then
    echo "→ リポジトリを更新します: ${INSTALL_DIR}"
    git -C "${INSTALL_DIR}" pull --ff-only
  else
    echo "→ リポジトリをクローンします: ${REPO_URL}"
    git clone "${REPO_URL}" "${INSTALL_DIR}"
  fi
  echo "→ 依存パッケージをインストールします"
  (cd "${INSTALL_DIR}" && npm install)
  echo "→ ビルドします"
  (cd "${INSTALL_DIR}" && npm run build)
  PLUGIN_DIR="${INSTALL_DIR}"
  echo "✓ ビルド完了: ${PLUGIN_DIR}"
fi

# ── 4. マーケットプレイス登録（未登録なら追加）──────────────────────────────
if claude plugin marketplace list 2>/dev/null | grep -q "^  ❯ ${MARKETPLACE_NAME}"; then
  echo "✓ marketplace '${MARKETPLACE_NAME}': already registered"
else
  echo "→ marketplace '${MARKETPLACE_NAME}' を登録します: ${PLUGIN_DIR}"
  claude plugin marketplace add "${PLUGIN_DIR}"
  echo "✓ marketplace '${MARKETPLACE_NAME}': registered"
fi

# ── 5. 既存インストールを削除 ────────────────────────────────────────────────
if claude plugin list 2>/dev/null | grep -q "❯ ${PLUGIN_NAME}@"; then
  echo "→ 既存の '${PLUGIN_NAME}' プラグインをアンインストールします"
  claude plugin uninstall "${PLUGIN_NAME}" --yes
  echo "✓ '${PLUGIN_NAME}': uninstalled"
else
  echo "✓ '${PLUGIN_NAME}': not installed (skip uninstall)"
fi

# ── 6. プラグインをインストール ──────────────────────────────────────────────
echo "→ '${PLUGIN_NAME}@${MARKETPLACE_NAME}' をインストールします"
claude plugin install "${PLUGIN_NAME}@${MARKETPLACE_NAME}"
echo "✓ '${PLUGIN_NAME}@${MARKETPLACE_NAME}': installed"

echo ""
echo "インストール完了。Claude Code を再起動してプラグインを有効化してください。"
