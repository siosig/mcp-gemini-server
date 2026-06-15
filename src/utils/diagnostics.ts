/**
 * Diagnostic metadata extraction from Gemini API responses (L1 extraction layer).
 *
 * To make "completed normally but the body is empty" cases (Safety / MAX_TOKENS / RECITATION, etc.)
 * traceable after the fact, this module provides pure functions that pull finish-reason metadata
 * out of a `GenerateContentResponse`.
 *
 * Design principles (spec 020):
 * - Do not import the SDK's `FinishReason` / `BlockedReason` enums; keep them as raw strings
 *   (so newly added SDK values across versions do not cause enumeration-gap regressions).
 * - Do not throw; fall back to undefined when metadata is missing (robustness).
 * - Do not reference fields unsupported by the Gemini API (blockReasonMessage / trafficType).
 */

/** Response diagnostic information. A string/number-based structure that survives logging and JSON serialization. */
export interface ResponseDiagnostics {
  /** candidates[0].finishReason ("STOP"|"MAX_TOKENS"|"SAFETY"|"RECITATION"|"OTHER", etc.). undefined if unavailable */
  finishReason?: string | undefined;
  /** promptFeedback.blockReason (prompt-side block; returned only when there are zero candidates). undefined if unavailable */
  blockReason?: string | undefined;
  /** Candidate or prompt safety ratings (category/probability). An empty array is normalized to undefined. */
  safetyRatings?: Array<{ category?: string; probability?: string }> | undefined;
  /** Candidate count (0 is a strong signal of a block-induced empty response). */
  candidateCount: number;
  /** Thinking tokens consumed (used to detect MAX_TOKENS exhaustion). */
  thoughtsTokenCount?: number | undefined;
  /** Human-readable finish-reason explanation (supplementary, optional). */
  finishMessage?: string | undefined;
}

/** Minimal structural subtype accepted by extractResponseDiagnostics (does not depend on SDK types). */
export interface DiagnosableResponse {
  candidates?:
    | Array<{
        finishReason?: string | undefined;
        finishMessage?: string | undefined;
        safetyRatings?: Array<{ category?: string; probability?: string }> | undefined;
      }>
    | undefined;
  promptFeedback?:
    | {
        blockReason?: string | undefined;
        safetyRatings?: Array<{ category?: string; probability?: string }> | undefined;
      }
    | undefined;
  usageMetadata?: { thoughtsTokenCount?: number | undefined } | undefined;
}

/** Normalize an empty array to undefined ([] is treated as equivalent to "none"). */
function normalizeRatings(
  ratings: Array<{ category?: string; probability?: string }> | undefined,
): Array<{ category?: string; probability?: string }> | undefined {
  if (!ratings || ratings.length === 0) return undefined;
  return ratings.map((r) => ({ category: r.category, probability: r.probability }));
}

/**
 * Pure function that extracts diagnostic metadata from a GenerateContentResponse.
 * No side effects, no exceptions. Returns a ResponseDiagnostics for any input (including all-undefined).
 */
export function extractResponseDiagnostics(response: DiagnosableResponse): ResponseDiagnostics {
  const candidates = response.candidates;
  const first = candidates?.[0];

  // Prefer the candidate-side safety ratings; fall back to the prompt-side if absent.
  const safetyRatings =
    normalizeRatings(first?.safetyRatings) ?? normalizeRatings(response.promptFeedback?.safetyRatings);

  return {
    finishReason: first?.finishReason,
    blockReason: response.promptFeedback?.blockReason,
    safetyRatings,
    candidateCount: candidates?.length ?? 0,
    thoughtsTokenCount: response.usageMetadata?.thoughtsTokenCount,
    finishMessage: first?.finishMessage,
  };
}

/**
 * Pure function that determines whether the response is an abnormal empty response,
 * i.e. the text is empty and it did not finish normally.
 *
 * - Always false if there is body text (whitespace-only is treated as "empty").
 * - If finishReason is undefined (unavailable), does not return true, to avoid false positives.
 */
export function isAbnormalEmpty(text: string, d: ResponseDiagnostics): boolean {
  if (text.trim().length > 0) return false;
  if (d.blockReason) return true;
  if (d.candidateCount === 0) return true;
  return d.finishReason !== undefined && d.finishReason !== "STOP";
}

/**
 * Shared helper that builds the warnings text for an abnormal empty response
 * (used by both the tools layer and generate_image).
 * Returns messages that include the cause (finishReason / blockReason) from the diagnostics.
 */
export function buildEmptyResponseWarnings(d: ResponseDiagnostics): string[] {
  const warnings: string[] = [];
  const reason = d.blockReason
    ? `prompt blocked (blockReason=${d.blockReason})`
    : d.finishReason
      ? `finishReason=${d.finishReason}`
      : "unknown cause";
  warnings.push(`Empty response from Gemini: ${reason}.`);

  if (d.finishReason === "MAX_TOKENS" && (d.thoughtsTokenCount ?? 0) > 0) {
    warnings.push(
      `Output truncated at MAX_TOKENS after consuming ${d.thoughtsTokenCount} thinking tokens (thinking budget likely exhausted before any answer). Consider raising max_tokens or lowering thinking_level.`,
    );
  }
  if (d.safetyRatings && d.safetyRatings.length > 0) {
    const cats = d.safetyRatings
      .map((r) => `${r.category ?? "?"}:${r.probability ?? "?"}`)
      .join(", ");
    warnings.push(`Safety ratings: ${cats}.`);
  }
  if (d.finishMessage) {
    warnings.push(`Model message: ${d.finishMessage}`);
  }
  return warnings;
}
