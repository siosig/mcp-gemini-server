import { describe, it, expect } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";

describe("smoke test", () => {
  it("entry point exists", () => {
    expect(existsSync(join(import.meta.dirname, "../src/index.ts"))).toBe(true);
  });
});
