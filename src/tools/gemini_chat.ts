import { z } from "zod";
import { geminiChat, DEFAULT_MODEL, DEFAULT_THINKING_LEVEL } from "../services/gemini_client.js";
import { isAbnormalEmpty, buildEmptyResponseWarnings } from "../utils/diagnostics.js";
import type { ToolResult } from "./registry.js";
import {
  booleanLike,
  numberLike,
  pinnedDefaultDescription,
  pinnedThinkingDescription,
  resolveServiceTier,
  safetySettingsSchema,
  serviceTierSchema,
  toSdkSafetySettings,
} from "../schemas/helpers.js";

export const geminiChatSchema = z.object({
  prompt: z.string().min(1),
  model: z
    .string()
    .optional()
    .default(DEFAULT_MODEL)
    .describe(pinnedDefaultDescription("gemini_chat", DEFAULT_MODEL)),
  system_instruction: z.string().optional(),
  temperature: numberLike.pipe(z.number().min(0).max(2)).optional(),
  max_tokens: numberLike.pipe(z.number().int().positive()).optional(),
  top_p: numberLike
    .pipe(z.number().min(0).max(1))
    .optional()
    .describe("Optional: Nucleus sampling probability mass, 0 to 1. Lower values are more deterministic."),
  top_k: numberLike
    .pipe(z.number().int().positive())
    .optional()
    .describe("Optional: A positive integer limiting sampling to the top-K tokens."),
  seed: numberLike
    .pipe(z.number().int())
    .optional()
    .describe("Optional: An integer seed for reproducibility."),
  stop_sequences: z
    .array(z.string().min(1))
    .max(5)
    .optional()
    .describe("Optional: An array of strings that stop generation (up to 5)."),
  safety_settings: safetySettingsSchema,
  json_mode: booleanLike.optional().default(false),
  grounding: booleanLike.optional().default(false),
  thinking_level: z
    .enum(["minimal", "low", "medium", "high"])
    .optional()
    .default(DEFAULT_THINKING_LEVEL)
    .describe(pinnedThinkingDescription("gemini_chat", DEFAULT_THINKING_LEVEL)),
  service_tier: serviceTierSchema,
}).strict();

export type GeminiChatArgs = z.infer<typeof geminiChatSchema>;

export async function handleGeminiChat(args: GeminiChatArgs): Promise<ToolResult> {
  const serviceTier = resolveServiceTier(args.service_tier);
  const safetySettings = await toSdkSafetySettings(args.safety_settings);
  const { text, usage, durationMs, actualServiceTier, diagnostics } = await geminiChat(args.prompt, {
    model: args.model,
    systemInstruction: args.system_instruction,
    temperature: args.temperature,
    maxTokens: args.max_tokens,
    topP: args.top_p,
    topK: args.top_k,
    seed: args.seed,
    stopSequences: args.stop_sequences,
    safetySettings,
    jsonMode: args.json_mode,
    grounding: args.grounding,
    thinkingLevel: args.thinking_level,
    toolName: "gemini_chat",
    serviceTier,
  });

  const displayText = actualServiceTier
    ? `${text}\n\n[Service Tier: ${actualServiceTier}]`
    : text;

  // spec 020: When the response is abnormally empty, attach warnings to surface the cause.
  const warnings = diagnostics && isAbnormalEmpty(text, diagnostics)
    ? buildEmptyResponseWarnings(diagnostics)
    : undefined;

  return {
    text: displayText,
    structured: {
      model: args.model,
      text,
      usage,
      duration_ms: Math.round(durationMs),
      ...(actualServiceTier ? { service_tier: actualServiceTier } : {}),
      ...(warnings ? { warnings } : {}),
    },
  };
}
