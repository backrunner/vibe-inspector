mod sqlite;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{error::AppError, project_discovery::ProjectCandidate};

pub use sqlite::SqliteCodexHistoryProvider;

pub const PARSER_VERSION: &str = "codex-state-sqlite-v1";
pub const MAX_HISTORY_THREADS: usize = 5_000;
pub const MAX_PROJECT_PROPOSALS: usize = 200;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCodexHistoryInput {
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProjectProposal {
    #[serde(flatten)]
    pub project: ProjectCandidate,
    pub conversation_ids: Vec<String>,
    pub latest_activity: i64,
    pub parser_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryScanResult {
    pub provider: String,
    pub source_path: String,
    pub proposals: Vec<CodexProjectProposal>,
    pub scanned_threads: usize,
    pub ignored_threads: usize,
    pub truncated: bool,
}

pub trait CodexHistoryProvider {
    fn scan(&self, input: &ScanCodexHistoryInput) -> Result<CodexHistoryScanResult, AppError>;
}

pub fn default_codex_state_path() -> Result<PathBuf, AppError> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(codex_home).join("state_5.sqlite"));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| AppError::CodexHistory("HOME is not available".to_string()))?;
    Ok(PathBuf::from(home).join(".codex/state_5.sqlite"))
}

pub fn scan_default_codex_history(
    input: ScanCodexHistoryInput,
) -> Result<CodexHistoryScanResult, AppError> {
    let provider = SqliteCodexHistoryProvider::new(default_codex_state_path()?);
    provider.scan(&input)
}
