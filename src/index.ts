/**
 * gemini-mcp-server entry point.
 * Switches between stdio / uds(http) via the MCP_TRANSPORT environment variable.
 */

import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import fs from "node:fs/promises";
import { once } from "node:events";
import { setSharedGenAI } from "./services/gemini_client.js";
import { logger } from "./utils/logger.js";
import { startHTTPServer } from "./server/http.js";
import { setUploadStore } from "./server/upload.js";
import { UploadStore } from "./services/upload_store.js";
import { resolveTransport, isUnknownTransport } from "./utils/transport.js";
import { validateEnv } from "./utils/env.js";
import { createMcpServer } from "./server.js";
import pkg from "../package.json" with { type: "json" };

// mcp-ts.md §10: startup-time Zod validation (Fail Fast). Exits if GEMINI_API_KEY is unset.
const env = validateEnv();

const PKG_VERSION: string = pkg.version;
const SERVER_NAME = "gemini-mcp-server";

const transportMode = resolveTransport(env.MCP_TRANSPORT);
if (isUnknownTransport(env.MCP_TRANSPORT)) {
  logger.warn({ MCP_TRANSPORT: env.MCP_TRANSPORT }, "Unknown MCP_TRANSPORT value, falling back to uds");
}
const MCP_SOCKET_PATH = env.MCP_SOCKET_PATH ?? "/run/user/1000/mcp-gemini.sock";

// In stdio mode, keep stdout exclusive to JSON-RPC by redirecting console.log/info to stderr.
// This prevents stray output from third-party libraries from corrupting the JSON-RPC stream.
if (transportMode === "stdio") {
  console.log = console.error;
  console.info = console.error;
}

function newMcpServer() {
  return createMcpServer({ name: SERVER_NAME, version: PKG_VERSION });
}

let uploadStore: UploadStore | null = null;
let httpServer: import("node:http").Server | undefined;

async function shutdown(signal: string): Promise<void> {
  logger.info({ signal }, `Received ${signal}, shutting down`);

  if (httpServer) {
    httpServer.close();
    httpServer.closeIdleConnections();

    const forceTimer = setTimeout(() => {
      logger.warn("Graceful shutdown timeout, forcing close");
      httpServer!.closeAllConnections();
    }, 10_000);
    forceTimer.unref();

    await once(httpServer, "close");
    clearTimeout(forceTimer);

    try {
      await fs.unlink(MCP_SOCKET_PATH);
    } catch {
      // Ignore if already removed.
    }
  }

  if (uploadStore) {
    await uploadStore.destroy();
  }
  setSharedGenAI(null);
  logger.flush();
  process.exit(0);
}

process.once("SIGTERM", () => void shutdown("SIGTERM"));
process.once("SIGINT", () => void shutdown("SIGINT"));

if (transportMode === "stdio") {
  const server = newMcpServer();
  const transport = new StdioServerTransport();
  await server.connect(transport);
  logger.info({ version: PKG_VERSION, transport: "stdio" }, `${SERVER_NAME} v${PKG_VERSION} started (stdio)`);
  // Keep the process alive so the SDK continues watching stdio.
  await new Promise(() => {});
} else {
  uploadStore = new UploadStore();
  await uploadStore.init();
  setUploadStore(uploadStore);

  httpServer = await startHTTPServer({ socketPath: MCP_SOCKET_PATH, createMcpServer: newMcpServer });
  logger.info(
    { version: PKG_VERSION, transport: "uds", socketPath: MCP_SOCKET_PATH },
    `${SERVER_NAME} v${PKG_VERSION} started (UDS: ${MCP_SOCKET_PATH})`,
  );
}
