---
name: inspector-review
description: Review a VibeInspector module's current logic against its source evidence and record confirmed bugs as tracked issue items with severity and status. Use when the user explicitly invokes $inspector-review or asks to review a module for severe (P0) problems after refreshing Inspector data. Do not use for first-time initialization ($inspector-init), evidence refresh ($inspector-sync), or decomposing broad modules ($inspector-clarify).
---

# Inspector Review

Refresh and verify an Inspector scope, review the module's logic against the real code, and archive confirmed problems as issue items attached to the module's current revision.

## Workflow

1. Read the repository's `AGENTS.md` and any agent documentation it ships (for example `.agents/` docs) when present; skip files that do not exist.
2. Resolve the registered project with `inspector.project.list`, then load the complete current snapshot with `inspector.project.get`. When no listed root contains the current working directory, auto-import it with `inspector.project.create` first; if auto-import fails, stop and ask the user. When the loaded snapshot has no modules, follow the `$inspector-init` workflow instead of reviewing.
3. Resolve scope in this order: explicit user prompt, selected/current conversation module, then the narrowest confidently inferred module. Ask for a target if none can be established. The review scope is one module including its flows.
4. Check evidence freshness before reviewing. When the module's source references are stale or unreadable, or the implementation changed since the snapshot was committed, first refresh the scope with the `$inspector-sync` workflow so the review targets current logic.
5. Review each flow in scope: compare its graph and explanation against the actual implementation read with `inspector.source.read` (use `inspector.source.locate` to map files back to flows). Hunt for P0-class problems: crashes, data loss or corruption, security exposure, incorrect core logic, and missing error handling on critical paths.
6. Deduplicate before recording. Call `inspector.issue.list` with the project and module IDs and status `open`; when an open issue already covers the same problem, refresh it with `inspector.issue.update` (using its current `revision` as `expectedRevision`) instead of creating a duplicate.
7. Record each confirmed issue with `inspector.issue.create`: a unique `requestId`, `projectId`, `moduleId`, optional `flowId`, a concrete `title`, a `description` with the evidence and impact, `severity` (`p0` for severe problems; `p1`/`p2` for lesser ones), optional `path`/`startLine`/`endLine` locating the faulty code, and `source: "inspector-review"`. The service anchors the module's current revision automatically.
8. Report the review outcome: issues created or updated (IDs, severity, location), the flows and files examined, and anything uncertain that was deliberately not recorded.

## Rules

- Only record issues verified against concrete code evidence; never record speculation or style preferences.
- `p0` means crash, data loss, security exposure, or incorrect core logic; `p1` means degraded behavior with a workaround; `p2` means minor. Never inflate severity.
- Never create a duplicate issue; update the existing open one instead.
- Never write repository source files or the SQLite database directly.
- On a revision conflict from `inspector.issue.update`, reload the module's issues with `inspector.issue.list` and retry with the new `expectedRevision`.
- Do not fix the bugs found; the review only records findings. Fixes land in normal code changes and the issue is marked `resolved` afterwards.
- Keep the review inside the selected scope unless a P0 problem is found in a direct dependency of the scope.
