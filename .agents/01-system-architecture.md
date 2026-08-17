# VibeInspector System Architecture

## 1. Architectural principles

- Local-first: repository content and inspector data remain on the Mac unless a future feature explicitly opts in.
- Process isolation: the always-on tray/MCP runtime does not share UI lifecycle assumptions with the Tauri window.
- Domain-first: parsing, path validation, identity, and mutation rules live outside UI adapters.
- Evidence over assertion: every AI-derived field carries provenance and confidence; source references are immutable observations until replaced by a sync.
- Read-only inspection: the app may read repository files and MCP-owned inspector snapshots but never
  writes source files or inspector entities from the desktop UI in v1. Inspector entity writes flow
  through MCP/domain commands; the desktop re-reads the selected snapshot when the window regains focus.

## 2. Process topology

```text
macOS Login Item
        |
        v
Tray / MCP process (Rust, `vibe-inspector-tray`)
  - launch-at-login lifecycle
  - authenticated streamable HTTP MCP on loopback
  - Unix socket health + single-instance lock
  - opens/focuses desktop app
        |
        | local authenticated IPC / deep link
        v
Tauri desktop process (Rust)
  - window/menu/tray handoff
  - Tauri commands
  - project + inspector repositories
  - filesystem read broker
        |
        v
SvelteKit UI (webview)
  - routes and view state
  - shadcn-svelte components + Zinc tokens
  - diagram renderer adapter
  - Monaco read-only adapter
```

The tray and desktop processes share the Rust storage, MCP, and filesystem-broker modules. The tray binds a per-user Unix socket for health, open, and quit control messages, while MCP uses streamable HTTP on `127.0.0.1` with an OS-random bearer token stored at mode `0600`. The token is never exposed through a Tauri command. The desktop probes the socket through a small bridge and uses Tauri's single-instance plugin to focus the existing `main` window.

The `vibe-inspector` desktop executable also dispatches the local setup CLI before constructing Tauri when it receives `setup`, `agents`, or `doctor` subcommands. This keeps the distributed command name stable while preserving the no-argument desktop launch. CLI and desktop integration controls call the same Rust agent-integration service.

The tray is the availability anchor for MCP: whenever the stdio MCP sidecar (`vibe-inspector-mcp`) starts, it first ensures the tray is running by probing the control socket and, on failure, spawning the sibling `vibe-inspector-tray` binary (or the `VIBE_INSPECTOR_TRAY_PATH` override) deployed next to it. Tray startup failure is recorded as a diagnostic event but never blocks the stdio MCP session, so agent-side MCP stays available even when the tray cannot be launched. The reverse direction needs no work: the tray hosts the HTTP MCP listener itself, and the single-instance socket claim makes repeated spawns idempotent.

Quitting from the tray menu (or the control-socket `quit` command) stops the tray and its embedded HTTP MCP together and records a quit-intent marker (`tray-quit.intent`, mode `0600`) in the runtime directory. While the marker exists, implicit launchers — the stdio MCP sidecar — must not respawn the tray; the sidecar records a `tray-start-suppressed` diagnostic and keeps serving stdio. Explicit user actions clear the marker and restore normal startup: opening the desktop app, enabling the login item, or any tray start (which clears the marker after claiming the control socket). A graceful quit exits with status 0, so the login item's `KeepAlive.SuccessfulExit=false` policy does not relaunch the tray; an enabled login item still starts the tray at the next login.

## 3. Proposed repository layout

```text
src/
  lib/
    domain/
      entities.ts          # UI-safe DTOs and discriminated states
      ids.ts                # stable ID and scope fingerprint helpers
      commands.ts           # command status and mutation result types
    stores/
      projects.svelte.ts
      inspector.svelte.ts
      activity.svelte.ts
    adapters/
      tauri.ts              # invoke/listen wrapper, no raw invoke in components
      projects.ts           # native directory picker and project discovery DTOs
      ui.ts                 # app-level theme and notification helpers
      diagram.ts            # renderer-neutral diagram model
      editor.ts             # Monaco lifecycle and read-only configuration
      runtime.ts            # tray health and login-item IPC DTOs
    components/
      shell/
      projects/
      modules/
      inspector/
      code/
      activity/
      settings/
    routes/
      +layout.svelte
      +page.svelte
      projects/[projectId]/+page.svelte
      projects/[projectId]/modules/[moduleId]/flows/[flowId]/+page.svelte
  app.html
src-tauri/
  src/
    main.rs                 # desktop entry only
    tray_runtime/            # shared paths, IPC, HTTP auth, and login-item lifecycle
    commands/                # thin Tauri command adapters
    domain/                  # shared with tray where possible
    storage/                 # SQLite/schema/repositories
      project_registry.rs   # versioned first-run root registry before SQLite migration
    fs_broker/               # canonical roots, path and range checks
    codegraph.rs             # optional read-only dependency lookup from an existing local index
    project_discovery.rs     # bounded root-marker scan and naming rules
    codex_history/           # history discovery parser
    mcp/                     # server and tool adapters
    cli.rs                  # shared setup/doctor/list CLI and terminal selector
    bin/
      vibe-inspector-tray.rs # separate always-on tray binary bundled beside the desktop binary
      vibe-inspector-mcp.rs  # authenticated stdio MCP entry for direct clients; ensures the tray is running before serving
```

Do not let `+page.svelte`, `main.rs`, or an MCP handler become a catch-all. Components dispatch typed store actions; only adapters call Tauri or MCP boundaries.

## 4. Data flow

1. UI calls a typed adapter (`projects.list`, `flows.get`, `files.readRange`).
2. Tauri command validates input and maps it to a domain service.
3. Domain service checks project root, authorization scope, current entity version, and idempotency key.
4. Repository reads SQLite/JSON metadata and the filesystem broker reads only inside the canonical root.
5. Result is a serializable DTO with `revision`, `updatedAt`, `provenance`, and stale/error information.
6. UI store updates optimistically only for local view state; persisted Inspector mutations originate
   from MCP/domain commands, and the desktop consumes a fresh selected snapshot on window focus.

Tray settings follow a separate narrow control path: the WebView invokes a typed Tauri command, the desktop sends a boolean control message over the protected Unix socket, and the tray process updates its own LaunchAgent and menu state. The WebView never receives the MCP token or invokes `launchctl` directly.

## 5. Persistence recommendation

Use SQLite through a Rust repository for normalized inspector metadata, migrations, indexes, and transactional updates. Store large generated diagrams and explanations as compressed JSON/text blobs only when the schema benefits from it; source code stays in the repository and is read on demand.

Confirmed roots first enter a minimal Rust-owned JSON bootstrap registry in the macOS app-data directory, then import idempotently into SQLite. The registry is schema-versioned, revisioned, timestamped, written atomically, and uses UUIDv7 project IDs. It stores roots and discovery evidence only; SQLite is authoritative for runtime project status and all Inspector content.

Minimum tables: `projects`, `project_sources`, `modules`, `flows`, `flow_steps`, `flow_edges`, `source_refs`, `explanations`, `command_runs`, `mcp_audit_events`, `mutation_events`, and `schema_migrations`.

Every entity has `id`, `project_id`, `revision`, `created_at`, `updated_at`, and `archived_at` where applicable. Use soft archive for structural entities so clarify/sync can preserve history.

## 6. Boundary rules

- Svelte components cannot import `@tauri-apps/api` directly.
- Rust Tauri command modules cannot parse LLM prose or decide UI layout.
- MCP handlers cannot bypass the domain service or filesystem broker.
- Diagram and editor libraries are adapters; the domain stores a renderer-neutral graph and source ranges.
- The desktop source viewer may read a complete selected UTF-8 file through the filesystem broker while preserving the persisted range as a highlight. MCP source reads remain bounded range reads.
- CodeGraph integration is optional and read-only. It may inspect only `<registered-root>/.codegraph/codegraph.db`, never creates or updates an index, and validates every related path against the registered project root before returning it as ephemeral UI or MCP context (`inspector.source.related`, and the `relatedFiles` field of `inspector.source.locate`). Dependency direction and kind come only from explicit non-containment CodeGraph edges; the app does not infer or persist relationships.
- Codex history scanning is best-effort and read-only. The first provider opens `~/.codex/state_5.sqlite` read-only, reads only thread identity/path metadata, resolves nested working directories upward to Git/workspace roots, and records parser versions and source paths.
- Manual collection discovery is bounded and root-marker based. Once a project root is accepted, scanning prunes that subtree so monorepo packages are not registered as unrelated projects.
