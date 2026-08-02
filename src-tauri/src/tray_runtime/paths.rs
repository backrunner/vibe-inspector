use std::{env, fs, os::unix::fs::PermissionsExt, path::PathBuf};

use super::TrayError;

const APP_IDENTIFIER: &str = "com.vibeinspector.desktop";
const LAUNCH_AGENT_LABEL: &str = "com.vibeinspector.tray";

#[derive(Debug, Clone)]
pub struct TrayPaths {
    pub app_support_dir: PathBuf,
    pub socket_path: PathBuf,
    pub token_path: PathBuf,
    pub endpoint_path: PathBuf,
    pub quit_intent_path: PathBuf,
    pub launch_agent_path: PathBuf,
}

impl TrayPaths {
    pub fn for_macos_user() -> Result<Self, TrayError> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| TrayError::Config("HOME is unavailable".to_string()))?;

        if let Some(runtime_dir) = env::var_os("VIBE_INSPECTOR_RUNTIME_DIR") {
            return Ok(Self::from_directories(
                PathBuf::from(runtime_dir),
                home.join("Library").join("LaunchAgents"),
            ));
        }

        Ok(Self::from_home(home))
    }

    pub fn from_home(home: PathBuf) -> Self {
        Self::from_directories(
            home.join("Library")
                .join("Application Support")
                .join(APP_IDENTIFIER),
            home.join("Library").join("LaunchAgents"),
        )
    }

    pub fn from_directories(app_support_dir: PathBuf, launch_agents_dir: PathBuf) -> Self {
        Self {
            socket_path: app_support_dir.join("tray.sock"),
            token_path: app_support_dir.join("mcp-token"),
            endpoint_path: app_support_dir.join("mcp-endpoint.json"),
            quit_intent_path: app_support_dir.join("tray-quit.intent"),
            launch_agent_path: launch_agents_dir.join(format!("{LAUNCH_AGENT_LABEL}.plist")),
            app_support_dir,
        }
    }

    pub fn ensure_runtime_directory(&self) -> Result<(), TrayError> {
        fs::create_dir_all(&self.app_support_dir)?;
        fs::set_permissions(&self.app_support_dir, fs::Permissions::from_mode(0o700))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_share_the_desktop_application_support_directory() {
        let paths = TrayPaths::from_home(PathBuf::from("/Users/example"));
        assert_eq!(
            paths.app_support_dir,
            PathBuf::from("/Users/example/Library/Application Support/com.vibeinspector.desktop")
        );
        assert_eq!(
            paths.launch_agent_path,
            PathBuf::from("/Users/example/Library/LaunchAgents/com.vibeinspector.tray.plist")
        );
    }
}
