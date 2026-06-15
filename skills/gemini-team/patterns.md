# gemini-team Patterns and Templates

A supplement to SKILL.md. Collects structured-prompt templates, rubric definitions, and success / failure-recovery examples.

---

## Structured-Prompt Templates

### `<role>` Definitions per Role

| Role | Example `<role>` definition |
|--------|----------------------------|
| analyst | A dispassionate analyst who extracts objective insights grounded in data and facts. Rigorously distinguishes correlation from causation and eliminates bias. |
| architect | A senior system architect who prioritizes non-functional requirements (scalability, security, maintainability). Always presents alternatives and trade-offs. |
| developer | A pragmatic developer who evaluates implementation feasibility and technical debt. Includes the perspectives of cost, maintainability, and performance. |
| reviewer | A reviewer who systematically validates deliverables against quality standards. Checks alignment with requirements, logical breakdowns, and security risks. |
| critic | A critic who, as a devil's advocate, exhaustively points out weaknesses, risks, and blind spots. Always attaches evidence and a countermeasure to each point. |
| summarizer | An organization specialist who structures large volumes of information, extracts key points, and summarizes them concisely. Clarifies the priority and hierarchy of information. |
| researcher | A researcher who organizes the primary sources Claude retrieved via `tavily_search` of `mcp-search` and reports them together with a source reliability level (high/medium/low). Has no grounding of its own. When there is no source, states explicitly that "no reliable source was found." |

### Full Template Example (for the architect in mul mode)

```
<role>
A senior system architect who prioritizes non-functional requirements
(scalability, security, maintainability). Always present alternatives and trade-offs.
</role>

<context>
${file contents Claude has Read, or background information}
</context>

<objective>
${the specific task split out from the user's request}
</objective>

<constraints>
- Avoid unfounded speculation; argue based on technical facts.
- Present alternatives and trade-offs together.
- If there are any security concerns, always point them out.
</constraints>

<output_schema>
## Analysis
### Recommendation
- Overview: ...
- Rationale: ...
### Alternatives
- Overview: ...
- Trade-offs: ...
### Risks and Caveats
- ...
</output_schema>
```

### Tag-Usage Guide

| Situation | Tags to use |
|-----------|-------------|
| Simple Q&A (crew) | `<objective>` alone is sufficient |
| Parallel analysis (mul) | `<role>` + `<context>` + `<objective>` + `<constraints>` |
| Review / evaluation | The above + `<output_schema>` to standardize the output format |
| Critique in iterative refinement (it) | The above + `<evaluation_rubric>` |

---

## it Mode Rubric — Detailed Definition

### Evaluation Criteria (4 items × 5 levels)

| Item | 1 (fail) | 2 (insufficient) | 3 (standard) | 4 (good) | 5 (excellent) |
|------|----------|------------------|--------------|----------|----------------|
| **Logic** | Logic is broken | Has leaps or contradictions | Mostly coherent but partly vague | Clear and consistent reasoning | Preempts and refutes counterarguments |
| **Fact-checking** | No evidence / misinformation | Some sources present | Sources for the main claims | Sources for all claims | Primary sources + reliability assessment |
| **Coverage** | Most requirements unconsidered | Only major requirements considered | Major requirements + some edge cases | Requirements + major edge cases covered | Requirements + edge cases + future concerns |
| **Actionability** | Abstract and unexecutable | Direction only | Mostly actionable | Concrete steps present | Steps + priority + resource estimates |

### Scoring Rules

- **Average 4.0 or above**: quality is sufficient. End the loop and treat it as the final deliverable.
- **Average 3.0–3.9**: room for improvement. Continue the loop.
- **Average below 3.0**: substantial improvement needed. Give the generating agent concrete revision instructions.
- **A specific item at 2 or below**: give the generating agent improvement instructions focused on that item.

### Example Output Format for the Critic Agent

```markdown
## Evaluation

| Item | Score (1-5) | Rationale |
|------|-------------|-----------|
| Logic | 4 | The claims are consistent, but the explanation of the X–Y relationship is insufficient |
| Fact-checking | 3 | Claim A has a source, but the source of statistic B is unknown |
| Coverage | 3 | The normal cases are covered, but edge case Z is not considered |
| Actionability | 4 | Concrete steps are shown. Adding priorities would make it even better |
| **Average** | **3.5** | |

## Key Improvements
1. Add a reliable source for statistic B.
2. Add handling for edge case Z (when XX occurs).
3. Assign priorities to the execution steps.
```

---

## Success and Failure-Recovery Examples

### Success Example: Research Task (mul mode)

**Task**: "Research a cost comparison of Kubernetes vs ECS."

**Good branching**:
- analyst produces a quantitative cost comparison table (EC2 instance cost + management cost).
- architect compares on non-functional requirements (operational burden, scaling characteristics).
- Claude retrieves the latest pricing from the AWS/GCP pricing pages via `tavily_search` of `mcp-search`, and researcher organizes it and reports it with URLs.

**Quality points**:
- Every agent's claim has a source attached.
- The analyst's and architect's perspectives are complementary (cost vs architecture).
- During Claude's integration, conflicts are made explicit and the decision rationale is described.

### Failure-Recovery Example: Insufficient Agent Response Quality

**Situation**: researcher was called without being given the primary sources (the results of `tavily_search` of `mcp-search`), so it answered from internal knowledge only (sources missing). Note that built-in agents have no grounding, so researcher cannot search the web on its own.

**Recovery steps**:
1. Claude detects that "sources are not attached."
2. Claude retrieves primary sources with the `tavily_search` tool of `mcp-search`, injects them into the researcher's `<context>`, and retries:
   ```
   <context>
   ${primary sources (with URLs) retrieved via tavily_search of mcp-search}
   </context>
   <constraints>
   - Base your answer solely on the sources given in context, and attach a URL to each claim.
   - Do not speculate about anything not in context; state explicitly that "no reliable source was found."
   </constraints>
   ```
3. If the retry is still insufficient → retry with a different model (fallback).
4. If it still fails → escalate to the user.

### Failure-Recovery Example: Score Does Not Improve in it Mode

**Situation**: the rubric average is still below 3.0 even at loop 2.

**Recovery steps**:
1. Claude analyzes the critique results and identifies the lowest-scoring item.
2. Change the generating agent's role (e.g., developer → architect).
3. Add `<constraints>` focused on the low-scoring item and regenerate.
4. If there is still no improvement at loop 3 → escalate to the user ("The XX perspective is not improving. Please provide additional context or direction.").

---

## mul Mode — Full Code Example

### Three-Agent Parallel Call (technical, analytical, and risk perspectives)

```javascript
// Agent 1: technical perspective
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A specialist who analyzes from the perspective of technical feasibility and architecture</role>
<context>${context}</context>
<objective>${split task 1}</objective>
<constraints>
- Avoid unfounded speculation; argue based on technical facts.
- Present alternatives and trade-offs together.
</constraints>`,
  role: "architect",
  thinking_level: "high"
})

// Agent 2: requirements-analysis / feasibility perspective
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A specialist who evaluates requirements analysis and feasibility</role>
<context>${context}</context>
<objective>${split task 2}</objective>
<constraints>
- Provide quantitative evidence.
- Include the perspectives of resources and effort.
</constraints>`,
  role: "analyst",
  thinking_level: "high"
})

// Agent 3: risk / quality perspective
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A specialist who, as a devil's advocate, exhaustively points out risks and weaknesses</role>
<context>${context}</context>
<objective>${split task 3}</objective>
<constraints>
- Doubt optimistic assumptions and assume the worst case.
- Always attach evidence and a countermeasure to each point.
</constraints>`,
  role: "critic",
  thinking_level: "high"
})

// --- Claude, as manager, aggregates all results ---
// Output format:
// ## Each Agent's Claims
// ### Agent-1 (architect): ...
// ### Agent-2 (analyst): ...
// ### Agent-3 (critic): ...
// ## Points of Agreement
// ## Points of Conflict and Decisions
// ## Final Conclusion
```

---

## it Mode — Full Code Example

### Loop 1: Initial Generation → Critique

```javascript
// Step 1: generating agent — generate a solution
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A specialist who generates a solution to the problem</role>
<context>${context}</context>
<objective>Generate a solution for the following problem.\n\nProblem: ${task content}</objective>
<constraints>
- State your rationale.
- If alternatives exist, present them together.
</constraints>`,
  role: "developer",
  thinking_level: "high"
})

// Step 2: critic agent — evaluate against the rubric
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A quality-assurance specialist who critically evaluates the solution</role>
<context>Original problem: ${task content}</context>
<objective>Evaluate the following solution against the evaluation criteria (rubric).\n\nSolution:\n${loop 1 generation result}</objective>
<evaluation_rubric>
Evaluate the following 4 items, each out of 5 points, and provide a score, rationale, and improvement suggestion:
1. Logic — Are the claims well-reasoned?
2. Fact-checking — Are they based on sources and evidence?
3. Coverage — Are requirements and edge cases considered?
4. Actionability — Can they be turned into concrete actions?
</evaluation_rubric>
<output_schema>
## Evaluation
| Item | Score (1-5) | Rationale |
|------|-------------|-----------|
| Logic | X | ... |
| Fact-checking | X | ... |
| Coverage | X | ... |
| Actionability | X | ... |
| **Average** | **X.X** | |
## Key Improvements
- ...
</output_schema>`,
  role: "critic",
  thinking_level: "high"
})
```

### Loop 2: Improvement Based on the Critique → Re-evaluation

```javascript
// Step 3: generating agent — produce an improved version reflecting the critique
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A specialist who improves the solution based on the critique</role>
<context>Original problem: ${task content}\n\nPrevious solution:\n${loop 1 generation result}</context>
<objective>Improve the solution based on the following critique.\n\nCritique:\n${loop 1 critique result}</objective>
<constraints>
- Address every item raised in the critique.
- For any item you cannot address, state the reason.
</constraints>`,
  role: "developer",
  thinking_level: "high"
})

// Step 4: critic agent — re-evaluate the improved version against the same rubric
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A quality-assurance specialist who re-evaluates the improved solution</role>
<context>Original problem: ${task content}\n\nPrevious critique:\n${loop 1 critique result}</context>
<objective>Re-evaluate the improved solution against the same evaluation criteria (rubric). Confirm that the previous critique has been properly reflected.\n\nImproved solution:\n${loop 2 generation result}</objective>
<evaluation_rubric>
Re-evaluate the following 4 items, each out of 5 points, and note the degree of improvement from the previous round:
1. Logic 2. Fact-checking 3. Coverage 4. Actionability
</evaluation_rubric>`,
  role: "critic",
  thinking_level: "high"
})

// === Claude, as manager, makes the final decision ===
// If the rubric average is below 4.0, consider an additional loop.
// Output format:
// ## Progress of Iterative Refinement
// ### Loop 1
// - Generation: ...
// - Critique (score): ...
// ### Loop 2
// - Improvement: ...
// - Re-evaluation (score): ...
// ## Remaining Points
// ## Final Deliverable
```
