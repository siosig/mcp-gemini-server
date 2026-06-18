/**
 * spec 019: Verifies that each MCP tool's input schema parses with its fixed
 * default values (schema-level verification of SC-001).
 *
 * Note: mocking the DEFAULT_* constants of gemini_client.js with vi.mock would
 * prevent verification, since each tool schema uses the fixed values at import time.
 * This test confirms that the real module's DEFAULT_* values match the spec 019 spec.
 */

import { describe, expect, it } from "vitest";

// Import tool schemas (no mocks, using the real DEFAULT_* values)
import { geminiChatSchema } from "../../src/tools/gemini_chat.js";
import { googleSearchSchema } from "../../src/tools/google_search.js";
import { customAgentSchema } from "../../src/tools/custom_agent.js";
import { analyzeMediaSchema } from "../../src/tools/analyze_media.js";
import { executeCodeSchema } from "../../src/tools/execute_code.js";
import { manageFilesSchema } from "../../src/tools/manage_files.js";
import { teamSchema } from "../../src/tools/team.js";

describe("Tool default profile (spec 019)", () => {
  it("gemini_chat: model=gemini-3.5-flash, thinking_level=medium", () => {
    const r = geminiChatSchema.parse({ prompt: "x" });
    expect(r.model).toBe("gemini-3.5-flash");
    expect(r.thinking_level).toBe("medium");
  });

  it("gemini_search: model=gemini-3.1-flash-lite, thinking_level not present in schema", () => {
    const r = googleSearchSchema.parse({ query: "x" });
    expect(r.model).toBe("gemini-3.1-flash-lite");
    // strict schema: passing thinking_level errors
    expect(googleSearchSchema.safeParse({ query: "x", thinking_level: "medium" }).success).toBe(false);
  });

  it("gemini_custom_agent: model=gemini-3.5-flash, thinking_level=high", () => {
    const r = customAgentSchema.parse({ task: "x", role: "developer" });
    expect(r.model).toBe("gemini-3.5-flash");
    expect(r.thinking_level).toBe("high");
  });

  it("gemini_analyze_media: model=gemini-3.1-flash-lite, thinking_level=medium", () => {
    const r = analyzeMediaSchema.parse({ prompt: "x", file_path: "/tmp/x.png" });
    expect(r.model).toBe("gemini-3.1-flash-lite");
    expect(r.thinking_level).toBe("medium");
  });

  it("gemini_execute_code: model=gemini-3.1-flash-lite, thinking_level=low", () => {
    const r = executeCodeSchema.parse({ prompt: "x" });
    expect(r.model).toBe("gemini-3.1-flash-lite");
    expect(r.thinking_level).toBe("low");
  });

  it("gemini_manage_files: model / thinking_level not present in schema", () => {
    const r = manageFilesSchema.parse({ action: "list" });
    // neither model nor thinking_level is in the type (guaranteed at type level)
    expect((r as Record<string, unknown>).model).toBeUndefined();
    expect((r as Record<string, unknown>).thinking_level).toBeUndefined();
    // strict schema: passing either one errors
    expect(manageFilesSchema.safeParse({ action: "list", model: "x" }).success).toBe(false);
    expect(manageFilesSchema.safeParse({ action: "list", thinking_level: "medium" }).success).toBe(false);
  });

  it("gemini_team: model=gemini-3.5-flash, thinking_level=high", () => {
    const r = teamSchema.parse({ task: "x", mode: "mul" });
    expect(r.model).toBe("gemini-3.5-flash");
    expect(r.thinking_level).toBe("high");
  });
});

describe("Schema description guard text (spec 019 SC-002)", () => {
  it.each([
    ["gemini_chat", "model", () => (geminiChatSchema as any).shape.model.description],
    ["gemini_chat", "thinking_level", () => (geminiChatSchema as any).shape.thinking_level.description],
    ["gemini_search", "model", () => (googleSearchSchema as any).shape.model.description],
    ["gemini_custom_agent", "model", () => (customAgentSchema as any).shape.model.description],
    ["gemini_custom_agent", "thinking_level", () => (customAgentSchema as any).shape.thinking_level.description],
    ["gemini_analyze_media", "model", () => (analyzeMediaSchema as any).shape.model.description],
    ["gemini_analyze_media", "thinking_level", () => (analyzeMediaSchema as any).shape.thinking_level.description],
    ["gemini_execute_code", "model", () => (executeCodeSchema as any).shape.model.description],
    ["gemini_execute_code", "thinking_level", () => (executeCodeSchema as any).shape.thinking_level.description],
  ])("%s.%s description has the [DEFAULT FIXED] prefix", (_tool, _field, getDesc) => {
    const desc = getDesc();
    expect(desc).toBeDefined();
    expect(desc).toContain("[DEFAULT FIXED]");
  });
});
