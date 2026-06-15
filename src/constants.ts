/**
 * Constants used throughout the application.
 */

/** Response character limit to protect the LLM context. */
export const CHARACTER_LIMIT = 25_000;

/** Message appended when the character limit is exceeded. */
export const TRUNCATION_SUFFIX =
  "\n\n... (Response truncated because it exceeded the limit. Add filter conditions or use a more specific query.)";

/**
 * Truncate the trailing portion of a response string if it exceeds CHARACTER_LIMIT.
 */
export function truncate(text: string): string {
  if (text.length <= CHARACTER_LIMIT) return text;
  return text.slice(0, CHARACTER_LIMIT) + TRUNCATION_SUFFIX;
}
