/**
 * Gemini API client module.
 * Provides singleton management, chat, search, media analysis, and code execution.
 *
 * Per mcp-ts.md §13.1: @google/genai is lazily loaded via dynamic import (to keep startup
 * responsive on stdio spawn). Only types are statically imported; values (classes, enums,
 * helper functions) are obtained through loadSdk().
 */

import fs from "node:fs/promises";
import path from "node:path";
import type {
  GenerateContentConfig,
  GoogleGenAI,
  Part,
  SafetySetting,
  ServiceTier as SdkServiceTier,
  ThinkingConfig,
  ThinkingLevel as SdkThinkingLevel,
} from "@google/genai";
import { withAbortableTimeout, withFetchRetry, withTelemetry } from "../utils/telemetry.js";
import { extractResponseDiagnostics, type ResponseDiagnostics } from "../utils/diagnostics.js";

// spec 019: Default models optimized per tool use case. Do not override without a clear reason.
// gemini_generate_image uses Nano Banana 2 (Flash Image) — 1/40 the cost of Pro, suited for mainstream use.
export const DEFAULT_MODEL = process.env.GEMINI_MODEL ?? "gemini-3.5-flash";
export const DEFAULT_AGENT_MODEL = process.env.GEMINI_AGENT_MODEL ?? "gemini-3.5-flash";
export const DEFAULT_SEARCH_MODEL = process.env.GEMINI_SEARCH_MODEL ?? "gemini-3.1-flash-lite";
export const DEFAULT_VISION_MODEL = process.env.GEMINI_VISION_MODEL ?? "gemini-3.1-flash-lite";
export const DEFAULT_CODE_MODEL = process.env.GEMINI_CODE_MODEL ?? "gemini-3.1-flash-lite";
export const DEFAULT_IMAGE_MODEL = process.env.GEMINI_IMAGE_MODEL ?? "gemini-3.1-flash-image-preview";
export const DEFAULT_TEAM_MODEL = process.env.GEMINI_TEAM_MODEL ?? DEFAULT_AGENT_MODEL;

/** Valid values for thinking_level */
export type ThinkingLevel = "minimal" | "low" | "medium" | "high";

/** Valid values for service_tier (the values accepted as MCP tool arguments) */
export type ServiceTierValue = "flex" | "priority" | "standard";

const VALID_SERVICE_TIERS: readonly string[] = ["flex", "priority", "standard"];

export function isServiceTier(value: string): value is ServiceTierValue {
  return VALID_SERVICE_TIERS.includes(value);
}

const VALID_THINKING_LEVELS: readonly string[] = ["minimal", "low", "medium", "high"];

function isThinkingLevel(value: string): value is ThinkingLevel {
  return VALID_THINKING_LEVELS.includes(value);
}

// spec 019: Fix each tool's default thinking_level per use case.
const rawThinkingLevel = process.env.GEMINI_THINKING_LEVEL ?? "medium";
export const DEFAULT_THINKING_LEVEL: ThinkingLevel = isThinkingLevel(rawThinkingLevel) ? rawThinkingLevel : "medium";

const rawAgentThinkingLevel = process.env.GEMINI_AGENT_THINKING_LEVEL ?? "high";
export const DEFAULT_AGENT_THINKING_LEVEL: ThinkingLevel = isThinkingLevel(rawAgentThinkingLevel) ? rawAgentThinkingLevel : "high";

const rawVisionThinkingLevel = process.env.GEMINI_VISION_THINKING_LEVEL ?? "medium";
export const DEFAULT_VISION_THINKING_LEVEL: ThinkingLevel = isThinkingLevel(rawVisionThinkingLevel) ? rawVisionThinkingLevel : "medium";

const rawCodeThinkingLevel = process.env.GEMINI_CODE_THINKING_LEVEL ?? "low";
export const DEFAULT_CODE_THINKING_LEVEL: ThinkingLevel = isThinkingLevel(rawCodeThinkingLevel) ? rawCodeThinkingLevel : "low";

const rawImageThinkingLevel = process.env.GEMINI_IMAGE_THINKING_LEVEL ?? "medium";
export const DEFAULT_IMAGE_THINKING_LEVEL: ThinkingLevel = isThinkingLevel(rawImageThinkingLevel) ? rawImageThinkingLevel : "medium";

const rawTeamThinkingLevel = process.env.GEMINI_TEAM_THINKING_LEVEL ?? "high";
export const DEFAULT_TEAM_THINKING_LEVEL: ThinkingLevel = isThinkingLevel(rawTeamThinkingLevel) ? rawTeamThinkingLevel : "high";
export const TIMEOUT_MS = parseInt(process.env.GEMINI_TIMEOUT ?? "360") * 1000;

/** Timeout for the flex tier (15 minutes = 900,000ms) */
export const FLEX_TIMEOUT_MS = 900_000;

/** Default service_tier from the environment variable. Invalid values are ignored (undefined). */
const rawServiceTier = process.env.GEMINI_SERVICE_TIER ?? "";
export const DEFAULT_SERVICE_TIER: ServiceTierValue | undefined =
  isServiceTier(rawServiceTier) ? rawServiceTier : undefined;

// For Gemini 2.5-series: mapping from thinking_level to token-based thinking_budget.
// minimal=512 is a safe value aligned with gemini-2.5-flash-lite's lower bound (512-24576).
export const THINKING_LEVEL_BUDGETS: Readonly<Record<ThinkingLevel, number>> = {
  minimal: 512,
  low: 1024,
  medium: 8192,
  high: 24576,
};

// ==================== @google/genai dynamic import ====================

let _sdk: typeof import("@google/genai") | null = null;
async function loadSdk(): Promise<typeof import("@google/genai")> {
  if (!_sdk) _sdk = await import("@google/genai");
  return _sdk;
}

// For Gemini 3.x-series: mapping from thinking_level to the SDK ThinkingLevel enum (built lazily)
let _thinkingLevel3xMap: Readonly<Record<ThinkingLevel, SdkThinkingLevel>> | null = null;
async function getThinkingLevel3xMap(): Promise<Readonly<Record<ThinkingLevel, SdkThinkingLevel>>> {
  if (!_thinkingLevel3xMap) {
    const sdk = await loadSdk();
    _thinkingLevel3xMap = Object.freeze({
      minimal: sdk.ThinkingLevel.MINIMAL,
      low: sdk.ThinkingLevel.LOW,
      medium: sdk.ThinkingLevel.MEDIUM,
      high: sdk.ThinkingLevel.HIGH,
    });
  }
  return _thinkingLevel3xMap;
}

/** Determines whether the model is a Gemini 3-series model (gemini-3.x, gemini-3-flash, etc.). */
export function isGemini3(model: string): boolean {
  return /^gemini-3(\.|-)/.test(model);
}

/**
 * Builds a ThinkingConfig appropriate for the model.
 * - Gemini 3-series: `thinkingLevel` ("MINIMAL"|"LOW"|"MEDIUM"|"HIGH")
 * - Gemini 2.5-series: `thinkingBudget` (token count 512-24576)
 *
 * Behavior when level === null (suppress thinking):
 * - Gemini 2.5-series: sends `thinkingBudget: 0` to fully turn off thinking (cost reduction)
 * - Gemini 3-series: returns `undefined` (defers to the API default).
 *   Reason: some models such as gemini-3.1-pro-preview reject `thinkingLevel: "MINIMAL"`, so
 *   force-sending MINIMAL for suppression causes a 400 INVALID_ARGUMENT.
 *
 * When level === undefined, nothing is set (defers to the API default).
 */
export async function buildThinkingConfig(
  model: string,
  level: ThinkingLevel | null | undefined,
): Promise<ThinkingConfig | undefined> {
  if (level === undefined) return undefined;
  if (level === null) {
    return isGemini3(model) ? undefined : { thinkingBudget: 0 };
  }
  if (isGemini3(model)) {
    const map = await getThinkingLevel3xMap();
    return { thinkingLevel: map[level] };
  }
  return { thinkingBudget: THINKING_LEVEL_BUDGETS[level] };
}

/** Converts ServiceTierValue to the SDK ServiceTier enum. "standard" is omitted (defers to API default behavior). */
async function toSdkServiceTier(tier: ServiceTierValue): Promise<SdkServiceTier | undefined> {
  if (tier === "standard") return undefined;
  const sdk = await loadSdk();
  if (tier === "flex") return sdk.ServiceTier.FLEX;
  return sdk.ServiceTier.PRIORITY;
}

// Create and share a singleton instance at application startup
let _sharedGenAI: GoogleGenAI | null = null;

export async function getSharedGenAI(): Promise<GoogleGenAI> {
  if (!_sharedGenAI) {
    const apiKey = process.env.GEMINI_API_KEY;
    if (!apiKey) {
      throw new Error("GEMINI_API_KEY environment variable is not set");
    }
    const sdk = await loadSdk();
    _sharedGenAI = new sdk.GoogleGenAI({ apiKey });
  }
  return _sharedGenAI;
}

export function setSharedGenAI(client: GoogleGenAI | null): void {
  _sharedGenAI = client;
}

/**
 * Returns the timeout value corresponding to the service_tier.
 */
function resolveTimeout(serviceTier: ServiceTierValue | undefined, baseTimeout: number): number {
  return serviceTier === "flex" ? FLEX_TIMEOUT_MS : baseTimeout;
}

/**
 * Builds a GenerateContentConfig from ChatOptions.
 * thinking_level switches between Gemini 3-series (thinkingLevel string) and 2.5-series
 * (thinkingBudget number) depending on the model.
 */
async function buildGenerateConfig(model: string, options: ChatOptions): Promise<GenerateContentConfig | undefined> {
  const config: GenerateContentConfig = {};
  if (options.systemInstruction) config.systemInstruction = options.systemInstruction;
  if (options.temperature !== undefined) config.temperature = options.temperature;
  if (options.maxTokens !== undefined) config.maxOutputTokens = options.maxTokens;
  if (options.topP !== undefined) config.topP = options.topP;
  if (options.topK !== undefined) config.topK = options.topK;
  if (options.seed !== undefined) config.seed = options.seed;
  if (options.stopSequences && options.stopSequences.length > 0) {
    config.stopSequences = options.stopSequences;
  }
  if (options.safetySettings && options.safetySettings.length > 0) {
    config.safetySettings = options.safetySettings;
  }
  if (options.jsonMode) config.responseMimeType = "application/json";
  if (options.grounding) config.tools = [{ googleSearch: {} }];
  const thinkingConfig = await buildThinkingConfig(model, options.thinkingLevel);
  if (thinkingConfig) config.thinkingConfig = thinkingConfig;
  if (options.serviceTier) {
    const sdkTier = await toSdkServiceTier(options.serviceTier);
    if (sdkTier) config.serviceTier = sdkTier;
  }
  return Object.keys(config).length > 0 ? config : undefined;
}

export interface ChatOptions {
  model?: string | undefined;
  systemInstruction?: string | undefined;
  temperature?: number | undefined;
  maxTokens?: number | undefined;
  topP?: number | undefined;
  topK?: number | undefined;
  seed?: number | undefined;
  stopSequences?: string[] | undefined;
  safetySettings?: SafetySetting[] | undefined;
  jsonMode?: boolean | undefined;
  grounding?: boolean | undefined;
  thinkingLevel?: ThinkingLevel | null | undefined;
  toolName?: string | undefined;
  serviceTier?: ServiceTierValue | undefined;
}

export interface UsageMetadata {
  promptTokenCount?: number | undefined;
  candidatesTokenCount?: number | undefined;
  totalTokenCount?: number | undefined;
  thoughtsTokenCount?: number | undefined;
}

/**
 * Calls the Gemini API and returns text.
 */
export async function geminiChat(
  prompt: string,
  options: ChatOptions = {}
): Promise<{ text: string; usage: UsageMetadata; durationMs: number; actualServiceTier?: string; diagnostics?: ResponseDiagnostics }> {
  const model = options.model ?? DEFAULT_MODEL;
  const timeout = resolveTimeout(options.serviceTier, TIMEOUT_MS);

  const baseConfig = await buildGenerateConfig(model, options);
  const genai = await getSharedGenAI();

  const { result: response, durationMs } = await withTelemetry(
    { toolName: options.toolName ?? "gemini_chat", model, thinkingLevel: options.thinkingLevel ?? "", serviceTier: options.serviceTier },
    () => withFetchRetry(() => withAbortableTimeout(
      (signal) => genai.models.generateContent({
        model,
        contents: prompt,
        config: { ...(baseConfig ?? {}), abortSignal: signal },
      }),
      timeout
    )),
  );

  // The actual serving tier is returned in sdkHttpResponse.headers["x-gemini-service-tier"] as "standard"/"flex"/"priority".
  // The official SDK field `usageMetadata.trafficType` is explicitly documented in the type definitions as "not supported on the Gemini API", so it is not used.
  const actualServiceTier = response.sdkHttpResponse?.headers?.["x-gemini-service-tier"];

  return {
    text: response.text ?? "",
    usage: {
      promptTokenCount: response.usageMetadata?.promptTokenCount ?? 0,
      candidatesTokenCount: response.usageMetadata?.candidatesTokenCount ?? 0,
      totalTokenCount: response.usageMetadata?.totalTokenCount ?? 0,
      thoughtsTokenCount: response.usageMetadata?.thoughtsTokenCount,
    },
    durationMs,
    ...(actualServiceTier ? { actualServiceTier } : {}),
    diagnostics: extractResponseDiagnostics(response),
  };
}

/**
 * Search using Google Search grounding.
 *
 * When thinkingLevel is unspecified, thinkingBudget=0 is explicitly sent to suppress thinking
 * (the main purpose is formatting search grounding results, so cost is reduced).
 */
export async function geminiSearch(
  query: string,
  limit = 0,
  raw = false,
  serviceTier?: ServiceTierValue,
  model: string = DEFAULT_SEARCH_MODEL,
  thinkingLevel?: ThinkingLevel | null,
): Promise<{ text: string; diagnostics?: ResponseDiagnostics }> {
  const prompt = limit > 0 ? `${query}\n(Provide top ${limit} results)` : query;
  const timeout = resolveTimeout(serviceTier, TIMEOUT_MS);

  const config: GenerateContentConfig = { tools: [{ googleSearch: {} }] };
  // Use thinkingLevel if specified; otherwise null (suppress thinking = thinkingBudget: 0)
  const thinkingConfig = await buildThinkingConfig(model, thinkingLevel ?? null);
  if (thinkingConfig) config.thinkingConfig = thinkingConfig;
  if (serviceTier) {
    const sdkTier = await toSdkServiceTier(serviceTier);
    if (sdkTier) config.serviceTier = sdkTier;
  }

  const genai = await getSharedGenAI();
  const { result: response } = await withTelemetry(
    { toolName: "gemini_search", model, thinkingLevel: thinkingLevel ?? "", serviceTier },
    () => withFetchRetry(() => withAbortableTimeout(
      (signal) => genai.models.generateContent({
        model,
        contents: prompt,
        config: { ...config, abortSignal: signal },
      }),
      timeout
    )),
  );

  const text = raw ? JSON.stringify(response) : (response.text ?? "No results found.");
  return { text, diagnostics: extractResponseDiagnostics(response) };
}

// Data URI pattern
const DATA_URI_PATTERN = /^data:(.+);base64,(.+)$/;

/**
 * Image and media analysis.
 */
export interface AnalyzeMediaOptions {
  fileUri?: string | undefined;
  filePath?: string | undefined;
  imageUrl?: string | undefined;
  imageBase64?: string | undefined;
  model?: string | undefined;
  serviceTier?: ServiceTierValue | undefined;
  thinkingLevel?: ThinkingLevel | null | undefined;
}

export async function analyzeMedia(
  prompt: string,
  opts: AnalyzeMediaOptions = {}
): Promise<{ text: string; diagnostics?: ResponseDiagnostics }> {
  const sdk = await loadSdk();
  const genai = await getSharedGenAI();
  const model = opts.model ?? DEFAULT_VISION_MODEL;
  const parts: Part[] = [];

  if (opts.imageBase64) {
    let mimeType = "image/jpeg";
    let data = opts.imageBase64;
    if (opts.imageBase64.startsWith("data:")) {
      const m = DATA_URI_PATTERN.exec(opts.imageBase64);
      if (m) {
        mimeType = m[1]!;
        data = m[2]!;
      }
    }
    // The base64 -> Buffer -> base64 round-trip normalizes the data: URI.
    parts.push(sdk.createPartFromBase64(Buffer.from(data, "base64").toString("base64"), mimeType));
  } else if (opts.imageUrl) {
    parts.push(sdk.createPartFromUri(opts.imageUrl, "image/jpeg"));
  } else if (opts.fileUri) {
    let fileName = path.basename(opts.fileUri);
    if (!fileName.startsWith("files/")) fileName = `files/${fileName}`;
    const fileInfo = await genai.files.get({ name: fileName });
    parts.push(sdk.createPartFromUri(opts.fileUri, fileInfo.mimeType ?? "application/octet-stream"));
  } else if (opts.filePath) {
    const buf = await fs.readFile(opts.filePath);
    const ext = path.extname(opts.filePath).toLowerCase();
    const mimeMap: Record<string, string> = {
      ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".png": "image/png",
      ".gif": "image/gif", ".webp": "image/webp", ".heic": "image/heic",
      ".heif": "image/heif", ".pdf": "application/pdf",
      ".mp4": "video/mp4", ".mpeg": "video/mpeg", ".mov": "video/quicktime",
      ".webm": "video/webm", ".mp3": "audio/mpeg", ".wav": "audio/wav",
      ".ogg": "audio/ogg", ".flac": "audio/flac",
    };
    const mimeType = mimeMap[ext] ?? "application/octet-stream";
    parts.push(sdk.createPartFromBase64(buf.toString("base64"), mimeType));
  }

  parts.push(sdk.createPartFromText(prompt));

  const timeout = resolveTimeout(opts.serviceTier, TIMEOUT_MS * 2);
  const config: GenerateContentConfig = {};
  const thinkingConfig = await buildThinkingConfig(model, opts.thinkingLevel);
  if (thinkingConfig) config.thinkingConfig = thinkingConfig;
  if (opts.serviceTier) {
    const sdkTier = await toSdkServiceTier(opts.serviceTier);
    if (sdkTier) config.serviceTier = sdkTier;
  }

  const { result: response } = await withTelemetry(
    { toolName: "gemini_analyze_media", model, thinkingLevel: opts.thinkingLevel ?? "", serviceTier: opts.serviceTier },
    () => withFetchRetry(() => withAbortableTimeout(
      (signal) => genai.models.generateContent({
        model,
        contents: [{ parts }],
        config: { ...config, abortSignal: signal },
      }),
      timeout
    )),
  );

  return { text: response.text ?? "", diagnostics: extractResponseDiagnostics(response) };
}

/**
 * spec 019: Image generation via the generateContent API (Nano Banana 2 = gemini-3.1-flash-image-preview, Flash Image).
 * Uses `models.generateContent` rather than the Imagen API (`models.generateImages`),
 * extracting the PNG bytes from the response's inlineData.
 */
export interface GenerateImageContentOptions {
  prompt: string;
  model: string;
  aspectRatio: string;
  imageSize: string;
  thinkingLevel?: ThinkingLevel | null | undefined;
  serviceTier?: ServiceTierValue | undefined;
}

export interface GenerateImageContentResult {
  imageBytes: string | undefined;
  mimeType: string | undefined;
  text: string;
  diagnostics?: ResponseDiagnostics | undefined;
}

export async function generateImageContent(
  opts: GenerateImageContentOptions,
): Promise<GenerateImageContentResult> {
  const timeout = resolveTimeout(opts.serviceTier, TIMEOUT_MS);
  const config: GenerateContentConfig = {
    imageConfig: {
      aspectRatio: opts.aspectRatio,
      imageSize: opts.imageSize,
    },
  };
  const thinkingConfig = await buildThinkingConfig(opts.model, opts.thinkingLevel);
  if (thinkingConfig) config.thinkingConfig = thinkingConfig;
  if (opts.serviceTier) {
    const sdkTier = await toSdkServiceTier(opts.serviceTier);
    if (sdkTier) config.serviceTier = sdkTier;
  }

  const genai = await getSharedGenAI();
  const { result: response } = await withTelemetry(
    { toolName: "gemini_generate_image", model: opts.model, thinkingLevel: opts.thinkingLevel ?? "", serviceTier: opts.serviceTier },
    () => withFetchRetry(() => withAbortableTimeout(
      (signal) => genai.models.generateContent({
        model: opts.model,
        contents: opts.prompt,
        config: { ...config, abortSignal: signal },
      }),
      timeout,
    )),
  );

  const parts = response.candidates?.[0]?.content?.parts ?? [];
  let imageBytes: string | undefined;
  let mimeType: string | undefined;
  let text = "";
  for (const part of parts) {
    if (part.inlineData?.data && imageBytes === undefined) {
      imageBytes = part.inlineData.data;
      mimeType = part.inlineData.mimeType ?? "image/png";
    } else if (part.text) {
      text += part.text;
    }
  }
  return { imageBytes, mimeType, text, diagnostics: extractResponseDiagnostics(response) };
}

/**
 * Python code execution (Gemini code execution feature).
 */
export async function executeCode(
  prompt: string,
  model = DEFAULT_CODE_MODEL,
  returnCode = false,
  serviceTier?: ServiceTierValue,
  thinkingLevel?: ThinkingLevel | null,
): Promise<{ text: string; code?: string; output?: string; diagnostics?: ResponseDiagnostics }> {
  const timeout = resolveTimeout(serviceTier, TIMEOUT_MS * 2);
  const config: GenerateContentConfig = { tools: [{ codeExecution: {} }] };
  const thinkingConfig = await buildThinkingConfig(model, thinkingLevel);
  if (thinkingConfig) config.thinkingConfig = thinkingConfig;
  if (serviceTier) {
    const sdkTier = await toSdkServiceTier(serviceTier);
    if (sdkTier) config.serviceTier = sdkTier;
  }

  const genai = await getSharedGenAI();
  const { result: response } = await withTelemetry(
    { toolName: "gemini_execute_code", model, thinkingLevel: thinkingLevel ?? "", serviceTier },
    () => withFetchRetry(() => withAbortableTimeout(
      (signal) => genai.models.generateContent({
        model,
        contents: prompt,
        config: { ...config, abortSignal: signal },
      }),
      timeout
    )),
  );

  const result: { text: string; code?: string; output?: string; diagnostics?: ResponseDiagnostics } = {
    text: response.text ?? "",
    diagnostics: extractResponseDiagnostics(response),
  };

  if (returnCode && response.candidates?.[0]?.content?.parts) {
    for (const part of response.candidates[0].content.parts) {
      if (part.executableCode?.code) result.code = part.executableCode.code;
      if (part.codeExecutionResult?.output) result.output = part.codeExecutionResult.output;
    }
  }

  return result;
}
