#!/usr/bin/env node
/**
 * gemini-mcp-server entry point.
 * stdio transport only: the MCP client spawns this process and talks over stdin/stdout.
 */

import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { setSharedGenAI } from "./services/gemini_client.js";
import { logger } from "./utils/logger.js";
import { validateEnv } from "./utils/env.js";
import { loadConfigFileIntoEnv } from "./utils/config_file.js";
import { createMcpServer } from "./server.js";
import pkg from "../package.json" with { type: "json" };

// Fill unset env vars from ~/.gemini-mcp.json (or $GEMINI_MCP_CONFIG) for clients
// that do not propagate the .mcp.json env block to the MCP process. Real env vars
// always win. Must run before validateEnv() so Fail-Fast sees the resolved env.
loadConfigFileIntoEnv();

// mcp-ts.md §10: startup-time Zod validation (Fail Fast). Exits if GEMINI_API_KEY is unset.
validateEnv();

const PKG_VERSION: string = pkg.version;
const SERVER_NAME = "gemini-mcp-server";

// Keep stdout exclusive to JSON-RPC by redirecting console.log/info to stderr.
// This prevents stray output from third-party libraries from corrupting the JSON-RPC stream.
console.log = console.error;
console.info = console.error;

async function shutdown(signal: string): Promise<void> {
  logger.info({ signal }, `Received ${signal}, shutting down`);
  setSharedGenAI(null);
  logger.flush();
  process.exit(0);
}

process.once("SIGTERM", () => void shutdown("SIGTERM"));
process.once("SIGINT", () => void shutdown("SIGINT"));

const server = createMcpServer({ name: SERVER_NAME, version: PKG_VERSION });
const transport = new StdioServerTransport();
await server.connect(transport);
logger.info({ version: PKG_VERSION, transport: "stdio" }, `${SERVER_NAME} v${PKG_VERSION} started (stdio)`);

// Keep the process alive so the SDK continues watching stdio.
await new Promise(() => {});
