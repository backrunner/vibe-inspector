---
name: inspector-init
description: Initialize or deepen VibeInspector data for the current repository (auto-importing it when needed) or a user-described scope. Use when the user invokes $inspector-init, asks to map repository business modules and flows, wants the first Inspector snapshot, or repeats init to improve coverage and granularity. Do not use for a narrow evidence-only refresh or source-code changes.
---

# Inspector Init

Create or deepen an evidence-backed Inspector snapshot through the VibeInspector MCP server.

## Workflow

1. Read the repository's `AGENTS.md` and any agent documentation it ships (for example `.agents/` docs) when present; skip files that do not exist.
2. Call `inspector.project.list`. Match the canonical current working directory to one registered project root. When no exact containing root is listed, auto-import the project with `inspector.project.create` using the canonical working-directory root; the call is idempotent and returns the already-registered project for the same root. If auto-import fails (for example the root is not a recognized project), stop and ask the user instead of retrying.
3. Call `inspector.project.get` for the resolved project before inspecting code. A snapshot with modules means this is a repeat init: preserve the complete snapshot, existing IDs, and unrelated entities while deepening coverage.
4. Treat the user's trailing prompt as the requested scope. Use the whole registered repository when no scope is supplied.
5. Build a complete inventory of in-scope source files with repository-aware file listing. Exclude generated output, dependencies, caches, vendored code, binaries, and ignored paths, but inspect every remaining code file; do not stop after sampling manifests or entry points. Work in bounded batches and record file coverage, unreadable paths, and an inventory fingerprint before drafting the graph.
6. Trace each file into business behavior by inspecting manifests, package boundaries, entry points, domain types, handlers, jobs, integrations, persistence, and tests. Group by cohesive business or architectural responsibility rather than directory or technical layer. Build the evidence map first; do not infer runtime relationships from directory proximity or naming.
7. Reconcile the discovered responsibilities with the current snapshot. Reuse and update an existing module or flow when its ID, slug, responsibility, or evidence matches; add only genuinely missing entities. Never create a parallel entity for an existing responsibility, and never omit or archive an existing entity merely because it was missed in an earlier inspection batch.
8. Produce a module hierarchy of one to three levels. Top levels may group related capabilities; leaf modules must be specific enough to describe one cohesive responsibility and must contain at least one business flow. The module hierarchy is rendered as a separate architecture graph; keep module containment out of business-flow steps. Split a broad module when it spans distinct outcomes, entry points, or state transitions.
9. Produce renderer-neutral graph nodes/edges, design rationale, and precise repository-relative source references for every flow. Classify each flow as `architecture`, `workflow`, `sequence`, `dataflow`, or `lifecycle`; use one clear main path and about 6-12 primary nodes. Set each edge's explicit `kind`, optional `mechanism`, and `sourceRefIds`; label decision branches with their actual outcomes. Every graph must have at least two labeled steps, a start, an end, at least one edge, unique IDs, and a connected route from a start through every step to an end. Keep each reference within the registered root and prefer symbol-sized line ranges.
10. Use temporary IDs prefixed with `new:` only for genuinely new entities. The server replaces them with UUIDv7 IDs. Keep every cross-reference internally consistent.
11. Call `inspector.command.run` with:

- `command: "inspector-init"`
- a unique `requestId`
- `projectId` and current `expectedRevision`
- project or explicit natural-language `scope`
- the complete proposed project snapshot
- optional conversation/model/prompt provenance
- `acceptStructuralChanges: true`

12. If the tool returns `render_error`, correct the named flow graph and retry with a new `requestId`. Do not remove the flow or claim success to bypass rendering validation.
13. If the proposal adds more than 25 modules plus flows, summarize the counts and confidence before calling the mutation tool; proceed only after user confirmation and set `acceptLargeChange: true`.
14. Report the committed revision, reused/updated/created module and flow counts, file coverage, structured graph diagnostics, needs-review items, and unreadable evidence. Do not claim success when the MCP mutation fails.

## Rules

- Never write repository source files.
- Never mutate SQLite directly or bypass `inspector.command.run`.
- Do not invent source references that were not inspected.
- Use `needs-review` with lower confidence when evidence is ambiguous.
- Preserve diagnostic codes, severity, evidence, and supported fixes when correcting a graph.
- Keep modules about business or architectural responsibility, not every directory. A leaf module may span multiple files, but it must not combine unrelated outcomes simply to keep the module count small.
- Cover all in-scope code with a clear set of top-level domains whose responsibilities are mutually exclusive and collectively cover the scope. Use second- and third-level modules where that makes broad domains inspectable; never exceed three module levels.
- Keep one module per responsibility. On repeat init, update and refine existing module IDs and slugs, add missing modules or children, and keep all other current entities in the full proposal. The service rejects duplicate slugs and hierarchies deeper than three levels.
- Business flows are required output, not optional decoration. Every leaf module needs at least one evidence-backed, renderable flow; a title and summary without a connected graph is incomplete.
- On a revision conflict from `inspector.command.run`, reload the current snapshot with `inspector.project.get`, re-apply the changes, and retry with the new `expectedRevision`.
