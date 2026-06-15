import { z } from "zod";
import { geminiSearch, DEFAULT_SEARCH_MODEL } from "../services/gemini_client.js";
import { isAbnormalEmpty, buildEmptyResponseWarnings } from "../utils/diagnostics.js";
import type { ToolResult } from "./registry.js";
import {
  booleanLike,
  numberLike,
  pinnedDefaultDescription,
  resolveServiceTier,
  serviceTierSchema,
} from "../schemas/helpers.js";

// spec 019: gemini_search specializes in formatting search grounding, so it does not expose `thinking_level` in its schema.
// Internally, thinking is suppressed with thinkingBudget=0, prioritizing lower cost and latency.
export const googleSearchSchema = z.object({
  query: z.string().min(1),
  limit: numberLike.pipe(z.number().int().min(0)).optional().default(0),
  raw: booleanLike.optional().default(false),
  model: z
    .string()
    .optional()
    .default(DEFAULT_SEARCH_MODEL)
    .describe(pinnedDefaultDescription("gemini_search", DEFAULT_SEARCH_MODEL)),
  service_tier: serviceTierSchema,
}).strict();

export type GoogleSearchArgs = z.infer<typeof googleSearchSchema>;

export async function handleGoogleSearch(args: GoogleSearchArgs): Promise<string | ToolResult> {
  // thinking_level cannot be passed by the caller. Pass undefined to preserve the internal thinkingBudget=0 suppression behavior.
  const { text, diagnostics } = await geminiSearch(
    args.query,
    args.limit,
    args.raw,
    resolveServiceTier(args.service_tier),
    args.model,
    undefined,
  );

  // spec 020: When the response is abnormally empty, attach warnings to surface the cause.
  if (diagnostics && isAbnormalEmpty(text, diagnostics)) {
    return {
      text,
      structured: { text, warnings: buildEmptyResponseWarnings(diagnostics) },
    };
  }
  return text;
}
