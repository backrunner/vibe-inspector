use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum AppError {
    #[error("The selected path could not be resolved: {0}")]
    InvalidPath(String),
    #[error("The selected path is not a directory: {0}")]
    NotDirectory(String),
    #[error("The selected directory is not a recognized project root: {0}")]
    NotProjectRoot(String),
    #[error("The project discovery task failed: {0}")]
    DiscoveryTask(String),
    #[error("Project registry storage failed: {0}")]
    Storage(String),
    #[error("Inspector database operation failed: {0}")]
    Database(String),
    #[error("Codex history could not be scanned: {0}")]
    CodexHistory(String),
    #[error("The requested entity was not found: {0}")]
    NotFound(String),
    #[error("The inspector payload is invalid: {0}")]
    Validation(String),
    #[error("The MCP session is not authorized for: {0}")]
    Unauthorized(String),
    #[error("The requested path escapes the registered project root: {0}")]
    PathTraversal(String),
    #[error("The source file could not be read as UTF-8 text: {0}")]
    SourceUnreadable(String),
    #[error("The source file is too large: {path} is {bytes} bytes, limit {limit}")]
    SourceTooLarge {
        path: String,
        bytes: u64,
        limit: u64,
    },
    #[error("The requested source range is invalid: lines {start}-{end}")]
    InvalidLineRange { start: usize, end: usize },
    #[error("The requested source range is too large: {bytes} bytes, limit {limit}")]
    SourceRangeTooLarge { bytes: usize, limit: usize },
    #[error("Revision conflict for {entity_id}: expected {expected}, actual {actual}")]
    RevisionConflict {
        entity_id: String,
        expected: u64,
        actual: u64,
    },
}
