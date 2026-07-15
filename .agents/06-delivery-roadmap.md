# Delivery Roadmap

The roadmap is ordered to reduce irreversible decisions and validate process boundaries early.

## Phase 0: Bootstrap and decisions

Deliver: Tauri v2 + SvelteKit shell, macOS-only packaging target, pnpm workspace, Rust workspace, lint/typecheck/test commands, CI skeleton, and this dependency matrix validated with real package versions.

Exit criteria: desktop window opens, `cargo test` and `pnpm run check` pass, the shadcn Zinc shell renders in light/dark mode, and a single-instance policy is documented.

## Phase 1: Local data foundation

Deliver: SQLite migrations, typed entities, repositories, project root canonicalization, bounded file reads, content hashing, and fixture repository scanner.

Exit criteria: path-security tests pass; a seeded project can be created, loaded, marked stale, and archived without UI coupling.

## Phase 2: Project discovery and shell

Deliver: Codex history provider trait plus first local parser, first-launch collection/single-project discovery with explicit confirmation, three-pane shell, project/module stores, empty/loading/error states, and keyboard navigation.

Exit criteria: a user can discover or manually choose a project, switch projects, expand a module tree, and recover from an unavailable history source.

## Phase 3: Inspector workspace

Deliver: flow detail route, renderer-neutral graph model, Mermaid adapter, explanation/evidence panel, source reference list, local file tree, and read-only Monaco integration.

Exit criteria: fixture flow renders graph + explanation + three source ranges; selecting a reference opens the correct read-only code lines; stale references are visible.

## Phase 4: Commands and MCP

Deliver: `$inspector-init`, `$inspector-sync`, `$inspector-clarify` skill handlers, command activity/provenance, MCP server CRUD tools, idempotency, revision conflicts, and audit events.

Exit criteria: command fixtures cover structural preservation rules; MCP and UI produce equivalent mutations through shared services; malformed model output is rejected or marked for review.

## Phase 5: Independent tray runtime

Deliver: tray binary/lifecycle, login-item setup, single-instance lock, local authenticated bridge, MCP health menu, desktop launch/focus, and graceful quit.

Exit criteria: tray starts without the desktop window, MCP health remains available after window close, and a tray action launches/focuses one desktop instance on macOS.

Implementation decision: ship `vibe-inspector-tray` in the macOS app bundle beside the desktop executable. It owns the menu-bar item, loopback MCP listener, Unix socket singleton, and LaunchAgent. The desktop may start the helper when launched directly, but the helper never depends on the desktop process remaining alive.

## Phase 6: Hardening and release

Deliver: signed/notarized macOS artifact strategy, crash/error telemetry kept local by default, performance profiling, accessibility pass, security review, migration backups, and user-facing recovery docs.

Exit criteria: release candidate passes all automated tests plus a manual macOS matrix for fresh install, upgrade, stale project, unavailable Codex history, large monorepo, dark mode, and reduced motion.

Implemented baseline: macOS CI and bundle smoke test, JavaScript/CSS budgets, metadata-only local diagnostics, SQLite online migration backups with bounded retention and integrity-checked recovery, and signing/notarization/recovery guidance in `.agents/07-release-and-recovery.md`. Apple credentialed signing and the final manual hardware/OS matrix remain release operations rather than repository-local automation.

## Vertical-slice order

The first demonstrable slice should be: fixture project -> project selection -> one module/flow -> diagram -> explanation -> source reference -> read-only Monaco. Do not wait for the complete Codex parser or tray packaging to validate the core inspection loop.

## Resolved implementation decisions

- Codex history reads `~/.codex/state_5.sqlite` in read-only mode through a versioned metadata-only provider.
- `rusqlite` owns schema migrations, transactions, online migration backups, and recovery validation.
- `rmcp` exposes authenticated stdio and loopback streamable HTTP transports; a protected bearer token and Host/Origin validation guard HTTP.
- Mermaid v11 is retained for v1 with strict rendering, a stable LR direction, bounded zoom, and horizontal overflow for narrow panes.
- Tray and desktop ship as one app bundle with three executables; a per-user LaunchAgent starts the tray helper at login.
- Zinc light/dark tokens, semantic status colors, radius scale, shadows, and adjacent-surface gradients are fixed in `src/app.css` and verified at the baseline and minimum window sizes.
