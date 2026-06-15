import { describe, expect, it } from "vitest";
import { ThinkingLevel as SdkThinkingLevel } from "@google/genai";
import { buildThinkingConfig, isGemini3 } from "../../src/services/gemini_client.js";

describe("isGemini3", () => {
  it.each([
    // New default models fixed in spec 019
    ["gemini-3.5-flash", true],
    ["gemini-3.1-flash-lite", true],
    ["gemini-3-pro-image-preview", true],
    ["gemini-3.1-flash-image-preview", true],
    // Existing preview models
    ["gemini-3.1-pro-preview", true],
    ["gemini-3-flash-preview", true],
    ["gemini-3.1-flash-lite-preview", true],
    // 2.5 series, 1.5 series, empty string
    ["gemini-2.5-pro", false],
    ["gemini-2.5-flash", false],
    ["gemini-2.5-flash-lite", false],
    ["gemini-1.5-pro", false],
    ["", false],
  ])("isGemini3(%s) === %s", (model, expected) => {
    expect(isGemini3(model)).toBe(expected);
  });
});

describe("buildThinkingConfig", () => {
  describe("Gemini 3.x series models", () => {
    it("returns thinkingLevel (SDK enum) when level is specified", async () => {
      expect(await buildThinkingConfig("gemini-3.1-pro-preview", "high")).toEqual({
        thinkingLevel: SdkThinkingLevel.HIGH,
      });
      expect(await buildThinkingConfig("gemini-3-flash-preview", "minimal")).toEqual({
        thinkingLevel: SdkThinkingLevel.MINIMAL,
      });
      expect(await buildThinkingConfig("gemini-3.1-pro-preview", "low")).toEqual({
        thinkingLevel: SdkThinkingLevel.LOW,
      });
      expect(await buildThinkingConfig("gemini-3.1-pro-preview", "medium")).toEqual({
        thinkingLevel: SdkThinkingLevel.MEDIUM,
      });
    });

    it("returns thinkingLevel correctly for the spec 019 new default models too", async () => {
      expect(await buildThinkingConfig("gemini-3.5-flash", "medium")).toEqual({
        thinkingLevel: SdkThinkingLevel.MEDIUM,
      });
      expect(await buildThinkingConfig("gemini-3.1-flash-lite", "low")).toEqual({
        thinkingLevel: SdkThinkingLevel.LOW,
      });
      expect(await buildThinkingConfig("gemini-3-pro-image-preview", "medium")).toEqual({
        thinkingLevel: SdkThinkingLevel.MEDIUM,
      });
      expect(await buildThinkingConfig("gemini-3.1-flash-image-preview", "medium")).toEqual({
        thinkingLevel: SdkThinkingLevel.MEDIUM,
      });
      expect(await buildThinkingConfig("gemini-3.5-flash", "high")).toEqual({
        thinkingLevel: SdkThinkingLevel.HIGH,
      });
    });

    it("level=null (suppress thinking) returns undefined (defer to API default)", async () => {
      // gemini-3.1-pro-preview etc. reject MINIMAL by spec, so send nothing for null
      expect(await buildThinkingConfig("gemini-3.1-pro-preview", null)).toBeUndefined();
      expect(await buildThinkingConfig("gemini-3-flash-preview", null)).toBeUndefined();
    });
  });

  describe("Gemini 2.5 series models", () => {
    it("returns thinkingBudget (number) when level is specified", async () => {
      expect(await buildThinkingConfig("gemini-2.5-pro", "high")).toEqual({ thinkingBudget: 24576 });
      expect(await buildThinkingConfig("gemini-2.5-flash", "medium")).toEqual({ thinkingBudget: 8192 });
      expect(await buildThinkingConfig("gemini-2.5-flash-lite", "low")).toEqual({ thinkingBudget: 1024 });
      // minimal is 512 to match the flash-lite lower bound (512-24576)
      expect(await buildThinkingConfig("gemini-2.5-pro", "minimal")).toEqual({ thinkingBudget: 512 });
    });

    it("returns thinkingBudget: 0 for level=null (suppress thinking)", async () => {
      expect(await buildThinkingConfig("gemini-2.5-flash-lite", null)).toEqual({ thinkingBudget: 0 });
    });
  });

  describe("level=undefined", () => {
    it("returns undefined for both Gemini 3 and 2.5 series (defer to API default)", async () => {
      expect(await buildThinkingConfig("gemini-3.1-pro-preview", undefined)).toBeUndefined();
      expect(await buildThinkingConfig("gemini-2.5-pro", undefined)).toBeUndefined();
    });
  });
});
