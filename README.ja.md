# mcp-gemini-server

Google Gemini API を、合成可能な少数のプリミティブとして公開する薄い **stdio** [Model Context Protocol (MCP)](https://modelcontextprotocol.io) サーバーです。MCP クライアント（Claude Desktop、Cursor、Cline など MCP 対応クライアント）が stdio で直接起動して利用することを想定しています。

TypeScript 6 / Node.js 24（ESM）で実装し、公式 [`@google/genai`](https://www.npmjs.com/package/@google/genai) SDK と [`@modelcontextprotocol/sdk`](https://www.npmjs.com/package/@modelcontextprotocol/sdk) の上に構築しています。

> **このプラグインは [`GEMINI_API_KEY`](https://aistudio.google.com/apikey) を発行すればすぐに使えます。**

> English version: [README.md](README.md)

## 特徴

- **8 つの合成可能な Gemini プリミティブ** — チャット、Web 検索、ロールベースエージェント、マルチモーダル解析、画像生成、コード実行、サーバーサイドのマルチエージェント統括、ファイル管理（[ツール](#ツール)参照）。
- **API 直結・CLI 非依存** — `@google/genai` 経由で Gemini API を直接呼び出します。外部 `gemini` CLI に依存しないため、認証/クォータの共有問題がなく、コンテナ内でもクリーンに動作します。
- **統一された `thinking_level`** — `minimal | low | medium | high` の 1 つのつまみが、モデル系列に応じて正しいフィールドへ自動マッピングされます（Gemini 3.x の `thinkingLevel` / Gemini 2.5 の `thinkingBudget`）。
- **`service_tier` によるコスト制御** — `flex`（約 50% 安価・レイテンシ許容）/ `priority` / `standard` を呼び出し単位または環境変数で選択できます。
- **ツール別に最適化されたデフォルト** — 各ツールは用途に合わせたデフォルトモデルと thinking level を持ちます。必要なときだけ上書きしてください。
- **薄い設計** — サーバーはプリミティブのみを提供します。マルチエージェントのオーケストレーションはクライアント側に委ねており（同梱の [`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) スキル参照）、MCP の関心分離に沿っています。
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
| `gemini_team` | サーバーサイドのマルチエージェント統括（mul / it / mulit モード）。ローカルファイルを読み込み、スペシャリストを並列実行し、最終結果のみを返す（Claude の context はファイルパスのみ保持） | `task`, `mode`（必須） |
| `gemini_manage_files` | Gemini Files API 管理（upload/list/status/delete） | `action`（必須） |

多くのツールは `model` / `thinking_level` / `service_tier` を任意で受け付けます。

## 要件

- Node.js >= 24.14.0
- [pnpm](https://pnpm.io) 10 以上（または npm）

## Claude Code プラグインとしてインストール（推奨）

Claude Code 利用者向けの最短経路。1 回の install で、`gemini` MCP サーバー（8 ツール）・
`gemini-team` スキル・`gemini-delegate` サブエージェント・delegation-check hook が
まとめて登録されます。

```text
# 1. このリポジトリをプラグインマーケットプレースとして追加
/plugin marketplace add siosig/mcp-gemini-server

# 2. プラグインを install
/plugin install mcp-gemini-server@mcp-gemini-server
```

続いて Gemini API キーを **いずれか** の方法で供給します:

- **環境変数**（env を MCP プロセスへ伝播するクライアント）:
  ```bash
  export GEMINI_API_KEY="your-api-key"
  ```
- **設定ファイル**（推奨フォールバック。`.mcp.json` の env ブロックを MCP プロセスへ
  渡さないクライアント（VS Code 拡張など）でも動作）:
  ```bash
  echo '{ "GEMINI_API_KEY": "your-api-key" }' > ~/.gemini-mcp.json
  ```
  優先順位は **環境変数 > 設定ファイル**。パスは `GEMINI_MCP_CONFIG=/path/to/file.json` で上書き可。

プラグインの MCP サーバーは `npx -y mcp-gemini-server@npm:mcp-gemini-server@2` で起動します。
`@npm:` エイリアスで `npx` にレジストリ解決を強制しています。素の `mcp-gemini-server@2` だと、
クライアントの作業ディレクトリがサーバー自身のリポジトリのとき npx がローカルパッケージと誤認し
（pnpm ワークスペースでは自パッケージの bin が self-link されないため）、
`sh: mcp-gemini-server: not found` で起動に失敗します。`npx` の
オンデマンド解決が不安定な環境（特に VS Code 拡張）では、bin をグローバル install して
直接起動に切り替えてください:

```bash
npm i -g mcp-gemini-server
```

Gemini API キーは [Google AI Studio](https://aistudio.google.com/apikey) で取得できます。

## 手動インストール（任意の MCP クライアント）

```bash
# 1. クローン
git clone https://github.com/siosig/mcp-gemini-server.git
cd mcp-gemini-server

# 2. 依存関係のインストール
pnpm install   # または: npm install

# 3. ビルド（TypeScript を dist/ にコンパイル）
pnpm build     # または: npm run build
```

### MCP クライアントへの登録

本サーバーは **stdio 専用** です（MCP クライアントがプロセスを起動し stdin/stdout で通信）。
以下は Claude Desktop・Cursor をはじめ多くの MCP クライアントが用いる標準的な `mcpServers` ブロックです:

```json
{
  "mcpServers": {
    "gemini": {
      "command": "node",
      "args": ["/absolute/path/to/mcp-gemini-server/dist/index.js"],
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
| `GEMINI_MCP_CONFIG` | 環境変数フォールバック用 JSON 設定ファイルのパス | `~/.gemini-mcp.json` |
| `GEMINI_MODEL` / `GEMINI_AGENT_MODEL` | `gemini_chat` / `gemini_custom_agent` のデフォルトモデル | `gemini-flash-latest` |
| `GEMINI_TEAM_MODEL` | `gemini_team` のデフォルトモデル | `GEMINI_AGENT_MODEL` を継承 |
| `GEMINI_SEARCH_MODEL` / `GEMINI_VISION_MODEL` / `GEMINI_CODE_MODEL` | 検索・メディア・コードツールのデフォルトモデル | `gemini-flash-lite-latest` |
| `GEMINI_IMAGE_MODEL` | `gemini_generate_image` のデフォルトモデル | `gemini-3.1-flash-image-preview` |
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
- **オーケストレーションではなくプリミティブ** — ツールはステートレスで合成可能な部品です。高次のワークフロー（マルチエージェントの議論・反復改善など）はクライアント側で合成します。同梱の [`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) スキルは、サーバー側に戦略コードを置かずに `gemini_custom_agent` の呼び出しを編成します。
- **stdio ファースト** — `stdout` は JSON-RPC 専用とし、ログはすべて `stderr` に出力します。

## 開発

```bash
pnpm dev            # ウォッチモード (tsx)
pnpm test           # 全テスト (vitest)
pnpm test:unit      # ユニットテストのみ
pnpm build          # 型チェック + コンパイル
```

## マルチエージェント統括

ワークフローを管理する側に応じて 2 つのアプローチを使い分けられます。

### `gemini_team` ツール（サーバーサイド）

`gemini_team` MCP ツールはマルチエージェントのパイプライン全体をサーバープロセス内で実行します。Claude はタスク・モード・任意のローカルファイルパスを渡すだけで、サーバーがファイル読み込み・スペシャリスト並列実行・結果集約を行い、最終回答のみを返します。Claude の context にはファイルパスだけが残り、ファイル内容は入りません。

| モード | パターン |
|--------|---------|
| `mul` | 並列スペシャリスト → Gemini による集約 → 最終回答 |
| `it` | 初期ドラフト生成 → 批評/生成ループ（`max_iterations`、デフォルト 2） |
| `mulit` | `mul` フェーズ 1 + `it` フェーズ 2 連結（最高品質・最低速） |

### `gemini-team` スキル（クライアントサイド）

[`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) は任意の MCP クライアント用スキルで、`gemini_custom_agent` 呼び出しをクライアント側で合成します。各エージェントの出力をステップごとに Claude が直接参照できるため、ループ途中でのロール変更・早期終了・検索結果注入などの動的制御が可能です。「サーバーは薄く保ち、クライアントが編成する」という意図された役割分担を示す実例です。

**使い分けの目安**: タスク入力が大きなファイルの場合や fire-and-forget で結果だけ欲しい場合は `gemini_team`（ツール）を使います。ループ途中で Claude が介入したい場合や各エージェントの出力を逐次確認したい場合は `gemini-team` スキルを使います。

## Gemini への委譲（gemini-delegate）

[`plugins/mcp-gemini-server/agents/gemini-delegate.md`](plugins/mcp-gemini-server/agents/gemini-delegate.md) は任意の Claude Code **サブエージェント**で、自己完結した単一タスクを *隔離されたコンテキスト* で Gemini に移譲し、蒸留した結果だけを返します。Gemini の冗長な生出力が main スレッドに入らないため、高価な main の会話を小さく保て、トークン消費削減と調査・開発の高速化につながります。ラッパは親スレッドのモデルを継承します。削減効果は安価なモデルではなく、コンテキスト隔離と「文脈整形 → 委譲 → 蒸留」という薄い責務から得られます。

### インストール

[Claude Code プラグイン](#claude-code-プラグインとしてインストール推奨)を入れれば
サブエージェントは自動登録されます。手動で入れる場合はファイルをコピーします:

```bash
# プロジェクト単位
mkdir -p .claude/agents && cp plugins/mcp-gemini-server/agents/gemini-delegate.md .claude/agents/

# 全プロジェクト（ユーザーレベル）
mkdir -p ~/.claude/agents && cp plugins/mcp-gemini-server/agents/gemini-delegate.md ~/.claude/agents/
```

### 任意: 委譲チェックの hook

Claude Code プラグインはこの hook を自動登録します。手動で追加する場合は、
`settings.json` に `UserPromptSubmit` hook を追加します。その stdout が context に注入されます。

> ⚠️ `command` は **必ず1行の JSON 文字列**にします。リテラル改行や複数行の command は `settings.json` 全体を「Invalid or malformed JSON」にし、*すべての*設定を無効化します。編集後は `python3 -c "import json;json.load(open('<path>/settings.json'))"` で検証してください。

```json
{
  "type": "command",
  "command": "printf '%s' '<delegation-check>文脈をまとめて渡せば完結する独立タスク（調査/レビュー/設計/要約/メディア解析/コード実行）がこのターンに含まれるなら、回答前に gemini-delegate への委譲を検討する。最終判断・ファイル編集/Git・オーケストレーションは Claude が担う。軽微な応答ではスキップ可。</delegation-check>'"
}
```

### 委譲ポリシー

| 状況 | 使うもの |
|------|---------|
| 自己完結した単一タスクを移譲 | **gemini-delegate** |
| 多角的レビュー / マルチエージェント統括 | **`gemini-team`**（コーディネーター / 反復） |
| 最終判断・ファイル編集/Git・オーケストレーション・密な逐次制御 | **Claude 本体が直接** |

詳細は [`plugins/mcp-gemini-server/agents/`](plugins/mcp-gemini-server/agents/) を参照してください。

## ライセンス

[MIT](LICENSE) © Daisuke ITO
