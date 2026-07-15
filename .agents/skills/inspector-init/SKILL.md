---
name: inspector-init
description: Initialize VibeInspector data for the current registered repository or a user-described scope. Use when the user explicitly invokes $inspector-init, asks to map repository business modules and flows into VibeInspector, or wants the first Inspector snapshot. Do not use for refreshing existing Inspector data or making source-code changes.
---

# Inspector Init

Create the first evidence-backed Inspector snapshot through the VibeInspector MCP server.

## Workflow

1. Read `AGENTS.md`, `.agents/01-system-architecture.md`, and `.agents/03-mcp-and-command-contracts.md`.
2. Call `inspector.project.list`. Match the canonical current working directory to one registered project root. Stop and ask the user to register the project when no exact containing root is allowed.
3. Treat the user's trailing prompt as the requested scope. Use the whole registered repository when no scope is supplied.
4. Inspect manifests, package boundaries, entry points, domain types, handlers, persistence, and tests. Trace business behavior rather than grouping only by folders.
5. Produce modules, flows, renderer-neutral graph nodes/edges, design rationale, and precise repository-relative source references. Keep each reference within the registered root and prefer symbol-sized line ranges.
6. Use temporary IDs prefixed with `new:` for new entities. The server replaces them with UUIDv7 IDs. Keep every cross-reference internally consistent.
7. Call `inspector.command.run` with:
   - `command: "inspector-init"`
   - a unique `requestId`
   - `projectId` and current `expectedRevision`
   - project or explicit natural-language `scope`
   - the complete proposed project snapshot
   - optional conversation/model/prompt provenance
   - `acceptStructuralChanges: true`
8. If the proposal adds more than 25 modules plus flows, summarize the counts and confidence before calling the mutation tool; proceed only after user confirmation and set `acceptLargeChange: true`.
9. Report the committed revision, created module/flow counts, needs-review items, and unreadable evidence. Do not claim success when the MCP mutation fails.

## Rules

- Never write repository source files.
- Never mutate SQLite directly or bypass `inspector.command.run`.
- Do not invent source references that were not inspected.
- Use `needs-review` with lower confidence when evidence is ambiguous.
- Keep modules about business or architectural responsibility, not every directory.
