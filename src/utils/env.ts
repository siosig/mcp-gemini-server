/**
 * Startup-time environment-variable validation (mcp-ts.md §10).
 * Fail Fast: terminate startup immediately if a required or enum item is invalid.
 * Does not check free-form strings such as model names (validated on the API side).
 */

import { z } from "zod";

const ThinkingLevelSchema = z
  .enum(["minimal", "low", "medium", "high"])
  .optional();

const ServiceTierSchema = z
  .enum(["flex", "priority", "standard"])
  .optional();

const NumericStringSchema = z
  .string()
  .optional()
  .refine(
    (v) => v === undefined || v === "" || !Number.isNaN(Number(v)),
    { message: "must be a numeric string" },
  );

const EnvSchema = z
  .object({
    GEMINI_API_KEY: z.string().min(1, "GEMINI_API_KEY is required"),

    // Model names are free strings (validated on the API side).
    GEMINI_MODEL: z.string().optional(),
    GEMINI_AGENT_MODEL: z.string().optional(),
    GEMINI_SEARCH_MODEL: z.string().optional(),
    GEMINI_VISION_MODEL: z.string().optional(),
    GEMINI_CODE_MODEL: z.string().optional(),
    GEMINI_IMAGE_MODEL: z.string().optional(),

    GEMINI_THINKING_LEVEL: ThinkingLevelSchema,
    GEMINI_AGENT_THINKING_LEVEL: ThinkingLevelSchema,
    GEMINI_VISION_THINKING_LEVEL: ThinkingLevelSchema,
    GEMINI_CODE_THINKING_LEVEL: ThinkingLevelSchema,
    GEMINI_IMAGE_THINKING_LEVEL: ThinkingLevelSchema,

    GEMINI_TIMEOUT: NumericStringSchema,
    GEMINI_SERVICE_TIER: ServiceTierSchema,

    IMAGEN_OUTPUT_DIR: z.string().optional(),

    LOG_LEVEL: z.string().optional(),
  })
  .passthrough(); // Allow unexpected environment variables (originating from other systems).

export type Env = z.infer<typeof EnvSchema>;

/**
 * Validate process.env with Zod. On failure, write the error to stderr and process.exit(1).
 * Must be called at the very start of the startup sequence.
 */
export function validateEnv(): Env {
  const result = EnvSchema.safeParse(process.env);
  if (!result.success) {
    // The logger may not be initialized at this point, so use process.stderr.write.
    // Zod errors are not Error instances, so write out the already-formatted string as-is.
    process.stderr.write(`[gemini-mcp-server] Environment validation failed:\n${result.error.toString()}\n`);
    process.exit(1);
  }
  return result.data;
}
