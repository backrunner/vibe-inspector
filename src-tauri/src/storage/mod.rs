pub mod commands;
pub mod inspector;
pub mod mcp;
mod migrations;
pub mod projects;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{params, Connection, OptionalExtension, Transaction, MAIN_DB};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{error::AppError, project_registry::RegisteredProject};

use migrations::{MIGRATION_1, MIGRATION_2, SCHEMA_VERSION};

const DATABASE_FILE: &str = "inspector.sqlite3";
const BACKUP_DIRECTORY: &str = "backups";
const MIGRATION_BACKUP_PREFIX: &str = "inspector-migration";
const MAX_MIGRATION_BACKUPS: usize = 5;

#[derive(Debug, Clone)]
pub struct InspectorDatabase {
    path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoredProject {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub status: ProjectStatus,
    pub source: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectStatus {
    Ready,
    Stale,
    Error,
    Archived,
}

impl ProjectStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Stale => "stale",
            Self::Error => "error",
            Self::Archived => "archived",
        }
    }

    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "ready" => Ok(Self::Ready),
            "stale" => Ok(Self::Stale),
            "error" => Ok(Self::Error),
            "archived" => Ok(Self::Archived),
            other => Err(AppError::Database(format!(
                "unknown project status {other}"
            ))),
        }
    }
}

impl InspectorDatabase {
    pub fn for_app(app: &AppHandle) -> Result<Self, AppError> {
        let directory = app.path().app_data_dir().map_err(database_error)?;
        fs::create_dir_all(&directory).map_err(database_error)?;
        Ok(Self::new(directory.join(DATABASE_FILE)))
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_macos_user() -> Result<Self, AppError> {
        if let Some(path) = std::env::var_os("VIBE_INSPECTOR_DATABASE_PATH") {
            return Ok(Self::new(PathBuf::from(path)));
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| AppError::Storage("HOME is unavailable".to_string()))?;
        let directory = home
            .join("Library")
            .join("Application Support")
            .join("com.vibeinspector.desktop");
        fs::create_dir_all(&directory).map_err(database_error)?;
        Ok(Self::new(directory.join(DATABASE_FILE)))
    }

    pub fn initialize(&self) -> Result<(), AppError> {
        let had_existing_database = self
            .path
            .metadata()
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false);
        self.secure_database_file_if_present()?;
        let mut connection = self.connect()?;
        self.secure_database_file_if_present()?;
        let current = schema_version(&connection)?;
        if had_existing_database && current < SCHEMA_VERSION {
            self.create_migration_backup(&connection, current)?;
        }
        migrate(&mut connection)
    }

    pub fn migration_backups(&self) -> Result<Vec<PathBuf>, AppError> {
        let directory = self.backup_directory()?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut backups = fs::read_dir(directory)
            .map_err(database_error)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with(MIGRATION_BACKUP_PREFIX) && name.ends_with(".sqlite3")
                    })
            })
            .collect::<Vec<_>>();
        backups.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
        Ok(backups)
    }

    pub fn restore_migration_backup(&self, backup_path: &Path) -> Result<(), AppError> {
        let backup_directory = self.backup_directory()?;
        let canonical_directory = backup_directory.canonicalize().map_err(database_error)?;
        let canonical_backup = backup_path.canonicalize().map_err(database_error)?;
        if !canonical_backup.starts_with(&canonical_directory) {
            return Err(AppError::InvalidPath(
                "backup must be inside the VibeInspector backup directory".to_string(),
            ));
        }

        let backup = Connection::open_with_flags(
            &canonical_backup,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(database_error)?;
        let integrity: String = backup
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(database_error)?;
        if integrity != "ok" {
            return Err(AppError::Database(format!(
                "backup integrity check failed: {integrity}"
            )));
        }
        drop(backup);

        let mut destination = self.connect()?;
        destination
            .restore(
                MAIN_DB,
                &canonical_backup,
                None::<fn(rusqlite::backup::Progress)>,
            )
            .map_err(database_error)?;
        Ok(())
    }

    pub fn import_registered_projects(
        &self,
        projects: &[RegisteredProject],
    ) -> Result<(), AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(database_error)?;

        for project in projects {
            let existing = transaction
                .query_row(
                    "SELECT id FROM projects WHERE root_path = ?1",
                    [&project.root_path],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(database_error)?;
            let persisted_id = if let Some(existing_id) = existing {
                existing_id
            } else {
                let id = project.id.to_string();
                let revision = revision_to_sql(project.revision)?;
                transaction
                    .execute(
                        "INSERT INTO projects (
                            id, name, root_path, status, source, revision, created_at, updated_at
                        ) VALUES (?1, ?2, ?3, 'ready', ?4, ?5, ?6, ?6)",
                        params![
                            id,
                            project.name,
                            project.root_path,
                            project.source.as_str(),
                            revision,
                            project.updated_at,
                        ],
                    )
                    .map_err(database_error)?;

                record_mutation(
                    &transaction,
                    Some(&id),
                    "project",
                    &id,
                    "import",
                    "desktop",
                    project.source.as_str(),
                    project.revision,
                    None,
                    &project.updated_at,
                )?;
                id
            };
            sync_project_sources(&transaction, &persisted_id, project)?;
        }

        transaction.commit().map_err(database_error)
    }

    pub fn list_projects(&self, include_archived: bool) -> Result<Vec<StoredProject>, AppError> {
        let connection = self.connect()?;
        let sql = if include_archived {
            "SELECT id, name, root_path, status, source, revision, created_at, updated_at, archived_at
             FROM projects ORDER BY name, root_path"
        } else {
            "SELECT id, name, root_path, status, source, revision, created_at, updated_at, archived_at
             FROM projects WHERE archived_at IS NULL ORDER BY name, root_path"
        };
        let mut statement = connection.prepare(sql).map_err(database_error)?;
        let rows = statement
            .query_map([], map_project)
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        rows.into_iter().map(parse_project).collect()
    }

    pub fn project(&self, project_id: &str) -> Result<StoredProject, AppError> {
        let connection = self.connect()?;
        let project = connection
            .query_row(
                "SELECT id, name, root_path, status, source, revision, created_at, updated_at, archived_at
                 FROM projects WHERE id = ?1",
                [project_id],
                map_project,
            )
            .optional()
            .map_err(database_error)?
            .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;
        parse_project(project)
    }

    pub fn set_project_status(
        &self,
        project_id: &str,
        expected_revision: u64,
        status: ProjectStatus,
        actor: &str,
        request_id: Option<&str>,
    ) -> Result<StoredProject, AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(database_error)?;
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| AppError::Database("project revision overflow".to_string()))?;
        let expected_revision_sql = revision_to_sql(expected_revision)?;
        let next_revision_sql = revision_to_sql(next_revision)?;
        let updated_at = timestamp();
        let archived_at = (status == ProjectStatus::Archived).then_some(updated_at.as_str());
        let changed = transaction
            .execute(
                "UPDATE projects SET status = ?1, revision = ?2, updated_at = ?3, archived_at = ?4
                 WHERE id = ?5 AND revision = ?6",
                params![
                    status.as_str(),
                    next_revision_sql,
                    updated_at,
                    archived_at,
                    project_id,
                    expected_revision_sql,
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
            status.as_str(),
            actor,
            "manual",
            next_revision,
            request_id,
            &updated_at,
        )?;
        transaction.commit().map_err(database_error)?;
        self.project(project_id)
    }

    pub fn mutation_count(&self, project_id: &str) -> Result<u64, AppError> {
        let connection = self.connect()?;
        let count = connection
            .query_row(
                "SELECT COUNT(*) FROM mutation_events WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(database_error)?;
        revision_from_sql(count)
    }

    fn connect(&self) -> Result<Connection, AppError> {
        let connection = Connection::open(&self.path).map_err(database_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(database_error)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(database_error)?;
        Ok(connection)
    }

    fn backup_directory(&self) -> Result<PathBuf, AppError> {
        self.path
            .parent()
            .map(|parent| parent.join(BACKUP_DIRECTORY))
            .ok_or_else(|| AppError::Database("database parent directory is unavailable".into()))
    }

    fn secure_database_file_if_present(&self) -> Result<(), AppError> {
        if self.path.exists() {
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
                .map_err(database_error)?;
        }
        Ok(())
    }

    fn create_migration_backup(
        &self,
        connection: &Connection,
        current_version: u32,
    ) -> Result<PathBuf, AppError> {
        let directory = self.backup_directory()?;
        fs::create_dir_all(&directory).map_err(database_error)?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(database_error)?;
        let backup_path = directory.join(format!(
            "{MIGRATION_BACKUP_PREFIX}-v{current_version}-before-v{SCHEMA_VERSION}-{}.sqlite3",
            timestamp()
        ));
        connection
            .backup(MAIN_DB, &backup_path, None)
            .map_err(database_error)?;
        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o600))
            .map_err(database_error)?;
        self.prune_migration_backups()?;
        Ok(backup_path)
    }

    fn prune_migration_backups(&self) -> Result<(), AppError> {
        for backup in self
            .migration_backups()?
            .into_iter()
            .skip(MAX_MIGRATION_BACKUPS)
        {
            fs::remove_file(backup).map_err(database_error)?;
        }
        Ok(())
    }
}

fn sync_project_sources(
    transaction: &Transaction<'_>,
    project_id: &str,
    project: &RegisteredProject,
) -> Result<(), AppError> {
    for evidence in &project.source_evidence {
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO project_sources (
                    id, project_id, kind, conversation_id, observed_path, observed_at, parser_version
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    evidence.id.to_string(),
                    project_id,
                    evidence.kind.as_str(),
                    evidence.conversation_id,
                    evidence.observed_path,
                    evidence.observed_at,
                    evidence.parser_version,
                ],
            )
            .map_err(database_error)?;
        if inserted > 0 {
            record_mutation(
                transaction,
                Some(project_id),
                "project_source",
                &evidence.id.to_string(),
                "upsert",
                "desktop",
                evidence.kind.as_str(),
                1,
                None,
                &evidence.observed_at,
            )?;
        }
    }
    Ok(())
}

type ProjectRow = (
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
);

fn map_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
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

fn parse_project(row: ProjectRow) -> Result<StoredProject, AppError> {
    Ok(StoredProject {
        id: row.0,
        name: row.1,
        root_path: row.2,
        status: ProjectStatus::parse(&row.3)?,
        source: row.4,
        revision: revision_from_sql(row.5)?,
        created_at: row.6,
        updated_at: row.7,
        archived_at: row.8,
    })
}

fn migrate(connection: &mut Connection) -> Result<(), AppError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )
        .map_err(database_error)?;
    let current = schema_version(connection)?;
    if current > SCHEMA_VERSION {
        return Err(AppError::Database(format!(
            "database schema {current} is newer than supported schema {SCHEMA_VERSION}"
        )));
    }
    if current < 1 {
        apply_migration(connection, 1, MIGRATION_1)?;
    }
    if current < 2 {
        apply_migration(connection, 2, MIGRATION_2)?;
    }
    Ok(())
}

fn schema_version(connection: &Connection) -> Result<u32, AppError> {
    let has_migration_table = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'schema_migrations'
            )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(database_error)?;
    if !has_migration_table {
        return Ok(0);
    }
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, u32>(0),
        )
        .map_err(database_error)
}

fn apply_migration(connection: &mut Connection, version: u32, sql: &str) -> Result<(), AppError> {
    let transaction = connection.transaction().map_err(database_error)?;
    transaction.execute_batch(sql).map_err(database_error)?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            params![version, timestamp()],
        )
        .map_err(database_error)?;
    transaction.commit().map_err(database_error)
}

#[allow(clippy::too_many_arguments)]
fn record_mutation(
    transaction: &Transaction<'_>,
    project_id: Option<&str>,
    entity_type: &str,
    entity_id: &str,
    action: &str,
    actor: &str,
    source: &str,
    revision: u64,
    request_id: Option<&str>,
    created_at: &str,
) -> Result<(), AppError> {
    let revision = revision_to_sql(revision)?;
    transaction
        .execute(
            "INSERT INTO mutation_events (
                id, project_id, entity_type, entity_id, action, actor, source, revision, request_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                Uuid::now_v7().to_string(),
                project_id,
                entity_type,
                entity_id,
                action,
                actor,
                source,
                revision,
                request_id,
                created_at,
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

fn revision_error(
    transaction: &Transaction<'_>,
    project_id: &str,
    expected_revision: u64,
) -> Result<AppError, AppError> {
    let actual = transaction
        .query_row(
            "SELECT revision FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(database_error)?;
    Ok(match actual {
        Some(actual_revision) => AppError::RevisionConflict {
            entity_id: project_id.to_string(),
            expected: expected_revision,
            actual: revision_from_sql(actual_revision)?,
        },
        None => AppError::NotFound(format!("project {project_id}")),
    })
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn database_error(error: impl std::fmt::Display) -> AppError {
    AppError::Database(error.to_string())
}

fn revision_to_sql(revision: u64) -> Result<i64, AppError> {
    i64::try_from(revision)
        .map_err(|_| AppError::Database(format!("revision {revision} exceeds SQLite INTEGER")))
}

fn revision_from_sql(revision: i64) -> Result<u64, AppError> {
    u64::try_from(revision)
        .map_err(|_| AppError::Database(format!("invalid negative revision {revision}")))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::MetadataExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        inspector::{
            EntityStatus, FlowNodeKind, InspectorExplanation, InspectorFlow, InspectorFlowEdge,
            InspectorFlowNode, InspectorGraph, InspectorModule, InspectorSnapshot,
            InspectorSourceRef, Provenance, ProvenanceSource, SourceRefStatus,
        },
        project_discovery::{ProjectKind, ProjectNameSource},
    };

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "vibe-inspector-storage-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn registered_project(root: &Path) -> RegisteredProject {
        RegisteredProject {
            id: Uuid::now_v7(),
            name: "Fixture".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            kind: ProjectKind::Repository,
            name_source: ProjectNameSource::Directory,
            markers: vec!["git".to_string()],
            source: crate::project_registry::RegistrationSource::Manual,
            source_evidence: vec![crate::project_registry::RegisteredSourceEvidence {
                id: Uuid::now_v7(),
                kind: crate::project_registry::RegistrationSource::Manual,
                conversation_id: None,
                observed_path: root.to_string_lossy().into_owned(),
                observed_at: timestamp(),
                parser_version: "manual-v1".to_string(),
            }],
            revision: 1,
            updated_at: timestamp(),
        }
    }

    fn inspector_snapshot(project_id: &str) -> InspectorSnapshot {
        let parent_id = Uuid::now_v7().to_string();
        let child_id = Uuid::now_v7().to_string();
        let flow_id = Uuid::now_v7().to_string();
        let source_id = Uuid::now_v7().to_string();
        let start_id = Uuid::now_v7().to_string();
        let end_id = Uuid::now_v7().to_string();
        let now = timestamp();
        let flow = InspectorFlow {
            id: flow_id.clone(),
            project_id: project_id.to_string(),
            module_id: child_id.clone(),
            name: "Refresh inspector".to_string(),
            summary: "Reconciles the current implementation with inspector data.".to_string(),
            status: EntityStatus::Ready,
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
            graph: InspectorGraph {
                nodes: vec![
                    InspectorFlowNode {
                        id: start_id.clone(),
                        label: "Read snapshot".to_string(),
                        kind: FlowNodeKind::Start,
                        description: Some("Load the current module state.".to_string()),
                        source_ref_ids: vec![source_id.clone()],
                        position: 0,
                    },
                    InspectorFlowNode {
                        id: end_id.clone(),
                        label: "Commit revision".to_string(),
                        kind: FlowNodeKind::End,
                        description: None,
                        source_ref_ids: vec![source_id.clone()],
                        position: 1,
                    },
                ],
                edges: vec![InspectorFlowEdge {
                    id: Uuid::now_v7().to_string(),
                    from: start_id,
                    to: end_id,
                    label: Some("validated".to_string()),
                }],
            },
            explanation: Some(InspectorExplanation {
                id: Uuid::now_v7().to_string(),
                project_id: project_id.to_string(),
                flow_id: flow_id.clone(),
                headline: "Preserve stable inspector identities.".to_string(),
                body: "A revision gate keeps concurrent updates from silently replacing evidence."
                    .to_string(),
                decisions: vec!["Archive removed entities instead of deleting them.".to_string()],
                confidence: Some(0.91),
                provenance: Provenance {
                    source: ProvenanceSource::InspectorInit,
                    conversation_id: Some("thread-fixture".to_string()),
                    model: Some("fixture-model".to_string()),
                    prompt_hash: Some("prompt-hash".to_string()),
                    generated_at: now.clone(),
                    confidence: Some(0.91),
                    evidence_ids: vec![source_id.clone()],
                },
                revision: 1,
                created_at: now.clone(),
                updated_at: now.clone(),
            }),
            source_refs: vec![InspectorSourceRef {
                id: source_id,
                project_id: project_id.to_string(),
                flow_id,
                path: "src/lib/inspector.ts".to_string(),
                start_line: 3,
                end_line: 18,
                start_column: Some(1),
                end_column: None,
                symbol: Some("refreshInspector".to_string()),
                label: Some("Reconciliation boundary".to_string()),
                content_hash: "fixture-hash".to_string(),
                status: SourceRefStatus::Current,
                revision: 1,
                created_at: now.clone(),
                updated_at: now.clone(),
            }],
        };

        InspectorSnapshot {
            project_id: project_id.to_string(),
            project_revision: 1,
            modules: vec![
                InspectorModule {
                    id: child_id,
                    project_id: project_id.to_string(),
                    parent_id: Some(parent_id.clone()),
                    name: "Inspector UI".to_string(),
                    slug: "inspector-ui".to_string(),
                    summary: "Presents flows and source evidence.".to_string(),
                    status: EntityStatus::Ready,
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    flows: vec![flow],
                },
                InspectorModule {
                    id: parent_id,
                    project_id: project_id.to_string(),
                    parent_id: None,
                    name: "Desktop".to_string(),
                    slug: "desktop".to_string(),
                    summary: "Desktop application modules.".to_string(),
                    status: EntityStatus::Ready,
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now,
                    flows: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn migrations_are_idempotent() {
        let directory = TestDirectory::new("migrations");
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("first migration");
        database.initialize().expect("second migration");

        let connection = database.connect().expect("connect");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        assert_eq!(count, i64::from(SCHEMA_VERSION));
        assert_eq!(fs::metadata(&database.path).unwrap().mode() & 0o777, 0o600);
    }

    #[test]
    fn migration_two_upgrades_an_existing_v1_database() {
        let directory = TestDirectory::new("migration-v2");
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        let mut connection = database.connect().expect("connect");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                );",
            )
            .expect("create migration table");
        apply_migration(&mut connection, 1, MIGRATION_1).expect("apply v1");
        drop(connection);

        database.initialize().expect("upgrade to v2");
        database.initialize().expect("repeat v2 initialization");

        let connection = database.connect().expect("connect after upgrade");
        let column_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('flow_steps')
                 WHERE name = 'source_ref_ids_json'",
                [],
                |row| row.get(0),
            )
            .expect("flow step column");
        let request_table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'mutation_requests'",
                [],
                |row| row.get(0),
            )
            .expect("mutation requests table");
        assert_eq!(column_count, 1);
        assert_eq!(request_table_count, 1);
        drop(connection);

        let backups = database.migration_backups().expect("migration backups");
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::metadata(&backups[0]).unwrap().mode() & 0o777, 0o600);
        let backup = Connection::open(&backups[0]).expect("open migration backup");
        assert_eq!(schema_version(&backup).expect("backup schema"), 1);
        drop(backup);

        database
            .restore_migration_backup(&backups[0])
            .expect("restore v1 backup");
        let restored = database.connect().expect("connect restored database");
        assert_eq!(schema_version(&restored).expect("restored schema"), 1);
        drop(restored);
        database.initialize().expect("migrate restored backup");
    }

    #[test]
    fn recovery_rejects_outside_and_corrupt_backups() {
        let directory = TestDirectory::new("recovery-validation");
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize database");
        let backup_directory = database.backup_directory().expect("backup directory");
        fs::create_dir_all(&backup_directory).expect("create backup directory");

        let outside = directory.path().join("outside.sqlite3");
        Connection::open(&outside).expect("create outside sqlite database");
        assert!(matches!(
            database.restore_migration_backup(&outside),
            Err(AppError::InvalidPath(_))
        ));

        let corrupt = backup_directory.join("inspector-migration-corrupt.sqlite3");
        fs::write(&corrupt, b"not a sqlite database").expect("write corrupt backup");
        assert!(matches!(
            database.restore_migration_backup(&corrupt),
            Err(AppError::Database(_))
        ));
    }

    #[test]
    fn inspector_snapshot_round_trips_and_replays_idempotently() {
        let directory = TestDirectory::new("inspector-round-trip");
        let project_root = directory.path().join("project");
        fs::create_dir(&project_root).expect("create project root");
        let project = registered_project(&project_root);
        let project_id = project.id.to_string();
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize");
        database
            .import_registered_projects(&[project])
            .expect("import project");
        let snapshot = inspector_snapshot(&project_id);

        let committed = database
            .commit_inspector_snapshot(&snapshot, 1, "test", "inspector-init", "request-1")
            .expect("commit snapshot");
        assert_eq!(committed.project_revision, 2);
        let loaded = database
            .load_inspector_snapshot(&project_id)
            .expect("load snapshot");
        assert_eq!(loaded, committed);

        let replayed = database
            .commit_inspector_snapshot(&snapshot, 1, "test", "inspector-init", "request-1")
            .expect("replay snapshot");
        assert_eq!(replayed, committed);
        assert_eq!(database.project(&project_id).expect("project").revision, 2);
    }

    #[test]
    fn inspector_snapshot_conflicts_and_archives_removed_entities() {
        let directory = TestDirectory::new("inspector-archive");
        let project_root = directory.path().join("project");
        fs::create_dir(&project_root).expect("create project root");
        let project = registered_project(&project_root);
        let project_id = project.id.to_string();
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize");
        database
            .import_registered_projects(&[project])
            .expect("import project");
        let snapshot = inspector_snapshot(&project_id);
        database
            .commit_inspector_snapshot(&snapshot, 1, "test", "inspector-init", "request-1")
            .expect("initial commit");

        let conflict = database
            .commit_inspector_snapshot(&snapshot, 1, "test", "inspector-sync", "request-2")
            .expect_err("stale revision must conflict");
        assert!(matches!(conflict, AppError::RevisionConflict { .. }));

        let removed_module_id = snapshot.modules[0].id.clone();
        let removed_flow_id = snapshot.modules[0].flows[0].id.clone();
        let mut reduced = snapshot;
        reduced.project_revision = 2;
        reduced.modules.remove(0);
        database
            .commit_inspector_snapshot(&reduced, 2, "test", "inspector-sync", "request-3")
            .expect("archive removed entities");
        let loaded = database
            .load_inspector_snapshot(&project_id)
            .expect("load reduced snapshot");
        assert_eq!(loaded.modules.len(), 1);

        let connection = database.connect().expect("connect");
        let module_archived: Option<String> = connection
            .query_row(
                "SELECT archived_at FROM modules WHERE id = ?1",
                [&removed_module_id],
                |row| row.get(0),
            )
            .expect("archived module");
        let flow_archived: Option<String> = connection
            .query_row(
                "SELECT archived_at FROM flows WHERE id = ?1",
                [&removed_flow_id],
                |row| row.get(0),
            )
            .expect("archived flow");
        assert!(module_archived.is_some());
        assert!(flow_archived.is_some());
    }

    #[test]
    fn inspector_snapshot_rejects_cyclic_module_parents() {
        let directory = TestDirectory::new("inspector-cycle");
        let project_root = directory.path().join("project");
        fs::create_dir(&project_root).expect("create project root");
        let project = registered_project(&project_root);
        let project_id = project.id.to_string();
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize");
        database
            .import_registered_projects(&[project])
            .expect("import project");
        let mut snapshot = inspector_snapshot(&project_id);
        snapshot.modules[1].parent_id = Some(snapshot.modules[0].id.clone());

        let error = database
            .commit_inspector_snapshot(&snapshot, 1, "test", "inspector-init", "request-cycle")
            .expect_err("cyclic parents must fail");
        assert!(matches!(error, AppError::Validation(_)));
        assert!(database
            .load_inspector_snapshot(&project_id)
            .expect("empty snapshot")
            .modules
            .is_empty());
    }

    #[test]
    fn project_lifecycle_is_revisioned_and_audited() {
        let directory = TestDirectory::new("lifecycle");
        let project_root = directory.path().join("project");
        fs::create_dir(&project_root).expect("create project root");
        let project = registered_project(&project_root);
        let project_id = project.id.to_string();
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize");
        database
            .import_registered_projects(std::slice::from_ref(&project))
            .expect("import project");
        database
            .import_registered_projects(&[project])
            .expect("repeat import");

        let loaded = database.project(&project_id).expect("load project");
        assert_eq!(loaded.status, ProjectStatus::Ready);
        assert_eq!(loaded.revision, 1);
        assert_eq!(
            database.mutation_count(&project_id).expect("audit count"),
            2
        );

        let stale = database
            .set_project_status(
                &project_id,
                1,
                ProjectStatus::Stale,
                "test",
                Some("stale-1"),
            )
            .expect("mark stale");
        assert_eq!(stale.status, ProjectStatus::Stale);
        assert_eq!(stale.revision, 2);

        let archived = database
            .set_project_status(
                &project_id,
                2,
                ProjectStatus::Archived,
                "test",
                Some("archive-1"),
            )
            .expect("archive");
        assert!(archived.archived_at.is_some());
        assert!(database
            .list_projects(false)
            .expect("active projects")
            .is_empty());
        assert_eq!(database.list_projects(true).expect("all projects").len(), 1);
        assert_eq!(
            database.mutation_count(&project_id).expect("audit count"),
            4
        );
    }

    #[test]
    fn project_availability_tracks_missing_and_restored_roots() {
        let directory = TestDirectory::new("project-availability");
        let project_root = directory.path().join("project");
        fs::create_dir(&project_root).expect("create project root");
        let project = registered_project(&project_root);
        let project_id = project.id.to_string();
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize");
        database
            .import_registered_projects(&[project])
            .expect("import project");

        fs::remove_dir(&project_root).expect("remove project root");
        let stale = database
            .refresh_project_availability("desktop")
            .expect("refresh missing root");
        assert_eq!(stale[0].status, ProjectStatus::Stale);
        assert_eq!(stale[0].revision, 2);

        fs::create_dir(&project_root).expect("restore project root");
        let ready = database
            .refresh_project_availability("desktop")
            .expect("refresh restored root");
        assert_eq!(ready[0].status, ProjectStatus::Ready);
        assert_eq!(ready[0].revision, 3);
        assert_eq!(database.mutation_count(&project_id).unwrap(), 4);
    }

    #[test]
    fn revision_conflict_rolls_back_mutation() {
        let directory = TestDirectory::new("conflict");
        let project_root = directory.path().join("project");
        fs::create_dir(&project_root).expect("create project root");
        let project = registered_project(&project_root);
        let project_id = project.id.to_string();
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize");
        database
            .import_registered_projects(&[project])
            .expect("import project");

        let error = database
            .set_project_status(&project_id, 99, ProjectStatus::Stale, "test", None)
            .expect_err("revision conflict");
        assert!(matches!(error, AppError::RevisionConflict { .. }));
        assert_eq!(database.project(&project_id).expect("project").revision, 1);
        assert_eq!(
            database.mutation_count(&project_id).expect("audit count"),
            2
        );
    }

    #[test]
    fn failed_project_batch_rolls_back_rows_and_audit_events() {
        let directory = TestDirectory::new("batch-rollback");
        let first_root = directory.path().join("first");
        let second_root = directory.path().join("second");
        fs::create_dir(&first_root).expect("create first root");
        fs::create_dir(&second_root).expect("create second root");
        let first = registered_project(&first_root);
        let mut second = registered_project(&second_root);
        second.id = first.id;
        let project_id = first.id.to_string();
        let database = InspectorDatabase::new(directory.path().join(DATABASE_FILE));
        database.initialize().expect("initialize");

        database
            .import_registered_projects(&[first, second])
            .expect_err("duplicate project id must fail the batch");

        assert!(database.list_projects(true).expect("projects").is_empty());
        assert_eq!(
            database.mutation_count(&project_id).expect("audit count"),
            0
        );
    }
}
