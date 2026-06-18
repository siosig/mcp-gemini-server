/**
 * spec 024: Schema unit tests for gemini_team.
 */

import { describe, expect, it } from "vitest";
import { teamSchema } from "../../src/tools/team.js";

describe("gemini_team schema (spec 024)", () => {
  it("mode=mul: defaults applied correctly", () => {
    const r = teamSchema.parse({ task: "analyze this", mode: "mul" });
    expect(r.model).toBe("gemini-flash-latest");
    expect(r.thinking_level).toBe("high");
    expect(r.roles).toEqual(["analyst", "architect", "developer", "reviewer", "critic"]);
    expect(r.max_iterations).toBe(2);
    expect(r.intermediate_results).toBe(false);
  });

  it("mode=it: defaults applied correctly", () => {
    const r = teamSchema.parse({ task: "write a proposal", mode: "it" });
    expect(r.max_iterations).toBe(2);
    expect(r.intermediate_results).toBe(false);
  });

  it("mode=mulit: defaults applied correctly", () => {
    const r = teamSchema.parse({ task: "design this", mode: "mulit" });
    expect(r.mode).toBe("mulit");
  });

  it("strict: unknown fields are rejected", () => {
    expect(
      teamSchema.safeParse({ task: "x", mode: "mul", unknown_field: true }).success,
    ).toBe(false);
  });

  it("mode enum: invalid value rejected", () => {
    expect(teamSchema.safeParse({ task: "x", mode: "invalid" }).success).toBe(false);
  });

  it("mode enum: valid values accepted", () => {
    expect(teamSchema.safeParse({ task: "x", mode: "mul" }).success).toBe(true);
    expect(teamSchema.safeParse({ task: "x", mode: "it" }).success).toBe(true);
    expect(teamSchema.safeParse({ task: "x", mode: "mulit" }).success).toBe(true);
  });

  it("max_iterations=0 is allowed", () => {
    const r = teamSchema.parse({ task: "x", mode: "it", max_iterations: 0 });
    expect(r.max_iterations).toBe(0);
  });

  it("max_iterations=10 is allowed", () => {
    const r = teamSchema.parse({ task: "x", mode: "it", max_iterations: 10 });
    expect(r.max_iterations).toBe(10);
  });

  it("max_iterations > 10 is rejected", () => {
    expect(
      teamSchema.safeParse({ task: "x", mode: "it", max_iterations: 11 }).success,
    ).toBe(false);
  });

  it("max_iterations < 0 is rejected", () => {
    expect(
      teamSchema.safeParse({ task: "x", mode: "it", max_iterations: -1 }).success,
    ).toBe(false);
  });

  it("intermediate_results accepts boolean true", () => {
    const r = teamSchema.parse({ task: "x", mode: "mul", intermediate_results: true });
    expect(r.intermediate_results).toBe(true);
  });

  it("intermediate_results accepts string 'true'", () => {
    const r = teamSchema.parse({ task: "x", mode: "mul", intermediate_results: "true" });
    expect(r.intermediate_results).toBe(true);
  });

  it("intermediate_results defaults to false", () => {
    const r = teamSchema.parse({ task: "x", mode: "mul" });
    expect(r.intermediate_results).toBe(false);
  });

  it("roles can be overridden", () => {
    const r = teamSchema.parse({
      task: "x",
      mode: "mul",
      roles: ["security_expert", "performance_engineer"],
    });
    expect(r.roles).toEqual(["security_expert", "performance_engineer"]);
  });

  it("file_paths is optional and accepts array", () => {
    const r = teamSchema.parse({
      task: "x",
      mode: "mul",
      file_paths: ["/path/to/spec.md", "/path/to/plan.md"],
    });
    expect(r.file_paths).toEqual(["/path/to/spec.md", "/path/to/plan.md"]);
  });

  it("file_paths is undefined when not provided", () => {
    const r = teamSchema.parse({ task: "x", mode: "mul" });
    expect(r.file_paths).toBeUndefined();
  });

  it("service_tier accepts valid values", () => {
    const r = teamSchema.parse({ task: "x", mode: "mul", service_tier: "flex" });
    expect(r.service_tier).toBe("flex");
  });

  it("task must not be empty", () => {
    expect(teamSchema.safeParse({ task: "", mode: "mul" }).success).toBe(false);
  });
});

describe("gemini_team schema description guards (spec 024)", () => {
  it("model description has [DEFAULT FIXED] prefix", () => {
    const desc = (teamSchema as unknown as { shape: Record<string, { description?: string }> })
      .shape["model"]?.description;
    expect(desc).toBeDefined();
    expect(desc).toContain("[DEFAULT FIXED]");
  });

  it("thinking_level description has [DEFAULT FIXED] prefix", () => {
    const desc = (teamSchema as unknown as { shape: Record<string, { description?: string }> })
      .shape["thinking_level"]?.description;
    expect(desc).toBeDefined();
    expect(desc).toContain("[DEFAULT FIXED]");
  });
});
