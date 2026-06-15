import { z } from "zod";
import { executeCode, DEFAULT_CODE_MODEL, DEFAULT_CODE_THINKING_LEVEL } from "../services/gemini_client.js";
import { isAbnormalEmpty, buildEmptyResponseWarnings } from "../utils/diagnostics.js";
import {
  booleanLike,
  pinnedDefaultDescription,
  pinnedThinkingDescription,
  resolveServiceTier,
  serviceTierSchema,
} from "../schemas/helpers.js";

export const executeCodeSchema = z.object({
  prompt: z.string().min(1),
  model: z
    .string()
    .optional()
    .default(DEFAULT_CODE_MODEL)
    .describe(pinnedDefaultDescription("gemini_execute_code", DEFAULT_CODE_MODEL)),
  thinking_level: z
    .enum(["minimal", "low", "medium", "high"])
    .optional()
    .default(DEFAULT_CODE_THINKING_LEVEL)
    .describe(pinnedThinkingDescription("gemini_execute_code", DEFAULT_CODE_THINKING_LEVEL)),
  return_code: booleanLike.optional().default(false),
  service_tier: serviceTierSchema,
}).strict();

export type ExecuteCodeArgs = z.infer<typeof executeCodeSchema>;

export async function handleExecuteCode(args: ExecuteCodeArgs): Promise<string> {
  const { text, code, output, diagnostics } = await executeCode(
    args.prompt,
    args.model,
    args.return_code,
    resolveServiceTier(args.service_tier),
    args.thinking_level,
  );

  // spec 020: When the response is abnormally empty, attach warnings to surface the cause (the diagnostics themselves are not included in the output).
  const warnings = diagnostics && isAbnormalEmpty(text, diagnostics)
    ? buildEmptyResponseWarnings(diagnostics)
    : undefined;

  const payload: { text: string; code?: string; output?: string; warnings?: string[] } = { text };
  if (code !== undefined) payload.code = code;
  if (output !== undefined) payload.output = output;
  if (warnings) payload.warnings = warnings;

  return JSON.stringify(payload, null, 2);
}
