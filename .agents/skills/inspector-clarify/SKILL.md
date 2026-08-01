---
name: inspector-clarify
description: Refine an existing VibeInspector module or flow that is too broad, vague, or conflates multiple business processes. Use when the user explicitly invokes $inspector-clarify or asks to split and clarify a selected Inspector scope. Requires an explicit or reliably selected module/flow and must not initialize a whole repository.
---

# Inspector Clarify

Turn a broad selected Inspector entity into precise, evidence-backed child modules or flows.

## Workflow

1. Read the repository's `AGENTS.md` and any agent documentation it ships (for example `.agents/` docs) when present; skip files that do not exist.
2. Resolve the registered project and load its complete snapshot with `inspector.project.list` and `inspector.project.get`. When no listed root contains the current working directory, auto-import it with `inspector.project.create`; if auto-import fails, stop and ask the user. When the snapshot has no modules, stop and ask the user to run `$inspector-init` first.
3. Require one target module or flow from the user's prompt or current selection. Stop and ask for a target when neither is available.
4. Inspect the target's source references, callers, state transitions, external boundaries, failure paths, and tests.
5. Refine around distinct responsibilities or end-to-end business outcomes. Avoid splitting solely by files, layers, or generic CRUD verbs.
6. Preserve existing IDs for entities that retain their meaning. Use consistent `new:` temporary IDs for new children. Keep superseded entities in the proposal so the service can archive them explicitly.
7. Summarize the proposed split, moved flows, archived entities, confidence, and evidence before mutation.
8. Call `inspector.command.run` with `command: "inspector-clarify"`, a unique `requestId`, current `expectedRevision`, the required module/flow scope, the complete proposed snapshot, and `acceptStructuralChanges: true` after user confirmation.
9. Report the committed revision and the old-to-new navigation mapping.

## Rules

- Never run without a module or flow target.
- Never delete or silently orphan superseded entities.
- Never write repository source files or SQLite directly.
- Keep unrelated modules and flows unchanged.
- On a revision conflict from `inspector.command.run`, reload the current snapshot with `inspector.project.get`, re-apply the changes, and retry with the new `expectedRevision`.
- Prefer a small precise split over a deep speculative hierarchy.
- Do not propose child modules that duplicate the slug or responsibility of an existing module; the service rejects snapshots with duplicate module slugs.
