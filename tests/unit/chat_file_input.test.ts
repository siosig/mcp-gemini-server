/**
 * Unit tests for file_path parameter support in gemini_chat and gemini_custom_agent tools.
 * Covers schema validation and backward-compatibility guarantees (spec 027).
 */

import { describe, it, expect } from "vitest";
import { geminiChatSchema } from "../../src/tools/gemini_chat.js";
import { customAgentSchema } from "../../src/tools/custom_agent.js";

describe("geminiChatSchema — file_path (spec 027)", () => {
  it("accepts prompt without file_path (backward compat)", () => {
    const r = geminiChatSchema.safeParse({ prompt: "hello" });
    expect(r.success).toBe(true);
    if (r.success) expect(r.data.file_path).toBeUndefined();
  });

  it("accepts prompt with a valid file_path string", () => {
    const r = geminiChatSchema.safeParse({ prompt: "review", file_path: "/tmp/file.ts" });
    expect(r.success).toBe(true);
    if (r.success) expect(r.data.file_path).toBe("/tmp/file.ts");
  });

  it("rejects unknown fields (strict schema still enforced)", () => {
    const r = geminiChatSchema.safeParse({ prompt: "hi", unknown_field: "x" });
    expect(r.success).toBe(false);
  });

  it("rejects non-string file_path values", () => {
    const r = geminiChatSchema.safeParse({ prompt: "hi", file_path: 42 });
    expect(r.success).toBe(false);
  });
});

describe("customAgentSchema — file_path (spec 027)", () => {
  it("accepts task+role without file_path (backward compat)", () => {
    const r = customAgentSchema.safeParse({ task: "review this", role: "reviewer" });
    expect(r.success).toBe(true);
    if (r.success) expect(r.data.file_path).toBeUndefined();
  });

  it("accepts task+role with a valid file_path string", () => {
    const r = customAgentSchema.safeParse({
      task: "find bugs",
      role: "developer",
      file_path: "/home/user/src/main.py",
    });
    expect(r.success).toBe(true);
    if (r.success) expect(r.data.file_path).toBe("/home/user/src/main.py");
  });

  it("rejects unknown fields (strict schema still enforced)", () => {
    const r = customAgentSchema.safeParse({ task: "t", role: "r", unknown_field: "x" });
    expect(r.success).toBe(false);
  });

  it("rejects non-string file_path values", () => {
    const r = customAgentSchema.safeParse({ task: "t", role: "r", file_path: true });
    expect(r.success).toBe(false);
  });
});
