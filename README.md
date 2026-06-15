# gemini-mcp-server

A thin, **stdio** [Model Context Protocol (MCP)](https://modelcontextprotocol.io) server that exposes the Google Gemini API as a small set of composable primitives. It is designed to be launched directly by an MCP client (Claude Desktop, Cursor, Cline, and any other MCP-compatible client) over stdio.

Built with TypeScript 6 / Node.js 24 (ESM) on top of the official [`@google/genai`](https://www.npmjs.com/package/@google/genai) SDK and [`@modelcontextprotocol/sdk`](https://www.npmjs.com/package/@modelcontextprotocol/sdk).

> 日本語版は [README.ja.md](README.ja.md) を参照してください。

## Features

- **7 composable Gemini primitives** — chat, web search, role-based agents, multimodal analysis, image generation, code execution, and file management (see [Tools](#tools)).
- **Direct API, no CLI dependency** — talks to the Gemini API directly via `@google/genai`. There is no dependency on an external `gemini` CLI, so there is no shared auth/quota coupling and it runs cleanly inside containers.
- **Unified `thinking_level`** — a single `minimal | low | medium | high` knob is mapped automatically to the correct field per model family (Gemini 3.x `thinkingLevel` vs. Gemini 2.5 `thinkingBudget`).
- **Cost control with `service_tier`** — opt into `flex` (≈50% cheaper, latency-tolerant), `priority`, or `standard` per call or via an environment variable.
- **Per-tool optimized defaults** — each tool ships with a sensible default model and thinking level tuned for its use case; override only when you need to.
- **Thin by design** — the server provides primitives only. Multi-agent orchestration is left to the client side (see the bundled [`skills/gemini-team`](skills/gemini-team) skill), keeping the server aligned with MCP's separation of concerns.
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
| `gemini_manage_files` | Manage the Gemini Files API (upload/list/status/delete) | `action` (required) |

Most tools also accept optional `model`, `thinking_level`, and `service_tier` parameters.

## Requirements

- Node.js >= 24.14.0
- [pnpm](https://pnpm.io) 10+ (or npm)

## Installation

```bash
# 1. Clone
git clone https://github.com/siosig/mcp-gemini.git
cd mcp-gemini

# 2. Install dependencies
pnpm install   # or: npm install

# 3. Build (compiles TypeScript to dist/)
pnpm build     # or: npm run build
```

### Register with your MCP client

Add the server to your MCP client's configuration. The example below is the standard `mcpServers` block used by Claude Desktop, Cursor, and most MCP clients:

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

Get a Gemini API key from [Google AI Studio](https://aistudio.google.com/apikey).

## Configuration

Only `GEMINI_API_KEY` is required. All other settings have sensible defaults and are documented in [`.env.example`](.env.example).

| Variable | Description | Default |
|----------|-------------|---------|
| `GEMINI_API_KEY` | Gemini API key (required) | — |
| `GEMINI_MODEL` / `GEMINI_AGENT_MODEL` / `GEMINI_SEARCH_MODEL` / `GEMINI_VISION_MODEL` / `GEMINI_CODE_MODEL` / `GEMINI_IMAGE_MODEL` | Per-tool default model | tuned per tool |
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
- **Primitives, not orchestration.** Tools are stateless, composable building blocks. Higher-level workflows (e.g. multi-agent debate/refinement) are composed on the client side — see the bundled [`skills/gemini-team`](skills/gemini-team) skill, which orchestrates `gemini_custom_agent` calls without any server-side strategy code.
- **stdio-first.** `stdout` is reserved for JSON-RPC; all logging goes to `stderr`.

## Development

```bash
pnpm dev            # watch mode (tsx)
pnpm test           # run all tests (vitest)
pnpm test:unit      # unit tests only
pnpm build          # type-check and compile
```

## Bundled skill

[`skills/gemini-team`](skills/gemini-team) is an optional MCP-client skill that composes the server's `gemini_custom_agent` primitive into multi-agent workflows (parallel "coordinator", iterative "generate → critique", and a combined mode). It demonstrates the intended division of labor: the server stays thin, the client orchestrates.

## License

[MIT](LICENSE) © Daisuke ITO
