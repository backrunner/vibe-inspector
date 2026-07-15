mod auth;
mod desktop;
mod diagnostics;
mod http;
mod ipc;
mod launch_agent;
mod paths;

use std::{io, net::SocketAddr};

use thiserror::Error;

pub use auth::load_or_create_bearer_token;
pub use desktop::{ensure_tray_running, open_desktop};
pub use diagnostics::{
    install_local_panic_hook, record_diagnostic, DiagnosticEvent, DiagnosticProcess,
};
pub use http::{bind_mcp_listener, serve_mcp_http};
pub use ipc::{
    claim_control_socket, serve_control_socket, set_tray_login_item, tray_health, ControlAction,
    SocketClaim, TrayHealth,
};
pub use launch_agent::{is_login_item_enabled, set_login_item_enabled};
pub use paths::TrayPaths;

pub const DEFAULT_MCP_BIND: &str = "127.0.0.1:47821";

#[derive(Debug, Error)]
pub enum TrayError {
    #[error("tray filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("tray configuration is invalid: {0}")]
    Config(String),
    #[error("tray control protocol failed: {0}")]
    Protocol(String),
    #[error("tray runtime failed: {0}")]
    Runtime(String),
}

pub fn configured_mcp_bind() -> Result<SocketAddr, TrayError> {
    std::env::var("VIBE_INSPECTOR_MCP_BIND")
        .unwrap_or_else(|_| DEFAULT_MCP_BIND.to_string())
        .parse()
        .map_err(|error| TrayError::Config(format!("invalid MCP bind address: {error}")))
}
