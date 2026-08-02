use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    time::{SystemTime, UNIX_EPOCH},
};

use uuid::Uuid;

use super::{TrayError, TrayPaths};

/// An explicit tray quit (menu or control socket) records a quit-intent marker.
/// While the marker exists, implicit launchers (the stdio MCP sidecar) must not
/// respawn the tray; explicit user actions (opening the desktop app, enabling
/// the login item, or any tray start) clear it first.
pub fn quit_intent_present(paths: &TrayPaths) -> bool {
    paths.quit_intent_path.is_file()
}

pub fn mark_quit_intent(paths: &TrayPaths) -> Result<(), TrayError> {
    paths.ensure_runtime_directory()?;
    let temporary_path = paths
        .app_support_dir
        .join(format!(".tray-quit-intent-{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary_path)?;
    let occurred_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string();
    file.write_all(occurred_at.as_bytes())?;
    file.sync_all()?;
    fs::rename(&temporary_path, &paths.quit_intent_path)?;
    Ok(())
}

pub fn clear_quit_intent(paths: &TrayPaths) -> Result<(), TrayError> {
    match fs::remove_file(&paths.quit_intent_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(TrayError::Io(error)),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn quit_intent_marks_and_clears_a_private_marker() {
        let root = PathBuf::from("/tmp").join(format!("vi-quit-intent-{}", Uuid::now_v7()));
        let paths = TrayPaths::from_directories(root.join("runtime"), root.join("agents"));

        assert!(!quit_intent_present(&paths));
        clear_quit_intent(&paths).expect("clearing a missing marker is a no-op");

        mark_quit_intent(&paths).expect("mark quit intent");
        assert!(quit_intent_present(&paths));
        assert_eq!(
            fs::metadata(&paths.quit_intent_path).unwrap().mode() & 0o777,
            0o600
        );

        clear_quit_intent(&paths).expect("clear quit intent");
        assert!(!quit_intent_present(&paths));

        fs::remove_dir_all(root).unwrap();
    }
}
