# it Mode Details (Iterative Refinement Pattern)

Claude manages the feedback loop. It runs the "generating agent" → "critic agent" iteration cycle for a default of **2 loops**.

## Execution Flow

```
=== Loop N ===
1. Generating agent (gemini_custom_agent) → generate/improve a solution
2. Critic agent (gemini_custom_agent, role: "critic") → evaluate against the rubric

=== Termination Conditions ===
- Rubric average score 4.0 or above → early termination
- Specified loop count reached → output as the final deliverable
```

- Each step runs **sequentially** (the output of the previous step is the input to the next).
- Specify the evaluation criteria to the critic agent via the `<evaluation_rubric>` tag.
- If the user specifies a loop count, follow it.

## Rubric Definition

| Axis | Points | Perspective |
|------|------:|-------------|
| Logic | 5 | Are claims and evidence logically connected? |
| Fact-checking | 5 | Verifiability / primary-source reference / falsifiability |
| Coverage | 5 | Omissions in expected use cases and edge cases |
| Actionability | 5 | Concreteness that works in practice; consideration of realistic constraints |

**Early termination when the average score is 4.0 or above.** At 3.0 or below, require the generator to make substantial improvements.

## Code Example

```javascript
// generating agent
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>A developer who produces ${task}</role>
<context>${the previous loop's critic feedback (empty in loop 1)}</context>
<objective>${task body}</objective>`,
  role: "developer", thinking_level: "medium"  // it is sequential, so the generator is fixed at medium
})

// critic agent
mcp__mcp-gemini__gemini_custom_agent({
  task: `<role>Rubric evaluator</role>
<context>${generator output}</context>
<objective>Score 1-5 against the rubric below and return improvement instructions as a bullet list</objective>
<evaluation_rubric>Logic / Fact-checking / Coverage / Actionability (each 1-5)</evaluation_rubric>`,
  role: "critic", thinking_level: "high"  // concentrate the budget on the critic
})
```

## Termination-Check Pseudocode

```python
for loop_n in range(max_loops):
    draft = generator_call(context=last_critic_feedback)
    scores = critic_call(content=draft)  # 4 items × 1-5
    if mean(scores) >= 4.0:
        return draft
    last_critic_feedback = scores.improvement_notes
return draft  # final loop
```
