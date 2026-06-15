import { describe, expect, it } from "vitest";
import {
  extractResponseDiagnostics,
  isAbnormalEmpty,
  buildEmptyResponseWarnings,
  type ResponseDiagnostics,
} from "../../src/utils/diagnostics.js";

describe("extractResponseDiagnostics", () => {
  it("normal (STOP): finishReason=STOP, candidateCount=1, normalized safetyRatings", () => {
    const d = extractResponseDiagnostics({
      candidates: [
        {
          finishReason: "STOP",
          safetyRatings: [{ category: "HARM_CATEGORY_HARASSMENT", probability: "NEGLIGIBLE" }],
        },
      ],
      usageMetadata: { thoughtsTokenCount: 0 },
    });
    expect(d.finishReason).toBe("STOP");
    expect(d.candidateCount).toBe(1);
    expect(d.blockReason).toBeUndefined();
    expect(d.safetyRatings).toEqual([
      { category: "HARM_CATEGORY_HARASSMENT", probability: "NEGLIGIBLE" },
    ]);
    expect(d.thoughtsTokenCount).toBe(0);
  });

  it("safety (candidate side): extracts finishReason=SAFETY and candidate-side safetyRatings", () => {
    const d = extractResponseDiagnostics({
      candidates: [
        {
          finishReason: "SAFETY",
          safetyRatings: [{ category: "HARM_CATEGORY_DANGEROUS_CONTENT", probability: "HIGH" }],
        },
      ],
    });
    expect(d.finishReason).toBe("SAFETY");
    expect(d.candidateCount).toBe(1);
    expect(d.safetyRatings?.[0]?.probability).toBe("HIGH");
  });

  it("safety (prompt side): blockReason and candidateCount=0 when candidates=[]", () => {
    const d = extractResponseDiagnostics({
      candidates: [],
      promptFeedback: {
        blockReason: "SAFETY",
        safetyRatings: [{ category: "HARM_CATEGORY_HATE_SPEECH", probability: "MEDIUM" }],
      },
    });
    expect(d.candidateCount).toBe(0);
    expect(d.blockReason).toBe("SAFETY");
    expect(d.finishReason).toBeUndefined();
    // No candidate side, so prompt-side safetyRatings is used
    expect(d.safetyRatings?.[0]?.category).toBe("HARM_CATEGORY_HATE_SPEECH");
  });

  it("MAX_TOKENS: thinking-exhaustion signal (thoughtsTokenCount recorded)", () => {
    const d = extractResponseDiagnostics({
      candidates: [{ finishReason: "MAX_TOKENS" }],
      usageMetadata: { thoughtsTokenCount: 24000 },
    });
    expect(d.finishReason).toBe("MAX_TOKENS");
    expect(d.thoughtsTokenCount).toBe(24000);
  });

  it("RECITATION: finishReason=RECITATION", () => {
    const d = extractResponseDiagnostics({
      candidates: [{ finishReason: "RECITATION" }],
    });
    expect(d.finishReason).toBe("RECITATION");
  });

  it("missing-field tolerance: no exception when all undefined, all optionals undefined", () => {
    const d = extractResponseDiagnostics({});
    expect(d.finishReason).toBeUndefined();
    expect(d.blockReason).toBeUndefined();
    expect(d.safetyRatings).toBeUndefined();
    expect(d.candidateCount).toBe(0);
    expect(d.thoughtsTokenCount).toBeUndefined();
  });

  it("normalizes an empty safetyRatings array to undefined", () => {
    const d = extractResponseDiagnostics({
      candidates: [{ finishReason: "STOP", safetyRatings: [] }],
      promptFeedback: { safetyRatings: [] },
    });
    expect(d.safetyRatings).toBeUndefined();
  });

  it("additionally extracts finishMessage", () => {
    const d = extractResponseDiagnostics({
      candidates: [{ finishReason: "OTHER", finishMessage: "stopped for other reason" }],
    });
    expect(d.finishMessage).toBe("stopped for other reason");
  });
});

describe("isAbnormalEmpty", () => {
  const base: ResponseDiagnostics = { candidateCount: 1 };

  it("always false when there is body text (even with an abnormal finish reason)", () => {
    expect(isAbnormalEmpty("hello", { ...base, finishReason: "SAFETY" })).toBe(false);
  });

  it("whitespace-only counts as empty + blockReason present -> true", () => {
    expect(isAbnormalEmpty("   \n\t ", { ...base, candidateCount: 0, blockReason: "SAFETY" })).toBe(true);
  });

  it("empty + candidateCount=0 -> true", () => {
    expect(isAbnormalEmpty("", { candidateCount: 0 })).toBe(true);
  });

  it("empty + finishReason!=STOP -> true", () => {
    expect(isAbnormalEmpty("", { ...base, finishReason: "MAX_TOKENS" })).toBe(true);
  });

  it("empty + finishReason=STOP -> false (normal-completion empty)", () => {
    expect(isAbnormalEmpty("", { ...base, finishReason: "STOP" })).toBe(false);
  });

  it("empty + finishReason=undefined (unobtainable) -> false (avoid false positives)", () => {
    expect(isAbnormalEmpty("", { ...base })).toBe(false);
  });
});

describe("buildEmptyResponseWarnings", () => {
  it("returns a warning message containing finishReason", () => {
    const w = buildEmptyResponseWarnings({ candidateCount: 1, finishReason: "SAFETY" });
    expect(w.length).toBeGreaterThan(0);
    expect(w.join(" ")).toContain("SAFETY");
  });

  it("returns a warning message containing blockReason", () => {
    const w = buildEmptyResponseWarnings({ candidateCount: 0, blockReason: "PROHIBITED_CONTENT" });
    expect(w.join(" ")).toContain("PROHIBITED_CONTENT");
  });

  it("returns an extra exhaustion warning for MAX_TOKENS + thinking-token consumption", () => {
    const w = buildEmptyResponseWarnings({
      candidateCount: 1,
      finishReason: "MAX_TOKENS",
      thoughtsTokenCount: 24000,
    });
    const joined = w.join(" ");
    expect(joined).toContain("MAX_TOKENS");
    expect(joined).toContain("24000");
    expect(joined).toContain("thinking");
  });

  it("includes safetyRatings and finishMessage in the warning", () => {
    const w = buildEmptyResponseWarnings({
      candidateCount: 1,
      finishReason: "SAFETY",
      safetyRatings: [{ category: "HARM_CATEGORY_HARASSMENT", probability: "HIGH" }],
      finishMessage: "blocked by safety",
    });
    const joined = w.join(" ");
    expect(joined).toContain("HARM_CATEGORY_HARASSMENT:HIGH");
    expect(joined).toContain("blocked by safety");
  });

  it("returns a warning even when the cause is unknown (no finishReason/blockReason)", () => {
    const w = buildEmptyResponseWarnings({ candidateCount: 1 });
    expect(w.join(" ")).toContain("unknown cause");
  });
});
