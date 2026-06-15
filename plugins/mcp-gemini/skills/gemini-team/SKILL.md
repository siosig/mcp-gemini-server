---
name: gemini-team
description: Gemini MCP multi-agent skill. Supports crew mode (single-shot execution), mul mode (Coordinator pattern, where Claude acts as a manager directing 3+ specialist agents), it mode (Iterative Refinement pattern, a generate-and-critique feedback loop), and mulit mode (mul→it chained, refining quality after gathering multi-perspective opinions). Claude orchestrates by composing gemini_custom_agent calls on the client side. Use for research, review, design, and documentation.
user-invocable: true
argument-hint: "[crew|mul|it|mulit] [flex|priority|standard] [task description]"
---

# Gemini Team Skill

## Task

$ARGUMENTS

## Argument Parsing

```
$ARGUMENTS → [mode] [service_tier?] [prompt...]
              1st word   2nd word        everything else
```

1. **1st word** → mode detection (`crew`/`mul`/`it`/`mulit`). If it matches none of these, treat the mode as `crew` and include the 1st word as part of the prompt.
2. **2nd word** → if it is one of `flex`/`priority`/`standard`, adopt it as `service_tier`. Otherwise it is part of the prompt.
3. **The rest** → the prompt (the task content).

When `service_tier` is specified, apply it to every MCP call in that session (except `gemini_generate_image`). When omitted, use the default (the API default).

## Mode Selection Guide

| Mode | Pattern | Use Case | Claude's Role | Details |
|--------|---------|----------|---------------|---------|
| `crew` | Single-shot execution | Simple, fast tasks | Receiving results, file operations | Bottom of this file |
| `mul` | Coordinator | Split a problem and gather opinions from multiple specialists in parallel | Manager (split, assign, aggregate, decide) | [references/mul-mode.md](references/mul-mode.md) |
| `it` | Iterative Refinement | Improve quality incrementally through generate→critique loops | Loop management (connecting inputs/outputs, final decision) | [references/it-mode.md](references/it-mode.md) |
| `mulit` | mul → it chained | Polish quality through iterative refinement after gathering multi-perspective opinions | Hand off the mul aggregation result as the initial input to it | [references/mulit-mode.md](references/mulit-mode.md) |

## Available Gemini MCP Tools

| Tool | Use |
|--------|-----|
| `gemini_chat` | Simple Q&A |
| `gemini_custom_agent` | Spawn a specialist agent (multi-agent orchestration is composed from this on the Claude side) |
| `gemini_analyze_media` | Analyze images, PDFs, video, and audio (prefer `file_path`) |
| `gemini_generate_image` | Generate images with Nano Banana 2 (Flash Image) (prompts must be in English; SynthID watermark applied) |
| `gemini_execute_code` | Execute Python code |
| `gemini_manage_files` | Manage Gemini file storage (up to 2GB, retained 48 hours) |

**Handled by Claude** (not possible in Gemini): reading/writing local files, Bash commands, Git operations, and **multi-agent orchestration (implement the mul/it/mulit strategies via parallel/iterative `gemini_custom_agent` calls)**.

### Built-in Agents (Roles)

Recommended roles to use as the `role` of `gemini_custom_agent`. **Free-form roles are also allowed.**

| Name | Description | grounding |
|------|-------------|-----------|
| `analyst` | Problem analysis, requirements definition | No |
| `architect` | System design, architecture | No |
| `developer` | Implementation, code generation | No |
| `reviewer` | Review, quality assurance | No |
| `critic` | Critical evaluation (identifying risks and weaknesses) | No |
| `summarizer` | Summarization, organization | No |
| `researcher` | Research, information gathering | No (*) |

> **(*) All built-in agents have Google Search grounding disabled** (including `researcher`). This is because thinking models such as `gemini-3.1-pro-preview` combined with `thinking_level: high` and grounding produce `MALFORMED_FUNCTION_CALL` / empty responses; grounding was disabled on 2026-06-03. When primary sources are needed, retrieve them with the `tavily_search` tool of `mcp-search` and pass them into `<context>` (see [Grounding Rules](#grounding-rules)).
>
> A custom definition with the same name as a built-in is an error. Define only roles that do not exist, using the dict form.

## Model Selection and Parameters

**For the model priority and the list of usable models, consult your MCP client's model configuration.**

| Parameter | Overview | Details |
|-----------|----------|---------|
| `thinking_level` | Four levels: minimal / low / medium / high. Optimized per role | [references/thinking-levels.md](references/thinking-levels.md) |
| `service_tier` | flex (50% reduction, 15-minute timeout) / priority / standard | Same as above |

**The default when classification is impossible or unclear is `thinking_level=medium`.** Uniformly assigning `high` is prohibited because it triples cost and induces overthinking.

## Grounding Rules

- **Built-in agents have no grounding**, so `researcher` does not search the web automatically. For research that needs primary sources for facts or statistics, Claude retrieves them with the **`tavily_search` tool of `mcp-search`** (SearXNG meta-search + Gemini rerank; set `include_answer=True` to obtain a cited summary) and passes them into the `researcher`'s `<context>`. The researcher then focuses solely on organizing the injected information and assessing its reliability.
- For factual or statistical claims, state the source (URL) and a reliability level (high: official sources/papers, medium: technical blogs, low: personal blogs).
- If no search result is found, state explicitly that "no reliable source was found" and do not present speculation as fact.

## Structured Prompts

In an agent's `task` parameter, separate role, context, and constraints with XML tags. **Use only the tags needed for the situation.**

| Tag | Use | Required |
|------|-----|----------|
| `<role>` | Persona, guiding principles | Recommended |
| `<context>` | Background information, file contents | Recommended |
| `<objective>` | The task to perform | Required |
| `<constraints>` | Constraints | Recommended |
| `<output_schema>` | Output format specification | Optional |
| `<evaluation_rubric>` | Evaluation criteria (for the critic in it mode) | In it mode |

> **Important (tag-name constraint)**: **Do not use `<task>`** as a body tag. The MCP tool's parameter name is `task`, and placing a literal `<task>` tag inside the value of the `task` parameter nests two identically named tags — the tool-call XML's delimiter tag (the `task` parameter) and the body tag `<task>` — which causes the caller to misread the parameter boundary and drop `task` (resulting in `Invalid input: expected string, received undefined`). Always use `<objective>` for the task body.

For per-role templates (Specialist / Devil's Advocate / Rubric Critic), see [references/structured-prompt.md](references/structured-prompt.md).

## Mode Overviews

### crew Mode (Gemini Single-Shot Execution)

Gemini executes on its own. When file writes or Git operations are needed, Claude performs them.

```javascript
mcp__mcp-gemini__gemini_chat({ prompt: task content })  // simple Q&A
mcp__mcp-gemini__gemini_custom_agent({                   // specialist execution
  task: `<role>...</role><context>...</context><objective>...</objective>`,
  role: "architect"
})
```

### mul Mode (Coordinator)

Claude acts as a manager, assigning the problem in parallel to three or more specialist agents (at least one of which is a devil's advocate), then aggregating and deciding. **Do not force conflicts into agreement; flag them explicitly as unresolved.** For the execution flow, the DA template, and parallel-call code examples, see [references/mul-mode.md](references/mul-mode.md).

### it Mode (Iterative Refinement)

Claude manages a "generate → critique" loop. Default of 2 loops, with early termination when the rubric average is 4.0 or above. For the rubric definition, code examples, and the termination-condition pseudocode, see [references/it-mode.md](references/it-mode.md).

### mulit Mode (mul → it chained)

The multi-perspective opinions gathered in mul are consolidated into a unified draft, then polished through the it loop. The key is **speculative parallel execution: calling the 5+ Phase 1 agents and the Phase 2 speculative draft in parallel within the same message.** **Fix all `gemini_custom_agent` calls to `model="gemini-3.1-pro-preview"`, `thinking_level="high"`** (because mulit is a quality-first mode for the highest-difficulty tasks; other modes keep the default model). For details, see [references/mulit-mode.md](references/mulit-mode.md).

## Fallback on Error

1. **Reduce scope** — narrow the range and retry with the same agent.
2. **Alternative approach** — retry with a different role or a fallback model (consult your MCP client's model configuration).
3. **Escalate** — report the situation to the user and ask for a decision.
