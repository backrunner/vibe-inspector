use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use super::{ipc::send_control_request, quit_intent_present, TrayError, TrayPaths};
use crate::tray_runtime::ipc::TrayControlRequest;

pub fn ensure_tray_running(paths: &TrayPaths) -> Result<(), TrayError> {
    let executable = resolve_tray_executable(&std::env::current_exe()?)
        .ok_or_else(|| TrayError::Config("tray helper executable was not found".to_string()))?;
    ensure_tray_running_at(paths, &executable)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureTrayOutcome {
    /// The tray already answered a health probe.
    Running,
    /// The tray was spawned by this call and became ready.
    Started,
    /// An explicit quit intent is recorded, so the implicit launcher backed off.
    Suppressed,
}

/// Ensure variant for implicit launchers (the stdio MCP sidecar): never respawns
/// the tray while a quit-intent marker is present.
pub fn ensure_tray_running_unless_quit(
    paths: &TrayPaths,
    executable: &Path,
) -> Result<EnsureTrayOutcome, TrayError> {
    if send_control_request(paths, TrayControlRequest::Health).is_ok() {
        return Ok(EnsureTrayOutcome::Running);
    }
    if quit_intent_present(paths) {
        return Ok(EnsureTrayOutcome::Suppressed);
    }
    ensure_tray_running_at(paths, executable)?;
    Ok(EnsureTrayOutcome::Started)
}

pub fn ensure_tray_running_at(paths: &TrayPaths, executable: &Path) -> Result<(), TrayError> {
    if send_control_request(paths, TrayControlRequest::Health).is_ok() {
        return Ok(());
    }

    Command::new(executable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    for _ in 0..10 {
        thread::sleep(Duration::from_millis(50));
        if send_control_request(paths, TrayControlRequest::Health).is_ok() {
            return Ok(());
        }
    }
    Err(TrayError::Runtime(
        "tray helper did not become ready".to_string(),
    ))
}

pub fn resolve_tray_executable(current_executable: &Path) -> Option<PathBuf> {
    std::env::var_os("VIBE_INSPECTOR_TRAY_PATH")
        .map(PathBuf::from)
        .or_else(|| Some(tray_executable_sibling(current_executable)))
        .filter(|path| path.is_file())
}

fn tray_executable_sibling(executable: &Path) -> PathBuf {
    executable
        .parent()
        .map(|parent| parent.join("vibe-inspector-tray"))
        .unwrap_or_else(|| PathBuf::from("vibe-inspector-tray"))
}

pub fn open_desktop() -> Result<(), TrayError> {
    let current_exe = std::env::current_exe()?;
    if let Some(bundle) = enclosing_app_bundle(&current_exe) {
        let status = Command::new("open").arg(bundle).status()?;
        if status.success() {
            return Ok(());
        }
        return Err(TrayError::Runtime(format!(
            "macOS open command exited with {status}"
        )));
    }

    let desktop = std::env::var_os("VIBE_INSPECTOR_DESKTOP_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            current_exe
                .parent()
                .map(|parent| parent.join("vibe-inspector"))
        })
        .filter(|path| path.is_file())
        .ok_or_else(|| TrayError::Config("desktop executable was not found".to_string()))?;
    Command::new(desktop)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn enclosing_app_bundle(executable: &Path) -> Option<PathBuf> {
    executable
        .ancestors()
        .find(|ancestor| {
            ancestor
                .extension()
                .is_some_and(|extension| extension == "app")
        })
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_helper_resolves_its_exact_app_bundle() {
        let executable =
            Path::new("/Applications/VibeInspector.app/Contents/MacOS/vibe-inspector-tray");
        assert_eq!(
            enclosing_app_bundle(executable),
            Some(PathBuf::from("/Applications/VibeInspector.app"))
        );
    }

    #[test]
    fn tray_executable_sibling_sits_beside_the_current_binary() {
        assert_eq!(
            tray_executable_sibling(Path::new(
                "/Applications/VibeInspector.app/Contents/MacOS/vibe-inspector-mcp"
            )),
            PathBuf::from("/Applications/VibeInspector.app/Contents/MacOS/vibe-inspector-tray")
        );
        assert_eq!(
            tray_executable_sibling(Path::new("vibe-inspector-mcp")),
            PathBuf::from("vibe-inspector-tray")
        );
    }

    #[test]
    fn implicit_ensure_is_suppressed_by_a_recorded_quit_intent() {
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("vi-ensure-suppressed-{}", uuid::Uuid::now_v7()));
        let paths = TrayPaths::from_directories(root.join("runtime"), root.join("agents"));
        crate::tray_runtime::mark_quit_intent(&paths).expect("mark quit intent");

        let outcome =
            ensure_tray_running_unless_quit(&paths, Path::new("/nonexistent/vibe-inspector-tray"))
                .expect("suppression is not an error");

        assert_eq!(outcome, EnsureTrayOutcome::Suppressed);
        assert!(quit_intent_present(&paths));

        std::fs::remove_dir_all(root).unwrap();
    }
}
