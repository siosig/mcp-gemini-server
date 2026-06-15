/**
 * Config-file fallback for environment variables.
 *
 * Some MCP clients (notably the Claude Code VS Code extension) do not propagate
 * the `env` block of `.mcp.json` / settings.json to the spawned MCP process.
 * To keep the plugin usable in those environments, the server can read
 * configuration (most importantly GEMINI_API_KEY) from a JSON file in the
 * user's home directory and use it to fill in *unset* environment variables.
 *
 * Precedence: real process environment variables ALWAYS win over the file.
 * The file only supplies keys that are currently undefined or empty.
 *
 * This must run BEFORE validateEnv() so that Fail-Fast Zod validation sees the
 * fully-resolved environment.
 */

import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

/** Resolve the config-file path: $GEMINI_MCP_CONFIG, else ~/.gemini-mcp.json. */
export function resolveConfigPath(): string {
  const override = process.env.GEMINI_MCP_CONFIG;
  if (override && override.trim() !== "") {
    return override;
  }
  return join(homedir(), ".gemini-mcp.json");
}

/**
 * Load `~/.gemini-mcp.json` (or $GEMINI_MCP_CONFIG) and populate any environment
 * variables that are not already set. Idempotent and best-effort:
 *
 * - File missing            → no-op.
 * - Read / JSON parse error  → warn to stderr and continue (file is optional).
 * - Value is a string and the env var is unset/empty → set it.
 * - Value is a string and the env var is already non-empty → skip (env wins).
 * - Value is not a string    → skip with a warning.
 */
export function loadConfigFileIntoEnv(): void {
  const path = resolveConfigPath();

  let raw: string;
  try {
    raw = readFileSync(path, "utf8");
  } catch (err) {
    // ENOENT (file absent) is the normal case — say nothing.
    if ((err as NodeJS.ErrnoException)?.code !== "ENOENT") {
      process.stderr.write(
        `[gemini-mcp-server] Could not read config file ${path}: ${(err as Error).message}\n`,
      );
    }
    return;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    process.stderr.write(
      `[gemini-mcp-server] Ignoring malformed config file ${path}: ${(err as Error).message}\n`,
    );
    return;
  }

  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    process.stderr.write(
      `[gemini-mcp-server] Ignoring config file ${path}: expected a JSON object\n`,
    );
    return;
  }

  for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
    if (typeof value !== "string") {
      process.stderr.write(
        `[gemini-mcp-server] Skipping config key "${key}": value is not a string\n`,
      );
      continue;
    }
    const current = process.env[key];
    if (current === undefined || current === "") {
      process.env[key] = value;
    }
    // else: a real environment variable is present — it takes precedence.
  }
}
