<div align="center">
  <img src="./src-tauri/icons/icon.png" width="112" alt="VibeInspector app icon" />
  <h1>VibeInspector</h1>
  <p><strong>A local-first architecture workbench for fast-moving codebases.</strong></p>
  <p>Map repository modules, trace business flows, and keep architectural explanations connected to the exact source evidence behind them.</p>

  <p>
    <a href="#requirements"><img src="https://img.shields.io/badge/platform-macOS-111111?logo=apple&amp;logoColor=white" alt="macOS" /></a>
    <a href="https://tauri.app/"><img src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&amp;logoColor=white" alt="Tauri 2" /></a>
    <a href="https://svelte.dev/"><img src="https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&amp;logoColor=white" alt="Svelte 5" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&amp;logoColor=white" alt="Rust stable" /></a>
    <a href="./LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="Apache-2.0 license" /></a>
  </p>
</div>

## What it does

VibeInspector turns a repository into a durable, navigable model of projects, modules, business flows, explanations, and source references. The desktop app is intentionally read-only: it helps you understand and review a system without becoming another code editor.

The independent menu-bar helper keeps authenticated MCP tools available even when the desktop window is closed, so Codex workflows can initialize, synchronize, and clarify inspector data in the background.

## Highlights

- Three-pane workspace for projects, nested modules, flows, diagrams, and source evidence.
- Bounded project discovery with root-marker detection, subtree pruning, and explicit confirmation.
- Mermaid flow diagrams paired with architectural rationale and provenance.
- Hierarchical source tree and read-only Monaco viewer with exact line-range highlighting.
- SQLite persistence with versioned mutations, audit history, migration backups, and recovery checks.
- Independent macOS tray runtime with authenticated loopback MCP, login-item control, and graceful shutdown.
- Twenty-one MCP tools plus `$inspector-init`, `$inspector-sync`, and `$inspector-clarify` project skills.
- Zinc-based light and dark themes built with Svelte 5, shadcn-svelte, and Bits UI.

## Architecture

```text
Codex / MCP clients
        |
        v
Menu-bar helper (Rust)
  - authenticated MCP over loopback HTTP
  - Unix socket health and lifecycle control
  - launch-at-login support
        |
        v
Tauri desktop (Rust)  <->  SQLite + read-only filesystem broker
        |
        v
SvelteKit workspace
  - project and module navigation
  - Mermaid flow visualization
  - read-only Monaco source evidence
```

The tray and desktop processes share domain, storage, MCP, discovery, and filesystem-security modules while keeping their lifecycles independent. See [`.agents/01-system-architecture.md`](./.agents/01-system-architecture.md) for process boundaries and data flow.

## Requirements

- macOS on Apple Silicon
- Node.js 20 or newer
- pnpm 11
- Stable Rust toolchain with the `aarch64-apple-darwin` target
- Xcode Command Line Tools

## Development

Install dependencies and launch the native desktop flow:

```bash
pnpm install
pnpm tauri dev
```

For browser-only UI work, run:

```bash
pnpm run dev
```

The development server listens on [http://localhost:4173](http://localhost:4173). Add `?demo=1` for the deterministic visual fixture; production Tauri data always comes from registered project roots and SQLite.

## Verification

Run the baseline checks before submitting a change:

```bash
pnpm run check
pnpm run test
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

Tray changes require an isolated running helper and the contract verifier:

```bash
pnpm run prepare:tray:dev
pnpm run verify:tray
```

The verifier checks tray and MCP health, confirms unauthenticated requests are rejected, completes an MCP handshake, and validates the 21-tool contract without printing the bearer token.

## Repository map

| Path                 | Responsibility                                                                       |
| -------------------- | ------------------------------------------------------------------------------------ |
| `src/`               | SvelteKit application, adapters, stores, domain models, and repo-owned UI components |
| `src-tauri/`         | Tauri shell, storage, discovery, MCP, filesystem broker, and tray runtime            |
| `scripts/`           | Bundle-budget, tray preparation, and runtime verification tooling                    |
| `.agents/`           | Product requirements, architecture, contracts, design system, and delivery standards |
| `.github/workflows/` | macOS build and verification pipeline                                                |

## Security and privacy

VibeInspector is local-first and has no network telemetry. Repository content is read only through a canonical-root filesystem broker. MCP HTTP listens only on loopback and requires a protected bearer token. Routine diagnostics contain process metadata only and never include source text, prompts, repository paths, or credentials.

Security and recovery invariants are documented in [`.agents/07-release-and-recovery.md`](./.agents/07-release-and-recovery.md).

## License

Licensed under the [Apache License 2.0](./LICENSE).
