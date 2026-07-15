fn main() {
    ensure_tray_sidecar_placeholder();
    tauri_build::build()
}

fn ensure_tray_sidecar_placeholder() {
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

    let Some(target) = std::env::var_os("TARGET") else {
        return;
    };
    let directory = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("binaries");
    let path = directory.join(format!("vibe-inspector-tray-{}", target.to_string_lossy()));
    if path.exists() {
        return;
    }
    fs::create_dir_all(&directory).expect("create generated sidecar directory");
    fs::write(
        &path,
        b"#!/bin/sh\necho 'VibeInspector tray helper was not prepared' >&2\nexit 1\n",
    )
    .expect("write generated sidecar placeholder");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("mark generated sidecar placeholder executable");
}
