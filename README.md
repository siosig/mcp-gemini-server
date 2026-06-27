# mcp-gemini-server

A thin, **stdio** [Model Context Protocol (MCP)](https://modelcontextprotocol.io) server that exposes the Google Gemini API as a small set of composable primitives. It is designed to be launched directly by an MCP client (Claude Desktop, Cursor, Cline, and any other MCP-compatible client) over stdio.

Built with TypeScript 6 / Node.js 24 (ESM) on top of the official [`@google/genai`](https://www.npmjs.com/package/@google/genai) SDK and [`@modelcontextprotocol/sdk`](https://www.npmjs.com/package/@modelcontextprotocol/sdk).

> **This plugin works out of the box** — just issue a [`GEMINI_API_KEY`](https://aistudio.google.com/apikey) and you're ready to go.

> 日本語版は [README.ja.md](README.ja.md) を参照してください。

## Features

- **8 composable Gemini primitives** — chat, web search, role-based agents, multimodal analysis, image generation, code execution, server-side team orchestration, and file management (see [Tools](#tools)).
- **Direct API, no CLI dependency** — talks to the Gemini API directly via `@google/genai`. There is no dependency on an external `gemini` CLI, so there is no shared auth/quota coupling and it runs cleanly inside containers.
- **Unified `thinking_level`** — a single `minimal | low | medium | high` knob is mapped automatically to the correct field per model family (Gemini 3.x `thinkingLevel` vs. Gemini 2.5 `thinkingBudget`).
- **Cost control with `service_tier`** — opt into `flex` (≈50% cheaper, latency-tolerant), `priority`, or `standard` per call or via an environment variable.
- **Per-tool optimized defaults** — each tool ships with a sensible default model and thinking level tuned for its use case; override only when you need to.
- **Thin by design** — the server provides primitives only. Multi-agent orchestration is left to the client side (see the bundled [`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) skill), keeping the server aligned with MCP's separation of concerns.
- **Operationally clean** — strict Zod schemas, structured tool output (`structuredContent`), retry/timeout handling, stderr-only logging, and hardened Node runtime flags. No external network or monitoring stack is assumed.

## Tools

| Tool | Description | Key inputs |
|------|-------------|-----------|
| `gemini_chat` | Chat with Gemini (thinking levels, grounding, JSON mode) | `prompt` (required) |
| `gemini_search` | Web search via Google using Gemini grounding | `query` (required) |
| `gemini_custom_agent` | Run a task with a specialized role | `task`, `role` (required) |
| `gemini_analyze_media` | Analyze images, PDF, video, or audio | `prompt` + one of `file_path` / `file_uri` / `image_url` / `image_base64` |
| `gemini_generate_image` | Generate a PNG with Gemini Flash Image (Nano Banana 2); images carry Google SynthID watermarking | `prompt` (required) |
| `gemini_execute_code` | Run Python in Gemini's sandbox (numpy/pandas/matplotlib) | `prompt` (required) |
| `gemini_team` | Server-side multi-agent orchestration (mul / it / mulit modes); reads local files and returns only the final result — Claude's context holds only file paths | `task`, `mode` (required) |
| `gemini_manage_files` | Manage the Gemini Files API (upload/list/status/delete) | `action` (required) |

Most tools also accept optional `model`, `thinking_level`, and `service_tier` parameters.

## Requirements

- [`bun`](https://bun.sh) on your PATH — runs the plugin's MCP server (`npm i -g bun`)
- Node.js >= 24.14.0
- [pnpm](https://pnpm.io) 10+ (or npm) — only needed to build from source

## Install as a Claude Code plugin (recommended)

The fastest path for Claude Code users. One install registers everything at once:
the `gemini` MCP server (all 7 tools), the `gemini-team` skill, the `gemini-delegate`
subagent, and a delegation-check hook.

```text
# 1. Add this repository as a plugin marketplace
/plugin marketplace add siosig/mcp-gemini-server

# 2. Install the plugin
/plugin install mcp-gemini-server@mcp-gemini-server
```

Or run the non-interactive installer (does the same two steps via the CLI):

```bash
./install_claude_plugin.sh
```

Then provide your Gemini API key in **one** of these ways:

- **Environment variable** (clients that propagate env to the MCP process):
  ```bash
  export GEMINI_API_KEY="your-api-key"
  ```
- **Config file** (recommended fallback; works even when the client does not pass
  the `.mcp.json` env block to the MCP process, e.g. the VS Code extension):
  ```bash
  echo '{ "GEMINI_API_KEY": "your-api-key" }' > ~/.gemini-mcp.json
  ```
  Precedence is **environment variable > config file**. Override the path with
  `GEMINI_MCP_CONFIG=/path/to/file.json`.

The plugin's MCP server launches via `bun ${CLAUDE_PLUGIN_ROOT}/dist/index.js` — a
single, self-contained bundle committed inside the plugin. No `npx`, no registry
fetch, and no `node_modules` to resolve at launch: it runs in **one ~75 MB process**.
(The previous `npx` launch forked a second Node process and idled at ~219 MB.)

**Requirement: [`bun`](https://bun.sh) must be on your PATH** (`npm i -g bun`, or see
bun.sh). The bundle is Node-compatible, so if you prefer Node you can change the
`command` in the plugin's `.mcp.json` from `bun` to `node`.

Get a Gemini API key from [Google AI Studio](https://aistudio.google.com/apikey).

## Configuration

Only `GEMINI_API_KEY` is required. All other settings have sensible defaults and are documented in [`.env.example`](.env.example).

| Variable | Description | Default |
|----------|-------------|---------|
| `GEMINI_API_KEY` | Gemini API key (required) | — |
| `GEMINI_MCP_CONFIG` | Path to a JSON config-file fallback for env vars | `~/.gemini-mcp.json` |
| `GEMINI_MODEL` / `GEMINI_AGENT_MODEL` | Default model for `gemini_chat` / `gemini_custom_agent` | `gemini-flash-latest` |
| `GEMINI_TEAM_MODEL` | Default model for `gemini_team` | inherits `GEMINI_AGENT_MODEL` |
| `GEMINI_SEARCH_MODEL` / `GEMINI_VISION_MODEL` / `GEMINI_CODE_MODEL` | Default model for search / media / code tools | `gemini-flash-lite-latest` |
| `GEMINI_IMAGE_MODEL` | Default model for `gemini_generate_image` | `gemini-3.1-flash-image-preview` |
| `GEMINI_*_THINKING_LEVEL` | Per-tool default thinking level | tuned per tool |
| `GEMINI_TIMEOUT` | Request timeout (seconds) | `360` |
| `GEMINI_SERVICE_TIER` | Default inference tier (`flex`/`priority`/`standard`) | API default |
| `IMAGEN_OUTPUT_DIR` | Output directory for generated images | `<tmpdir>/mcp-gemini/imagen` |
| `LOG_LEVEL` | Log level (logs go to stderr only) | `info` |

## Architecture

The server is a thin, layered wrapper. Each layer has a single responsibility:

```
MCP client (Claude Desktop / Cursor / ...)
        │  JSON-RPC over stdio
        ▼
src/index.ts        ── entrypoint: validate env, wire transport
src/server.ts       ── register every tool from the registry in a loop
src/tools/*.ts      ── thin handlers: Zod input schema + a small handler
src/tools/registry.ts ── single source of truth for the tool list
        │
        ▼
src/services/gemini_client.ts ── the only place that talks to @google/genai
        │  (singleton SDK client, retry, timeout, diagnostics)
        ▼
Google Gemini API
```

Supporting modules under `src/utils/` handle cross-cutting concerns: environment validation (`env.ts`), stderr logging (`logger.ts`), retry/timeout wrappers (`telemetry.ts`), empty-response diagnostics (`diagnostics.ts`), and error formatting (`errors.ts`).

Design principles:

- **Single integration point.** All Gemini SDK calls go through `gemini_client.ts`, so model/version differences (e.g. Gemini 3.x vs. 2.5 thinking config) are absorbed in one place.
- **Primitives, not orchestration.** Tools are stateless, composable building blocks. Higher-level workflows (e.g. multi-agent debate/refinement) are composed on the client side — see the bundled [`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) skill, which orchestrates `gemini_custom_agent` calls without any server-side strategy code.
- **stdio-first.** `stdout` is reserved for JSON-RPC; all logging goes to `stderr`.

## Development

```bash
pnpm dev            # watch mode (tsx)
pnpm test           # run all tests (vitest)
pnpm test:unit      # unit tests only
pnpm build          # type-check and compile
pnpm build:plugin   # bundle the self-contained plugin server (bun)
```

**Maintainers:** the plugin ships a committed, self-contained bundle at
`plugins/mcp-gemini-server/dist/index.js`. After changing `src/`, regenerate and
commit it so the GitHub-marketplace install picks up the change:

```bash
pnpm build:plugin
git add plugins/mcp-gemini-server/dist/index.js
git commit && git push
```

## Multi-agent orchestration

Two complementary approaches are available depending on who manages the workflow:

### `gemini_team` tool (server-side)

The `gemini_team` MCP tool runs the full multi-agent pipeline inside the server process. Claude passes a task, a mode, and optional local file paths; the server reads the files, fans out to Gemini specialist agents, aggregates the result, and returns only the final answer. Claude's context never holds file contents — only the file paths — keeping the main conversation lean.

| Mode | Pattern |
|------|---------|
| `mul` | Parallel specialist agents → Gemini aggregation → final answer |
| `it` | Initial draft → critic/generator loop (`max_iterations`, default 2) |
| `mulit` | `mul` Phase 1 + `it` Phase 2 chained; highest quality, slowest |

### `gemini-team` skill (client-side)

[`plugins/mcp-gemini-server/skills/gemini-team`](plugins/mcp-gemini-server/skills/gemini-team) is an optional MCP-client skill that composes `gemini_custom_agent` calls on the client side. This gives Claude direct visibility into each agent's output at each step, enabling dynamic mid-loop decisions (e.g. early exit, role adjustment, injecting search results). It demonstrates the intended division of labor: the server stays thin, the client orchestrates.

**When to use which**: choose `gemini_team` (tool) when the task input is large files or when you want a fire-and-forget call. Choose the `gemini-team` skill when you need mid-loop steering or want to interleave Claude's reasoning with each agent's output.

## Delegating to Gemini (gemini-delegate)

[`plugins/mcp-gemini-server/agents/gemini-delegate.md`](plugins/mcp-gemini-server/agents/gemini-delegate.md) is an optional Claude Code **subagent** that offloads a single, self-contained task to Gemini in an *isolated context* and returns only a distilled result. Because Gemini's verbose output never enters the main thread, it keeps the (expensive) main Claude conversation small — cutting token use and speeding up research/dev. The wrapper inherits the parent thread's model; the savings come from context isolation and a thin "package → delegate → distill" responsibility, not from a cheaper model.

### Install

Installing the [Claude Code plugin](#install-as-a-claude-code-plugin-recommended)
registers the subagent automatically. To install it manually instead, copy the file:

```bash
# Per-project
mkdir -p .claude/agents && cp plugins/mcp-gemini-server/agents/gemini-delegate.md .claude/agents/

# All projects (user-level)
mkdir -p ~/.claude/agents && cp plugins/mcp-gemini-server/agents/gemini-delegate.md ~/.claude/agents/
```

### Optional: a delegation-check hook

The Claude Code plugin registers this hook automatically. To add it manually,
put a `UserPromptSubmit` hook in your `settings.json`. Its stdout is injected into context.

> ⚠️ The `command` **must be a single-line JSON string**. A literal newline / multi-line command makes the whole `settings.json` "Invalid or malformed JSON" and disables *all* settings. After editing, validate with `python3 -c "import json;json.load(open('<path>/settings.json'))"`.

```json
{
  "type": "command",
  "command": "printf '%s' '<delegation-check>If this turn contains an independent, context-packageable task (research/review/design/summarize/media analysis/code execution), consider delegating it to gemini-delegate before answering. Final decisions, file edits/Git, and orchestration stay with Claude. Skip for trivial replies.</delegation-check>'"
}
```

### Delegation policy

| Situation | Use |
|-----------|-----|
| A single, self-contained task to offload | **gemini-delegate** |
| Multi-perspective review / multi-agent orchestration | **`gemini-team`** (coordinator / iterative) |
| Final decisions, file edits / Git, orchestration, tight step-by-step control | **Main Claude, directly** |

See [`plugins/mcp-gemini-server/agents/`](plugins/mcp-gemini-server/agents/) for details.

## License

[MIT](LICENSE) © Daisuke ITO
