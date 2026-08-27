use rusqlite::{params, OptionalExtension};
use schemars::JsonSchema;
use serde::Serialize;

use crate::error::AppError;

use super::{database_error, timestamp, InspectorDatabase};

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoredCommandRun {
    pub request_id: String,
    pub project_id: String,
    pub command: String,
    pub scope_fingerprint: Option<String>,
    pub actor: String,
    pub status: String,
    pub result_json: Option<String>,
    pub coverage_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl InspectorDatabase {
    pub fn command_run(&self, request_id: &str) -> Result<Option<StoredCommandRun>, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT request_id, project_id, command, scope_fingerprint, actor, status,
                        result_json, coverage_json, created_at, updated_at
                 FROM command_runs WHERE request_id = ?1",
                [request_id],
                map_command_run,
            )
            .optional()
            .map_err(database_error)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_command_run(
        &self,
        request_id: &str,
        project_id: &str,
        command: &str,
        scope_fingerprint: &str,
        actor: &str,
        status: &str,
        result_json: &str,
        coverage_json: Option<&str>,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let now = timestamp();
        connection
            .execute(
                "INSERT INTO command_runs (
                    id, request_id, project_id, command, scope_fingerprint, actor, status,
                    result_json, coverage_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                 ON CONFLICT(request_id) DO UPDATE SET status = excluded.status,
                    result_json = excluded.result_json, coverage_json = excluded.coverage_json,
                    updated_at = excluded.updated_at",
                params![
                    uuid::Uuid::now_v7().to_string(),
                    request_id,
                    project_id,
                    command,
                    scope_fingerprint,
                    actor,
                    status,
                    result_json,
                    coverage_json,
                    now,
                ],
            )
            .map_err(database_error)?;
        Ok(())
    }

    pub fn list_command_runs(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<StoredCommandRun>, AppError> {
        let connection = self.connect()?;
        let limit = i64::try_from(limit.clamp(1, 200))
            .map_err(|_| AppError::Validation("invalid command run limit".to_string()))?;
        let mut statement = connection
            .prepare(
                "SELECT request_id, project_id, command, scope_fingerprint, actor, status,
                        result_json, coverage_json, created_at, updated_at
                 FROM command_runs WHERE project_id = ?1
                 ORDER BY updated_at DESC, id DESC LIMIT ?2",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(params![project_id, limit], map_command_run)
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        Ok(rows)
    }
}

fn map_command_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredCommandRun> {
    Ok(StoredCommandRun {
        request_id: row.get(0)?,
        project_id: row.get(1)?,
        command: row.get(2)?,
        scope_fingerprint: row.get(3)?,
        actor: row.get(4)?,
        status: row.get(5)?,
        result_json: row.get(6)?,
        coverage_json: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}
