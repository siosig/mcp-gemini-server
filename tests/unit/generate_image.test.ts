/**
 * Unit tests for the gemini_generate_image tool (spec 019: Nano Banana 2 / Flash Image path).
 * - Zod schema validation (required fields, enums, boundary values)
 * - Filename collision avoidance
 * - Output directory resolution priority
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

// Mock gemini_client (to avoid API calls).
// generate_image.ts imports DEFAULT_IMAGE_MODEL / DEFAULT_IMAGE_THINKING_LEVEL / generateImageContent,
// so all of them must be exported from the mock.
vi.mock("../../src/services/gemini_client.js", () => ({
  DEFAULT_IMAGE_MODEL: "gemini-3.1-flash-image-preview",
  DEFAULT_IMAGE_THINKING_LEVEL: "medium",
  generateImageContent: vi.fn(async () => ({
    imageBytes: Buffer.from("fake-png").toString("base64"),
    mimeType: "image/png",
    text: "",
  })),
}));

import {
  generateImageSchema,
  resolveOutputDir,
  pickUniqueFilename,
} from "../../src/tools/generate_image.js";

describe("generateImageSchema", () => {
  describe("prompt (US1)", () => {
    it("requires prompt and rejects an empty string", () => {
      expect(() => generateImageSchema.parse({})).toThrow();
      expect(() => generateImageSchema.parse({ prompt: "" })).toThrow();
    });

    it("applies spec 019 fixed default values with only prompt", () => {
      const parsed = generateImageSchema.parse({ prompt: "Robot" });
      expect(parsed.prompt).toBe("Robot");
      expect(parsed.model).toBe("gemini-3.1-flash-image-preview");
      expect(parsed.aspect_ratio).toBe("1:1");
      expect(parsed.image_size).toBe("1K");
      expect(parsed.thinking_level).toBe("medium");
      expect(parsed.file_prefix).toBe("imagen");
    });
  });

  describe("model (US1: fixed enum)", () => {
    it("rejects old Imagen model names, the old Pro name, and arbitrary strings", () => {
      for (const invalid of [
        "imagen-4.0-generate-001",
        "imagen-4.0-ultra-generate-001",
        "gemini-3-pro-image-preview",
        "gemini-3.5-flash",
        "foo",
      ]) {
        const result = generateImageSchema.safeParse({
          prompt: "p",
          model: invalid,
        });
        expect(result.success).toBe(false);
        if (!result.success) {
          const msg = JSON.stringify(result.error.issues);
          expect(msg).toContain("gemini-3.1-flash-image-preview");
        }
      }
    });

    it("accepts the only allowed value gemini-3.1-flash-image-preview", () => {
      const r = generateImageSchema.safeParse({
        prompt: "p",
        model: "gemini-3.1-flash-image-preview",
      });
      expect(r.success).toBe(true);
    });
  });

  describe("aspect_ratio (US1: 14 values)", () => {
    it("rejects aspect_ratio values outside the enum", () => {
      for (const invalid of ["2:1", "1:2", "16:10", "6:5"]) {
        const r = generateImageSchema.safeParse({
          prompt: "p",
          aspect_ratio: invalid,
        });
        expect(r.success).toBe(false);
      }
    });

    it("accepts all 14 allowed values", () => {
      for (const ar of [
        "1:1",
        "2:3",
        "3:2",
        "3:4",
        "4:3",
        "4:5",
        "5:4",
        "9:16",
        "16:9",
        "21:9",
        "1:4",
        "4:1",
        "1:8",
        "8:1",
      ]) {
        const r = generateImageSchema.safeParse({
          prompt: "p",
          aspect_ratio: ar,
        });
        expect(r.success).toBe(true);
      }
    });
  });

  describe("image_size (US1: 4 Flash-spec values)", () => {
    it("rejects image_size values outside the enum", () => {
      for (const invalid of ["8K", "HD", "3K", "0.25K"]) {
        const r = generateImageSchema.safeParse({
          prompt: "p",
          image_size: invalid,
        });
        expect(r.success).toBe(false);
      }
    });

    it("accepts the 4 allowed values (0.5K/1K/2K/4K)", () => {
      for (const size of ["0.5K", "1K", "2K", "4K"]) {
        const r = generateImageSchema.safeParse({
          prompt: "p",
          image_size: size,
        });
        expect(r.success).toBe(true);
      }
    });
  });

  describe("thinking_level (US1: fixed medium)", () => {
    it("rejects thinking_level values outside the enum", () => {
      const r = generateImageSchema.safeParse({
        prompt: "p",
        thinking_level: "extra",
      });
      expect(r.success).toBe(false);
    });

    it("accepts the 4 allowed values (minimal/low/medium/high)", () => {
      for (const level of ["minimal", "low", "medium", "high"]) {
        const r = generateImageSchema.safeParse({
          prompt: "p",
          thinking_level: level,
        });
        expect(r.success).toBe(true);
      }
    });
  });

  describe("rejection of removed parameters (spec 019 Breaking Changes)", () => {
    it("rejects number_of_images via the strict schema", () => {
      const r = generateImageSchema.safeParse({
        prompt: "p",
        number_of_images: 2,
      });
      expect(r.success).toBe(false);
    });

    it("rejects person_generation via the strict schema", () => {
      const r = generateImageSchema.safeParse({
        prompt: "p",
        person_generation: "allow_adult",
      });
      expect(r.success).toBe(false);
    });
  });

  describe("file_prefix", () => {
    it.each(["../etc", "foo bar", ""])("rejects unsafe %s", (v) => {
      const r = generateImageSchema.safeParse({ prompt: "p", file_prefix: v });
      expect(r.success).toBe(false);
    });

    it.each(["demo", "img_01", "my-prefix"])("accepts safe %s", (v) => {
      const r = generateImageSchema.safeParse({ prompt: "p", file_prefix: v });
      expect(r.success).toBe(true);
    });
  });
});

describe("pickUniqueFilename", () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "genimg-test-"));
  });

  afterEach(async () => {
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  it("returns <prefix>-<index>.png in an empty directory", async () => {
    const p = await pickUniqueFilename(tmpDir, "demo", 1);
    expect(p).toBe(path.join(tmpDir, "demo-1.png"));
  });

  it("returns <prefix>-<index>-<8hex>.png when a file with the same name exists", async () => {
    const existing = path.join(tmpDir, "demo-1.png");
    await fs.writeFile(existing, "x");
    const p = await pickUniqueFilename(tmpDir, "demo", 1);
    expect(p).not.toBe(existing);
    expect(path.basename(p)).toMatch(/^demo-1-[0-9a-f]{8}\.png$/);
  });
});

describe("resolveOutputDir", () => {
  const origEnv = process.env.IMAGEN_OUTPUT_DIR;
  let tmpRoot: string;

  beforeEach(async () => {
    tmpRoot = await fs.mkdtemp(path.join(os.tmpdir(), "genimg-resolve-"));
  });

  afterEach(async () => {
    if (origEnv === undefined) {
      delete process.env.IMAGEN_OUTPUT_DIR;
    } else {
      process.env.IMAGEN_OUTPUT_DIR = origEnv;
    }
    await fs.rm(tmpRoot, { recursive: true, force: true });
  });

  it("resolves with priority argument > env > tmpdir", async () => {
    const argDir = path.join(tmpRoot, "from-arg");
    const envDir = path.join(tmpRoot, "from-env");
    process.env.IMAGEN_OUTPUT_DIR = envDir;

    expect(await resolveOutputDir(argDir)).toBe(argDir);
    expect(await resolveOutputDir()).toBe(envDir);

    delete process.env.IMAGEN_OUTPUT_DIR;
    const def = await resolveOutputDir();
    expect(def).toBe(path.join(os.tmpdir(), "mcp-gemini", "imagen"));
  });

  it("creates a non-existent directory (equivalent to mkdir -p)", async () => {
    const target = path.join(tmpRoot, "deep", "nest", "dir");
    await resolveOutputDir(target);
    const stat = await fs.stat(target);
    expect(stat.isDirectory()).toBe(true);
  });
});
