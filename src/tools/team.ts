import fs from "node:fs/promises";
import path from "node:path";
import { z } from "zod";
import {
  geminiChat,
  DEFAULT_TEAM_MODEL,
  DEFAULT_TEAM_THINKING_LEVEL,
  type ThinkingLevel,
} from "../services/gemini_client.js";
import {
  booleanLike,
  numberLike,
  pinnedDefaultDescription,
  pinnedThinkingDescription,
  resolveServiceTier,
  serviceTierSchema,
} from "../schemas/helpers.js";
import type { ToolResult } from "./registry.js";

// ==================== Schema ====================

export const teamSchema = z
  .object({
    task: z
      .string()
      .min(1)
      .describe(
        "[REQUIRED] The task for the team. For 'it' mode: describe what to generate and include rubric criteria. For 'mul'/'mulit': describe the analysis or decision to make.",
      ),
    mode: z
      .enum(["mul", "it", "mulit"])
      .describe(
        "[REQUIRED] Orchestration mode. mul=parallel specialists+aggregate, it=generate→critique loop, mulit=mul Phase1 then it Phase2 chained.",
      ),
    file_paths: z
      .array(z.string())
      .optional()
      .describe(
        "Optional: Local UTF-8 text file paths. The MCP server reads these files server-side; Claude does NOT need to read them. Binary files are not supported.",
      ),
    roles: z
      .array(z.string())
      .optional()
      .default(["analyst", "architect", "developer", "reviewer", "critic"])
      .describe(
        'Optional: Specialist agent roles for Phase1 (mul/mulit). Defaults: ["analyst","architect","developer","reviewer","critic"].',
      ),
    max_iterations: numberLike
      .pipe(z.number().int().min(0).max(10))
      .optional()
      .default(2)
      .describe(
        "Optional: Number of critic→generator iterations for it/mulit. Default: 2. 0=skip loop.",
      ),
    intermediate_results: booleanLike
      .optional()
      .default(false)
      .describe(
        "Optional: If true, returns structured output including each agent's individual output. Default: false.",
      ),
    model: z
      .string()
      .optional()
      .default(DEFAULT_TEAM_MODEL)
      .describe(pinnedDefaultDescription("gemini_team", DEFAULT_TEAM_MODEL)),
    thinking_level: z
      .enum(["minimal", "low", "medium", "high"])
      .optional()
      .default(DEFAULT_TEAM_THINKING_LEVEL)
      .describe(pinnedThinkingDescription("gemini_team", DEFAULT_TEAM_THINKING_LEVEL)),
    service_tier: serviceTierSchema,
  })
  .strict();

export type TeamArgs = z.infer<typeof teamSchema>;

// ==================== Internal Types ====================

interface AgentResult {
  role: string;
  output: string;
  durationMs: number;
  error?: string;
}

interface TeamMetadata {
  mode: "mul" | "it" | "mulit";
  agentCount: number;
  iterations: number;
  fileCount: number;
  failedAgents: string[];
  totalDurationMs: number;
}

// ==================== File Context Helper ====================

async function buildFileContext(filePaths: string[]): Promise<string> {
  const parts = await Promise.all(
    filePaths.map(async (fp) => {
      const content = await fs.readFile(fp, "utf-8");
      const name = path.basename(fp);
      return `--- File: ${name} (${fp}) ---\n${content}`;
    }),
  );
  return parts.join("\n\n");
}

// ==================== Phase 1: Parallel Specialist Agents ====================

async function runPhase1Agents(
  fullTask: string,
  args: TeamArgs,
): Promise<{ results: AgentResult[]; failedRoles: string[] }> {
  const roles = args.roles ?? ["analyst", "architect", "developer", "reviewer", "critic"];
  const serviceTier = resolveServiceTier(args.service_tier);

  const settled = await Promise.allSettled(
    roles.map(async (role): Promise<AgentResult> => {
      const start = Date.now();
      const { text } = await geminiChat(fullTask, {
        model: args.model,
        systemInstruction: `You are a ${role}. Apply your expertise to analyze the task and provide your perspective.`,
        temperature: 0.7,
        thinkingLevel: args.thinking_level as ThinkingLevel,
        toolName: "gemini_team",
        serviceTier,
      });
      return { role, output: text, durationMs: Date.now() - start };
    }),
  );

  const results: AgentResult[] = [];
  const failedRoles: string[] = [];

  for (let i = 0; i < settled.length; i++) {
    const item = settled[i];
    const role = roles[i] ?? `role_${i}`;
    if (item?.status === "fulfilled") {
      results.push(item.value);
    } else {
      const reason = item?.status === "rejected" ? String(item.reason) : "unknown error";
      failedRoles.push(role);
      results.push({ role, output: "", durationMs: 0, error: reason });
    }
  }

  const successCount = results.filter((r) => !r.error).length;
  if (successCount === 0) {
    const lastError = results[results.length - 1]?.error ?? "all agents failed";
    throw new Error(`gemini_team: All specialist agents failed. Last error: ${lastError}`);
  }

  return { results, failedRoles };
}

// ==================== Phase 1: Aggregation ====================

async function aggregatePhase1(
  agentResults: AgentResult[],
  task: string,
  args: TeamArgs,
): Promise<string> {
  const successResults = agentResults.filter((r) => !r.error);
  const combined = successResults
    .map((r) => `--- ${r.role.toUpperCase()} ---\n${r.output}`)
    .join("\n\n");

  const aggregationPrompt = `${combined}\n\n--- Original Task ---\n${task}`;
  const serviceTier = resolveServiceTier(args.service_tier);

  const { text } = await geminiChat(aggregationPrompt, {
    model: args.model,
    systemInstruction:
      "You are a coordinator. Synthesize the multiple specialist perspectives above into a unified recommendation. " +
      "Identify consensus and explicitly flag any unresolved conflicts with each side's rationale.",
    temperature: 0.5,
    thinkingLevel: args.thinking_level as ThinkingLevel,
    toolName: "gemini_team",
    serviceTier,
  });
  return text;
}

// ==================== mul mode ====================

async function runMul(
  fullTask: string,
  args: TeamArgs,
): Promise<{ text: string; agentResults: AgentResult[]; failedRoles: string[] }> {
  const { results, failedRoles } = await runPhase1Agents(fullTask, args);
  const text = await aggregatePhase1(results, fullTask, args);
  return { text, agentResults: results, failedRoles };
}

// ==================== it mode ====================

async function generateInitialDraft(fullTask: string, args: TeamArgs): Promise<string> {
  const serviceTier = resolveServiceTier(args.service_tier);
  const { text } = await geminiChat(fullTask, {
    model: args.model,
    systemInstruction:
      "You are a writer. Generate an initial draft based on the following task description.",
    temperature: 0.7,
    thinkingLevel: args.thinking_level as ThinkingLevel,
    toolName: "gemini_team",
    serviceTier,
  });
  return text;
}

function extractRubricScore(critiqueText: string): number | undefined {
  const match = critiqueText.match(/(?:overall|average|score)[^\d]*(\d+(?:\.\d+)?)\s*(?:\/\s*5)?/i);
  if (match?.[1]) {
    const score = parseFloat(match[1]);
    return isNaN(score) ? undefined : score;
  }
  return undefined;
}

async function runItLoop(
  initialDraft: string,
  fullTask: string,
  args: TeamArgs,
): Promise<{ text: string; iterations: number }> {
  const serviceTier = resolveServiceTier(args.service_tier);
  let draft = initialDraft;
  let actualIterations = 0;

  for (let i = 1; i <= args.max_iterations; i++) {
    const { text: critique } = await geminiChat(
      `--- Draft ---\n${draft}\n\n--- Original Task ---\n${fullTask}`,
      {
        model: args.model,
        systemInstruction:
          "You are a critic. Evaluate the draft against the task requirements. " +
          "Provide specific, actionable feedback. " +
          "End your response with: 'Overall score: X/5' where X is an average quality score (1-5).",
        temperature: 0.3,
        thinkingLevel: args.thinking_level as ThinkingLevel,
        toolName: "gemini_team",
        serviceTier,
      },
    );

    actualIterations = i;
    const score = extractRubricScore(critique);
    if (score !== undefined && score >= 4.0) {
      break;
    }

    if (i < args.max_iterations) {
      const { text: improved } = await geminiChat(
        `--- Current Draft ---\n${draft}\n\n--- Critic Feedback ---\n${critique}\n\n--- Original Task ---\n${fullTask}`,
        {
          model: args.model,
          systemInstruction:
            "You are a writer. Improve the draft based on the critic's feedback while preserving its strengths.",
          temperature: 0.7,
          thinkingLevel: args.thinking_level as ThinkingLevel,
          toolName: "gemini_team",
          serviceTier,
        },
      );
      draft = improved;
    }
  }

  return { text: draft, iterations: actualIterations };
}

async function runIt(
  fullTask: string,
  args: TeamArgs,
): Promise<{ text: string; iterations: number }> {
  if (args.max_iterations === 0) {
    const text = await generateInitialDraft(fullTask, args);
    return { text, iterations: 0 };
  }
  const initialDraft = await generateInitialDraft(fullTask, args);
  return runItLoop(initialDraft, fullTask, args);
}

// ==================== mulit mode ====================

async function runMulit(
  fullTask: string,
  originalTask: string,
  args: TeamArgs,
): Promise<{ text: string; agentResults: AgentResult[]; failedRoles: string[]; iterations: number }> {
  const roles = args.roles ?? ["analyst", "architect", "developer", "reviewer", "critic"];
  const serviceTier = resolveServiceTier(args.service_tier);

  // Phase1 specialists + Phase2 speculative initial draft in parallel
  const phase1Promises = roles.map(async (role): Promise<AgentResult> => {
    const start = Date.now();
    const { text } = await geminiChat(fullTask, {
      model: args.model,
      systemInstruction: `You are a ${role}. Apply your expertise to analyze the task and provide your perspective.`,
      temperature: 0.7,
      thinkingLevel: args.thinking_level as ThinkingLevel,
      toolName: "gemini_team",
      serviceTier,
    });
    return { role, output: text, durationMs: Date.now() - start };
  });

  const speculativePromise = geminiChat(originalTask, {
    model: args.model,
    systemInstruction: "You are a writer. Generate an initial draft based on the following task description.",
    temperature: 0.7,
    thinkingLevel: args.thinking_level as ThinkingLevel,
    toolName: "gemini_team",
    serviceTier,
  }).then((r) => r.text);

  const [settledPhase1, speculativeDraft] = await Promise.all([
    Promise.allSettled(phase1Promises),
    speculativePromise,
  ]);

  // Process Phase1 results
  const agentResults: AgentResult[] = [];
  const failedRoles: string[] = [];
  for (let i = 0; i < settledPhase1.length; i++) {
    const item = settledPhase1[i];
    const role = roles[i] ?? `role_${i}`;
    if (item?.status === "fulfilled") {
      agentResults.push(item.value);
    } else {
      const reason = item?.status === "rejected" ? String(item.reason) : "unknown error";
      failedRoles.push(role);
      agentResults.push({ role, output: "", durationMs: 0, error: reason });
    }
  }

  const successCount = agentResults.filter((r) => !r.error).length;
  if (successCount === 0) {
    throw new Error("gemini_team: All Phase1 specialist agents failed.");
  }

  // Phase1 aggregation
  const aggregatedContext = await aggregatePhase1(agentResults, fullTask, args);

  // Phase2 it loop using speculative draft as initial + aggregated context
  const { text, iterations } = await runItLoop(
    speculativeDraft,
    `${aggregatedContext}\n\n--- Original Task ---\n${originalTask}`,
    args,
  );

  return { text, agentResults, failedRoles, iterations };
}

// ==================== Main Handler ====================

export async function handleTeam(args: TeamArgs): Promise<string | ToolResult> {
  const startMs = Date.now();

  const fileContext =
    args.file_paths && args.file_paths.length > 0 ? await buildFileContext(args.file_paths) : "";

  const fullTask = fileContext ? `${fileContext}\n\n--- Task ---\n${args.task}` : args.task;
  const fileCount = args.file_paths?.length ?? 0;

  if (args.mode === "mul") {
    const { text, agentResults, failedRoles } = await runMul(fullTask, args);

    if (!args.intermediate_results) return text;

    const metadata: TeamMetadata = {
      mode: "mul",
      agentCount: agentResults.filter((r) => !r.error).length,
      iterations: 0,
      fileCount,
      failedAgents: failedRoles,
      totalDurationMs: Date.now() - startMs,
    };
    return { text, structured: { phases: agentResults, metadata } };
  }

  if (args.mode === "it") {
    const { text, iterations } = await runIt(fullTask, args);

    if (!args.intermediate_results) return text;

    const metadata: TeamMetadata = {
      mode: "it",
      agentCount: 0,
      iterations,
      fileCount,
      failedAgents: [],
      totalDurationMs: Date.now() - startMs,
    };
    return { text, structured: { phases: [], metadata } };
  }

  // mulit
  const { text, agentResults, failedRoles, iterations } = await runMulit(
    fullTask,
    args.task,
    args,
  );

  if (!args.intermediate_results) return text;

  const metadata: TeamMetadata = {
    mode: "mulit",
    agentCount: agentResults.filter((r) => !r.error).length,
    iterations,
    fileCount,
    failedAgents: failedRoles,
    totalDurationMs: Date.now() - startMs,
  };
  return { text, structured: { phases: agentResults, metadata } };
}
