/**
 * Module that centralizes cross-cutting concerns of API calls (timeout, instrumentation, logging, error handling).
 */

import { logger, logError } from "./logger.js";

/** Patterns for detecting transient network errors such as "fetch failed". */
const TRANSIENT_ERROR_PATTERNS = [
  "fetch failed",
  "ECONNRESET",
  "ECONNREFUSED",
  "ETIMEDOUT",
  "UND_ERR_CONNECT_TIMEOUT",
  "network error",
];

function isTransientError(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  const msg = error.message.toLowerCase();
  return TRANSIENT_ERROR_PATTERNS.some((p) => msg.includes(p.toLowerCase()));
}

/**
 * Retry with exponential backoff on transient network errors (e.g. "fetch failed").
 * An additional retry layer for cases not resolved by the @google/genai SDK's internal retries (maxRetries=2).
 */
export async function withFetchRetry<T>(
  fn: () => Promise<T>,
  maxRetries = 3,
  baseDelayMs = 1000,
): Promise<T> {
  let lastError: unknown;
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;
      if (!isTransientError(error) || attempt === maxRetries) {
        throw error;
      }
      const delay = baseDelayMs * 2 ** attempt;
      const errorMessage = error instanceof Error ? error.message : String(error);
      logger.warn({ attempt: attempt + 1, maxRetries, delay, error: errorMessage }, `[fetchRetry] Transient network error detected, retrying after ${delay}ms (${attempt + 1}/${maxRetries})`);
      await new Promise((resolve) => setTimeout(resolve, delay));
    }
  }
  throw lastError;
}

/**
 * Timeout wrapper that passes an AbortSignal to the caller.
 * By passing the received signal to `@google/genai`'s `config.abortSignal`, the caller enables
 * true cancellation on the SDK side (aborting the HTTP request).
 *
 * Note: per the SDK's behavior, the server-side request is not cancelled and billing still applies (client-side only).
 */
export function withAbortableTimeout<T>(
  fn: (signal: AbortSignal) => Promise<T>,
  ms: number,
): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(
    () => controller.abort(new Error(`Request timed out after ${ms / 1000}s`)),
    ms,
  );
  return fn(controller.signal).finally(() => clearTimeout(timer));
}

/**
 * Simple timeout. For calls that do not accept an AbortSignal (short operations such as the Files API).
 * Promise.race returns the timeout error first, but note that the background API call continues.
 * Use `withAbortableTimeout` when true cancellation is required.
 */
export function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return Promise.race([
    promise,
    new Promise<never>((_, reject) => {
      timer = setTimeout(
        () => reject(new Error(`Request timed out after ${ms / 1000}s`)),
        ms,
      );
    }),
  ]).finally(() => {
    if (timer) clearTimeout(timer);
  });
}

export interface TelemetryOptions {
  toolName: string;
  model: string;
  thinkingLevel?: string | undefined;
  serviceTier?: string | undefined;
}

/**
 * Higher-order function that centralizes cross-cutting concerns of API calls (instrumentation, error handling).
 *
 * Returns the elapsed time (durationMs) to the caller on success. Writes an error log to stderr only on failure
 * (success access logs, pricing, and syslog forwarding were removed in spec 021).
 */
export async function withTelemetry<T>(
  opts: TelemetryOptions,
  apiFn: () => Promise<T>,
): Promise<{ result: T; durationMs: number }> {
  const start = performance.now();
  try {
    const result = await apiFn();
    const durationMs = performance.now() - start;
    return { result, durationMs };
  } catch (err) {
    const durationMs = performance.now() - start;
    logError("mcp_error", opts.toolName, opts.model, durationMs, err, opts.thinkingLevel ?? "");
    throw err;
  }
}
