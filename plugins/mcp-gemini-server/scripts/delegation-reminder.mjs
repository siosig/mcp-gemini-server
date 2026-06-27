#!/usr/bin/env node
/**
 * UserPromptSubmit hook for the mcp-gemini-server plugin.
 *
 * Emits a <delegation-policy> block to stdout, which Claude Code injects into
 * context. This is a DEFAULT-ON delegation rule: read-only comprehension /
 * summarization / investigation / review of existing local code/docs whose
 * context fits one prompt is delegated to the gemini-delegate subagent by
 * default, with precision guardrails (exclusions) carried inline so the rule
 * never erodes correctness.
 *
 * No input, no side effects, exit 0.
 */

process.stdout.write(
  "<delegation-policy>\nDEFAULT-ON: For self-contained, read-only work over existing local code/docs — comprehension, summarization, investigation, review — whose full context fits in one prompt, delegate to the gemini-delegate subagent by default and consume only the distilled conclusion. Do NOT delegate (exclusions): (1) final decisions, file edits, Git, and orchestration stay with the main Claude; (2) latest library/SDK/API specs — do not let Gemini adjudicate them; verify via context7/Claude; (3) tasks needing fine-grained sequential control (debugging isolation, staged refactors). Always finally verify delegated results before using them.\n</delegation-policy>",
);
