# Development Standards

## 1. TypeScript and Svelte

- `strict` TypeScript is mandatory. Do not use `any` at Tauri/MCP boundaries; use `unknown` plus validation.
- Use Svelte 5 runes consistently within a feature. Do not mix legacy `$:` patterns into new stores without a documented reason.
- Keep route components orchestration-focused. Put reusable UI in feature directories and all IPC in `src/lib/adapters/tauri.ts`.
- Define stable `data-testid` attributes only for integration-critical controls; prefer accessible roles and names in tests.
- Normalize dates, paths, and IDs in adapters, not throughout templates.

## 2. Rust

- Rust commands return `Result<T, AppError>` and serialize stable error codes.
- Use `thiserror` for domain errors; do not stringify errors at the repository layer.
- Keep filesystem and process operations async where appropriate and never block the UI thread on repository scans.
- All external input is validated at the command/MCP boundary and again at domain boundaries where invariants matter.
- Use migrations for schema changes; no ad-hoc production table edits.

## 3. Security and privacy

- Treat Codex history and repository files as sensitive local data. Do not send contents to a network service in v1.
- Canonicalize project roots, reject path traversal and escaping symlinks, and cap read size and line ranges.
- Do not expose arbitrary filesystem or shell Tauri commands to the webview.
- Store MCP session secrets in macOS Keychain or a per-session protected channel; never commit secrets or print them to logs.
- Redact source text, prompts, and tokens from routine logs. Audit metadata only.
- Use Tauri capability allow-lists; grant the webview only the commands it needs.

## 4. Testing requirements

Unit tests are required for:

- Codex history path extraction and malformed input handling.
- Project collection root detection, maximum depth, subtree pruning, manifest-name precedence, ignored generated directories, and invalid single-project roots.
- Path canonicalization, symlink escape detection, byte/line limits, and stale file hashes.
- Stable ID and scope fingerprint generation.
- Init/sync/clarify structural rules and revision conflicts.
- MCP authorization, idempotency, error mapping, and CRUD parity with UI services.

Integration tests are required for:

- SQLite migrations and transaction rollback.
- Tauri command serialization and a fixture project scan.
- A read-only Monaco smoke test that confirms keyboard editing and save paths are disabled.
- Tray health, desktop launch/focus, and clean quit on macOS CI or a documented local verification lane.
- Schema upgrades creating a private SQLite online backup and recovery rejecting corrupt or out-of-directory files.

## 5. Performance budgets

- Initial webview interactive target: <2 seconds on a recent Apple Silicon Mac with a seeded project.
- Project list and module tree should render 1,000 rows without long tasks caused by full-tree rerenders.
- Module-tree filtering, descendant flow counts, and visible-row construction use one linear domain pass and are covered by a 1,000-module fixture; templates must not recurse once per rendered row.
- The desktop viewer may read the complete selected UTF-8 file up to the filesystem broker's file-size limit so evidence can be understood in context. MCP reads stay range-bounded, and related-file discovery must query only an existing local CodeGraph index.
- Debounce history scans and file hash refreshes. Never scan the whole monorepo on every tree expansion.
- Manual collection discovery visits at most 4,096 directories, returns at most 200 candidates, skips symlinks, and stops below a recognized project root.
- Keep diagram node updates incremental; preserve viewport and selection on data refresh.
- Keep generated JavaScript/CSS within `pnpm run check:bundle-budget`; adjust a budget only with a measured startup or feature justification.

## 6. Git and review

- Commit format: `type(scope): description`.
- PRs describe affected process boundaries, migration changes, security implications, and manual macOS checks.
- No drive-by formatting or generated lockfile churn unrelated to the change.
- Every new persisted field includes migration, DTO, fixture, and test updates.
- The bootstrap JSON project registry is schema-versioned, revisioned, timestamped, attributed to `manual`, and written atomically. Migrate it into SQLite rather than extending it into a general database.
- UI review must include screenshots or a short recording for meaningful layout/state changes.

## 7. Definition of done for a feature

The feature has typed contracts, domain tests, failure and empty states, accessibility names/focus behavior, shadcn/Zinc token usage, migration (if needed), documentation update, and verification commands recorded in the PR. It does not silently bypass the tray, MCP, filesystem broker, or provenance rules.
