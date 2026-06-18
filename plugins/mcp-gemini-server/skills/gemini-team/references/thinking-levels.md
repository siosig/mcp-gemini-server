# thinking_level / service_tier — Detailed Assignment

## thinking_level Usage

| thinking_level | Use | Example target roles |
|----------------|-----|----------------------|
| `minimal` | Classification, routing, simple extraction | parallel workers |
| `low` | Translation, summarization, lightweight analysis | summarizer |
| `medium` | Code generation, standard analysis | developer, analyst |
| `high` | Complex reasoning, design, orchestration, review | architect, reviewer, critic |

## `gemini_custom_agent` Assignment by Role

**Premise**: the orchestrator (Claude) runs at Opus 4.7 effort=max (with deep integration and critical ability). The Gemini side handles exploration of specialist knowledge and generation of counterarguments — an asymmetric design.

| Role | mul | it | mulit | Recommended Model |
|------|-----|-----|-------|-------------------|
| **Specialist** (architect, analyst, developer, reviewer) | `high` | — | Phase 1: `high` | `gemini-flash-latest` (mul); fixed `gemini-3.1-pro-preview` (mulit) |
| **Generator** (code/draft) | — | `medium` | Phase 1 speculative & Phase 2 loop: `medium` | `gemini-flash-latest` (it); fixed `gemini-3.1-pro-preview` (mulit) |
| **Critic - Devil's Advocate** (the refuter in mul) | `high` | — | Phase 1 DA: `high` | `gemini-flash-latest` or `gemini-3.1-pro-preview` for hardest tasks; fixed Pro (mulit) |
| **Critic - Rubric scoring** (it / mulit Phase 2) | — | `high` | Phase 2 loop: `high` | `gemini-flash-latest` (it); fixed `gemini-3.1-pro-preview` (mulit) |
| **Summarizer** | `low` | — | — | `gemini-flash-lite-latest` |
| **Researcher** (grounding needed) | `medium` | — | Phase 1: `medium` | `gemini-flash-latest` |

### Design Principles

- **The mul parallel critical path** is bound by the "slowest agent," so setting Specialists and the DA critic all to `high` does not change the overall time versus a single agent (3× the cost, but quality is maximized). Conversely, dropping only the Specialists to medium yields little time benefit.
- **In the it sequential accumulation**, setting the generator to `high` makes 4 steps reach a P95 of about 29 minutes, exceeding the flex tier timeout (15 minutes), so it is fixed at `medium`. This concentrates the budget on the critic — the Reflexion / Tree of Thoughts pattern (academically proven effective).
- **The default when classification is impossible or unclear is `medium`.**
- **Fallback**: if timeouts or cost overruns occur frequently, demote all agents to `medium` (a safety measure recommended by the performance-engineer).
- **Uniformly assigning `high` is prohibited**: 3× the cost + hallucination induced by overthinking + rate-limit risk.

## service_tier Usage

Can be specified for all tools except `gemini_generate_image`. Priority: tool argument > environment variable `GEMINI_SERVICE_TIER` > none (API default).

| service_tier | Use | Notes |
|-------------|-----|-------|
| `flex` | 50% cost reduction, tolerant of high latency | timeout automatically extended to 15 minutes |
| `priority` | High reliability, premium price | for production / demos |
| `standard` | Explicitly revert to the API default behavior | for overriding the environment variable |
