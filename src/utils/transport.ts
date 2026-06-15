/**
 * Utility for resolving the transport mode from the MCP_TRANSPORT environment variable.
 * Has no logger dependency (pure function) and is therefore unit-testable.
 */

export type TransportMode = "stdio" | "uds";

/**
 * Resolve the transport mode from the value of the MCP_TRANSPORT environment variable.
 * - "stdio" → stdio mode
 * - "uds" / "http" / undefined → UDS mode (default)
 * - any other unknown value → fall back to UDS mode (the caller should emit a warning)
 */
export function resolveTransport(envValue: string | undefined): TransportMode {
  if (envValue === "stdio") return "stdio";
  return "uds";
}

/**
 * Determine whether the MCP_TRANSPORT value is unknown (anything other than "stdio" / "uds" / "http").
 * If true, the caller should emit a warning log.
 */
export function isUnknownTransport(envValue: string | undefined): boolean {
  return envValue !== undefined && envValue !== "stdio" && envValue !== "uds" && envValue !== "http";
}
