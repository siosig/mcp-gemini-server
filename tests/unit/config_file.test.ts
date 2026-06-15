import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadConfigFileIntoEnv, resolveConfigPath } from "../../src/utils/config_file.js";

let dir: string;
const TOUCHED = [
  "GEMINI_MCP_CONFIG",
  "GEMINI_API_KEY",
  "LOG_LEVEL",
  "GEMINI_TIMEOUT",
] as const;
let saved: Record<string, string | undefined>;

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "gemini-cfg-"));
  saved = {};
  for (const k of TOUCHED) {
    saved[k] = process.env[k];
    delete process.env[k];
  }
});

afterEach(() => {
  for (const k of TOUCHED) {
    if (saved[k] === undefined) delete process.env[k];
    else process.env[k] = saved[k];
  }
  rmSync(dir, { recursive: true, force: true });
  vi.restoreAllMocks();
});

function writeConfig(obj: unknown): string {
  const p = join(dir, "config.json");
  writeFileSync(p, typeof obj === "string" ? obj : JSON.stringify(obj), "utf8");
  process.env.GEMINI_MCP_CONFIG = p;
  return p;
}

describe("resolveConfigPath", () => {
  it("uses $GEMINI_MCP_CONFIG when set", () => {
    process.env.GEMINI_MCP_CONFIG = "/custom/path.json";
    expect(resolveConfigPath()).toBe("/custom/path.json");
  });

  it("falls back to ~/.gemini-mcp.json when override is empty", () => {
    process.env.GEMINI_MCP_CONFIG = "";
    expect(resolveConfigPath().endsWith(".gemini-mcp.json")).toBe(true);
  });
});

describe("loadConfigFileIntoEnv", () => {
  it("1. file absent → process.env unchanged, no throw", () => {
    process.env.GEMINI_MCP_CONFIG = join(dir, "does-not-exist.json");
    expect(() => loadConfigFileIntoEnv()).not.toThrow();
    expect(process.env.GEMINI_API_KEY).toBeUndefined();
  });

  it("2. valid JSON + env unset → env is populated", () => {
    writeConfig({ GEMINI_API_KEY: "from-file", LOG_LEVEL: "debug" });
    loadConfigFileIntoEnv();
    expect(process.env.GEMINI_API_KEY).toBe("from-file");
    expect(process.env.LOG_LEVEL).toBe("debug");
  });

  it("3. real env wins over config file (not overwritten)", () => {
    process.env.GEMINI_API_KEY = "from-env";
    writeConfig({ GEMINI_API_KEY: "from-file" });
    loadConfigFileIntoEnv();
    expect(process.env.GEMINI_API_KEY).toBe("from-env");
  });

  it("3b. empty-string env is treated as unset and gets filled", () => {
    process.env.GEMINI_API_KEY = "";
    writeConfig({ GEMINI_API_KEY: "from-file" });
    loadConfigFileIntoEnv();
    expect(process.env.GEMINI_API_KEY).toBe("from-file");
  });

  it("4. malformed JSON → warns and continues, env unchanged", () => {
    const warn = vi.spyOn(process.stderr, "write").mockReturnValue(true);
    writeConfig("{ not valid json ");
    expect(() => loadConfigFileIntoEnv()).not.toThrow();
    expect(process.env.GEMINI_API_KEY).toBeUndefined();
    expect(warn).toHaveBeenCalled();
  });

  it("5. GEMINI_MCP_CONFIG path override is honored", () => {
    const p = writeConfig({ GEMINI_API_KEY: "via-override" });
    expect(resolveConfigPath()).toBe(p);
    loadConfigFileIntoEnv();
    expect(process.env.GEMINI_API_KEY).toBe("via-override");
  });

  it("6. non-string values are skipped (with warning)", () => {
    const warn = vi.spyOn(process.stderr, "write").mockReturnValue(true);
    writeConfig({ GEMINI_API_KEY: "ok", GEMINI_TIMEOUT: 360 });
    loadConfigFileIntoEnv();
    expect(process.env.GEMINI_API_KEY).toBe("ok");
    expect(process.env.GEMINI_TIMEOUT).toBeUndefined();
    expect(warn).toHaveBeenCalled();
  });

  it("non-object JSON (array) → ignored with warning", () => {
    const warn = vi.spyOn(process.stderr, "write").mockReturnValue(true);
    writeConfig(["a", "b"]);
    expect(() => loadConfigFileIntoEnv()).not.toThrow();
    expect(warn).toHaveBeenCalled();
  });
});
