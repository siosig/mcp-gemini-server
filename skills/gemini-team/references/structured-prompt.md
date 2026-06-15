# Structured Prompts — Details

In an agent's `task` parameter, separate role, context, and constraints with XML tags. **Use only the tags needed for the situation.**

| Tag | Use | Required |
|------|-----|----------|
| `<role>` | Persona, guiding principles | Recommended |
| `<context>` | Background information, file contents | Recommended |
| `<objective>` | The task to perform | Required |
| `<constraints>` | Constraints | Recommended |
| `<output_schema>` | Output format specification | Optional |
| `<evaluation_rubric>` | Evaluation criteria (for the critic in it mode) | In it mode |

## Template Examples

### Specialist (architect)

```xml
<role>A system architect who calmly analyzes technical feasibility and trade-offs</role>
<context>${codebase summary / related files / existing architecture}</context>
<objective>${specific design problem}</objective>
<constraints>Prefer the existing stack / no over-engineering / state verifiable decision rationale</constraints>
<output_schema>## Conclusion / ## Rationale / ## Trade-offs / ## Remaining Issues</output_schema>
```

### Critic (Devil's Advocate)

```xml
<role>Devil's advocate. No matter how correct the other agents' claims appear, always keep presenting logical counterarguments, refutations, and alternatives. Agreement and compromise are forbidden.</role>
<context>${context including a summary of the other agents' claims}</context>
<objective>For ${target theme}, enumerate the weaknesses of the expected rationale, hidden assumptions, falsifiability, and conflicting scenarios, and construct at least 3 counterarguments</objective>
<constraints>Expressing agreement or partial endorsement is forbidden. Only when you judge there is no point to refute, abstain while stating the reason for that judgment</constraints>
```

### Critic (Rubric scoring, it mode)

```xml
<role>Rubric evaluator. Mechanically score the presented deliverable</role>
<context>${deliverable body + rubric definition}</context>
<objective>Score each item 1-5 and return improvement instructions as a bullet list</objective>
<evaluation_rubric>
- Logic (1-5): connection between claims and evidence
- Fact-checking (1-5): verifiability / primary-source reference
- Coverage (1-5): few omissions
- Actionability (1-5): concreteness that works in practice
</evaluation_rubric>
<output_schema>per-item score table + improvement instructions + overall average</output_schema>
```
