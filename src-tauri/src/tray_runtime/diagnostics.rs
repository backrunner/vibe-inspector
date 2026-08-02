use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use super::{TrayError, TrayPaths};

const DIAGNOSTICS_FILE: &str = "diagnostics.jsonl";
const MAX_DIAGNOSTICS_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticProcess {
    Desktop,
    Mcp,
    Tray,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticEvent {
    Started,
    Panic,
    McpOnline,
    McpUnavailable,
    TrayHelperReady,
    TrayHelperUnavailable,
    TrayStartSuppressed,
    Shutdown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticRecord {
    schema_version: u32,
    process: DiagnosticProcess,
    event: DiagnosticEvent,
    pid: u32,
    occurred_at: String,
}

pub fn record_diagnostic(
    paths: &TrayPaths,
    process: DiagnosticProcess,
    event: DiagnosticEvent,
) -> Result<(), TrayError> {
    paths.ensure_runtime_directory()?;
    let path = diagnostics_path(paths);
    rotate_if_needed(&path)?;
    let record = DiagnosticRecord {
        schema_version: 1,
        process,
        event,
        pid: std::process::id(),
        occurred_at: unix_epoch_millis(),
    };
    let mut payload =
        serde_json::to_vec(&record).map_err(|error| TrayError::Protocol(error.to_string()))?;
    payload.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    file.write_all(&payload)?;
    Ok(())
}

pub fn install_local_panic_hook(paths: TrayPaths, process: DiagnosticProcess) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = record_diagnostic(&paths, process, DiagnosticEvent::Panic);
        previous(panic);
    }));
}

fn rotate_if_needed(path: &PathBuf) -> Result<(), TrayError> {
    let should_rotate = path
        .metadata()
        .map(|metadata| metadata.len() >= MAX_DIAGNOSTICS_BYTES)
        .unwrap_or(false);
    if !should_rotate {
        return Ok(());
    }
    let rotated = path.with_extension("jsonl.1");
    if rotated.exists() {
        fs::remove_file(&rotated)?;
    }
    fs::rename(path, rotated)?;
    Ok(())
}

fn diagnostics_path(paths: &TrayPaths) -> PathBuf {
    paths.app_support_dir.join(DIAGNOSTICS_FILE)
}

fn unix_epoch_millis() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::{os::unix::fs::MetadataExt, path::PathBuf};

    use uuid::Uuid;

    use super::*;

    #[test]
    fn diagnostics_are_private_and_contain_only_bounded_metadata() {
        let root = PathBuf::from("/tmp").join(format!("vi-diagnostics-{}", Uuid::now_v7()));
        let paths = TrayPaths::from_directories(root.join("runtime"), root.join("agents"));
        record_diagnostic(&paths, DiagnosticProcess::Tray, DiagnosticEvent::McpOnline)
            .expect("record diagnostic");

        let path = diagnostics_path(&paths);
        let payload = fs::read_to_string(&path).expect("read diagnostic");
        assert!(payload.contains("\"mcp-online\""));
        assert!(!payload.contains("token"));
        assert!(!payload.contains("project"));
        assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, 0o600);

        fs::remove_dir_all(root).unwrap();
    }
}
