/**
 * Shared schema helpers.
 * Type-coercion utilities for cases where the LLM passes values as strings.
 *
 * Per mcp-ts.md §13.1: value dependencies on @google/genai are lazily loaded via
 * dynamic import, deferring startup cost until the first tool invocation
 * (to keep responsiveness on stdio-mode spawn).
 */

import { z } from "zod";
import type { SafetySetting } from "@google/genai";
import { DEFAULT_SERVICE_TIER, type ServiceTierValue } from "../services/gemini_client.js";

/**
 * Boolean schema that accepts both the strings "true"/"false" and a native boolean.
 * Prevents "Invalid arguments" errors when the LLM passes a string to z.boolean().
 */
export const booleanLike = z.preprocess(
  (v) => (v === "true" ? true : v === "false" ? false : v),
  z.boolean(),
);

/**
 * Number schema that accepts both numeric strings and a native number.
 * Prevents "Invalid arguments" errors when the LLM passes a string like "10" to z.number().
 */
export const numberLike = z.preprocess(
  (v) => (typeof v === "string" && v.trim() !== "" ? Number(v) : v),
  z.number(),
);

/**
 * service_tier schema. Shared across all applicable tools.
 */
export const serviceTierSchema = z
  .enum(["flex", "priority", "standard"])
  .optional()
  .describe(
    "Inference tier. flex (low cost, high latency) / priority (high reliability) / standard (API default). If omitted, uses the GEMINI_SERVICE_TIER environment variable or the API default.",
  );

/**
 * Resolve the service_tier from the tool argument.
 * Priority: tool argument > environment variable > undefined.
 * "standard" overrides the environment-variable default to undefined (API default behavior).
 */
export function resolveServiceTier(
  toolArg: ServiceTierValue | undefined,
): ServiceTierValue | undefined {
  const effective = toolArg ?? DEFAULT_SERVICE_TIER;
  if (!effective || effective === "standard") return undefined;
  return effective;
}

/**
 * Map from safety-settings shorthand to SDK enums. Built on first use via dynamic import.
 */
type HarmCategoryShort = "harassment" | "hate_speech" | "sexually_explicit" | "dangerous_content";
type HarmThresholdShort = "low" | "medium" | "high" | "none";

interface HarmMaps {
  category: Readonly<Record<HarmCategoryShort, SafetySetting["category"]>>;
  threshold: Readonly<Record<HarmThresholdShort, SafetySetting["threshold"]>>;
}

let _harmMaps: HarmMaps | null = null;
async function getHarmMaps(): Promise<HarmMaps> {
  if (!_harmMaps) {
    const sdk = await import("@google/genai");
    _harmMaps = {
      category: Object.freeze({
        harassment: sdk.HarmCategory.HARM_CATEGORY_HARASSMENT,
        hate_speech: sdk.HarmCategory.HARM_CATEGORY_HATE_SPEECH,
        sexually_explicit: sdk.HarmCategory.HARM_CATEGORY_SEXUALLY_EXPLICIT,
        dangerous_content: sdk.HarmCategory.HARM_CATEGORY_DANGEROUS_CONTENT,
      }),
      threshold: Object.freeze({
        low: sdk.HarmBlockThreshold.BLOCK_LOW_AND_ABOVE,
        medium: sdk.HarmBlockThreshold.BLOCK_MEDIUM_AND_ABOVE,
        high: sdk.HarmBlockThreshold.BLOCK_ONLY_HIGH,
        none: sdk.HarmBlockThreshold.BLOCK_NONE,
      }),
    };
  }
  return _harmMaps;
}

/**
 * safety_settings schema. Uses LLM-friendly shorthand that is converted internally to SDK enums.
 */
export const safetySettingsSchema = z
  .array(
    z
      .object({
        category: z
          .enum(["harassment", "hate_speech", "sexually_explicit", "dangerous_content"])
          .describe("[REQUIRED] Harm category"),
        threshold: z
          .enum(["low", "medium", "high", "none"])
          .describe(
            "[REQUIRED] Block threshold. low=block low probability and above / medium=medium and above / high=high only / none=do not block",
          ),
      })
      .strict(),
  )
  .optional()
  .describe(
    'Optional: safety filter. Example: [{"category":"harassment","threshold":"high"}]. Defaults to the API default if omitted.',
  );

export type SafetySettingInput = z.infer<typeof safetySettingsSchema>;

/** Convert shorthand safety_settings to the SDK's SafetySetting[] (async due to SDK value dependency). */
export async function toSdkSafetySettings(input: SafetySettingInput): Promise<SafetySetting[] | undefined> {
  if (!input || input.length === 0) return undefined;
  const maps = await getHarmMaps();
  return input.map((s) => ({
    category: maps.category[s.category],
    threshold: maps.threshold[s.threshold],
  }));
}

/**
 * Returns the standard "pinned default, override discouraged" wording for the `model` parameter description.
 * spec 019: each tool pins a default model optimized for its purpose; override only with a clear reason.
 */
export function pinnedDefaultDescription(toolName: string, defaultModel: string): string {
  return `[DEFAULT FIXED] ${toolName} is optimized for its purpose to run on ${defaultModel}. Do not override unless there is a clear reason (e.g. cost or a specific task's quality requirement).`;
}

/**
 * Returns the standard "pinned default, override discouraged" wording for the `thinking_level` parameter description.
 */
export function pinnedThinkingDescription(toolName: string, defaultLevel: string): string {
  return `[DEFAULT FIXED] The thinking depth of ${toolName} is optimized at ${defaultLevel}. Do not override unless there is a clear reason. Values: minimal/low/medium/high.`;
}
