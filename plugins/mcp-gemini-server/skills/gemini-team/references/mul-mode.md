# mul Mode Details (Coordinator Pattern)

Claude acts as a manager, splitting the problem and assigning it in parallel to **five or more** Gemini specialist agents, then aggregating and deciding on the results.

## Execution Flow

1. Analyze the task → split the problem and decide the roles of three or more specialist agents. **Assign at least one of them as a "devil's advocate (critic role)" whose job is to keep logically opposing the opinions of the other (n-1) agents, no matter how correct they appear** (to prevent groupthink and to validate assumptions).
2. Read local files (when needed) → embed them into the prompt as context.
3. **Call three or more `gemini_custom_agent` in parallel** (assign each agent a different specialist perspective).
4. Receive and validate all results.
5. Claude, as manager, writes the final answer:
   - A summary of each agent's claims
   - An organization of points of agreement and conflict
   - A decision on each point of conflict, with rationale
   - The final conclusion and recommended actions
   - **Do not force opinions into agreement. When a conflict remains, report the reason (differences in underlying assumptions, values, or trade-offs) and each side's rationale as-is.** Flag any point Claude cannot adjudicate as an "unresolved conflict."

## Devil's Advocate Instruction Template

```javascript
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>Devil's advocate. No matter how correct the other agents' claims appear, always keep presenting logical counterarguments, refutations, and alternatives. Agreement and compromise are forbidden.</role>
<context>${context including a summary of the other agents' claims}</context>
<objective>For ${target theme}, enumerate the weaknesses of the expected rationale, hidden assumptions, falsifiability, and conflicting scenarios, and construct at least 3 counterarguments</objective>
<constraints>Expressing agreement or partial endorsement is forbidden. Only when you judge there is no point to refute, abstain while stating the reason for that judgment</constraints>`,
  role: "critic", thinking_level: "high"  // the mul DA runs in parallel with specialists and is not on the critical path → quality-first high
})
```

## mul Mode — Full Code Example

```javascript
// Call three or more specialist agents in parallel (e.g., architect + analyst + critic[devil's advocate])
// mul mode default model: gemini-flash-latest (balanced; escalate to gemini-3.1-pro-preview for hard critique)
// mul Specialists (architect/analyst/developer/reviewer) use thinking_level: "high"
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>An architect who analyzes technical feasibility</role>
<context>${context}</context>
<objective>${split task}</objective>`,
  role: "architect", thinking_level: "high"  // model default: gemini-flash-latest
})

mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>An analyst who analyzes business requirements and user value</role>
<context>${context}</context>
<objective>${split task}</objective>`,
  role: "analyst", thinking_level: "high"  // model default: gemini-flash-latest
})

// + devil's advocate (see the template above)
```

## Parallelization Rules

- The three `gemini_custom_agent` above **must be issued in parallel within a single message**.
- Firing them serially inflates the LLM wait time by a factor of n (the "always check for parallelizability" principle in CLAUDE.md).
