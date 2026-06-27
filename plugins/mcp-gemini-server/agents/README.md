# Bundled Claude Code agents

This directory ships Claude Code **subagent** definitions that compose this MCP
server's Gemini primitives. They are *not* loaded automatically — copy the one
you want into your own Claude Code agents directory:

```bash
# Per-project
mkdir -p .claude/agents && cp agents/gemini-delegate.md .claude/agents/

# All projects (user-level)
mkdir -p ~/.claude/agents && cp agents/gemini-delegate.md ~/.claude/agents/
```

| Agent | Purpose |
|-------|---------|
| [`gemini-delegate.md`](gemini-delegate.md) | Offload a single, self-contained task (research / review / design / summarize / media analysis / code execution) to Gemini in an **isolated context** and return only a distilled result — keeping the main Claude thread small and cheap. |

For multi-agent workflows (parallel coordinator, iterative generate→critique),
see the bundled [`skills/gemini-team`](../skills/gemini-team) skill instead.

The delegation policy is installed as an always-loaded rule at
`~/.claude/rules/mcp-gemini-server.md` by `install_claude_plugin.sh` (no
`UserPromptSubmit` hook). See the README delegation section
([en](../../../README.md) / [ja](../../../README.ja.md)) for details.
