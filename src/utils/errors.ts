/**
 * Shared error handling for MCP tool handlers.
 * Classifies API errors by HTTP status and returns a clear message conveyed to the LLM.
 */

import { logger } from "./logger.js";

/**
 * HTTP status codes that may be attached to an API error object.
 */
interface ApiErrorLike extends Error {
  status?: number;
  statusCode?: number;
}

/**
 * Interpret an unknown error as an API error and return an explanatory English string.
 * Can be called directly from a tool handler's catch block.
 */
export function handleApiError(error: unknown, toolName = "unknown"): string {
  if (error instanceof Error) {
    const apiErr = error as ApiErrorLike;
    const status = apiErr.status ?? apiErr.statusCode;
    switch (status) {
      case 400:
        return `Error: The request is invalid. Please check the parameters. (${error.message})`;
      case 401:
        return "Error: Authentication failed. Please check the API key.";
      case 403:
        return "Error: Access denied. Please check the credentials or scopes.";
      case 404:
        return "Error: Resource not found. Please check the ID or path.";
      case 429:
        return "Error: Rate limit reached. Please wait a moment and try again.";
      case 500:
      case 502:
      case 503:
        return `Error: A server error occurred (status: ${status}). Please wait a moment and try again.`;
      default:
        if (error.message.includes("timed out")) {
          return `Error: The request timed out. The operation may be too complex. (${error.message})`;
        }
        logger.error({ err: error, toolName }, "Unexpected error during tool execution");
        return `Error: ${error.message}`;
    }
  }
  return `Error: An unexpected error occurred: ${String(error)}`;
}
