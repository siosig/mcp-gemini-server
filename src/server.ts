/**
 * McpServer factory.
 * Registers all tools via registerTool and centralizes the shared handler logic
 * (progress notifications, truncation, and error formatting).
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { allTools } from "./tools/registry.js";
import { handleApiError } from "./utils/errors.js";
import { truncate } from "./constants.js";

export interface CreateMcpServerOptions {
  name: string;
  version: string;
}

export function createMcpServer(opts: CreateMcpServerOptions): McpServer {
  const server = new McpServer({ name: opts.name, version: opts.version });

  for (const tool of allTools) {
    server.registerTool(
      tool.name,
      {
        title: tool.title,
        description: tool.description,
        inputSchema: tool.schema,
        annotations: tool.annotations,
      },
      async (args, extra) => {
        try {
          const progressToken = extra._meta?.progressToken;
          const progress =
            progressToken !== undefined && progressToken !== null
              ? async (current: number, total?: number, message?: string) => {
                  await extra.sendNotification({
                    method: "notifications/progress",
                    params: {
                      progressToken,
                      progress: current,
                      ...(total !== undefined ? { total } : {}),
                      ...(message !== undefined ? { message } : {}),
                    },
                  });
                }
              : undefined;
          const ret = await tool.handler(
            args as Record<string, unknown>,
            progress ? { progress } : {},
          );
          const text = typeof ret === "string" ? ret : ret.text;
          const structured = typeof ret === "string" ? undefined : ret.structured;
          return {
            content: [{ type: "text" as const, text: truncate(text) }],
            ...(structured ? { structuredContent: structured } : {}),
          };
        } catch (error: unknown) {
          const message = handleApiError(error, tool.name);
          return {
            content: [{ type: "text" as const, text: message }],
            isError: true,
          };
        }
      },
    );
  }

  return server;
}
