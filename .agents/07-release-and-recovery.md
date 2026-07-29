# VibeInspector Release and Recovery

## 1. Release posture

VibeInspector v1 is a macOS-only, local-first application. The app bundle contains three executables in `Contents/MacOS`: `vibe-inspector` for the Tauri desktop, `vibe-inspector-tray` for the independent menu-bar/HTTP MCP runtime, and `vibe-inspector-mcp` for authenticated stdio clients. The Tauri build must package and sign all three executables together; the tray uses the `externalBin` preparation pipeline.

The project has no network telemetry. Local diagnostics are metadata-only JSON Lines records in the application support directory. They contain process, event type, PID, and timestamp only; they must never contain prompts, source, repository paths, bearer tokens, or error strings.

## 2. Automated release gate

The macOS CI lane runs Svelte checks/tests/build, bundle budgets, Rust format/clippy/tests, and a debug `.app` bundle smoke test. It asserts that the tray helper is executable inside the final bundle. A release candidate must also pass:

```bash
pnpm install --frozen-lockfile
pnpm run check
pnpm run test
pnpm run build
pnpm run check:bundle-budget
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
pnpm exec tauri build --bundles app --ci
```

With the tray running, `pnpm run verify:tray` must report tray/MCP online, an unauthenticated 401, initialize 200, and 21 tools.

## 3. Signing and notarization

Release builds require a Developer ID Application certificate and Tauri's Apple signing/notarization environment. Keep certificate material and Apple credentials in the release CI secret store, never in repository files. Use App Store Connect issuer/key credentials or an Apple ID app-specific password supported by the current Tauri release tooling.

After building, verify the nested helper and complete bundle before distribution:

```bash
codesign --verify --deep --strict --verbose=2 src-tauri/target/release/bundle/macos/VibeInspector.app
spctl --assess --type execute --verbose=2 src-tauri/target/release/bundle/macos/VibeInspector.app
xcrun stapler validate src-tauri/target/release/bundle/macos/VibeInspector.app
```

The LaunchAgent references the helper's exact installed bundle path. Moving the app after enabling `Start at Login` requires toggling that menu item off and on so the plist is rewritten.

## 4. Migration backups

Before upgrading a non-empty older SQLite schema, VibeInspector uses SQLite's online backup API to create a consistent snapshot under:

```text
~/Library/Application Support/com.vibeinspector.desktop/backups/
```

The primary database and backup files are user-only (`0600`); the backup directory is `0700`. The newest five migration backups are retained. Fresh databases and repeated starts on the current schema do not create backups.

## 5. Recovery procedure

1. Quit `VibeInspector Services` from the menu-bar item and close the desktop app.
2. In Finder, use Go to Folder and open `~/Library/Application Support/com.vibeinspector.desktop/`.
3. Preserve the failed `inspector.sqlite3` plus any `-wal` and `-shm` files before changing them.
4. Choose the newest valid file in `backups/`, copy it to the parent directory as `inspector.sqlite3`, and ensure only the user can read it.
5. Remove stale `inspector.sqlite3-wal` and `inspector.sqlite3-shm` files only while both processes are stopped.
6. Start VibeInspector again. The app validates and migrates the restored snapshot.

Do not restore a database from another user or from outside VibeInspector's backup directory through application code. `restore_migration_backup` canonicalizes the path and runs `PRAGMA integrity_check` before replacement.

## 6. Manual macOS matrix

Release candidates must cover fresh install, upgrade from the previous schema, tray-only login start, desktop launch/focus twice, window close with MCP still online, graceful tray quit, missing project root, unavailable Codex history, large collection scan, light/dark, `1100x680` and `1440x900`, keyboard focus, VoiceOver labels, and reduced motion. Record the macOS version, architecture, result, and any residual issue in the release ticket.
