---
name: vibe-inspector-setup
description: Detect installed coding agents and configure VibeInspector's global MCP server and inspector skills with the vibe-inspector CLI. Use when the user asks to install, repair, update, inspect, or automate VibeInspector integration for Codex, Claude Code, OpenCode, Cursor, Grok, or Kimi Code.
---

# VibeInspector Setup

Configure only the global agent targets the user requests.

## Workflow

1. Verify the command is installed with `command -v vibe-inspector`.
2. Run `vibe-inspector --json doctor`. Authentication is not required; stop if the `vibe-inspector-mcp` helper is missing.
3. Run `vibe-inspector --json agents list` and use the stable agent IDs from its response.
4. Preview writes with `vibe-inspector --json setup --agent <id> --dry-run`.
5. When the user requested installation and the plan targets the intended global paths, run `vibe-inspector --json setup --agent <id> --yes`. Repeat `--agent` or use a comma-separated value for multiple targets.
6. Run `vibe-inspector --json agents list` again and report each target's MCP and skill state.

## Rules

- Prefer the installed CLI over editing agent configuration directly.
- Never run `setup --all --yes` unless the user explicitly asked to configure every detected agent.
- Do not print or inspect `VIBE_INSPECTOR_SESSION_SECRET` values.
- Do not edit repository-local agent files; setup is global per user.
- Treat a partial multi-agent result as a failure for the affected targets while preserving successful installs.
- Existing configs receive a first-write `.vibe-inspector-backup`; do not replace that backup.

## Examples

```bash
vibe-inspector --json doctor
vibe-inspector --json agents list
vibe-inspector --json setup --agent codex,opencode --dry-run
```
