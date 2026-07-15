use rusqlite::params;
use schemars::JsonSchema;
use serde::Serialize;

use crate::error::AppError;

use super::{database_error, timestamp, InspectorDatabase};

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpAuditEvent {
    pub id: String,
    pub session_id: String,
    pub tool: String,
    pub project_id: Option<String>,
    pub entity_ids: Vec<String>,
    pub result: String,
    pub duration_ms: u64,
    pub created_at: String,
}

impl InspectorDatabase {
    pub fn record_mcp_audit(
        &self,
        session_id: &str,
        tool: &str,
        project_id: Option<&str>,
        entity_ids: &[String],
        result: &str,
        duration_ms: u64,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let duration_ms = i64::try_from(duration_ms)
            .map_err(|_| AppError::Database("MCP duration overflow".to_string()))?;
        connection
            .execute(
                "INSERT INTO mcp_audit_events (
                    id, session_id, tool, project_id, entity_ids_json, result, duration_ms, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    uuid::Uuid::now_v7().to_string(),
                    session_id,
                    tool,
                    project_id,
                    serde_json::to_string(entity_ids).map_err(database_error)?,
                    result,
                    duration_ms,
                    timestamp(),
                ],
            )
            .map_err(database_error)?;
        Ok(())
    }

    pub fn list_mcp_audit(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<McpAuditEvent>, AppError> {
        let connection = self.connect()?;
        let limit = i64::try_from(limit.clamp(1, 200))
            .map_err(|_| AppError::Validation("invalid audit limit".to_string()))?;
        let mut statement = connection
            .prepare(
                "SELECT id, session_id, tool, project_id, entity_ids_json, result, duration_ms,
                        created_at
                 FROM mcp_audit_events WHERE project_id = ?1
                 ORDER BY created_at DESC, id DESC LIMIT ?2",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(params![project_id, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(McpAuditEvent {
                    id: row.0,
                    session_id: row.1,
                    tool: row.2,
                    project_id: row.3,
                    entity_ids: serde_json::from_str(&row.4).map_err(database_error)?,
                    result: row.5,
                    duration_ms: u64::try_from(row.6).map_err(database_error)?,
                    created_at: row.7,
                })
            })
            .collect()
    }
}
