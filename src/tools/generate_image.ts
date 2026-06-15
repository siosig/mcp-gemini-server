/**
 * spec 019: Image generation MCP tool (Nano Banana 2 = gemini-3.1-flash-image-preview, Flash Image).
 * Generates an image via the `models.generateContent` API and saves it locally as PNG.
 *
 * Changes from the initial spec 019 release (Nano Banana Pro):
 * - Fixed the model enum to the single value `gemini-3.1-flash-image-preview` (Pro -> Flash, output cost 1/40)
 * - Expanded aspect_ratio to 14 values (the existing 10 plus Flash-only 1:4 / 4:1 / 1:8 / 8:1)
 * - Expanded image_size to 4 values (0.5K / 1K / 2K / 4K), changed the default from 2K to 1K (Flash official default)
 * - Continued support for the thinking_level parameter
 * - number_of_images and person_generation remain removed (generateContent produces a single output by default)
 */

import crypto from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { z } from "zod";
import {
  DEFAULT_IMAGE_MODEL,
  DEFAULT_IMAGE_THINKING_LEVEL,
  generateImageContent,
} from "../services/gemini_client.js";
import { buildEmptyResponseWarnings } from "../utils/diagnostics.js";
import type { ToolResult } from "./registry.js";
import {
  pinnedDefaultDescription,
  pinnedThinkingDescription,
  resolveServiceTier,
  serviceTierSchema,
} from "../schemas/helpers.js";

const MODEL_VALUES = ["gemini-3.1-flash-image-preview"] as const;

const ASPECT_RATIO_VALUES = [
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
] as const;

const IMAGE_SIZE_VALUES = ["0.5K", "1K", "2K", "4K"] as const;

const FILE_PREFIX_PATTERN = /^[A-Za-z0-9_-]+$/;

export const generateImageSchema = z
  .object({
    prompt: z
      .string()
      .min(1)
      .describe(
        "[REQUIRED] A prompt describing the image content (English recommended; the maximum token count follows the model specification).",
      ),
    model: z
      .enum(MODEL_VALUES)
      .optional()
      .default(DEFAULT_IMAGE_MODEL as (typeof MODEL_VALUES)[number])
      .describe(pinnedDefaultDescription("gemini_generate_image", DEFAULT_IMAGE_MODEL)),
    aspect_ratio: z
      .enum(ASPECT_RATIO_VALUES)
      .optional()
      .default("1:1")
      .describe(
        '[OPTIONAL] Aspect ratio. 14 options ("1:1"(default) / "2:3" / "3:2" / "3:4" / "4:3" / "4:5" / "5:4" / "9:16" / "16:9" / "21:9" / "1:4" / "4:1" / "1:8" / "8:1").',
      ),
    image_size: z
      .enum(IMAGE_SIZE_VALUES)
      .optional()
      .default("1K")
      .describe('[OPTIONAL] Output image resolution. "0.5K" / "1K"(default) / "2K" / "4K".'),
    thinking_level: z
      .enum(["minimal", "low", "medium", "high"])
      .optional()
      .default(DEFAULT_IMAGE_THINKING_LEVEL)
      .describe(pinnedThinkingDescription("gemini_generate_image", DEFAULT_IMAGE_THINKING_LEVEL)),
    output_dir: z
      .string()
      .min(1)
      .optional()
      .describe(
        "[OPTIONAL] Output directory (absolute path recommended). If omitted, falls back to the env var IMAGEN_OUTPUT_DIR, and otherwise to <os.tmpdir>/mcp-gemini/imagen. Created automatically if it does not exist.",
      ),
    file_prefix: z
      .string()
      .min(1)
      .regex(FILE_PREFIX_PATTERN, "Only filename-safe characters (A-Za-z0-9_-) are allowed")
      .optional()
      .default("imagen")
      .describe(
        '[OPTIONAL] Prefix for output filenames. Default "imagen". Saved in the form <prefix>-1.png.',
      ),
    service_tier: serviceTierSchema,
  })
  .strict();

export type GenerateImageArgs = z.infer<typeof generateImageSchema>;

const SYNTH_ID_WARNING =
  "All generated images carry SynthID watermarking by Google.";

/**
 * Resolves the output directory, creating it if it does not exist.
 * Priority: argument -> env IMAGEN_OUTPUT_DIR -> <os.tmpdir>/mcp-gemini/imagen
 */
export async function resolveOutputDir(arg?: string): Promise<string> {
  const dir =
    arg ??
    process.env.IMAGEN_OUTPUT_DIR ??
    path.join(os.tmpdir(), "mcp-gemini", "imagen");
  await fs.mkdir(dir, { recursive: true });
  return dir;
}

/**
 * Returns a collision-avoiding filename. Base form `<prefix>-<index>.png`, with an 8-char UUID appended on collision.
 */
export async function pickUniqueFilename(
  dir: string,
  prefix: string,
  index: number,
): Promise<string> {
  const base = path.join(dir, `${prefix}-${index}.png`);
  try {
    await fs.access(base);
  } catch {
    return base;
  }
  const suffix = crypto.randomUUID().slice(0, 8);
  return path.join(dir, `${prefix}-${index}-${suffix}.png`);
}

interface GeneratedImageFile {
  path: string;
  index: number;
  size_bytes: number;
}

export async function handleGenerateImage(args: GenerateImageArgs): Promise<ToolResult> {
  const outputDir = await resolveOutputDir(args.output_dir);

  const result = await generateImageContent({
    prompt: args.prompt,
    model: args.model,
    aspectRatio: args.aspect_ratio,
    imageSize: args.image_size,
    thinkingLevel: args.thinking_level,
    serviceTier: resolveServiceTier(args.service_tier),
  });

  const warnings: string[] = [];

  if (!result.imageBytes) {
    // spec 020: Surface the cause of missing image bytes (policy block / upstream error) via the shared diagnostics helper.
    if (result.diagnostics) {
      warnings.push(...buildEmptyResponseWarnings(result.diagnostics));
    } else {
      warnings.push("Image generation returned no image bytes (policy block or upstream error).");
    }
    const empty = {
      model: args.model,
      prompt: args.prompt,
      count: 0,
      files: [],
      text: result.text,
      warnings,
    };
    return { text: JSON.stringify(empty), structured: empty };
  }

  const filePath = await pickUniqueFilename(outputDir, args.file_prefix, 1);
  const files: GeneratedImageFile[] = [];
  try {
    const buf = Buffer.from(result.imageBytes, "base64");
    await fs.writeFile(filePath, buf);
    files.push({ path: filePath, index: 1, size_bytes: buf.byteLength });
  } catch (err) {
    warnings.push(
      `Failed to write image to ${filePath}: ${(err as Error).message}`,
    );
  }

  if (files.length === 0) {
    throw new Error(
      `Failed to write image file. warnings: ${warnings.join(" | ")}`,
    );
  }

  warnings.push(SYNTH_ID_WARNING);

  const payload = {
    model: args.model,
    prompt: args.prompt,
    count: files.length,
    files,
    text: result.text,
    warnings,
  };
  return { text: JSON.stringify(payload), structured: payload };
}
