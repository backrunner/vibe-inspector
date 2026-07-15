use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream},
    },
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};
use tokio_util::sync::CancellationToken;

use super::{TrayError, TrayPaths};

const MAX_CONTROL_MESSAGE_BYTES: usize = 16 * 1024;
const CONTROL_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub enum SocketClaim {
    Claimed(StdUnixListener),
    AlreadyRunning,
}

#[derive(Debug, Clone, Copy)]
pub enum ControlAction {
    OpenDesktop,
    LoginItemChanged(bool),
    Quit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayHealth {
    pub tray: String,
    pub mcp: String,
    pub endpoint: Option<String>,
    pub error: Option<String>,
    pub login_item_enabled: bool,
    pub pid: u32,
    pub started_at: String,
    pub checked_at: String,
}

impl TrayHealth {
    pub fn starting(login_item_enabled: bool) -> Self {
        let now = unix_epoch_millis();
        Self {
            tray: "online".to_string(),
            mcp: "starting".to_string(),
            endpoint: None,
            error: None,
            login_item_enabled,
            pid: std::process::id(),
            started_at: now.clone(),
            checked_at: now,
        }
    }

    pub fn mark_online(&mut self, endpoint: String) {
        self.mcp = "online".to_string();
        self.endpoint = Some(endpoint);
        self.error = None;
        self.checked_at = unix_epoch_millis();
    }

    pub fn mark_error(&mut self, error: String) {
        self.mcp = "error".to_string();
        self.error = Some(error);
        self.checked_at = unix_epoch_millis();
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "camelCase")]
pub(super) enum TrayControlRequest {
    Health,
    OpenDesktop,
    SetLoginItem { enabled: bool },
    Quit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TrayControlResponse {
    ok: bool,
    health: Option<TrayHealth>,
    error: Option<String>,
}

pub fn claim_control_socket(paths: &TrayPaths) -> Result<SocketClaim, TrayError> {
    paths.ensure_runtime_directory()?;
    if paths.socket_path.exists() {
        if notify_existing_instance(paths).is_ok() {
            return Ok(SocketClaim::AlreadyRunning);
        }
        match fs::remove_file(&paths.socket_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    match StdUnixListener::bind(&paths.socket_path) {
        Ok(listener) => {
            listener.set_nonblocking(true)?;
            fs::set_permissions(&paths.socket_path, fs::Permissions::from_mode(0o600))?;
            Ok(SocketClaim::Claimed(listener))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            if notify_existing_instance(paths).is_ok() {
                Ok(SocketClaim::AlreadyRunning)
            } else {
                Err(error.into())
            }
        }
        Err(error) => Err(error.into()),
    }
}

fn notify_existing_instance(paths: &TrayPaths) -> Result<(), TrayError> {
    let mut stream = StdUnixStream::connect(&paths.socket_path)?;
    stream.set_write_timeout(Some(CONTROL_TIMEOUT))?;
    let payload = serde_json::to_vec(&TrayControlRequest::OpenDesktop)
        .map_err(|error| TrayError::Protocol(error.to_string()))?;
    stream.write_all(&payload)?;
    stream.write_all(b"\n")?;
    Ok(())
}

pub fn tray_health(paths: &TrayPaths) -> Result<TrayHealth, TrayError> {
    let response = send_control_request(paths, TrayControlRequest::Health)?;
    if response.ok {
        response
            .health
            .ok_or_else(|| TrayError::Protocol("health response was empty".to_string()))
    } else {
        Err(TrayError::Protocol(response.error.unwrap_or_else(|| {
            "tray health request failed".to_string()
        })))
    }
}

pub fn set_tray_login_item(paths: &TrayPaths, enabled: bool) -> Result<TrayHealth, TrayError> {
    let response = send_control_request(paths, TrayControlRequest::SetLoginItem { enabled })?;
    if response.ok {
        response.health.ok_or_else(|| {
            TrayError::Protocol("login item response did not include health".to_string())
        })
    } else {
        Err(TrayError::Runtime(
            response
                .error
                .unwrap_or_else(|| "login item update failed".to_string()),
        ))
    }
}

pub(super) fn send_control_request(
    paths: &TrayPaths,
    request: TrayControlRequest,
) -> Result<TrayControlResponse, TrayError> {
    let mut stream = StdUnixStream::connect(&paths.socket_path)?;
    stream.set_read_timeout(Some(CONTROL_TIMEOUT))?;
    stream.set_write_timeout(Some(CONTROL_TIMEOUT))?;
    let payload =
        serde_json::to_vec(&request).map_err(|error| TrayError::Protocol(error.to_string()))?;
    stream.write_all(&payload)?;
    stream.write_all(b"\n")?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut payload = Vec::new();
    stream
        .take(MAX_CONTROL_MESSAGE_BYTES as u64)
        .read_to_end(&mut payload)?;
    serde_json::from_slice(&payload).map_err(|error| TrayError::Protocol(error.to_string()))
}

pub async fn serve_control_socket(
    listener: StdUnixListener,
    health: Arc<RwLock<TrayHealth>>,
    action: Arc<dyn Fn(ControlAction) + Send + Sync>,
    cancellation: CancellationToken,
) -> Result<(), TrayError> {
    let listener = UnixListener::from_std(listener)?;
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let health = health.clone();
                let action = action.clone();
                tokio::spawn(async move {
                    let _ = handle_connection(stream, health, action).await;
                });
            }
        }
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    health: Arc<RwLock<TrayHealth>>,
    action: Arc<dyn Fn(ControlAction) + Send + Sync>,
) -> Result<(), TrayError> {
    let payload = read_control_message(&mut stream).await?;
    let request: TrayControlRequest =
        serde_json::from_slice(&payload).map_err(|error| TrayError::Protocol(error.to_string()))?;
    let response = match request {
        TrayControlRequest::Health => {
            let mut health = health
                .read()
                .map_err(|_| TrayError::Runtime("tray health lock is poisoned".to_string()))?
                .clone();
            health.checked_at = unix_epoch_millis();
            TrayControlResponse {
                ok: true,
                health: Some(health),
                error: None,
            }
        }
        TrayControlRequest::OpenDesktop => {
            action(ControlAction::OpenDesktop);
            TrayControlResponse {
                ok: true,
                health: None,
                error: None,
            }
        }
        TrayControlRequest::SetLoginItem { enabled } => {
            let result = std::env::current_exe()
                .map_err(TrayError::from)
                .and_then(|executable| {
                    super::set_login_item_enabled(
                        &TrayPaths::for_macos_user()?,
                        &executable,
                        enabled,
                    )
                });
            match result {
                Ok(()) => {
                    action(ControlAction::LoginItemChanged(enabled));
                    let mut updated = health.write().map_err(|_| {
                        TrayError::Runtime("tray health lock is poisoned".to_string())
                    })?;
                    updated.login_item_enabled = enabled;
                    updated.checked_at = unix_epoch_millis();
                    TrayControlResponse {
                        ok: true,
                        health: Some(updated.clone()),
                        error: None,
                    }
                }
                Err(error) => TrayControlResponse {
                    ok: false,
                    health: None,
                    error: Some(error.to_string()),
                },
            }
        }
        TrayControlRequest::Quit => {
            action(ControlAction::Quit);
            TrayControlResponse {
                ok: true,
                health: None,
                error: None,
            }
        }
    };
    let payload =
        serde_json::to_vec(&response).map_err(|error| TrayError::Protocol(error.to_string()))?;
    stream.write_all(&payload).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn read_control_message(stream: &mut UnixStream) -> Result<Vec<u8>, TrayError> {
    let mut payload = Vec::with_capacity(256);
    let mut chunk = [0_u8; 512];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        payload.extend_from_slice(&chunk[..read]);
        if payload.len() > MAX_CONTROL_MESSAGE_BYTES {
            return Err(TrayError::Protocol(
                "tray control message exceeded the size limit".to_string(),
            ));
        }
        if let Some(newline) = payload.iter().position(|byte| *byte == b'\n') {
            payload.truncate(newline);
            break;
        }
    }
    if payload.is_empty() {
        return Err(TrayError::Protocol(
            "tray control message was empty".to_string(),
        ));
    }
    Ok(payload)
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
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::*;

    fn temporary_paths() -> TrayPaths {
        let root = PathBuf::from("/tmp").join(format!("vi-ipc-{}", Uuid::now_v7()));
        TrayPaths::from_directories(root.join("runtime"), root.join("agents"))
    }

    #[test]
    fn second_socket_claim_detects_the_live_owner() {
        let paths = temporary_paths();
        let first = claim_control_socket(&paths).expect("first claim");
        assert!(matches!(first, SocketClaim::Claimed(_)));
        assert!(matches!(
            claim_control_socket(&paths).expect("second claim"),
            SocketClaim::AlreadyRunning
        ));

        fs::remove_file(&paths.socket_path).unwrap();
        fs::remove_dir_all(PathBuf::from(paths.app_support_dir.parent().unwrap())).unwrap();
    }

    #[test]
    fn login_item_control_message_is_boolean_and_bounded() {
        let payload = serde_json::to_string(&TrayControlRequest::SetLoginItem { enabled: true })
            .expect("serialize login item request");
        assert_eq!(payload, r#"{"command":"setLoginItem","enabled":true}"#);
        assert!(payload.len() < MAX_CONTROL_MESSAGE_BYTES);
    }
}
