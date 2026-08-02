use rusqlite::{params, OptionalExtension, ToSql, Transaction};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

use super::{
    database_error, record_mutation, revision_from_sql, revision_to_sql, timestamp,
    InspectorDatabase,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IssueSeverity {
    P0,
    P1,
    P2,
}

impl IssueSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::P0 => "p0",
            Self::P1 => "p1",
            Self::P2 => "p2",
        }
    }

    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "p0" => Ok(Self::P0),
            "p1" => Ok(Self::P1),
            "p2" => Ok(Self::P2),
            other => Err(AppError::Database(format!(
                "unknown issue severity {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IssueStatus {
    Open,
    Resolved,
    Wontfix,
}

impl IssueStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Wontfix => "wontfix",
        }
    }

    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "open" => Ok(Self::Open),
            "resolved" => Ok(Self::Resolved),
            "wontfix" => Ok(Self::Wontfix),
            other => Err(AppError::Database(format!(
                "unknown issue status {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub project_id: String,
    pub module_id: String,
    pub flow_id: Option<String>,
    pub module_revision: Option<u64>,
    pub title: String,
    pub description: String,
    pub severity: IssueSeverity,
    pub status: IssueStatus,
    pub path: Option<String>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub actor: String,
    pub source: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct IssuePatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub severity: Option<IssueSeverity>,
    pub status: Option<IssueStatus>,
    pub flow_id: Option<Option<String>>,
    pub path: Option<Option<String>>,
    pub start_line: Option<Option<usize>>,
    pub end_line: Option<Option<usize>>,
}

impl InspectorDatabase {
    #[allow(clippy::too_many_arguments)]
    pub fn create_issue(
        &self,
        project_id: &str,
        module_id: &str,
        flow_id: Option<&str>,
        module_revision: Option<u64>,
        title: &str,
        description: &str,
        severity: IssueSeverity,
        status: Option<IssueStatus>,
        path: Option<&str>,
        start_line: Option<usize>,
        end_line: Option<usize>,
        actor: &str,
        source: &str,
        request_id: Option<&str>,
    ) -> Result<Issue, AppError> {
        validate_issue_fields(title, path, start_line, end_line)?;
        let status = status.unwrap_or(IssueStatus::Open);
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(database_error)?;
        ensure_module_active(&transaction, project_id, module_id)?;
        if let Some(flow_id) = flow_id {
            ensure_flow_in_module(&transaction, module_id, flow_id)?;
        }
        let id = Uuid::now_v7().to_string();
        let now = timestamp();
        let resolved_at = (status != IssueStatus::Open).then_some(now.as_str());
        transaction
            .execute(
                "INSERT INTO issues (
                    id, project_id, module_id, flow_id, module_revision, title, description,
                    severity, status, path, start_line, end_line, actor, source, revision,
                    created_at, updated_at, resolved_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 1, ?15, ?15, ?16
                 )",
                params![
                    id,
                    project_id,
                    module_id,
                    flow_id,
                    module_revision.map(revision_to_sql).transpose()?,
                    title,
                    description,
                    severity.as_str(),
                    status.as_str(),
                    path,
                    start_line.map(i64_from_usize).transpose()?,
                    end_line.map(i64_from_usize).transpose()?,
                    actor,
                    source,
                    now,
                    resolved_at,
                ],
            )
            .map_err(database_error)?;
        record_mutation(
            &transaction,
            Some(project_id),
            "issue",
            &id,
            "create",
            actor,
            source,
            1,
            request_id,
            &now,
        )?;
        transaction.commit().map_err(database_error)?;
        self.get_issue(&id)
    }

    pub fn list_issues(
        &self,
        project_id: &str,
        module_id: Option<&str>,
        status: Option<IssueStatus>,
        limit: usize,
    ) -> Result<Vec<Issue>, AppError> {
        let connection = self.connect()?;
        let limit = i64::try_from(limit.clamp(1, 200))
            .map_err(|_| AppError::Validation("invalid issue list limit".to_string()))?;
        let mut sql = String::from(
            "SELECT id, project_id, module_id, flow_id, module_revision, title, description,
                    severity, status, path, start_line, end_line, actor, source, revision,
                    created_at, updated_at, resolved_at, archived_at
             FROM issues WHERE project_id = ?1 AND archived_at IS NULL",
        );
        let mut bindings: Vec<&dyn ToSql> = vec![&project_id];
        let mut next_parameter = 2;
        if let Some(module_id) = module_id.as_ref() {
            sql.push_str(&format!(" AND module_id = ?{next_parameter}"));
            next_parameter += 1;
            bindings.push(module_id);
        }
        let status = status.map(IssueStatus::as_str);
        if let Some(status) = status.as_ref() {
            sql.push_str(&format!(" AND status = ?{next_parameter}"));
            next_parameter += 1;
            bindings.push(status);
        }
        sql.push_str(&format!(
            " ORDER BY created_at DESC, id DESC LIMIT ?{next_parameter}"
        ));
        bindings.push(&limit);
        let mut statement = connection.prepare(&sql).map_err(database_error)?;
        let rows = statement
            .query_map(bindings.as_slice(), map_issue)
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        rows.into_iter().map(parse_issue).collect()
    }

    pub fn get_issue(&self, issue_id: &str) -> Result<Issue, AppError> {
        let connection = self.connect()?;
        load_issue(&connection, issue_id, true)?
            .ok_or_else(|| AppError::NotFound(format!("issue {issue_id}")))
    }

    pub fn update_issue(
        &self,
        issue_id: &str,
        expected_revision: u64,
        patch: &IssuePatch,
        actor: &str,
        source: &str,
        request_id: Option<&str>,
    ) -> Result<Issue, AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(database_error)?;
        let current = load_issue(&transaction, issue_id, false)?
            .ok_or_else(|| AppError::NotFound(format!("issue {issue_id}")))?;
        if current.revision != expected_revision {
            return Err(AppError::RevisionConflict {
                entity_id: issue_id.to_string(),
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| AppError::Database("issue revision overflow".to_string()))?;
        let now = timestamp();

        let title = patch.title.clone().unwrap_or_else(|| current.title.clone());
        let description = patch
            .description
            .clone()
            .unwrap_or_else(|| current.description.clone());
        let severity = patch.severity.unwrap_or(current.severity);
        let status = patch.status.unwrap_or(current.status);
        let flow_id = patch
            .flow_id
            .clone()
            .unwrap_or_else(|| current.flow_id.clone());
        let path = patch.path.clone().unwrap_or_else(|| current.path.clone());
        let start_line = patch.start_line.unwrap_or(current.start_line);
        let end_line = patch.end_line.unwrap_or(current.end_line);
        validate_issue_fields(&title, path.as_deref(), start_line, end_line)?;
        if let Some(flow_id) = flow_id.as_deref() {
            ensure_flow_in_module(&transaction, &current.module_id, flow_id)?;
        }
        let resolved_at = match (current.status, status) {
            (IssueStatus::Open, IssueStatus::Resolved | IssueStatus::Wontfix) => {
                Some(now.clone())
            }
            (IssueStatus::Resolved | IssueStatus::Wontfix, IssueStatus::Open) => None,
            _ => current.resolved_at.clone(),
        };

        let changed = transaction
            .execute(
                "UPDATE issues SET flow_id = ?1, title = ?2, description = ?3, severity = ?4,
                    status = ?5, path = ?6, start_line = ?7, end_line = ?8, revision = ?9,
                    updated_at = ?10, resolved_at = ?11
                 WHERE id = ?12 AND revision = ?13 AND archived_at IS NULL",
                params![
                    flow_id,
                    title,
                    description,
                    severity.as_str(),
                    status.as_str(),
                    path,
                    start_line.map(i64_from_usize).transpose()?,
                    end_line.map(i64_from_usize).transpose()?,
                    revision_to_sql(next_revision)?,
                    now,
                    resolved_at,
                    issue_id,
                    revision_to_sql(expected_revision)?,
                ],
            )
            .map_err(database_error)?;
        if changed == 0 {
            return Err(AppError::RevisionConflict {
                entity_id: issue_id.to_string(),
                expected: expected_revision,
                actual: current.revision,
            });
        }
        record_mutation(
            &transaction,
            Some(&current.project_id),
            "issue",
            issue_id,
            "update",
            actor,
            source,
            next_revision,
            request_id,
            &now,
        )?;
        transaction.commit().map_err(database_error)?;
        self.get_issue(issue_id)
    }

    pub fn archive_issue(
        &self,
        issue_id: &str,
        expected_revision: u64,
        actor: &str,
        source: &str,
        request_id: Option<&str>,
    ) -> Result<Issue, AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(database_error)?;
        let current = load_issue(&transaction, issue_id, false)?
            .ok_or_else(|| AppError::NotFound(format!("issue {issue_id}")))?;
        if current.revision != expected_revision {
            return Err(AppError::RevisionConflict {
                entity_id: issue_id.to_string(),
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| AppError::Database("issue revision overflow".to_string()))?;
        let now = timestamp();
        transaction
            .execute(
                "UPDATE issues SET revision = ?1, updated_at = ?2, archived_at = ?2
                 WHERE id = ?3 AND revision = ?4 AND archived_at IS NULL",
                params![
                    revision_to_sql(next_revision)?,
                    now,
                    issue_id,
                    revision_to_sql(expected_revision)?,
                ],
            )
            .map_err(database_error)?;
        record_mutation(
            &transaction,
            Some(&current.project_id),
            "issue",
            issue_id,
            "archive",
            actor,
            source,
            next_revision,
            request_id,
            &now,
        )?;
        transaction.commit().map_err(database_error)?;
        self.get_issue(issue_id)
    }
}

fn validate_issue_fields(
    title: &str,
    path: Option<&str>,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<(), AppError> {
    if title.trim().is_empty() {
        return Err(AppError::Validation(
            "issue title must not be empty".to_string(),
        ));
    }
    if path.is_some_and(|path| path.trim().is_empty()) {
        return Err(AppError::Validation(
            "issue path must not be empty".to_string(),
        ));
    }
    if matches!(start_line, Some(0)) {
        return Err(AppError::Validation(
            "issue startLine must be positive".to_string(),
        ));
    }
    if let (Some(start), Some(end)) = (start_line, end_line) {
        if end < start {
            return Err(AppError::Validation(
                "issue endLine must not precede startLine".to_string(),
            ));
        }
    }
    Ok(())
}

fn ensure_module_active(
    transaction: &Transaction<'_>,
    project_id: &str,
    module_id: &str,
) -> Result<(), AppError> {
    let exists = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM modules
                WHERE id = ?1 AND project_id = ?2 AND archived_at IS NULL
            )",
            params![module_id, project_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(database_error)?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("module {module_id}")))
    }
}

fn ensure_flow_in_module(
    transaction: &Transaction<'_>,
    module_id: &str,
    flow_id: &str,
) -> Result<(), AppError> {
    let exists = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM flows
                WHERE id = ?1 AND module_id = ?2 AND archived_at IS NULL
            )",
            params![flow_id, module_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(database_error)?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("flow {flow_id}")))
    }
}

fn load_issue(
    connection: &rusqlite::Connection,
    issue_id: &str,
    include_archived: bool,
) -> Result<Option<Issue>, AppError> {
    let sql = if include_archived {
        "SELECT id, project_id, module_id, flow_id, module_revision, title, description,
                severity, status, path, start_line, end_line, actor, source, revision,
                created_at, updated_at, resolved_at, archived_at
         FROM issues WHERE id = ?1"
    } else {
        "SELECT id, project_id, module_id, flow_id, module_revision, title, description,
                severity, status, path, start_line, end_line, actor, source, revision,
                created_at, updated_at, resolved_at, archived_at
         FROM issues WHERE id = ?1 AND archived_at IS NULL"
    };
    connection
        .query_row(sql, [issue_id], map_issue)
        .optional()
        .map_err(database_error)?
        .map(parse_issue)
        .transpose()
}

#[allow(clippy::type_complexity)]
type IssueRow = (
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
);

fn map_issue(row: &rusqlite::Row<'_>) -> rusqlite::Result<IssueRow> {
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
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
        row.get(18)?,
    ))
}

fn parse_issue(row: IssueRow) -> Result<Issue, AppError> {
    Ok(Issue {
        id: row.0,
        project_id: row.1,
        module_id: row.2,
        flow_id: row.3,
        module_revision: row.4.map(revision_from_sql).transpose()?,
        title: row.5,
        description: row.6,
        severity: IssueSeverity::parse(&row.7)?,
        status: IssueStatus::parse(&row.8)?,
        path: row.9,
        start_line: row.10.map(usize_from_sql).transpose()?,
        end_line: row.11.map(usize_from_sql).transpose()?,
        actor: row.12,
        source: row.13,
        revision: revision_from_sql(row.14)?,
        created_at: row.15,
        updated_at: row.16,
        resolved_at: row.17,
        archived_at: row.18,
    })
}

fn usize_from_sql(value: i64) -> Result<usize, AppError> {
    usize::try_from(value).map_err(|_| AppError::Database(format!("invalid integer {value}")))
}

fn i64_from_usize(value: usize) -> Result<i64, AppError> {
    i64::try_from(value).map_err(|_| AppError::Validation(format!("integer {value} is too large")))
}
