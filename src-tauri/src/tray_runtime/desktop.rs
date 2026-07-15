use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use super::{ipc::send_control_request, TrayError, TrayPaths};
use crate::tray_runtime::ipc::TrayControlRequest;

pub fn ensure_tray_running(paths: &TrayPaths) -> Result<(), TrayError> {
    if send_control_request(paths, TrayControlRequest::Health).is_ok() {
        return Ok(());
    }

    let executable = tray_executable_for(&std::env::current_exe()?)
        .ok_or_else(|| TrayError::Config("tray helper executable was not found".to_string()))?;
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

fn tray_executable_for(desktop_executable: &Path) -> Option<PathBuf> {
    std::env::var_os("VIBE_INSPECTOR_TRAY_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            desktop_executable
                .parent()
                .map(|parent| parent.join("vibe-inspector-tray"))
        })
        .filter(|path| path.is_file())
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
}
