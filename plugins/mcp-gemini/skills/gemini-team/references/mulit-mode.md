# mulit Mode Details (mul → it chained)

A composite mode that takes the result of gathering and aggregating multi-perspective opinions in `mul` mode, hands it off as the initial draft for `it` mode, and polishes it through iterative refinement. Effective for tasks such as design, proposals, and reviews where you want to "gather broad opinions and then refine."

## Model and thinking_level (fixed for mulit)

Because mulit mode is a highest-difficulty task that requires integrating multi-perspective opinions and iterative refinement, it prioritizes quality above all. **Fix the following for every `gemini_custom_agent` call** (Phase 1 Specialist / devil's advocate / researcher, and Phase 2 speculative generation / generator / critic):

| Parameter | Value |
|-----------|-------|
| `model` | `gemini-3.1-pro-preview` |
| `thinking_level` | `high` |

> Other modes (`crew`/`mul`/`it`) keep the default model (`gemini-flash-latest`) and a per-role `thinking_level`. Fixing to Pro is limited to mulit mode (cost optimization). Do not use `thinking_level="minimal"`, because it is rejected by the API for `gemini-3.1-pro-preview`.

## Execution Flow (Speculative Parallel Execution)

Avoid serial execution: **call the 5+ Phase 1 agents and the Phase 2 initial-draft generating agent in parallel within the same message**. This removes the latency of one round trip for the Phase 2 loop 1 generator call.

```
=== Phase 1 + Phase 2 initial generation (parallel, submitted together in one message) ===
1. Analyze the task → split the problem and decide the roles of five or more specialist agents. **Assign at least one of them as a "devil's advocate (critic role)" whose job is to keep logically opposing the opinions of the other (n-1) agents, no matter how correct they appear** (for the instruction template, see mul mode).
2. Read local files (when needed).
3. Call all of the following **in parallel within the same message**:
   - Phase 1: five or more gemini_custom_agent (for gathering multi-perspective opinions, one of which is a devil's advocate)
     - **All agents `model="gemini-3.1-pro-preview"`, `thinking_level="high"`** (architect/analyst/developer/reviewer/devil's-advocate critic/researcher, all of them)
   - Phase 2 speculative: one generating agent (generate an initial draft ahead of time using only the task description as context; `model="gemini-3.1-pro-preview"`, `thinking_level="high"`)

=== Phase 1 aggregation ===
4. Claude, as manager, aggregates the Phase 1 results into a "unified draft."
   - A summary of each agent's claims / an organization of points of agreement and conflict with decision rationale / conclusion and recommended actions
   - **Do not force opinions into agreement. When a conflict remains, write the reason (differences in underlying assumptions, values, or trade-offs) and each side's rationale into the unified draft as-is, and hand it off explicitly to the Phase 2 critic as an "unresolved issue."**

=== Phase 2 it loop ===
5. Submit all of the following together to the loop 1 critic (gemini_custom_agent, role="critic", `model="gemini-3.1-pro-preview"`, `thinking_level="high"`):
   - The Phase 2 speculative initial draft (`<draft>`)
   - The Phase 1 unified draft (`<context>`, with conflicts and unresolved issues made explicit in `<objective>`)
   - The rubric (`<evaluation_rubric>`)
6. Run the critic → generator → critic iteration for a default of 2 loops (**all `model="gemini-3.1-pro-preview"`, `thinking_level="high"`**).
7. Output the final deliverable upon the early-termination condition (rubric average 4.0 or above) or reaching the specified loop count.
```

## Parallelization Rules

- **Step 3 must issue all MCP calls in parallel within a single message.** Throwing the Phase 2 speculative call only after Phase 1 completes serializes it and slows it down.
- The speculative draft is generated without Phase 1 context, so it may not reflect Phase 1's multiple perspectives. The loop 1 critic evaluates the gap between the two, and the loop 2 generator fills it.
- Phase 1 aggregation (step 4) is done locally by Claude, so no MCP round trip is needed. Run it immediately once all the parallel calls from step 3 are in.
- **mulit mode unifies all roles to `model="gemini-3.1-pro-preview"`, `thinking_level="high"`** (see "Model and thinking_level (fixed for mulit)" above). Because the Phase 1 parallel critical path is bound by the slowest agent, setting all Specialists to `high` does not change the overall time while maximizing quality. The generator is also `high`, ensuring maximum precision at each round trip of the iterative refinement.

## Hand-off Rules

- Pass the Phase 1 unified draft in the `<context>` tag.
- Make the conflicts and unresolved issues identified in Phase 1 explicit in the `<objective>` tag, and **try to resolve them with priority in Phase 2. However, do not force agreement; any conflict still unresolved in Phase 2 is retained in the "Unresolved Issues" section of the final deliverable, together with each side's rationale.**
- Add the following perspectives to the rubric of the Phase 2 critic agent: "Are Phase 1's multiple perspectives sufficiently reflected?", "Is the difference from the speculative draft integrated appropriately?", and "Have the Phase 1 devil's advocate's counterarguments been adequately considered and reflected?"

## For Details on Each Phase, See the mul / it Mode Documentation

- [mul mode details](mul-mode.md)
- [it mode details](it-mode.md)
