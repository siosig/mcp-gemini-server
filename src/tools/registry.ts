/**
 * MCP tool registry.
 * Centralizes all tool definitions so they can be registered in a loop from index.ts.
 */

import type { z, ZodRawShape } from "zod";
import { geminiChatSchema, handleGeminiChat } from "./gemini_chat.js";
import { googleSearchSchema, handleGoogleSearch } from "./google_search.js";
import { customAgentSchema, handleCustomAgent } from "./custom_agent.js";
import { analyzeMediaSchema, handleAnalyzeMedia } from "./analyze_media.js";
import { generateImageSchema, handleGenerateImage } from "./generate_image.js";
import { executeCodeSchema, handleExecuteCode } from "./execute_code.js";
import { manageFilesSchema, handleManageFiles } from "./manage_files.js";

/**
 * Local definition of the required MCP ToolAnnotations fields (the SDK type is not publicly exported).
 * The field definitions match @modelcontextprotocol/sdk's ToolAnnotationsSchema.
 */
interface ToolAnnotations {
  title?: string;
  readOnlyHint?: boolean;
  destructiveHint?: boolean;
  idempotentHint?: boolean;
  openWorldHint?: boolean;
}

/**
 * A tool's return value. `text` is the primary display text for the LLM,
 * and `structured` is structured data returned as MCP `structuredContent`.
 */
export interface ToolResult {
  text: string;
  structured?: Record<string, unknown>;
}

/**
 * Progress notification callback for long-running tools.
 * Sends the MCP protocol's `notifications/progress` together with the client-side progressToken.
 * If the client does not pass a progressToken, it becomes undefined in index.ts,
 * so tool implementations can simply call it (no need to worry about send failures).
 */
export type ProgressCallback = (
  current: number,
  total?: number,
  message?: string,
) => void | Promise<void>;

/** Execution context injected into tool handlers. Leaves room for SDK extension. */
export interface ToolContext {
  progress?: ProgressCallback | undefined;
}

/**
 * Tool definition to pass to the MCP SDK's server.registerTool().
 * At the SDK boundary, the match between the ZodRawShape and the handler type cannot be expressed statically,
 * so ToolDefinition uses erased types (ZodRawShape / Record<string, unknown>).
 * Type safety is guaranteed at the point defineTool() is called.
 *
 * handler return value:
 * - Use `string` when you only need to return a plain string
 * - Use `ToolResult` when you also want to return structured information (passed to MCP as `structuredContent`)
 */
export interface ToolDefinition {
  name: string;
  title: string;
  description: string;
  schema: z.ZodObject<ZodRawShape>;
  annotations: ToolAnnotations;
  handler: (
    args: Record<string, unknown>,
    ctx?: ToolContext,
  ) => string | ToolResult | Promise<string | ToolResult>;
}

/**
 * Helper that lets individual tool definitions be written type-safely and converts them to the common ToolDefinition.
 * Generics ensure the handler's argument types are accurately inferred at the call site.
 * The type-erasing cast is confined to this function (an SDK boundary constraint).
 */
function defineTool<T extends ZodRawShape>(def: {
  name: string;
  title: string;
  description: string;
  schema: z.ZodObject<T>;
  annotations: ToolAnnotations;
  handler: (
    args: z.infer<z.ZodObject<T>>,
    ctx?: ToolContext,
  ) => string | ToolResult | Promise<string | ToolResult>;
}): ToolDefinition {
  // Type erasure for the SDK boundary: type safety is already guaranteed by the defineTool<T> generics
  return def as unknown as ToolDefinition;
}

export const allTools: ToolDefinition[] = [
  defineTool({
    name: "gemini_chat",
    title: "Gemini Chat",
    description: "Chat with Gemini. Supports thinking levels, grounding, and JSON mode.",
    schema: geminiChatSchema,
    annotations: {
      readOnlyHint: true,
      openWorldHint: true,
    },
    handler: handleGeminiChat,
  }),
  defineTool({
    name: "gemini_search",
    title: "Google Search",
    description: "Search the web via Google using Gemini Grounding.",
    schema: googleSearchSchema,
    annotations: {
      readOnlyHint: true,
      openWorldHint: true,
    },
    handler: handleGoogleSearch,
  }),
  defineTool({
    name: "gemini_custom_agent",
    title: "Custom Agent",
    description:
      'Run a task with a specialized agent role. REQUIRED: task (string), role (string — e.g. "architect" | "reviewer" | "developer" | "analyst" | "critic" | "summarizer" | "researcher"). Any free-form role string is also accepted. Example: { "task": "Review this PR", "role": "reviewer" }.',
    schema: customAgentSchema,
    annotations: {
      readOnlyHint: true,
      openWorldHint: true,
    },
    handler: handleCustomAgent,
  }),
  defineTool({
    name: "gemini_analyze_media",
    title: "Media Analysis",
    description: "Analyze images, PDF, video, or audio using Gemini vision.",
    schema: analyzeMediaSchema,
    annotations: {
      readOnlyHint: true,
      openWorldHint: true,
    },
    handler: handleAnalyzeMedia,
  }),
  defineTool({
    name: "gemini_generate_image",
    title: "Gemini Image Generation (Nano Banana 2)",
    description:
      'Generate a single image via Gemini Flash Image (Nano Banana 2, model fixed to gemini-3.1-flash-image-preview) and save as PNG. REQUIRED: prompt (string, English recommended). Optional: aspect_ratio (14 values incl. 1:1/16:9/9:16/1:4/4:1/etc., default 1:1), image_size ("0.5K"|"1K"|"2K"|"4K", default "1K"), thinking_level ("minimal"|"low"|"medium"|"high", default "medium"), output_dir, file_prefix (default "imagen"). One image per call (loop on caller side for multiple). All generated images carry SynthID watermarking by Google.',
    schema: generateImageSchema,
    annotations: {
      readOnlyHint: false,
      destructiveHint: false,
      openWorldHint: true,
    },
    handler: handleGenerateImage,
  }),
  defineTool({
    name: "gemini_execute_code",
    title: "Code Execution",
    description: "Execute Python code in Gemini's sandbox (numpy, pandas, matplotlib available).",
    schema: executeCodeSchema,
    annotations: {
      readOnlyHint: false,
      destructiveHint: false,
      openWorldHint: true,
    },
    handler: handleExecuteCode,
  }),
  defineTool({
    name: "gemini_manage_files",
    title: "File Management",
    description: "Manage files in Gemini (upload, list, status, delete). Files stored 48h, up to 2GB.",
    schema: manageFilesSchema,
    annotations: {
      readOnlyHint: false,
      destructiveHint: true,
      openWorldHint: true,
    },
    handler: handleManageFiles,
  }),
];
