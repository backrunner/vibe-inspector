use std::{fs, path::Path};

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::error::AppError;

use super::{
    database_error, record_mutation, revision_error, revision_from_sql, revision_to_sql, timestamp,
    InspectorDatabase, ProjectStatus, StoredProject,
};

impl InspectorDatabase {
    pub fn refresh_project_availability(
        &self,
        actor: &str,
    ) -> Result<Vec<StoredProject>, AppError> {
        let projects = self.list_projects(false)?;
        for project in &projects {
            let status = match fs::metadata(&project.root_path) {
                Ok(metadata) if metadata.is_dir() => ProjectStatus::Ready,
                Ok(_) => ProjectStatus::Error,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => ProjectStatus::Stale,
                Err(_) => ProjectStatus::Error,
            };
            if status == project.status {
                continue;
            }

            let mut connection = self.connect()?;
            let transaction = connection.transaction().map_err(database_error)?;
            let revision = project
                .revision
                .checked_add(1)
                .ok_or_else(|| AppError::Database("project revision overflow".to_string()))?;
            let updated_at = timestamp();
            let changed = transaction
                .execute(
                    "UPDATE projects SET status = ?1, revision = ?2, updated_at = ?3
                     WHERE id = ?4 AND revision = ?5 AND archived_at IS NULL",
                    params![
                        status.as_str(),
                        revision_to_sql(revision)?,
                        updated_at,
                        project.id,
                        revision_to_sql(project.revision)?,
                    ],
                )
                .map_err(database_error)?;
            if changed == 0 {
                return Err(revision_error(&transaction, &project.id, project.revision)?);
            }
            let request_id = format!(
                "availability:{}:{}:{}",
                project.id,
                project.revision,
                status.as_str()
            );
            record_mutation(
                &transaction,
                Some(&project.id),
                "project",
                &project.id,
                status.as_str(),
                actor,
                "availability-check",
                revision,
                Some(&request_id),
                &updated_at,
            )?;
            transaction.commit().map_err(database_error)?;
        }
        self.list_projects(false)
    }

    pub fn create_project(
        &self,
        name: &str,
        root_path: &str,
        actor: &str,
        source: &str,
        request_id: &str,
    ) -> Result<StoredProject, AppError> {
        let name = name.trim();
        if name.is_empty() || name.len() > 160 {
            return Err(AppError::Validation("invalid project name".to_string()));
        }
        let canonical = fs::canonicalize(root_path)
            .map_err(|_| AppError::InvalidPath(root_path.to_string()))?;
        if !canonical.is_dir() {
            return Err(AppError::NotDirectory(root_path.to_string()));
        }
        let canonical = canonical.to_string_lossy().into_owned();
        let mut connection = self.connect()?;
        if let Some(existing_id) = connection
            .query_row(
                "SELECT id FROM projects WHERE root_path = ?1 AND archived_at IS NULL",
                [&canonical],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(database_error)?
        {
            return self.project(&existing_id);
        }
        // A project archived earlier keeps the root_path uniqueness claim;
        // revive it instead of failing on the constraint so callers get
        // ensure semantics for archived roots as well.
        if let Some((archived_id, archived_revision)) = connection
            .query_row(
                "SELECT id, revision FROM projects WHERE root_path = ?1 AND archived_at IS NOT NULL",
                [&canonical],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(database_error)?
        {
            let next_revision = revision_from_sql(archived_revision)?
                .checked_add(1)
                .ok_or_else(|| AppError::Database("project revision overflow".to_string()))?;
            let now = timestamp();
            let transaction = connection.transaction().map_err(database_error)?;
            transaction
                .execute(
                    "UPDATE projects SET name = ?1, status = ?2, revision = ?3, updated_at = ?4,
                        archived_at = NULL WHERE id = ?5",
                    params![
                        name,
                        ProjectStatus::Ready.as_str(),
                        revision_to_sql(next_revision)?,
                        now,
                        archived_id
                    ],
                )
                .map_err(database_error)?;
            record_mutation(
                &transaction,
                Some(&archived_id),
                "project",
                &archived_id,
                "revive",
                actor,
                source,
                next_revision,
                Some(request_id),
                &now,
            )?;
            transaction.commit().map_err(database_error)?;
            return self.project(&archived_id);
        }
        let transaction = connection.transaction().map_err(database_error)?;
        let id = Uuid::now_v7().to_string();
        let now = timestamp();
        transaction
            .execute(
                "INSERT INTO projects (
                    id, name, root_path, status, source, revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'ready', ?4, 1, ?5, ?5)",
                params![id, name, canonical, source, now],
            )
            .map_err(database_error)?;
        transaction
            .execute(
                "INSERT INTO project_sources (
                    id, project_id, kind, observed_path, observed_at, parser_version
                 ) VALUES (?1, ?2, 'manual', ?3, ?4, 'mcp-v1')",
                params![Uuid::now_v7().to_string(), id, canonical, now],
            )
            .map_err(database_error)?;
        record_mutation(
            &transaction,
            Some(&id),
            "project",
            &id,
            "create",
            actor,
            source,
            1,
            Some(request_id),
            &now,
        )?;
        transaction.commit().map_err(database_error)?;
        self.project(&id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_project(
        &self,
        project_id: &str,
        expected_revision: u64,
        name: Option<&str>,
        status: Option<ProjectStatus>,
        actor: &str,
        source: &str,
        request_id: &str,
    ) -> Result<StoredProject, AppError> {
        let current = self.project(project_id)?;
        let next_name = name.unwrap_or(&current.name).trim();
        if next_name.is_empty() || next_name.len() > 160 {
            return Err(AppError::Validation("invalid project name".to_string()));
        }
        let next_status = status.unwrap_or(current.status);
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| AppError::Database("project revision overflow".to_string()))?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(database_error)?;
        let updated_at = timestamp();
        let archived_at = (next_status == ProjectStatus::Archived).then_some(updated_at.as_str());
        let changed = transaction
            .execute(
                "UPDATE projects SET name = ?1, status = ?2, revision = ?3, updated_at = ?4,
                    archived_at = ?5 WHERE id = ?6 AND revision = ?7",
                params![
                    next_name,
                    next_status.as_str(),
                    revision_to_sql(next_revision)?,
                    updated_at,
                    archived_at,
                    project_id,
                    revision_to_sql(expected_revision)?,
                ],
            )
            .map_err(database_error)?;
        if changed == 0 {
            return Err(revision_error(&transaction, project_id, expected_revision)?);
        }
        record_mutation(
            &transaction,
            Some(project_id),
            "project",
            project_id,
            if next_status == ProjectStatus::Archived {
                "archive"
            } else {
                "update"
            },
            actor,
            source,
            next_revision,
            Some(request_id),
            &updated_at,
        )?;
        transaction.commit().map_err(database_error)?;
        self.project(project_id)
    }

    pub fn project_root_exists(&self, root: &Path) -> Result<bool, AppError> {
        let canonical = fs::canonicalize(root).map_err(database_error)?;
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE root_path = ?1 AND archived_at IS NULL)",
                [canonical.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .map_err(database_error)
    }
}
