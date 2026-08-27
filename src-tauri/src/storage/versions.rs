use rusqlite::{params, OptionalExtension, ToSql, Transaction};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;

use super::{database_error, revision_from_sql, revision_to_sql, InspectorDatabase};

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityVersionSummary {
    pub id: String,
    pub project_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub revision: u64,
    pub actor: String,
    pub source: String,
    pub request_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityVersion {
    pub id: String,
    pub project_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub revision: u64,
    pub snapshot_json: serde_json::Value,
    pub actor: String,
    pub source: String,
    pub request_id: Option<String>,
    pub created_at: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_entity_version(
    transaction: &Transaction<'_>,
    project_id: &str,
    entity_type: &str,
    entity_id: &str,
    revision: u64,
    snapshot_json: &str,
    actor: &str,
    source: &str,
    request_id: Option<&str>,
    created_at: &str,
) -> Result<(), AppError> {
    transaction
        .execute(
            "INSERT INTO entity_versions (
                id, project_id, entity_type, entity_id, revision, snapshot_json,
                actor, source, request_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                Uuid::now_v7().to_string(),
                project_id,
                entity_type,
                entity_id,
                revision_to_sql(revision)?,
                snapshot_json,
                actor,
                source,
                request_id,
                created_at,
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

impl InspectorDatabase {
    pub fn list_entity_versions(
        &self,
        project_id: &str,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EntityVersionSummary>, AppError> {
        let connection = self.connect()?;
        let limit = i64::try_from(limit.clamp(1, 200))
            .map_err(|_| AppError::Validation("invalid version list limit".to_string()))?;
        let mut sql = String::from(
            "SELECT id, project_id, entity_type, entity_id, revision, actor, source, request_id, created_at
             FROM entity_versions WHERE project_id = ?1",
        );
        let mut bindings: Vec<&dyn ToSql> = vec![&project_id];
        let mut next_parameter = 2;
        if let Some(entity_type) = entity_type.as_ref() {
            sql.push_str(&format!(" AND entity_type = ?{next_parameter}"));
            next_parameter += 1;
            bindings.push(entity_type);
        }
        if let Some(entity_id) = entity_id.as_ref() {
            sql.push_str(&format!(" AND entity_id = ?{next_parameter}"));
            next_parameter += 1;
            bindings.push(entity_id);
        }
        sql.push_str(&format!(
            " ORDER BY created_at DESC, id DESC LIMIT ?{next_parameter}"
        ));
        bindings.push(&limit);
        let mut statement = connection.prepare(&sql).map_err(database_error)?;
        let rows = statement
            .query_map(bindings.as_slice(), map_version)
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        rows.into_iter().map(parse_summary).collect()
    }

    pub fn get_entity_version(&self, version_id: &str) -> Result<EntityVersion, AppError> {
        let connection = self.connect()?;
        let row = connection
            .query_row(
                "SELECT id, project_id, entity_type, entity_id, revision, snapshot_json,
                        actor, source, request_id, created_at
                 FROM entity_versions WHERE id = ?1",
                [version_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, String>(9)?,
                    ))
                },
            )
            .optional()
            .map_err(database_error)?
            .ok_or_else(|| AppError::NotFound(format!("entity version {version_id}")))?;
        let summary = parse_summary((
            row.0, row.1, row.2, row.3, row.4, row.6, row.7, row.8, row.9,
        ))?;
        Ok(EntityVersion {
            id: summary.id,
            project_id: summary.project_id,
            entity_type: summary.entity_type,
            entity_id: summary.entity_id,
            revision: summary.revision,
            snapshot_json: serde_json::from_str(&row.5).map_err(database_error)?,
            actor: summary.actor,
            source: summary.source,
            request_id: summary.request_id,
            created_at: summary.created_at,
        })
    }
}

type VersionRow = (
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
    String,
);

fn map_version(row: &rusqlite::Row<'_>) -> rusqlite::Result<VersionRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}

fn parse_summary(row: VersionRow) -> Result<EntityVersionSummary, AppError> {
    Ok(EntityVersionSummary {
        id: row.0,
        project_id: row.1,
        entity_type: row.2,
        entity_id: row.3,
        revision: revision_from_sql(row.4)?,
        actor: row.5,
        source: row.6,
        request_id: row.7,
        created_at: row.8,
    })
}
