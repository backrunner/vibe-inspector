# VibeInspector Engineering Docs

These documents are the implementation baseline for the macOS-only Tauri + SvelteKit application.

| Document                          | Use it for                                                         |
| --------------------------------- | ------------------------------------------------------------------ |
| `00-product-requirements.md`      | Scope, journeys, states, and acceptance criteria                   |
| `01-system-architecture.md`       | Process topology, module boundaries, persistence, and data flow    |
| `02-dependency-matrix.md`         | Package choices, shadcn/Bits UI integration, and dependency gates  |
| `03-mcp-and-command-contracts.md` | MCP CRUD tools, slash commands, entity DTOs, and safety rules      |
| `04-design-system.md`             | Zinc/shadcn layout, tokens, interaction, and visual QA             |
| `05-development-standards.md`     | TypeScript/Svelte/Rust, testing, security, performance, and review |
| `06-delivery-roadmap.md`          | Phases, vertical slice, exit criteria, and resolved decisions      |
| `07-release-and-recovery.md`      | macOS CI, signing, diagnostics, backups, and recovery              |

The implementation decisions for Codex history, SQLite, MCP transports, Mermaid, and macOS packaging are recorded as resolved baselines. Credentialed signing/notarization and the final hardware/OS matrix remain release operations.
