import { z } from "zod";
import { geminiChat, DEFAULT_AGENT_MODEL, DEFAULT_AGENT_THINKING_LEVEL } from "../services/gemini_client.js";
import {
  pinnedDefaultDescription,
  pinnedThinkingDescription,
  resolveServiceTier,
  serviceTierSchema,
} from "../schemas/helpers.js";

export const customAgentSchema = z.object({
  task: z
    .string()
    .min(1)
    .describe("[REQUIRED] The task for the agent to perform, described in natural language."),
  role: z
    .string()
    .min(1)
    .describe(
      '[REQUIRED] The agent\'s specialized role. Recommended values: "analyst" | "architect" | "developer" | "reviewer" | "critic" | "summarizer" | "researcher". Any free-form string (e.g. "security expert") is also accepted.',
    ),
  persona: z
    .string()
    .optional()
    .describe("Optional: Additional instructions for the agent's personality, tone, or style."),
  model: z
    .string()
    .optional()
    .default(DEFAULT_AGENT_MODEL)
    .describe(pinnedDefaultDescription("gemini_custom_agent", DEFAULT_AGENT_MODEL)),
  thinking_level: z
    .enum(["minimal", "low", "medium", "high"])
    .optional()
    .default(DEFAULT_AGENT_THINKING_LEVEL)
    .describe(pinnedThinkingDescription("gemini_custom_agent", DEFAULT_AGENT_THINKING_LEVEL)),
  service_tier: serviceTierSchema,
  file_path: z
    .string()
    .optional()
    .describe(
      "Optional: Absolute path to a file the server should read and include with the task. " +
      "Supported: source code, Markdown, JSON, YAML, PDF, images, and other common formats.",
    ),
}).strict();

export type CustomAgentArgs = z.infer<typeof customAgentSchema>;

export async function handleCustomAgent(args: CustomAgentArgs): Promise<string> {
  const systemParts = [`You are a ${args.role}.`];
  if (args.persona) {
    systemParts.push(`\nPersonality/style: ${args.persona}`);
  }
  systemParts.push("\n\nApply your expertise to respond to the task.");

  const { text } = await geminiChat(args.task, {
    model: args.model,
    systemInstruction: systemParts.join(""),
    temperature: 0.7,
    thinkingLevel: args.thinking_level,
    toolName: "gemini_custom_agent",
    serviceTier: resolveServiceTier(args.service_tier),
    filePath: args.file_path,
  });
  return text;
}
