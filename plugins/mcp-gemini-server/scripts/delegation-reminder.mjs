#!/usr/bin/env node
/**
 * UserPromptSubmit hook for the mcp-gemini-server plugin.
 *
 * Emits a single-line <delegation-check> reminder to stdout, which Claude Code
 * injects into context. Nudges Claude to consider delegating a self-contained
 * task to the gemini-delegate subagent (or the gemini-team skill) before
 * answering, keeping the main thread small.
 *
 * No input, no side effects, exit 0.
 */

process.stdout.write(
  "<delegation-check>If this turn contains an independent, context-packageable task (research/review/design/summarize/media analysis/code execution), consider delegating it to gemini-delegate before answering. Final decisions, file edits/Git, and orchestration stay with Claude. Skip for trivial replies.</delegation-check>",
);
