import http from "node:http";
import fs from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import { logger } from "../utils/logger.js";
import { handleUpload } from "./upload.js";

// Time before an idle session is automatically destroyed (5 minutes)
const SESSION_TTL_MS = 5 * 60 * 1000;

type SessionEntry = {
  transport: StreamableHTTPServerTransport;
  timer: ReturnType<typeof setTimeout>;
};

export type HTTPServerOptions = {
  socketPath: string;
  createMcpServer: () => McpServer;
};

export async function startHTTPServer(options: HTTPServerOptions): Promise<http.Server> {
  const { socketPath, createMcpServer } = options;
  const sessions = new Map<string, SessionEntry>();

  function resetTTL(sessionId: string): void {
    const entry = sessions.get(sessionId);
    if (!entry) return;
    clearTimeout(entry.timer);
    entry.timer = setTimeout(() => {
      sessions.delete(sessionId);
      void entry.transport.close();
      logger.info({ sessionId }, "MCP session TTL expired (auto-destroyed)");
    }, SESSION_TTL_MS);
  }

  const server = http.createServer(
    {
      keepAlive: true,
      keepAliveInitialDelay: 0,
      // Header parsing: 15 seconds is sufficient
      headersTimeout: 15_000,
      // Whole request: unlimited to support long-lived SSE connections
      requestTimeout: 0,
    },
    async (req, res) => {
      const url = new URL(req.url ?? "/", "http://localhost");

      if (req.method === "GET" && url.pathname === "/health") {
        const body = {
          status: "ok",
          sessions: sessions.size,
        };
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify(body));
        return;
      }

      if (req.method === "POST" && url.pathname === "/upload") {
        await handleUpload(req, res);
        return;
      }

      if (req.method === "POST" && url.pathname === "/mcp") {
        const rawSessionId = req.headers["mcp-session-id"];
        const sessionId =
          typeof rawSessionId === "string" ? rawSessionId : undefined;

        if (sessionId) {
          const entry = sessions.get(sessionId);
          if (entry) {
            resetTTL(sessionId);
            await entry.transport.handleRequest(req, res);
            return;
          }
          // Unknown session ID -> return 404 to prompt reinitialization (per MCP spec)
          logger.info({ sessionId }, "Unknown MCP session (requesting client reinitialization)");
          res.writeHead(404, { "Content-Type": "application/json" });
          res.end(JSON.stringify({
            jsonrpc: "2.0",
            error: { code: -32000, message: "Session not found. Please reinitialize." },
            id: null,
          }));
          return;
        }

        // New session
        const newSessionId = randomUUID();
        const transport = new StreamableHTTPServerTransport({
          sessionIdGenerator: () => newSessionId,
          onsessioninitialized: (id) => {
            const timer = setTimeout(() => {
              sessions.delete(id);
              void transport.close();
              logger.info({ sessionId: id }, "MCP session TTL expired (auto-destroyed)");
            }, SESSION_TTL_MS);
            sessions.set(id, { transport, timer });
            logger.info({ sessionId: id }, "MCP session started");
          },
        });

        transport.onclose = () => {
          const entry = sessions.get(newSessionId);
          if (entry) {
            clearTimeout(entry.timer);
            sessions.delete(newSessionId);
          }
          logger.info({ sessionId: newSessionId }, "MCP session ended");
        };

        const mcpServer = createMcpServer();
        // StreamableHTTPServerTransport's onclose accessor is typed `(() => void) | undefined`,
        // but the Transport interface declares `onclose?: () => void` (which disallows undefined
        // under exactOptionalPropertyTypes), causing a type mismatch inside the SDK. This has no
        // effect on runtime behavior, so we cast to Transport.
        await mcpServer.connect(transport as Transport);
        await transport.handleRequest(req, res);
        return;
      }

      res.writeHead(404, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: "Not found" }));
    },
  );

  // Disable the socket timeout to support long-lived SSE connections
  server.timeout = 0;
  // Set longer than nginx keepalive_timeout (65s) so Node.js does not disconnect first
  server.keepAliveTimeout = 75_000;

  // Remove any stale socket file (guards against a previous abnormal shutdown)
  try {
    await fs.unlink(socketPath);
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
  }

  // Temporarily change umask so the socket is created with 0666 from the start (avoids TOCTOU)
  const oldUmask = process.umask(0o000);

  server.listen(socketPath, 1024, () => {
    process.umask(oldUmask);
    logger.info({ socketPath }, "MCP HTTP server started (UDS)");
  });

  return server;
}
