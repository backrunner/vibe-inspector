# MCP and Slash Command Contracts

## 1. Wire conventions

- JSON payloads use camelCase at the UI/MCP boundary and map to Rust snake_case internally.
- IDs are opaque UUIDv7 strings. They are never derived from display names.
- Every mutation includes `requestId`, `expectedRevision` (when updating), `actor`, and optional `scopeFingerprint`.
- Responses include `schemaVersion`, `requestId`, `revision`, and `provenance` where data was AI-derived.
- Mutations are idempotent for the same `requestId` within the retention window.
- Errors are structured: `{ code, message, retryable, fieldErrors?, entityId? }`.

## 2. Core entities

```ts
type Project = {
  id: string;
  name: string;
  rootPath: string;
  status: 'ready' | 'stale' | 'error';
  sourceEvidence: SourceEvidence[];
  revision: number;
  createdAt: string;
  updatedAt: string;
};

type SourceEvidence = {
  id: string;
  kind: 'codex-history' | 'manual';
  conversationId?: string;
  observedPath: string;
  observedAt: string;
  parserVersion: string;
};

type Module = {
  id: string;
  projectId: string;
  parentId: string | null;
  name: string;
  slug: string;
  summary: string;
  status: 'draft' | 'ready' | 'needs-review' | 'archived';
  flowIds: string[];
  revision: number;
};

type Flow = {
  id: string;
  moduleId: string;
  name: string;
  summary: string;
  status: 'draft' | 'ready' | 'stale' | 'needs-review';
  graph: {
    kind?: 'architecture' | 'workflow' | 'sequence' | 'dataflow' | 'lifecycle';
    nodes: FlowNode[];
    edges: FlowEdge[];
  };
  explanationId: string | null;
  sourceRefIds: string[];
  revision: number;
};

type FlowNode = {
  id: string;
  label: string;
  kind: 'start' | 'action' | 'decision' | 'external' | 'end';
  description?: string;
  sourceRefIds: string[];
};

type FlowEdge = {
  id: string;
  from: string;
  to: string;
  label?: string;
  kind: 'control' | 'data' | 'async' | 'error' | 'dependency';
  mechanism?: string;
  sourceRefIds: string[];
};

type InspectionCoverage = {
  totalFiles: number;
  inspectedFiles: number;
  ignoredFiles?: number;
  generatedFiles?: number;
  unreadableFiles?: string[];
  sourceRefCount: number;
  completed: boolean;
  inventoryFingerprint?: string;
};

type SourceRef = {
  id: string;
  projectId: string;
  path: string;
  startLine: number;
  endLine: number;
  startColumn?: number;
  endColumn?: number;
  symbol?: string;
  label?: string;
  contentHash: string;
  status: 'current' | 'stale' | 'unreadable';
};

type Provenance = {
  source: 'inspector-init' | 'inspector-sync' | 'inspector-clarify' | 'mcp' | 'manual';
  conversationId?: string;
  model?: string;
  promptHash?: string;
  generatedAt: string;
  confidence?: number;
  evidenceIds: string[];
};

type EntityVersionSummary = {
  id: string;
  projectId: string;
  entityType: 'module' | 'flow';
  entityId: string;
  revision: number;
  actor: string;
  source: string;
  requestId?: string;
  createdAt: string;
};

type EntityVersion = EntityVersionSummary & {
  snapshotJson: unknown; // serialized entity snapshot as of `revision`
};

type Issue = {
  id: string;
  projectId: string;
  moduleId: string;
  flowId?: string;
  moduleRevision?: number;
  title: string;
  description: string;
  severity: 'p0' | 'p1' | 'p2';
  status: 'open' | 'resolved' | 'wontfix';
  path?: string;
  startLine?: number;
  endLine?: number;
  actor: string;
  source: string;
  revision: number;
  createdAt: string;
  updatedAt: string;
  resolvedAt?: string;
  archivedAt?: string;
};
```

## 3. MCP tools

Read tools:

- `inspector.project.list`: projects with status and latest activity.
- `inspector.project.get`: project metadata and module root.
- `inspector.module.list`: module tree for a project, optionally including flow counts.
- `inspector.module.get`: one module with its flows, graphs, explanations, and source references.
- `inspector.flow.list`: flows for a module with status and revision.
- `inspector.flow.get`: graph, explanation summary, and source references.
- `inspector.search`: keyword search over project name/root path and module name/slug/summary; scoped to one project or the session allow-list. Use before creating modules to avoid duplicates.
- `inspector.source.read`: bounded source range by `sourceRefId` or validated relative path.
- `inspector.source.locate`: resolve a code file (absolute path, or `projectId` plus relative path, with an optional line range) to its containing allowed project and the modules/flows whose source references cover it; also returns CodeGraph-related files when the project has an index.
- `inspector.source.related`: CodeGraph dependency/dependent files for project-relative source paths; read-only and empty when no `.codegraph/codegraph.db` exists.
- `inspector.activity.list`: command runs, sync status, and audit events.
- `inspector.version.list`: recorded entity versions for a project, newest first, filterable by `entityType`/`entityId`.
- `inspector.version.get`: one entity version with its snapshot payload; versions of other projects answer `not_found`.
- `inspector.issue.list`: active issues for a project, filterable by `moduleId` and `status`.

Mutation tools:

- `inspector.project.create`, `.update`, `.archive`
- `inspector.module.create`, `.update`, `.archive`
- `inspector.flow.create`, `.update`, `.archive`
- `inspector.sourceRef.upsert`, `.archive`
- `inspector.explanation.upsert`
- `inspector.issue.create`, `.update`, `.archive`
- `inspector.command.run` for a registered slash command; it must not execute arbitrary text as a shell command.

MCP tools must call the same domain services as the UI. They may not mutate SQLite directly. `inspector.source.read` returns UTF-8 text plus a content hash and never returns data outside the registered project root.

`inspector.project.create` has ensure semantics: it canonicalizes the supplied root, requires project root markers, and returns the already-registered project for the same canonical root instead of failing; a project previously archived with the same root is revived (cleared `archived_at`, bumped revision) rather than rejected. Agents call it to auto-import the project they are running in when `inspector.project.list` has no match; a project registered this way joins the calling session's allow-list immediately. Projects created through MCP live in SQLite only — desktop `register_projects` reuses the existing SQLite project ID for roots it already knows, and agent allow-list syncs (including fresh installs) always cover the union of registry and SQLite project IDs.

`inspector.module.create` has ensure semantics on the module slug: when an active module with the same slug already exists in the project, the tool returns that module unchanged with `reusedExisting: true` instead of creating a duplicate. Snapshot commits (including `inspector.command.run` proposals) are validated to reject duplicate module slugs. Callers must still list existing modules first and prefer `inspector.module.update` for entities that already exist.

Snapshot commits allow a module hierarchy of one to three levels. Every business flow is validated before command review and again before persistence: it must contain unique non-empty node and edge IDs, bounded non-empty labels, at least one start and one end, and a connected path that makes every node reachable from a start and able to reach an end. `$inspector-init` additionally requires at least one flow on every leaf module. A violation returns `render_error` with a correction message; the calling agent must repair the graph and retry rather than omit the flow.

Each flow declares one diagram kind (`architecture`, `workflow`, `sequence`, `dataflow`, or `lifecycle`). Relationship semantics are explicit: `kind` describes control/data/async/error/dependency intent, `mechanism` names the transport or call boundary, and `sourceRefIds` points to direct evidence. The renderer may style these values but must not infer runtime causality from file proximity, names, or edge labels. Command proposals may include an `InspectionCoverage` manifest; completed coverage must account for every in-scope file, and unreadable paths must be repository-relative. Validation diagnostics are structured with a stable code, severity, subject, evidence, and `supportedFixes`. Non-blocking diagnostics (severity `warning`) do not fail the command; they are returned on `inspector.command.run` results as `graphWarnings: GraphDiagnostic[]`, and idempotent replays of the same `requestId` carry the same list.

The desktop presents two distinct graph surfaces. The module architecture graph is derived from the module hierarchy and answers responsibility/containment questions; it remains available even when a module has no business flow. A business-flow graph is scoped to one module outcome and answers runtime behavior questions. Do not encode module containment as flow steps, and do not infer module dependencies from directory proximity. If explicit cross-module dependencies are added later, they must be represented as evidence-backed architecture relationships with their own stable IDs.

`inspector.issue.*` tools manage review issue entries, which live outside the `InspectorSnapshot`: they do not take `expectedProjectRevision` and never bump the project revision. `inspector.issue.create` fills `moduleRevision` from the module's current revision server-side and defaults `source` to `mcp` (callers such as the review skill may pass their own, e.g. `inspector-review`). `inspector.issue.update`/`.archive` use entity-level `expectedRevision` optimistic locking; in `update`, `flowId`/`path`/`startLine`/`endLine` are tri-state — omitted leaves the value untouched, explicit `null` clears it. All three are idempotent for the same `requestId`, and issue IDs of other projects answer `not_found`.

The desktop UI is a read-only Inspector client. It does not expose lifecycle badges for `draft`,
`ready`, `stale`, or `needs-review`, and it does not expose a manual content-refresh action. Inspector
entity writes are initiated through MCP tools; the desktop re-reads the selected project snapshot when
its window regains focus while preserving the current selection.

## 4. Agent skill behavior

Codex discovers the repo-owned skills under `.agents/skills`. Users invoke them as
`$inspector-init`, `$inspector-sync`, `$inspector-clarify`, `$inspector-review`, and `$vibe-inspector-setup`. Arbitrary project-owned `/name`
slash commands are not a supported Codex extension surface. The stable MCP/domain command names
remain `inspector-init`, `inspector-sync`, and `inspector-clarify`; `inspector-review` is a skill-only
workflow that records findings through the `inspector.issue.*` tools rather than a snapshot command.

### `$inspector-init [prompt]`

Input context: current project root, current Codex conversation ID, optional natural-language scope. Output: a proposed or committed module/flow graph, explanations, source references, and an inspection coverage manifest. Default scope is the whole repository. Before proposing, the agent inventories and inspects every in-scope, non-generated source file in bounded batches; it records the inventory and coverage before drafting the graph. The result must describe concrete business responsibilities rather than only top-level directories or frameworks. For each flow, classify the diagram kind, choose one clear main path, keep roughly 6-12 primary nodes, label decisions with explicit branch outcomes, and attach symbol-sized evidence to nodes and edges. When the current working directory is not inside a registered project root, the skill auto-imports it through `inspector.project.create` before proposing anything. On repeated initialization, the agent lists and reads the existing model, reuses stable IDs/slugs, updates existing responsibilities and evidence, and creates only missing or finer-grained entities. It reports file coverage plus reused, updated, and created counts. The command must show a summary and confidence before committing when the result changes more than a configured entity threshold.

### `$inspector-sync [prompt]`

Input context: current project and conversation module/flow context. Output: refreshed source hashes, explanations, and evidence. Existing module/flow IDs are preserved. Structural changes become a proposal with `added`, `removed`, and `renamed` lists; the caller must explicitly accept structural changes. When the project was never initialized (registered but no modules), the skill falls back to the `$inspector-init` workflow instead of failing.

Structural review also reports changed graph semantics and changed evidence references. A graph-only change is not treated as a rename; it is reconciled by stable node/edge IDs and reviewed with the structured diagnostics report.

### `$inspector-clarify [prompt]`

Input context: a selected module or flow. Output: narrower child modules/flows and updated parent summaries. Existing entities remain addressable; superseded entities are archived, not deleted. The command should reject a prompt that has no selected scope and no explicit target.

### `$inspector-review [prompt]`

Input context: a selected module. The skill first refreshes stale evidence through the `$inspector-sync` workflow, then reviews each flow's graph and explanation against the actual source read through `inspector.source.read`. Output: confirmed problems recorded as issue items via `inspector.issue.create` with `source: "inspector-review"`, anchored to the module's current revision. Severity is `p0` for crashes, data loss, security exposure, or incorrect core logic (`p1`/`p2` for lesser findings); status flows `open` → `resolved`/`wontfix` and can be updated from both MCP (`inspector.issue.update`) and the desktop UI. Repeating a review must deduplicate against `inspector.issue.list` and update existing open issues instead of creating duplicates.

## 5. Safety and authorization

- Bind each MCP session to the user session and a project allow-list.
- Require a project ID for all reads except project discovery.
- Require explicit `expectedRevision` on destructive or structural updates.
- Validate relative paths with canonicalization, reject symlinks that escape the root, and cap bytes/lines returned.
- Log actor, tool, project, entity IDs, result, and duration without logging source contents or secrets.

## 6. Local transports

- `vibe-inspector-mcp` remains the authenticated stdio transport for direct clients that provide session environment variables.
- `vibe-inspector-tray` hosts streamable HTTP at the URL written to `mcp-endpoint.json` in the application support directory. It binds only to loopback. Tray-hosted MCP sessions allow `inspector.project.create` so desktop-local clients can auto-import a project; the new project joins that session's allow-list.
- Every HTTP request requires `Authorization: Bearer <token>`. The OS-random 256-bit token is stored separately in `mcp-token` with mode `0600`; it is never returned by a Tauri command or diagnostic event.
- HTTP middleware validates `Host` and, when present, `Origin` before checking the token to prevent DNS-rebinding/browser-origin access.
- A per-user `tray.sock` handles health, open-desktop, and quit control messages. It is not an MCP transport and is mode `0600`.
- The desktop may send `setLoginItem { enabled }` over the same socket. The tray process performs the LaunchAgent update, refreshes its menu checkbox and health snapshot, and returns no executable path or secret to the WebView.

## 7. Agent integrations

The desktop detects locally installed coding agents and, with explicit user selection, deploys the inspector skills and registers the `vibe-inspector` MCP server into each agent's global configuration. Repositories are never modified; installation is global per agent.

| Agent       | Detection                                                                   | Skills directory                                              | MCP config                                                  |
| ----------- | --------------------------------------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------- |
| Codex       | `~/.codex/` or `codex` on `PATH`                                            | `~/.codex/skills/<name>/` (`SKILL.md` + `agents/openai.yaml`) | `[mcp_servers.vibe-inspector]` in `~/.codex/config.toml`    |
| Claude Code | `~/.claude/` or `claude` on `PATH`                                          | `~/.claude/skills/<name>/SKILL.md`                            | `mcpServers["vibe-inspector"]` in `~/.claude.json`          |
| OpenCode    | `~/.config/opencode/` or `opencode` on `PATH` (`XDG_CONFIG_HOME` respected) | `~/.config/opencode/skills/<name>/SKILL.md`                   | `mcp.vibe-inspector` in `opencode.jsonc` or `opencode.json` |
| Cursor      | `~/.cursor/`, `Cursor.app`, or `cursor-agent`/`cursor` on `PATH`            | `~/.cursor/skills/<name>/SKILL.md`                            | `mcpServers["vibe-inspector"]` in `~/.cursor/mcp.json`      |
| Grok        | `~/.grok/` or `grok` on `PATH` (`GROK_HOME` respected)                      | `~/.grok/skills/<name>/SKILL.md`                              | `[mcp_servers.vibe-inspector]` in `~/.grok/config.toml`     |
| Kimi Code   | `~/.kimi-code/` or `kimi` on `PATH` (`KIMI_CODE_HOME` respected)            | `~/.kimi-code/skills/<name>/SKILL.md`                         | `mcpServers["vibe-inspector"]` in `~/.kimi-code/mcp.json`   |

Rules:

- Skill templates ship inside the app bundle (`src-tauri/templates/skills`) and are embedded with `include_str!`. Install state is derived by comparing on-disk content with the bundled template: `installed`, `outdated`, or `missing`.
- `vibe-inspector setup` is the shared CLI entry point. With a TTY it opens a compact inline multi-select TUI; `--agent`, `--all`, `--yes`, `--dry-run`, and `--json` provide deterministic automation. `vibe-inspector agents list` reports every supported target and `vibe-inspector --json doctor` reports helper availability without exposing secrets. The same executable starts the desktop when invoked without a CLI subcommand.
- OpenCode's `opencode.jsonc` is parsed as JSONC and rewritten with unknown fields preserved; Grok's TOML uses the same nested MCP table shape as Codex. Cursor receives the documented `type: "stdio"` field and the standard `mcpServers` object.
- Every MCP entry is a stdio server whose `command` is the `vibe-inspector-mcp` executable next to the running desktop binary. Environment per agent: `VIBE_INSPECTOR_SESSION_SECRET` (random 64-hex, generated per agent at install), `VIBE_INSPECTOR_SESSION_ID` (`agent-<id>`), `VIBE_INSPECTOR_ALLOWED_PROJECTS` (registered project IDs, re-synced after every `register_projects`), and `VIBE_INSPECTOR_ALLOW_PROJECT_CREATE=1` so agents can auto-import the project they run in through `inspector.project.create`. The allow-list sync also upserts `VIBE_INSPECTOR_ALLOW_PROJECT_CREATE`, upgrading agent configs installed before auto-import existed. Auto-import stays bounded: the root must canonicalize inside the filesystem, carry recognized project root markers, and dedupe by canonical root path; bulk candidate discovery still requires user confirmation in the desktop UI.
- Writes are limited to each detected agent configuration root (inside the user home unless an explicit `XDG_CONFIG_HOME`, `GROK_HOME`, or `KIMI_CODE_HOME` override is set); directories and files are canonicalized and symlink escapes rejected. Existing config files are backed up to `<file>.vibe-inspector-backup` before the first mutation; newly created secret-bearing files are mode `0600`.
- A failure for one agent must not abort installation for the remaining selected agents.
