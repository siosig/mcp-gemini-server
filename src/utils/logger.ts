import pino from "pino";

// In stdio MCP, keep stdout exclusive to JSON-RPC, so logs are fixed to stderr.
// External log forwarding, success access logs, and pricing were removed in spec 021; only error logs are written to stderr.
export const logger = pino({
  level: process.env.LOG_LEVEL ?? "info",
  base: null, // Exclude pid and hostname to keep the JSON simple (pino removes all when set to null).
  timestamp: pino.stdTimeFunctions.epochTime,
}, pino.destination(2)); // fd 2 = stderr

export function logError(
  log_type: "mcp_error",
  tool_name: string,
  model: string,
  duration_ms: number,
  error: unknown,
  thinking_level = ""
): void {
  const error_type = error instanceof Error ? error.constructor.name : "Error";
  const error_message = error instanceof Error ? error.message : String(error);
  const entry = {
    log_type,
    tool_name,
    model,
    status: "error" as const,
    duration_ms: Math.round(duration_ms * 1000) / 1000,
    thinking_level,
    error_type,
    error_message,
  };
  logger.error(entry, `[${tool_name}] ${error_type}: ${error_message}`);
}
