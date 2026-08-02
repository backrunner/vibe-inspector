use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::Command,
};

use uuid::Uuid;

use super::{TrayError, TrayPaths};

const LABEL: &str = "com.vibeinspector.tray";

pub fn is_login_item_enabled(paths: &TrayPaths) -> bool {
    paths.launch_agent_path.is_file()
}

pub fn set_login_item_enabled(
    paths: &TrayPaths,
    executable: &Path,
    enabled: bool,
) -> Result<(), TrayError> {
    if enabled {
        install_launch_agent(paths, executable)
    } else {
        uninstall_launch_agent(paths)
    }
}

fn install_launch_agent(paths: &TrayPaths, executable: &Path) -> Result<(), TrayError> {
    let parent = paths.launch_agent_path.parent().ok_or_else(|| {
        TrayError::Config("LaunchAgents directory could not be resolved".to_string())
    })?;
    fs::create_dir_all(parent)?;
    let plist = render_launch_agent(executable);
    let temporary_path = parent.join(format!(".{LABEL}-{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&temporary_path)?;
    file.write_all(plist.as_bytes())?;
    file.sync_all()?;
    fs::rename(&temporary_path, &paths.launch_agent_path)?;

    let domain = launchctl_domain();
    let _ = Command::new("launchctl")
        .args(["bootout", &domain])
        .arg(&paths.launch_agent_path)
        .status();
    let status = Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(&paths.launch_agent_path)
        .status()?;
    if !status.success() {
        return Err(TrayError::Runtime(format!(
            "launchctl bootstrap exited with {status}"
        )));
    }
    // Enabling the login item is an explicit user action.
    let _ = super::clear_quit_intent(paths);
    Ok(())
}

fn uninstall_launch_agent(paths: &TrayPaths) -> Result<(), TrayError> {
    if !paths.launch_agent_path.exists() {
        return Ok(());
    }
    let domain = launchctl_domain();
    let _ = Command::new("launchctl")
        .args(["bootout", &domain])
        .arg(&paths.launch_agent_path)
        .status();
    fs::remove_file(&paths.launch_agent_path)?;
    Ok(())
}

fn launchctl_domain() -> String {
    format!("gui/{}", unsafe { libc::geteuid() })
}

fn render_launch_agent(executable: &Path) -> String {
    let executable = escape_xml(&executable.to_string_lossy());
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
  <key>Label</key>\n\
  <string>{LABEL}</string>\n\
  <key>ProgramArguments</key>\n\
  <array>\n\
    <string>{executable}</string>\n\
  </array>\n\
  <key>RunAtLoad</key>\n\
  <true/>\n\
  <key>KeepAlive</key>\n\
  <dict>\n\
    <key>SuccessfulExit</key>\n\
    <false/>\n\
  </dict>\n\
  <key>ProcessType</key>\n\
  <string>Interactive</string>\n\
</dict>\n\
</plist>\n"
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_agent_escapes_executable_and_only_restarts_failures() {
        let plist = render_launch_agent(Path::new("/Applications/A & B/VibeInspector.app/tray"));
        assert!(plist.contains("A &amp; B"));
        assert!(plist.contains("<key>SuccessfulExit</key>"));
        assert!(plist.contains("<false/>"));
        assert!(!plist.contains("KeepAlive</key>\n  <true/>"));
    }
}
