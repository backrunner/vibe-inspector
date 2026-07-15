# VibeInspector Agent Instructions

## Scope

VibeInspector is a macOS-only Tauri desktop application. The user-facing shell is SvelteKit, and the UI must use Bits UI primitives through the repo-owned shadcn-svelte components in `src/lib/components/ui`. The companion tray process is responsible for MCP server availability and must not require the desktop window to be open.

Read the project documents in `.agents/` before making architectural or UI changes:

- `.agents/00-product-requirements.md`: product scope, user journeys, and acceptance criteria.
- `.agents/01-system-architecture.md`: process boundaries, data flow, and module ownership.
- `.agents/02-dependency-matrix.md`: dependency decisions and installation order.
- `.agents/03-mcp-and-command-contracts.md`: MCP tools, slash commands, IDs, and data contracts.
- `.agents/04-design-system.md`: Zinc/shadcn visual and interaction rules.
- `.agents/05-development-standards.md`: code organization, testing, security, and review rules.
- `.agents/06-delivery-roadmap.md`: staged implementation plan and definition of done.
- `.agents/07-release-and-recovery.md`: release gates, signing, local diagnostics, migration backups, and recovery.

## Required working conventions

- This repository is macOS-first. Do not add Windows or Linux support unless the product decision changes.
- Use `pnpm` for JavaScript dependencies and prefix shell commands with `rtk` as required by `~/.codex/RTK.md`.
- Keep SvelteKit UI, Rust Tauri commands, tray/MCP runtime, storage, and domain logic in separate modules. Do not grow a single route or Rust command file into a service object.
- Use shadcn-svelte controls from `$lib/components/ui`, Bits UI for missing accessible primitives, and `@lucide/svelte` for icons. Do not introduce a second component system or hand-roll SVG icons.
- Zinc tokens in `src/app.css` are the color source of truth. Blue is reserved for selection/focus signals; semantic status colors must stay restrained.
- Code shown in the UI is always read-only. File access must be scoped to a registered project root and must reject path traversal.
- Project collection discovery must be bounded, root-marker based, skip symlinks/generated directories, and prune a subtree once a project root is recognized. Never register candidates without user confirmation.
- Every persisted inspector mutation must be versioned, timestamped, and attributable to a command or MCP call.
- Any new third-party dependency requires an entry in `.agents/02-dependency-matrix.md` and a lockfile review.
- Schema upgrades must preserve the automatic SQLite migration backup and recovery invariants in `.agents/07-release-and-recovery.md`.
- Tray changes must pass `pnpm run verify:tray` against a running isolated helper in addition to Rust tests.
- Use focused tests for domain parsers, path security, command idempotency, and MCP authorization before visual polish.
- Use Conventional Commit format: `type(scope): description`.

## Verification baseline

Before handing off a change, run the narrowest relevant checks, then the full checks when available:

```bash
rtk pnpm run check
rtk pnpm run test
rtk cargo test --workspace
```

For UI changes, also run the desktop dev flow and inspect light/dark, narrow/large window, loading, empty, error, keyboard focus, and reduced-motion states.
