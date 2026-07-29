---
name: inspector-sync
description: Refresh existing VibeInspector explanations, graphs, hashes, and source evidence for the current conversation scope while preserving stable module and flow IDs. Use when the user explicitly invokes $inspector-sync or asks to synchronize Inspector data with recent implementation changes. Do not use for first-time initialization or broad structural redesign without review.
---

# Inspector Sync

Reconcile current repository evidence with an existing Inspector snapshot without navigation churn.

## Workflow

1. Read `AGENTS.md`, `.agents/01-system-architecture.md`, and `.agents/03-mcp-and-command-contracts.md`.
2. Resolve the registered project with `inspector.project.list`, then load the complete current snapshot with `inspector.project.get`.
3. Resolve scope in this order: explicit user prompt, selected/current conversation flow or module, then the narrowest confidently inferred entity. Ask for a target if none can be established.
4. Reinspect implementation and tests for the selected scope. Refresh summaries, graph details, explanations, source ranges, hashes, confidence, and stale/unreadable status.
5. Preserve every existing module and flow ID. Preserve unrelated entities byte-for-byte in the submitted full snapshot.
6. Treat added, removed, reparented, or moved modules/flows as structural changes. Summarize `added`, `removed`, `renamed`, and `moved` before mutation and ask for explicit acceptance.
7. Call `inspector.command.run` with `command: "inspector-sync"`, a unique `requestId`, current `expectedRevision`, explicit scope, and the complete proposed snapshot. Set `acceptStructuralChanges` only after approval.
8. Report the committed revision, refreshed evidence counts, stale references, and any structural proposal left uncommitted.

## Rules

- Never replace a stable entity ID to express a rename.
- Never delete entities; accepted removals are archived by the service.
- Never write repository source files or SQLite directly.
- Reject malformed or unverified source ranges instead of guessing.
- Keep changes outside the selected scope unchanged unless required for referential integrity.
