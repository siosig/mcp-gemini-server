# gemini-mcp-server

Google Gemini API を、合成可能な少数のプリミティブとして公開する薄い **stdio** [Model Context Protocol (MCP)](https://modelcontextprotocol.io) サーバーです。MCP クライアント（Claude Desktop、Cursor、Cline など MCP 対応クライアント）が stdio で直接起動して利用することを想定しています。

TypeScript 6 / Node.js 24（ESM）で実装し、公式 [`@google/genai`](https://www.npmjs.com/package/@google/genai) SDK と [`@modelcontextprotocol/sdk`](https://www.npmjs.com/package/@modelcontextprotocol/sdk) の上に構築しています。

> English version: [README.md](README.md)

## 特徴

- **7 つの合成可能な Gemini プリミティブ** — チャット、Web 検索、ロールベースエージェント、マルチモーダル解析、画像生成、コード実行、ファイル管理（[ツール](#ツール)参照）。
- **API 直結・CLI 非依存** — `@google/genai` 経由で Gemini API を直接呼び出します。外部 `gemini` CLI に依存しないため、認証/クォータの共有問題がなく、コンテナ内でもクリーンに動作します。
- **統一された `thinking_level`** — `minimal | low | medium | high` の 1 つのつまみが、モデル系列に応じて正しいフィールドへ自動マッピングされます（Gemini 3.x の `thinkingLevel` / Gemini 2.5 の `thinkingBudget`）。
- **`service_tier` によるコスト制御** — `flex`（約 50% 安価・レイテンシ許容）/ `priority` / `standard` を呼び出し単位または環境変数で選択できます。
- **ツール別に最適化されたデフォルト** — 各ツールは用途に合わせたデフォルトモデルと thinking level を持ちます。必要なときだけ上書きしてください。
- **薄い設計** — サーバーはプリミティブのみを提供します。マルチエージェントのオーケストレーションはクライアント側に委ねており（同梱の [`skills/gemini-team`](skills/gemini-team) スキル参照）、MCP の関心分離に沿っています。
- **運用上クリーン** — 厳格な Zod スキーマ、構造化ツール出力（`structuredContent`）、リトライ/タイムアウト処理、stderr のみへのログ出力、ハードニングした Node ランタイムフラグ。外部ネットワークや監視スタックを前提としません。

## ツール

| ツール | 説明 | 主な入力 |
|--------|------|----------|
| `gemini_chat` | Gemini とチャット（thinking level・grounding・JSON モード） | `prompt`（必須） |
| `gemini_search` | Gemini Grounding 経由の Google Web 検索 | `query`（必須） |
| `gemini_custom_agent` | 指定ロールでタスクを実行 | `task`, `role`（必須） |
| `gemini_analyze_media` | 画像・PDF・動画・音声を解析 | `prompt` + `file_path` / `file_uri` / `image_url` / `image_base64` のいずれか |
| `gemini_generate_image` | Gemini Flash Image（Nano Banana 2）で PNG を生成（Google SynthID 透かし付き） | `prompt`（必須） |
| `gemini_execute_code` | Gemini サンドボックスで Python 実行（numpy/pandas/matplotlib） | `prompt`（必須） |
| `gemini_manage_files` | Gemini Files API 管理（upload/list/status/delete） | `action`（必須） |

多くのツールは `model` / `thinking_level` / `service_tier` を任意で受け付けます。

## 要件

- Node.js >= 24.14.0
- [pnpm](https://pnpm.io) 10 以上（または npm）

## インストール

```bash
# 1. クローン
git clone https://github.com/siosig/mcp-gemini.git
cd mcp-gemini

# 2. 依存関係のインストール
pnpm install   # または: npm install

# 3. ビルド（TypeScript を dist/ にコンパイル）
pnpm build     # または: npm run build
```

### MCP クライアントへの登録

MCP クライアントの設定にサーバーを追加します。以下は Claude Desktop・Cursor をはじめ多くの MCP クライアントが用いる標準的な `mcpServers` ブロックです:

```json
{
  "mcpServers": {
    "gemini": {
      "command": "node",
      "args": ["/absolute/path/to/gemini-mcp-server/dist/index.js"],
      "env": {
        "GEMINI_API_KEY": "your-api-key"
      }
    }
  }
}
```

Gemini API キーは [Google AI Studio](https://aistudio.google.com/apikey) で取得できます。

## 設定

必須は `GEMINI_API_KEY` のみです。その他は妥当なデフォルトを持ち、[`.env.example`](.env.example) に文書化しています。

| 変数 | 説明 | デフォルト |
|------|------|-----------|
| `GEMINI_API_KEY` | Gemini API キー（必須） | — |
| `GEMINI_MODEL` / `GEMINI_AGENT_MODEL` / `GEMINI_SEARCH_MODEL` / `GEMINI_VISION_MODEL` / `GEMINI_CODE_MODEL` / `GEMINI_IMAGE_MODEL` | ツール別デフォルトモデル | ツールごとに最適化 |
| `GEMINI_*_THINKING_LEVEL` | ツール別デフォルト thinking level | ツールごとに最適化 |
| `GEMINI_TIMEOUT` | リクエストタイムアウト（秒） | `360` |
| `GEMINI_SERVICE_TIER` | デフォルト推論ティア（`flex`/`priority`/`standard`） | API デフォルト |
| `IMAGEN_OUTPUT_DIR` | 生成画像の出力先 | `<tmpdir>/mcp-gemini/imagen` |
| `LOG_LEVEL` | ログレベル（ログは stderr のみ） | `info` |

## アーキテクチャ

本サーバーは薄い階層構造のラッパーです。各層は単一の責務を持ちます:

```
MCP クライアント (Claude Desktop / Cursor / ...)
        │  stdio 上の JSON-RPC
        ▼
src/index.ts        ── エントリポイント: env 検証・トランスポート結線
src/server.ts       ── レジストリから全ツールをループ登録
src/tools/*.ts      ── 薄いハンドラ: Zod 入力スキーマ + 小さなハンドラ
src/tools/registry.ts ── ツール一覧の単一の真実源
        │
        ▼
src/services/gemini_client.ts ── @google/genai を呼ぶ唯一の場所
        │  (SDK シングルトン・リトライ・タイムアウト・診断)
        ▼
Google Gemini API
```

`src/utils/` 配下のモジュールが横断的関心事を担います: 環境変数検証（`env.ts`）、stderr ログ（`logger.ts`）、リトライ/タイムアウトラッパー（`telemetry.ts`）、空レスポンス診断（`diagnostics.ts`）、エラー整形（`errors.ts`）。

設計方針:

- **単一の統合点** — Gemini SDK 呼び出しはすべて `gemini_client.ts` を通すため、モデル/バージョン差（Gemini 3.x と 2.5 の thinking 設定など）を 1 箇所で吸収します。
- **オーケストレーションではなくプリミティブ** — ツールはステートレスで合成可能な部品です。高次のワークフロー（マルチエージェントの議論・反復改善など）はクライアント側で合成します。同梱の [`skills/gemini-team`](skills/gemini-team) スキルは、サーバー側に戦略コードを置かずに `gemini_custom_agent` の呼び出しを編成します。
- **stdio ファースト** — `stdout` は JSON-RPC 専用とし、ログはすべて `stderr` に出力します。

## 開発

```bash
pnpm dev            # ウォッチモード (tsx)
pnpm test           # 全テスト (vitest)
pnpm test:unit      # ユニットテストのみ
pnpm build          # 型チェック + コンパイル
```

## 同梱スキル

[`skills/gemini-team`](skills/gemini-team) は任意の MCP クライアント用スキルで、サーバーの `gemini_custom_agent` プリミティブをマルチエージェントワークフロー（並列の「コーディネーター」、反復の「生成 → 批評」、およびそれらの連結モード）に合成します。「サーバーは薄く保ち、クライアントが編成する」という意図された役割分担を示す実例です。

## ライセンス

[MIT](LICENSE) © Daisuke ITO
