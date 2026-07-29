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
  graph: { nodes: FlowNode[]; edges: FlowEdge[] };
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

type FlowEdge = { id: string; from: string; to: string; label?: string };

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
```

## 3. MCP tools

Read tools:

- `inspector.project.list`: projects with status and latest activity.
- `inspector.project.get`: project metadata and module root.
- `inspector.module.list`: module tree for a project, optionally including flow counts.
- `inspector.module.get`: one module and child modules.
- `inspector.flow.list`: flows for a module with status and revision.
- `inspector.flow.get`: graph, explanation summary, and source references.
- `inspector.source.read`: bounded source range by `sourceRefId` or validated relative path.
- `inspector.activity.list`: command runs, sync status, and audit events.

Mutation tools:

- `inspector.project.create`, `.update`, `.archive`
- `inspector.module.create`, `.update`, `.archive`
- `inspector.flow.create`, `.update`, `.archive`
- `inspector.sourceRef.upsert`, `.archive`
- `inspector.explanation.upsert`
- `inspector.command.run` for a registered slash command; it must not execute arbitrary text as a shell command.

MCP tools must call the same domain services as the UI. They may not mutate SQLite directly. `inspector.source.read` returns UTF-8 text plus a content hash and never returns data outside the registered project root.

The desktop UI is a read-only Inspector client. It does not expose lifecycle badges for `draft`,
`ready`, `stale`, or `needs-review`, and it does not expose a manual content-refresh action. Inspector
entity writes are initiated through MCP tools; the desktop re-reads the selected project snapshot when
its window regains focus while preserving the current selection.

## 4. Agent skill behavior

Codex discovers the repo-owned skills under `.agents/skills`. Users invoke them as
`$inspector-init`, `$inspector-sync`, and `$inspector-clarify`. Arbitrary project-owned `/name`
slash commands are not a supported Codex extension surface. The stable MCP/domain command names
remain `inspector-init`, `inspector-sync`, and `inspector-clarify`.

### `$inspector-init [prompt]`

Input context: current project root, current Codex conversation ID, optional natural-language scope. Output: a proposed or committed module/flow graph, explanations, and source references. Default scope is the whole repository. The command must show a summary and confidence before committing when the result changes more than a configured entity threshold.

### `$inspector-sync [prompt]`

Input context: current project and conversation module/flow context. Output: refreshed source hashes, explanations, and evidence. Existing module/flow IDs are preserved. Structural changes become a proposal with `added`, `removed`, and `renamed` lists; the caller must explicitly accept structural changes.

### `$inspector-clarify [prompt]`

Input context: a selected module or flow. Output: narrower child modules/flows and updated parent summaries. Existing entities remain addressable; superseded entities are archived, not deleted. The command should reject a prompt that has no selected scope and no explicit target.

## 5. Safety and authorization

- Bind each MCP session to the user session and a project allow-list.
- Require a project ID for all reads except project discovery.
- Require explicit `expectedRevision` on destructive or structural updates.
- Validate relative paths with canonicalization, reject symlinks that escape the root, and cap bytes/lines returned.
- Log actor, tool, project, entity IDs, result, and duration without logging source contents or secrets.

## 6. Local transports

- `vibe-inspector-mcp` remains the authenticated stdio transport for direct clients that provide session environment variables.
- `vibe-inspector-tray` hosts streamable HTTP at the URL written to `mcp-endpoint.json` in the application support directory. It binds only to loopback.
- Every HTTP request requires `Authorization: Bearer <token>`. The OS-random 256-bit token is stored separately in `mcp-token` with mode `0600`; it is never returned by a Tauri command or diagnostic event.
- HTTP middleware validates `Host` and, when present, `Origin` before checking the token to prevent DNS-rebinding/browser-origin access.
- A per-user `tray.sock` handles health, open-desktop, and quit control messages. It is not an MCP transport and is mode `0600`.
- The desktop may send `setLoginItem { enabled }` over the same socket. The tray process performs the LaunchAgent update, refreshes its menu checkbox and health snapshot, and returns no executable path or secret to the WebView.

## 7. Agent integrations

The desktop detects locally installed coding agents and, with explicit user selection, deploys the inspector skills and registers the `vibe-inspector` MCP server into each agent's global configuration. Repositories are never modified; installation is global per agent.

| Agent | Detection | Skills directory | MCP config |
| --- | --- | --- | --- |
| Codex | `~/.codex/` or `codex` on `PATH` | `~/.codex/skills/<name>/` (`SKILL.md` + `agents/openai.yaml`) | `[mcp_servers.vibe-inspector]` in `~/.codex/config.toml` |
| Claude Code | `~/.claude/` or `claude` on `PATH` | `~/.claude/skills/<name>/SKILL.md` | `mcpServers["vibe-inspector"]` in `~/.claude.json` |
| Kimi Code | `~/.kimi-code/` or `kimi` on `PATH` (`KIMI_CODE_HOME` respected) | `~/.kimi-code/skills/<name>/SKILL.md` | `mcpServers["vibe-inspector"]` in `~/.kimi-code/mcp.json` |

Rules:

- Skill templates ship inside the app bundle (`src-tauri/templates/skills`) and are embedded with `include_str!`. Install state is derived by comparing on-disk content with the bundled template: `installed`, `outdated`, or `missing`.
- Every MCP entry is a stdio server whose `command` is the `vibe-inspector-mcp` executable next to the running desktop binary. Environment per agent: `VIBE_INSPECTOR_SESSION_SECRET` (random 64-hex, generated per agent at install), `VIBE_INSPECTOR_SESSION_ID` (`agent-<id>`), and `VIBE_INSPECTOR_ALLOWED_PROJECTS` (registered project IDs, re-synced after every `register_projects`). `VIBE_INSPECTOR_ALLOW_PROJECT_CREATE` stays unset; project registration always goes through the desktop UI.
- Writes are limited to the detected agent configuration directories inside the user home; paths are canonicalized and symlink escapes rejected. Existing config files are backed up to `<file>.vibe-inspector-backup` before the first mutation; newly created secret-bearing files are mode `0600`.
- A failure for one agent must not abort installation for the remaining selected agents.
