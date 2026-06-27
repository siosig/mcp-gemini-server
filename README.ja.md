# mcp-gemini-server

Google Gemini API を、合成可能な少数のプリミティブとして公開する薄い **stdio** [Model Context Protocol (MCP)](https://modelcontextprotocol.io) サーバーです。MCP クライアント（Claude Desktop、Cursor、Cline など MCP 対応クライアント）が stdio で直接起動して利用することを想定しています。

**Rust**（stable）で実装し、[`rmcp`](https://crates.io/crates/rmcp)（公式 Rust MCP SDK）と [`reqwest`](https://crates.io/crates/reqwest) の上に構築、Gemini の Generative Language REST API を直接呼び出します。単一の自己完結ネイティブバイナリとして配布され、Node.js / Python / 外部 CLI への実行時依存はありません。

> **このプラグインは [`GEMINI_API_KEY`](https://aistudio.google.com/apikey) を発行すればすぐに使えます。**

> English version: [README.md](README.md)

## 特徴

- **8 つの合成可能な Gemini プリミティブ** — チャット、Web 検索、ロールベースエージェント、マルチモーダル解析、画像生成、コード実行、サーバーサイドのマルチエージェント統括、ファイル管理（[ツール](#ツール)参照）。
- **API 直結・CLI 非依存** — `reqwest` 経由で Gemini REST API を直接呼び出します。外部 `gemini` CLI に依存しないため、認証/クォータの共有問題がなく、コンテナ内でもクリーンに動作します。
- **統一された `thinking_level`** — `minimal | low | medium | high` の 1 つのつまみが、モデル系列に応じて正しいフィールドへ自動マッピングされます（Gemini 3.x の `thinkingLevel` / Gemini 2.5 の `thinkingBudget`）。
- **`service_tier` によるコスト制御** — `flex`（約 50% 安価・レイテンシ許容）/ `priority` / `standard` を呼び出し単位または環境変数で選択できます。
- **ツール別に最適化されたデフォルト** — 各ツールは用途に合わせたデフォルトモデルと thinking level を持ちます。必要なときだけ上書きしてください。
- **薄い設計** — サーバーはプリミティブのみを提供します。マルチエージェントのオーケストレーションはクライアント側に委ねており（同梱の [`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) スキル参照）、MCP の関心分離に沿っています。
- **運用上クリーン** — 厳格な `serde` + `schemars` 入力スキーマ、構造化ツール出力（`structuredContent`）、リトライ/タイムアウト処理、stderr のみへの構造化ログ出力（stdout は JSON-RPC 専用）。外部ネットワークや監視スタックを前提としません。

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

- [`GEMINI_API_KEY`](https://aistudio.google.com/apikey)（環境変数または `~/.gemini-mcp.json`）
- ネイティブバイナリ — `install_claude_plugin.sh` が GitHub Release から自動取得、またはローカルビルド
- [Rust](https://rustup.rs)（stable、≥ 1.85）— pre-built バイナリが無い場合にソースからビルドするときのみ必要

## Claude Code プラグインとしてインストール（推奨）

Claude Code 利用者向けの最短経路。1 回の install で、`gemini` MCP サーバー（8 ツール）・
`gemini-team` スキル・`gemini-delegate` サブエージェントがまとめて登録されます。
インストーラは委譲ポリシーを `~/.claude/rules/mcp-gemini-server.md`（常時ロードされる
rules、毎ターンの hook 無し）としても書き込みます。

```text
# 1. このリポジトリをプラグインマーケットプレースとして追加
/plugin marketplace add siosig/mcp-gemini-server

# 2. プラグインを install
/plugin install mcp-gemini-server@mcp-gemini-server
```

または、非対話インストーラを実行（CLI で上記2ステップを実施）:

```bash
./install_claude_plugin.sh
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

プラグインの MCP サーバーはネイティブ Rust バイナリです。`install_claude_plugin.sh` が
`~/.local/share/mcp-gemini-server/mcp-gemini-server` にインストールし（OS/アーキ別の
GitHub Release からダウンロード、無ければ `cargo build --release` でローカルビルド）、
プラグインの `.mcp.json` がそれを直接起動します。Node.js も `npx` も `node_modules` 解決も
不要で、単一の静的プロセスとして動作します。

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
src/main.rs         ── エントリポイント: 設定読込・env 検証・stdio serve
src/server.rs       ── rmcp の #[tool_router] マクロで全ツールを登録
src/tools/*.rs      ── 薄いハンドラ: serde/schemars 入力スキーマ + 小さなハンドラ
        │
        ▼
src/services/gemini_client.rs ── Gemini REST API を呼ぶ唯一の場所
        │  (reqwest クライアント・リトライ・タイムアウト・診断)
        ▼
Google Gemini API
```

`src/utils/` 配下のモジュールが横断的関心事を担います: 環境変数検証（`env.rs`）、stderr ログ（`logger.rs`）、リトライ/タイムアウトラッパー（`telemetry.rs`）、空レスポンス診断（`diagnostics.rs`）、エラー整形（`errors.rs`）。Gemini のワイヤ型は `src/services/types.rs` にあります。

設計方針:

- **単一の統合点** — Gemini API 呼び出しはすべて `gemini_client.rs` を通すため、モデル/バージョン差（Gemini 3.x と 2.5 の thinking 設定など）を 1 箇所で吸収します。
- **オーケストレーションではなくプリミティブ** — ツールはステートレスで合成可能な部品です。高次のワークフロー（マルチエージェントの議論・反復改善など）はクライアント側で合成します。同梱の [`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) スキルは、サーバー側に戦略コードを置かずに `gemini_custom_agent` の呼び出しを編成します。
- **stdio ファースト** — `stdout` は JSON-RPC 専用とし、ログはすべて `stderr` に出力します。

## 開発

```bash
cargo build --release   # 最適化バイナリをコンパイル (target/release/mcp-gemini-server)
cargo test              # 全ユニットテスト
cargo clippy --all-targets --all-features --locked -- -D warnings   # lint（警告ゼロ）
```

**リリース:** 注釈付き `vX.Y.Z` タグを push します。[`release.yml`](.github/workflows/release.yml)
ワークフローが Linux（x86_64 / aarch64）・macOS（aarch64）・Windows（x86_64）向けに
クロスコンパイルし、アーカイブを GitHub Release アセットとして公開します
（`install_claude_plugin.sh` がそれをダウンロードします）。

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

### 委譲ルール（hook 不使用）

`install_claude_plugin.sh` は委譲ポリシーを `~/.claude/rules/mcp-gemini-server.md`
に常時ロードされる rules として書き込み、`-d` で削除します。これは従来の
`UserPromptSubmit` hook を意図的に置き換えるもので、毎ターンの入力に何も注入されません。
スクリプトを使わず手動でエージェントだけ入れる場合は、同じ内容の rules ファイルを
自分で作成してください。

### 委譲ポリシー

| 状況 | 使うもの |
|------|---------|
| 自己完結した単一タスクを移譲 | **gemini-delegate** |
| 多角的レビュー / マルチエージェント統括 | **`gemini-team`**（コーディネーター / 反復） |
| 最終判断・ファイル編集/Git・オーケストレーション・密な逐次制御 | **Claude 本体が直接** |

詳細は [`plugins/mcp-gemini-server/agents/`](plugins/mcp-gemini-server/agents/) を参照してください。

## ライセンス

[MIT](LICENSE) © Daisuke ITO
