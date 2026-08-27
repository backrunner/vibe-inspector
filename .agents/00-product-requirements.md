# VibeInspector Product Requirements

## 1. Product read

VibeInspector is a local-first observability workbench for complex VibeCoding repositories, especially large monorepos. It gives a developer a durable, navigable model of repository modules and business flows while code is changing quickly through Codex or other LLM workflows.

Primary audience: a developer or technical lead who needs to review architecture and implementation decisions without losing the link between a business flow, its explanation, and the exact read-only source files involved.

Platform scope for v1: macOS only. The desktop application is launched on demand. A separate tray process can start at login, expose MCP servers, and launch the desktop window.

## 2. Goals

- Discover local Codex conversation history and propose project entries from repository paths.
- Bind each project entry to a canonical, validated local repository root.
- Represent a repository as modules, business flows, explanations, and source references.
- Present module architecture and business-flow behavior as distinct graphs: architecture shows responsibility containment, while a flow graph shows one runtime outcome.
- Let a user inspect a flow as a diagram plus an AI explanation and exact read-only code locations.
- Allow Codex to initialize, synchronize, and clarify inspector data through slash commands.
- Expose safe CRUD MCP tools for projects, modules, business flows, explanations, and source references.
- Keep the product useful when the desktop UI is closed through the tray/MCP process.

## 3. Non-goals for v1

- Editing source code, applying patches, or running arbitrary shell commands from the UI.
- Replacing an IDE, code search engine, issue tracker, or CI dashboard.
- Supporting remote repositories, cloud sync, team accounts, or multi-user permissions.
- Automatically treating an LLM output as truth without provenance, confidence, and review state.
- Full cross-platform packaging.

## 4. Core user journeys

### 4.1 Discover and register a project

1. A first-launch setup screen offers metadata-only Codex history discovery, a projects collection directory, or one exact project root.
2. Collection discovery performs a bounded shallow scan. It recognizes only root-level repository/workspace manifests, skips generated directories and symlinks, and stops descending after a project root is found.
3. Scanner normalizes paths, de-duplicates candidates by canonical root, and records the root markers and name source used for each proposal.
4. User reviews and confirms candidates before registration. A collection scan never silently registers every child directory.
5. App creates stable project IDs, stores canonical roots, and runs a lightweight repository inventory.
6. Empty project state points the user to the Agent-owned `$inspector-init` repository skill.

Acceptance: a project is never created from an unvalidated path; nested packages inside an identified monorepo are not promoted to separate projects; inaccessible or deleted roots remain unavailable without exposing invalid Inspector content.

### 4.2 Inspect a module and business flow

1. User selects a project in the collapsible left rail.
2. User expands a module stub in the middle rail and selects a business flow.
3. Main pane provides a clearly labeled module architecture graph alongside the selected flow's business-flow diagram, explanation panel, and referenced files.
4. Selecting a file opens a read-only Monaco editor at the referenced line range.
5. The flow overview remains above the source evidence workspace so the user can compare architecture and implementation without changing views or losing selection.

Acceptance: module architecture remains visible even when the selected module has no flow; architecture containment is never presented as runtime flow; deep links preserve project, module, flow, file, and line-range selection; missing files are visible as stale references rather than silently removed.

### 4.3 Initialize, synchronize, clarify

- `$inspector-init [scope prompt]` inventories and inspects every in-scope, non-generated source file before creating a detailed module/flow model for the requested scope or the whole repository when no scope is supplied. Every leaf module has at least one renderable business flow.
- Repeating `$inspector-init` refines the existing model: it reuses stable module and flow IDs, updates responsibilities and evidence in place, and adds only missing or more detailed modules instead of duplicating existing entities.
- `$inspector-sync [scope prompt]` refreshes evidence and explanations for the current conversation context; structural changes require a change proposal and explicit confirmation.
- `$inspector-clarify [scope prompt]` decomposes an overly broad module or flow while preserving IDs and history for existing entities.
- `$inspector-review [scope prompt]` refreshes the selected module's evidence, reviews its logic against the source, and records confirmed problems as severity-tagged issue items with an updatable status.

Acceptance: commands are idempotent by conversation turn and scope fingerprint; the module hierarchy contains one to three levels; invalid or unrenderable business flows are rejected with a correction request; every mutation records source command, model metadata, timestamp, and changed entity IDs.

### 4.4 Tray-first operation

1. A launch-at-login tray process starts independently of the Tauri desktop window.
2. Tray keeps MCP endpoints available and reports health in its menu.
3. User chooses `Open VibeInspector` to launch or focus the desktop app.
4. Closing the window leaves the tray process and MCP server alive; quitting from the tray stops both.

Acceptance: MCP health does not depend on a visible window; one tray instance and one MCP listener exist per user session.

## 5. Information architecture

- Project rail: filterable projects, recent activity, collapse toggle. Inspector lifecycle states
  remain in MCP/storage data and are not rendered as routine status indicators.
- Module rail: repository modules in a tree/stub list with at most three levels and flow counts. Long trees scroll horizontally. Initialization remains Agent-owned and is not exposed as a desktop UI action.
- Inspector workspace: flow header, explicit Architecture/Flow/Rationale views in the upper band, and source references/read-only code in the lower band.
- Secondary surfaces: command activity, provenance, settings, MCP health, keyboard shortcut help.

## 6. State model visible to users

Entities expose explicit states to MCP/storage: `draft`, `ready`, `stale`, `needs-review`, `syncing`, `error`, and `archived`. The desktop inspector is a read-only view and does not render routine lifecycle badges or provide manual content refresh controls. Loading is represented by shape-preserving skeletons; empty states provide context; errors identify the affected scope. The selected project snapshot is re-read automatically when the desktop window regains focus.

## 7. Product acceptance checklist

- A new user can register a repository and reach an empty inspector state without reading documentation.
- A seeded fixture can render a three-level module/flow tree and a flow diagram with at least three source references.
- Monaco never enables editing, paste-to-write, or file-system save actions.
- Stale paths, malformed LLM responses, duplicate command requests, and unavailable Codex history are recoverable.
- Tray start, desktop launch/focus, MCP health, and graceful quit are independently testable.
