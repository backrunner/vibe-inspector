# Dependency Matrix and Integration Decisions

Versions are locked in `pnpm-lock.yaml` and `src-tauri/Cargo.lock`. Update this document whenever a dependency or ownership boundary changes.

| Area                  | Choice                                              | Why                                                                            | Boundary / caution                                                                                                  |
| --------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| Desktop shell         | Tauri v2 (`tauri`, `@tauri-apps/api`)               | Native macOS window and small local footprint                                  | Keep Rust commands thin and capabilities explicit                                                                   |
| Native folder picker  | `@tauri-apps/plugin-dialog` + `tauri-plugin-dialog` | macOS directory selection without broad filesystem permissions                 | Dialog chooses paths only; Rust validates and scans canonical roots                                                 |
| UI framework          | SvelteKit + Svelte 5 + Vite                         | Requested application stack                                                    | Static/client rendering for the Tauri webview; no server assumptions                                                |
| SvelteKit adapter     | `@sveltejs/adapter-static`                          | Self-contained webview output                                                  | Preserve SPA fallback and relative asset behavior                                                                   |
| Styling               | Tailwind CSS v4 + `@tailwindcss/vite`               | Token-driven utility styling without runtime CSS                               | Zinc tokens in `src/app.css` are authoritative                                                                      |
| Component system      | shadcn-svelte (Nova preset)                         | Repo-owned, inspectable components with compact professional defaults          | Components live under `src/lib/components/ui`; do not add a second design system                                    |
| Accessible primitives | `bits-ui`                                           | Focus, keyboard, portal, menu, scroll, and tooltip behavior                    | Prefer generated shadcn wrappers; import Bits UI directly only for a missing primitive                              |
| Icons                 | `@lucide/svelte`                                    | Consistent technical action vocabulary                                         | One icon family; stroke width 1.7-2; no hand-drawn SVG controls                                                     |
| Component utilities   | `tailwind-variants`, `tailwind-merge`, `clsx`       | Variant typing and predictable class merging                                   | Use `$lib/utils` `cn()` instead of ad hoc concatenation for reusable controls                                       |
| Typography            | macOS system UI font + SF Mono                      | Native optical sizing and rasterization without runtime font loading           | Use system-first stacks; do not load remote fonts at runtime                                                        |
| Code editor           | `monaco-editor` through local Svelte adapter        | Read-only syntax rendering and exact line ranges                               | No save action, no editing, no TS language service worker                                                           |
| Diagram               | `mermaid` v11 through a local Svelte adapter        | Mature flow layout, readable branches, and deterministic SVG output            | Dynamic import in the browser, `securityLevel: 'strict'`, Zinc theme variables, and renderer-neutral domain graphs  |
| State                 | Svelte 5 runes in feature stores                    | Small local desktop state surface                                              | Persistence only through typed Tauri adapters                                                                       |
| Persistence           | `rusqlite` with bundled SQLite + online backup      | Transactions, migrations, indexed local data, and consistent upgrade snapshots | Do not split writes between JS and Rust; keep migration backups private and bounded                                 |
| Content hashing       | RustCrypto `sha2`                                   | Deterministic source evidence hashes and stale-reference detection             | Hash bounded local files only; never log source contents                                                            |
| MCP server SDK        | Official Rust SDK `rmcp` 2.2                        | Typed MCP tools plus stdio and streamable HTTP transports                      | The tray hosts authenticated loopback HTTP; handlers call shared domain services and never access SQLite directly   |
| HTTP transport        | `axum` 0.8                                          | Local streamable HTTP routing and authentication middleware                    | Bind only to loopback, validate Host/Origin, require the protected bearer token, and expose no source-writing route |
| Tray shell            | `tray-icon` 0.24 + `tao` 0.35                       | Native macOS menu-bar lifecycle on the required main thread                    | The tray is a separate accessory process with no Dock icon and owns no desktop window                               |
| Desktop singleton     | `tauri-plugin-single-instance` 2.4                  | Focus the existing desktop process when the tray launches it twice             | Register before other Tauri plugins and only show/unminimize/focus the `main` window                                |
| Cancellation          | `tokio-util` 0.7 `CancellationToken`                | Coordinated HTTP, Unix socket, and tray shutdown                               | Quit waits for listeners to stop and removes the per-user socket                                                    |
| Secret generation     | `getrandom` 0.4                                     | OS-backed 256-bit local MCP bearer token                                       | Persist with mode `0600`; never return the token to the WebView or write it to logs                                 |
| macOS user identity   | `libc` 0.2                                          | Resolve the effective UID for the per-user `launchctl gui/<uid>` domain        | Use only for the LaunchAgent domain; no privilege escalation or cross-user operations                               |
| Async runtime         | `tokio` 1.53                                        | Independent MCP/tray process lifecycle and non-blocking local I/O              | Keep blocking SQLite and repository scans behind bounded blocking tasks                                             |
| Tool schemas          | `schemars` 1.2                                      | Generates stable JSON Schema for MCP tool inputs and structured output         | Schema derives belong on wire DTOs only; domain validation remains authoritative                                    |
| Serialization         | Rust `serde` / `serde_json`                         | Stable IPC and MCP DTOs                                                        | Version wire contracts and reject invalid inputs                                                                    |
| CLI arguments         | `clap` 4.5                                          | Stable `vibe-inspector setup`, discovery, doctor, and JSON command contracts   | The desktop binary dispatches CLI subcommands before starting Tauri; no-argument launch behavior is unchanged       |
| Terminal setup UI     | `ratatui` 0.29 + `crossterm` 0.28                   | Keyboard-accessible inline multi-agent selection without a second runtime     | TUI is used only on an interactive terminal; `--agent`, `--yes`, `--dry-run`, and `--json` cover automation         |
| JSONC parsing         | `json5` 1.3                                         | Structured parsing of OpenCode's supported `opencode.jsonc` global config      | Preserve unknown fields and create the first private backup before serializing the managed MCP entry                |
| Manifest parsing      | Rust `toml`                                         | Structured Cargo and Python project-name/workspace detection                   | Read root manifests only and cap manifest size                                                                      |
| Project identity      | Rust `uuid` with UUIDv7                             | Opaque time-sortable IDs for confirmed manual project registrations            | IDs are created in Rust after root revalidation, never from display names                                           |
| Testing               | Vitest, svelte-check, Playwright CLI, Cargo tests   | Domain, compile, browser, and desktop coverage                                 | Visual checks do not replace path-security or idempotency tests                                                     |

CodeGraph uses the existing `rusqlite` dependency only to query a project-owned `.codegraph/codegraph.db` in read-only mode. VibeInspector never creates, migrates, or mutates that index.

## shadcn/Bits UI contract

Generated components are project-owned and imported from stable local paths:

```svelte
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
</script>
```

Rules:

1. Add a standard control with `pnpm dlx shadcn-svelte@latest add <component>` before implementing a local replacement.
2. Keep Bits UI behavior and ARIA contracts intact when styling generated components.
3. Put domain-specific composition under `src/lib/components/<feature>`, never in `components/ui`.
4. Use Zinc theme variables and Tailwind utilities; do not hard-code a parallel palette.
5. Review generated component diffs and lockfile changes before handoff.

## Current JavaScript inventory

Runtime and build packages include:

```text
bits-ui
shadcn-svelte
tailwindcss
@tailwindcss/vite
@lucide/svelte
tailwind-variants
tailwind-merge
clsx
tw-animate-css
monaco-editor
mermaid
@tauri-apps/api
@tauri-apps/plugin-dialog
```

## Dependency gate

Before adding a package, check Svelte 5 and Tauri compatibility, license, maintenance, and bundle impact. Packages that duplicate shadcn/Bits UI, Lucide, Tailwind variants, state management, or Mermaid diagram behavior require an explicit architecture decision.

Agent integrations (skill/MCP deployment to local coding agents) reuse `serde_json`, `toml`, `json5`, and `getrandom` plus `std::fs`; the CLI/TUI uses `clap`, `ratatui`, and `crossterm`, while the frontend continues to use the established adapter/store pattern.
