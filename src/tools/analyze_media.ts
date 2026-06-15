import { z } from "zod";
import { analyzeMedia, DEFAULT_VISION_MODEL, DEFAULT_VISION_THINKING_LEVEL } from "../services/gemini_client.js";
import { isAbnormalEmpty, buildEmptyResponseWarnings, type ResponseDiagnostics } from "../utils/diagnostics.js";
import type { ToolResult } from "./registry.js";
import { getUploadStore } from "../server/upload.js";
import {
  booleanLike,
  pinnedDefaultDescription,
  pinnedThinkingDescription,
  resolveServiceTier,
  serviceTierSchema,
} from "../schemas/helpers.js";

export const analyzeMediaSchema = z.object({
  prompt: z.string().min(1),
  uploaded_file_id: z.string().optional(),
  file_uri: z.string().optional(),
  file_path: z.string().optional(),
  image_url: z.string().optional(),
  image_base64: z.string().optional(),
  model: z
    .string()
    .optional()
    .default(DEFAULT_VISION_MODEL)
    .describe(pinnedDefaultDescription("gemini_analyze_media", DEFAULT_VISION_MODEL)),
  thinking_level: z
    .enum(["minimal", "low", "medium", "high"])
    .optional()
    .default(DEFAULT_VISION_THINKING_LEVEL)
    .describe(pinnedThinkingDescription("gemini_analyze_media", DEFAULT_VISION_THINKING_LEVEL)),
  wait_for_processing: booleanLike.optional().default(true),
  service_tier: serviceTierSchema,
}).strict();

export type AnalyzeMediaArgs = z.infer<typeof analyzeMediaSchema>;

/** spec 020: Shared conversion that attaches warnings when the response is abnormally empty. */
function toResult(result: { text: string; diagnostics?: ResponseDiagnostics }): string | ToolResult {
  const { text, diagnostics } = result;
  if (diagnostics && isAbnormalEmpty(text, diagnostics)) {
    return { text, structured: { text, warnings: buildEmptyResponseWarnings(diagnostics) } };
  }
  return text;
}

export async function handleAnalyzeMedia(args: AnalyzeMediaArgs): Promise<string | ToolResult> {
  // Resolve uploaded_file_id (highest priority)
  if (args.uploaded_file_id) {
    const store = getUploadStore();
    if (!store) {
      throw new Error("UploadStore is not initialized (HTTP transport required)");
    }
    const entry = store.get(args.uploaded_file_id);
    if (!entry) {
      throw new Error(`Uploaded file not found or expired: ${args.uploaded_file_id}`);
    }
    return toResult(await analyzeMedia(args.prompt, {
      filePath: entry.storagePath,
      model: args.model,
      thinkingLevel: args.thinking_level,
      serviceTier: resolveServiceTier(args.service_tier),
    }));
  }

  if (!args.file_uri && !args.file_path && !args.image_url && !args.image_base64) {
    throw new Error("Either uploaded_file_id, file_uri, file_path, image_url, or image_base64 must be provided");
  }
  return toResult(await analyzeMedia(args.prompt, {
    fileUri: args.file_uri,
    filePath: args.file_path,
    imageUrl: args.image_url,
    imageBase64: args.image_base64,
    model: args.model,
    thinkingLevel: args.thinking_level,
    serviceTier: resolveServiceTier(args.service_tier),
  }));
}
