mod crud;

use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::{Arc, RwLock},
    time::Instant,
};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, Json, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    codegraph::{related_files, RelatedFile},
    command_service::{run_inspector_command, RunInspectorCommandInput, RunInspectorCommandResult},
    error::AppError,
    fs_broker::{
        canonical_root, read_source_range, validated_relative_path, ReadSourceInput, SourceRange,
    },
    inspector::{EntityStatus, InspectorFlow, InspectorModule, InspectorSnapshot},
    storage::{
        commands::StoredCommandRun,
        issues::{Issue, IssueStatus},
        mcp::McpAuditEvent,
        versions::{EntityVersion, EntityVersionSummary},
        InspectorDatabase, StoredProject,
    },
};

#[derive(Debug, Clone)]
pub struct McpAuthorization {
    session_id: String,
    allowed_project_ids: Arc<RwLock<HashSet<String>>>,
    allow_project_create: bool,
}

impl McpAuthorization {
    pub fn new(
        session_id: impl Into<String>,
        allowed_project_ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, AppError> {
        let session_id = session_id.into();
        let allowed_project_ids = allowed_project_ids.into_iter().collect::<HashSet<_>>();
        validate_session_id(&session_id)?;
        if allowed_project_ids.is_empty() {
            return Err(AppError::Validation(
                "MCP session requires a project allow-list".to_string(),
            ));
        }
        Ok(Self {
            session_id,
            allowed_project_ids: Arc::new(RwLock::new(allowed_project_ids)),
            allow_project_create: false,
        })
    }

    pub fn for_local_tray(
        session_id: impl Into<String>,
        allowed_project_ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, AppError> {
        let session_id = session_id.into();
        validate_session_id(&session_id)?;
        Ok(Self {
            session_id,
            allowed_project_ids: Arc::new(RwLock::new(allowed_project_ids.into_iter().collect())),
            allow_project_create: true,
        })
    }

    pub fn from_env() -> Result<Self, AppError> {
        let secret = std::env::var("VIBE_INSPECTOR_SESSION_SECRET")
            .map_err(|_| AppError::Validation("MCP session secret is missing".to_string()))?;
        if secret.len() < 32 {
            return Err(AppError::Validation(
                "MCP session secret must contain at least 32 characters".to_string(),
            ));
        }
        let session_id = std::env::var("VIBE_INSPECTOR_SESSION_ID")
            .map_err(|_| AppError::Validation("MCP session ID is missing".to_string()))?;
        let projects = std::env::var("VIBE_INSPECTOR_ALLOWED_PROJECTS")
            .map_err(|_| AppError::Validation("MCP project allow-list is missing".to_string()))?;
        let mut authorization = Self::new(
            session_id,
            projects
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        )?;
        authorization.allow_project_create =
            std::env::var("VIBE_INSPECTOR_ALLOW_PROJECT_CREATE").as_deref() == Ok("1");
        Ok(authorization)
    }

    fn authorize(&self, project_id: &str) -> Result<(), AppError> {
        let allowed = self
            .allowed_project_ids
            .read()
            .expect("project allow-list lock");
        if allowed.contains(project_id) {
            Ok(())
        } else {
            Err(AppError::Unauthorized(format!("project {project_id}")))
        }
    }

    /// Grant this session access to a project it just registered through
    /// `inspector.project.create`, so follow-up tools in the same session pass
    /// the allow-list gate without a desktop-side re-sync.
    fn allow_project(&self, project_id: &str) {
        self.allowed_project_ids
            .write()
            .expect("project allow-list lock")
            .insert(project_id.to_string());
    }

    fn authorize_project_create(&self) -> Result<(), AppError> {
        if self.allow_project_create {
            Ok(())
        } else {
            Err(AppError::Unauthorized("project creation".to_string()))
        }
    }
}

fn validate_session_id(session_id: &str) -> Result<(), AppError> {
    if session_id.trim().is_empty() || session_id.len() > 128 {
        Err(AppError::Validation("invalid MCP session ID".to_string()))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct VibeInspectorMcp {
    database: InspectorDatabase,
    authorization: McpAuthorization,
    tool_router: ToolRouter<Self>,
}

impl VibeInspectorMcp {
    pub fn new(database: InspectorDatabase, authorization: McpAuthorization) -> Self {
        Self {
            database,
            authorization,
            tool_router: Self::tool_router(),
        }
    }

    async fn execute<T, F>(
        &self,
        tool: &'static str,
        project_id: Option<String>,
        entity_ids: Vec<String>,
        operation: F,
    ) -> Result<Json<T>, String>
    where
        T: Send + 'static,
        F: FnOnce(&InspectorDatabase) -> Result<T, AppError> + Send + 'static,
    {
        if let Some(project_id) = project_id.as_deref() {
            if let Err(error) = self.authorization.authorize(project_id) {
                let mut denied_entities = entity_ids.clone();
                denied_entities.push(project_id.to_string());
                let _ = self.database.record_mcp_audit(
                    &self.authorization.session_id,
                    tool,
                    None,
                    &denied_entities,
                    "unauthorized",
                    0,
                );
                return Err(tool_error(error));
            }
        }
        let database = self.database.clone();
        let session_id = self.authorization.session_id.clone();
        tokio::task::spawn_blocking(move || {
            let started = Instant::now();
            let result = operation(&database);
            let result_label = if result.is_ok() { "ok" } else { "error" };
            database
                .record_mcp_audit(
                    &session_id,
                    tool,
                    project_id.as_deref(),
                    &entity_ids,
                    result_label,
                    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                )
                .map_err(tool_error)?;
            result.map(Json).map_err(tool_error)
        })
        .await
        .map_err(|error| tool_error(AppError::DiscoveryTask(error.to_string())))?
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub project_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInput {
    pub project_id: String,
    pub module_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlowInput {
    pub project_id: String,
    pub flow_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceReadInput {
    pub project_id: String,
    pub source_ref_id: Option<String>,
    pub relative_path: Option<String>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub expected_hash: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInput {
    pub project_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    pub query: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocateInput {
    pub project_id: Option<String>,
    pub path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceRelatedInput {
    pub project_id: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchModuleMatch {
    pub project_id: String,
    pub module_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub status: EntityStatus,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub projects: Vec<StoredProject>,
    pub modules: Vec<SearchModuleMatch>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocateResult {
    pub project: StoredProject,
    pub relative_path: String,
    pub modules: Vec<InspectorModule>,
    pub related_files: Vec<RelatedFile>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RelatedFileList {
    pub related: Vec<RelatedFile>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: StoredProject,
    pub snapshot: InspectorSnapshot,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectList {
    pub projects: Vec<StoredProject>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleList {
    pub modules: Vec<InspectorModule>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlowList {
    pub flows: Vec<InspectorFlow>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityList {
    pub command_runs: Vec<StoredCommandRun>,
    pub audit_events: Vec<McpAuditEvent>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionListInput {
    pub request_id: Option<String>,
    pub project_id: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionGetInput {
    pub request_id: Option<String>,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueListInput {
    pub request_id: Option<String>,
    pub project_id: String,
    pub module_id: Option<String>,
    pub status: Option<IssueStatus>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionSummaryList {
    pub versions: Vec<EntityVersionSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    pub version: EntityVersion,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueListResult {
    pub issues: Vec<Issue>,
}

#[tool_router(router = tool_router)]
impl VibeInspectorMcp {
    #[tool(
        name = "inspector.project.list",
        description = "List Inspector projects allowed for this authenticated MCP session"
    )]
    async fn project_list(&self) -> Result<Json<ProjectList>, String> {
        let allowed = self
            .authorization
            .allowed_project_ids
            .read()
            .expect("project allow-list lock")
            .clone();
        self.execute(
            "inspector.project.list",
            None,
            Vec::new(),
            move |database| {
                Ok(ProjectList {
                    projects: database
                        .list_projects(false)?
                        .into_iter()
                        .filter(|project| allowed.contains(&project.id))
                        .collect(),
                })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.project.get",
        description = "Get project metadata and its complete active Inspector snapshot"
    )]
    async fn project_get(
        &self,
        Parameters(input): Parameters<ProjectInput>,
    ) -> Result<Json<ProjectDetail>, String> {
        let project_id = input.project_id.clone();
        self.execute(
            "inspector.project.get",
            Some(project_id.clone()),
            Vec::new(),
            move |database| {
                Ok(ProjectDetail {
                    project: database.project(&project_id)?,
                    snapshot: database.load_inspector_snapshot(&project_id)?,
                })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.module.list",
        description = "List active modules for a project"
    )]
    async fn module_list(
        &self,
        Parameters(input): Parameters<ProjectInput>,
    ) -> Result<Json<ModuleList>, String> {
        let project_id = input.project_id.clone();
        self.execute(
            "inspector.module.list",
            Some(project_id.clone()),
            Vec::new(),
            move |database| {
                Ok(ModuleList {
                    modules: database.load_inspector_snapshot(&project_id)?.modules,
                })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.module.get",
        description = "Get one active Inspector module with its flows, graphs, explanations, and source references"
    )]
    async fn module_get(
        &self,
        Parameters(input): Parameters<ModuleInput>,
    ) -> Result<Json<InspectorModule>, String> {
        let project_id = input.project_id.clone();
        let module_id = input.module_id.clone();
        self.execute(
            "inspector.module.get",
            Some(project_id.clone()),
            vec![module_id.clone()],
            move |database| {
                database
                    .load_inspector_snapshot(&project_id)?
                    .modules
                    .into_iter()
                    .find(|module| module.id == module_id)
                    .ok_or_else(|| AppError::NotFound(format!("module {module_id}")))
            },
        )
        .await
    }

    #[tool(
        name = "inspector.flow.list",
        description = "List active flows for one module"
    )]
    async fn flow_list(
        &self,
        Parameters(input): Parameters<ModuleInput>,
    ) -> Result<Json<FlowList>, String> {
        let project_id = input.project_id.clone();
        let module_id = input.module_id.clone();
        self.execute(
            "inspector.flow.list",
            Some(project_id.clone()),
            vec![module_id.clone()],
            move |database| {
                database
                    .load_inspector_snapshot(&project_id)?
                    .modules
                    .into_iter()
                    .find(|module| module.id == module_id)
                    .map(|module| FlowList {
                        flows: module.flows,
                    })
                    .ok_or_else(|| AppError::NotFound(format!("module {module_id}")))
            },
        )
        .await
    }

    #[tool(
        name = "inspector.flow.get",
        description = "Get one active flow with graph, rationale, and evidence"
    )]
    async fn flow_get(
        &self,
        Parameters(input): Parameters<FlowInput>,
    ) -> Result<Json<InspectorFlow>, String> {
        let project_id = input.project_id.clone();
        let flow_id = input.flow_id.clone();
        self.execute(
            "inspector.flow.get",
            Some(project_id.clone()),
            vec![flow_id.clone()],
            move |database| {
                database
                    .load_inspector_snapshot(&project_id)?
                    .modules
                    .into_iter()
                    .flat_map(|module| module.flows)
                    .find(|flow| flow.id == flow_id)
                    .ok_or_else(|| AppError::NotFound(format!("flow {flow_id}")))
            },
        )
        .await
    }

    #[tool(
        name = "inspector.source.read",
        description = "Read a bounded UTF-8 source range inside an allowed registered project root"
    )]
    async fn source_read(
        &self,
        Parameters(input): Parameters<SourceReadInput>,
    ) -> Result<Json<SourceRange>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = input.source_ref_id.iter().cloned().collect();
        self.execute(
            "inspector.source.read",
            Some(project_id.clone()),
            entity_ids,
            move |database| {
                let project = database.project(&project_id)?;
                let source_input = if let Some(source_ref_id) = input.source_ref_id {
                    let source = database
                        .load_inspector_snapshot(&project_id)?
                        .modules
                        .into_iter()
                        .flat_map(|module| module.flows)
                        .flat_map(|flow| flow.source_refs)
                        .find(|source| source.id == source_ref_id)
                        .ok_or_else(|| {
                            AppError::NotFound(format!("source reference {source_ref_id}"))
                        })?;
                    ReadSourceInput {
                        relative_path: source.path,
                        start_line: source.start_line,
                        end_line: source.end_line,
                        expected_hash: Some(source.content_hash),
                    }
                } else {
                    ReadSourceInput {
                        relative_path: input.relative_path.ok_or_else(|| {
                            AppError::Validation(
                                "sourceRefId or relativePath is required".to_string(),
                            )
                        })?,
                        start_line: input.start_line.ok_or_else(|| {
                            AppError::Validation("startLine is required".to_string())
                        })?,
                        end_line: input.end_line.ok_or_else(|| {
                            AppError::Validation("endLine is required".to_string())
                        })?,
                        expected_hash: input.expected_hash,
                    }
                };
                read_source_range(std::path::Path::new(&project.root_path), source_input)
            },
        )
        .await
    }

    #[tool(
        name = "inspector.activity.list",
        description = "List command runs and MCP audit metadata for one project"
    )]
    async fn activity_list(
        &self,
        Parameters(input): Parameters<ActivityInput>,
    ) -> Result<Json<ActivityList>, String> {
        let project_id = input.project_id.clone();
        let limit = input.limit.unwrap_or(50).clamp(1, 200);
        self.execute(
            "inspector.activity.list",
            Some(project_id.clone()),
            Vec::new(),
            move |database| {
                Ok(ActivityList {
                    command_runs: database.list_command_runs(&project_id, limit)?,
                    audit_events: database.list_mcp_audit(&project_id, limit)?,
                })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.search",
        description = "Search projects and modules by keyword across name, slug, summary, and root path. Scoped to one project when projectId is supplied, otherwise to every project allowed for this session. Use this before creating modules to find existing ones."
    )]
    async fn search(
        &self,
        Parameters(input): Parameters<SearchInput>,
    ) -> Result<Json<SearchResults>, String> {
        let allowed = self
            .authorization
            .allowed_project_ids
            .read()
            .expect("project allow-list lock")
            .clone();
        self.execute(
            "inspector.search",
            input.project_id.clone(),
            Vec::new(),
            move |database| search_projects_modules(database, &allowed, input),
        )
        .await
    }

    #[tool(
        name = "inspector.source.locate",
        description = "Resolve a code file to its Inspector project and the modules and flows whose source references cover it. Accepts an absolute path (the containing allowed project is detected) or a project-relative path with projectId. Optional line range narrows the match. Also returns CodeGraph-related files when the project has an index."
    )]
    async fn source_locate(
        &self,
        Parameters(input): Parameters<SourceLocateInput>,
    ) -> Result<Json<SourceLocateResult>, String> {
        let allowed = self
            .authorization
            .allowed_project_ids
            .read()
            .expect("project allow-list lock")
            .clone();
        let entity_ids = input.project_id.iter().cloned().collect();
        let audit_resolved_project = input.project_id.is_none();
        let result = self
            .execute(
                "inspector.source.locate",
                input.project_id.clone(),
                entity_ids,
                move |database| locate_source(database, &allowed, input),
            )
            .await?;
        if audit_resolved_project {
            // `execute` already audited the call without a project; add the
            // project the path resolved to so the trail stays complete.
            let database = self.database.clone();
            let session_id = self.authorization.session_id.clone();
            let project_id = result.0.project.id.clone();
            let _ = tokio::task::spawn_blocking(move || {
                database.record_mcp_audit(
                    &session_id,
                    "inspector.source.locate",
                    Some(&project_id),
                    &[],
                    "ok",
                    0,
                )
            })
            .await;
        }
        Ok(result)
    }

    #[tool(
        name = "inspector.source.related",
        description = "List CodeGraph dependency and dependent files for the given project-relative source paths. Read-only; returns an empty list when the project has no .codegraph/codegraph.db index."
    )]
    async fn source_related(
        &self,
        Parameters(input): Parameters<SourceRelatedInput>,
    ) -> Result<Json<RelatedFileList>, String> {
        let project_id = input.project_id.clone();
        self.execute(
            "inspector.source.related",
            Some(project_id.clone()),
            Vec::new(),
            move |database| {
                let project = database.project(&project_id)?;
                Ok(RelatedFileList {
                    related: related_files(Path::new(&project.root_path), &input.paths)?,
                })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.command.run",
        description = "Validate and run inspector-init, inspector-sync, or inspector-clarify through the shared command service"
    )]
    async fn command_run(
        &self,
        Parameters(input): Parameters<RunInspectorCommandInput>,
    ) -> Result<Json<RunInspectorCommandResult>, String> {
        let project_id = input.project_id.clone();
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.command.run",
            Some(project_id),
            Vec::new(),
            move |database| run_inspector_command(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.project.create",
        description = "Register a recognized local project root, or return the project already registered for that root. Call this with the canonical working-directory root to auto-import a project that inspector.project.list does not match; the project is authorized for the rest of this session."
    )]
    async fn project_create(
        &self,
        Parameters(input): Parameters<crud::ProjectCreateInput>,
    ) -> Result<Json<crud::ProjectMutationResult>, String> {
        self.authorization
            .authorize_project_create()
            .map_err(tool_error)?;
        let actor = format!("mcp:{}", self.authorization.session_id);
        let result = self
            .execute(
                "inspector.project.create",
                None,
                Vec::new(),
                move |database| crud::project_create(database, input, &actor),
            )
            .await?;
        self.authorization.allow_project(&result.0.project.id);
        Ok(result)
    }

    #[tool(
        name = "inspector.project.update",
        description = "Update a project name with optimistic revision checking"
    )]
    async fn project_update(
        &self,
        Parameters(input): Parameters<crud::ProjectUpdateInput>,
    ) -> Result<Json<crud::ProjectMutationResult>, String> {
        let project_id = input.project_id.clone();
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.project.update",
            Some(project_id),
            Vec::new(),
            move |database| crud::project_update(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.project.archive",
        description = "Soft-archive a project with optimistic revision checking"
    )]
    async fn project_archive(
        &self,
        Parameters(input): Parameters<crud::ProjectArchiveInput>,
    ) -> Result<Json<crud::ProjectMutationResult>, String> {
        let project_id = input.project_id.clone();
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.project.archive",
            Some(project_id),
            Vec::new(),
            move |database| crud::project_archive(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.module.create",
        description = "Create a module with a server-generated UUIDv7 ID. When an active module with the same slug already exists, that module is returned unchanged (reusedExisting: true, and the expectedProjectRevision check is skipped) instead of creating a duplicate. List modules first and prefer inspector.module.update for existing modules."
    )]
    async fn module_create(
        &self,
        Parameters(input): Parameters<crud::ModuleCreateInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.module.create",
            Some(project_id),
            Vec::new(),
            move |database| crud::module_create(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.module.update",
        description = "Update module metadata with project and entity revision checks"
    )]
    async fn module_update(
        &self,
        Parameters(input): Parameters<crud::ModuleUpdateInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.module_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.module.update",
            Some(project_id),
            entity_ids,
            move |database| crud::module_update(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.module.archive",
        description = "Soft-archive a module subtree and its flows"
    )]
    async fn module_archive(
        &self,
        Parameters(input): Parameters<crud::EntityArchiveInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.entity_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.module.archive",
            Some(project_id),
            entity_ids,
            move |database| crud::module_archive(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.flow.create",
        description = "Create a flow with a server-generated UUIDv7 ID"
    )]
    async fn flow_create(
        &self,
        Parameters(input): Parameters<crud::FlowCreateInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.module_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.flow.create",
            Some(project_id),
            entity_ids,
            move |database| crud::flow_create(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.flow.update",
        description = "Update or move a flow with project and entity revision checks"
    )]
    async fn flow_update(
        &self,
        Parameters(input): Parameters<crud::FlowUpdateInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.flow_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.flow.update",
            Some(project_id),
            entity_ids,
            move |database| crud::flow_update(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.flow.archive",
        description = "Soft-archive one flow and its evidence"
    )]
    async fn flow_archive(
        &self,
        Parameters(input): Parameters<crud::EntityArchiveInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.entity_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.flow.archive",
            Some(project_id),
            entity_ids,
            move |database| crud::flow_archive(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.sourceRef.upsert",
        description = "Create or update a bounded source reference using the broker-computed hash"
    )]
    async fn source_ref_upsert(
        &self,
        Parameters(input): Parameters<crud::SourceRefUpsertInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = input.source_ref_id.iter().cloned().collect();
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.sourceRef.upsert",
            Some(project_id),
            entity_ids,
            move |database| crud::source_ref_upsert(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.sourceRef.archive",
        description = "Soft-archive one source reference and remove graph evidence links"
    )]
    async fn source_ref_archive(
        &self,
        Parameters(input): Parameters<crud::EntityArchiveInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.entity_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.sourceRef.archive",
            Some(project_id),
            entity_ids,
            move |database| crud::source_ref_archive(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.explanation.upsert",
        description = "Create or update one flow rationale with MCP provenance"
    )]
    async fn explanation_upsert(
        &self,
        Parameters(input): Parameters<crud::ExplanationUpsertInput>,
    ) -> Result<Json<crud::InspectorMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = input.explanation_id.iter().cloned().collect();
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.explanation.upsert",
            Some(project_id),
            entity_ids,
            move |database| crud::explanation_upsert(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.version.list",
        description = "List recorded entity versions for one project, newest first, optionally filtered by entityType and entityId"
    )]
    async fn version_list(
        &self,
        Parameters(input): Parameters<VersionListInput>,
    ) -> Result<Json<VersionSummaryList>, String> {
        let project_id = input.project_id.clone();
        self.execute(
            "inspector.version.list",
            Some(project_id.clone()),
            Vec::new(),
            move |database| {
                Ok(VersionSummaryList {
                    versions: database.list_entity_versions(
                        &project_id,
                        input.entity_type.as_deref(),
                        input.entity_id.as_deref(),
                        input.limit.unwrap_or(50),
                    )?,
                })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.version.get",
        description = "Get one recorded entity version with its snapshot payload"
    )]
    async fn version_get(
        &self,
        Parameters(input): Parameters<VersionGetInput>,
    ) -> Result<Json<VersionDetail>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.version_id.clone()];
        self.execute(
            "inspector.version.get",
            Some(project_id.clone()),
            entity_ids,
            move |database| {
                let version = database.get_entity_version(&input.version_id)?;
                if version.project_id != project_id {
                    // Hide versions of other projects so a session cannot
                    // probe version IDs outside its allow-list.
                    return Err(AppError::NotFound(format!(
                        "entity version {}",
                        input.version_id
                    )));
                }
                Ok(VersionDetail { version })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.issue.list",
        description = "List active issues for one project, newest first, optionally filtered by moduleId and status"
    )]
    async fn issue_list(
        &self,
        Parameters(input): Parameters<IssueListInput>,
    ) -> Result<Json<IssueListResult>, String> {
        let project_id = input.project_id.clone();
        self.execute(
            "inspector.issue.list",
            Some(project_id.clone()),
            Vec::new(),
            move |database| {
                Ok(IssueListResult {
                    issues: database.list_issues(
                        &project_id,
                        input.module_id.as_deref(),
                        input.status,
                        input.limit.unwrap_or(50),
                    )?,
                })
            },
        )
        .await
    }

    #[tool(
        name = "inspector.issue.create",
        description = "Create an issue entry on a module with a server-generated UUIDv7 ID. Issues are not part of the Inspector snapshot and do not bump the project revision. Idempotent for the same requestId."
    )]
    async fn issue_create(
        &self,
        Parameters(input): Parameters<crud::IssueCreateInput>,
    ) -> Result<Json<crud::IssueMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.module_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.issue.create",
            Some(project_id),
            entity_ids,
            move |database| crud::issue_create(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.issue.update",
        description = "Update an issue entry with entity-level optimistic revision checking. Idempotent for the same requestId."
    )]
    async fn issue_update(
        &self,
        Parameters(input): Parameters<crud::IssueUpdateInput>,
    ) -> Result<Json<crud::IssueMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.issue_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.issue.update",
            Some(project_id),
            entity_ids,
            move |database| crud::issue_update(database, input, &actor),
        )
        .await
    }

    #[tool(
        name = "inspector.issue.archive",
        description = "Soft-archive an issue entry with entity-level optimistic revision checking. Idempotent for the same requestId."
    )]
    async fn issue_archive(
        &self,
        Parameters(input): Parameters<crud::IssueArchiveInput>,
    ) -> Result<Json<crud::IssueMutationResult>, String> {
        let project_id = input.project_id.clone();
        let entity_ids = vec![input.issue_id.clone()];
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.issue.archive",
            Some(project_id),
            entity_ids,
            move |database| crud::issue_archive(database, input, &actor),
        )
        .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VibeInspectorMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "vibe-inspector",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Local-only repository inspection. All project reads require session allow-list authorization; source files are read-only and bounded. When the current project is not registered, auto-import it with inspector.project.create before other tools. Before creating modules, list existing ones and reuse or update matches instead of creating duplicates.",
            )
    }
}

const MAX_SEARCH_QUERY_CHARS: usize = 200;
const MAX_SEARCH_PROJECTS: usize = 20;
const MAX_SEARCH_MODULES: usize = 50;

fn search_projects_modules(
    database: &InspectorDatabase,
    allowed: &HashSet<String>,
    input: SearchInput,
) -> Result<SearchResults, AppError> {
    let query = input.query.trim().to_lowercase();
    if query.is_empty() || query.len() > MAX_SEARCH_QUERY_CHARS {
        return Err(AppError::Validation("invalid search query".to_string()));
    }
    let projects = database
        .list_projects(false)?
        .into_iter()
        .filter(|project| {
            input
                .project_id
                .as_ref()
                .map_or_else(|| allowed.contains(&project.id), |id| id == &project.id)
        })
        .collect::<Vec<_>>();
    let mut matched_projects = Vec::new();
    let mut matched_modules = Vec::new();
    for project in projects {
        if matched_projects.len() < MAX_SEARCH_PROJECTS
            && contains(&query, &[&project.name, &project.root_path])
        {
            matched_projects.push(project.clone());
        }
        if matched_modules.len() >= MAX_SEARCH_MODULES {
            continue;
        }
        for module in database.load_inspector_snapshot(&project.id)?.modules {
            if matched_modules.len() >= MAX_SEARCH_MODULES {
                break;
            }
            if contains(&query, &[&module.name, &module.slug, &module.summary]) {
                matched_modules.push(SearchModuleMatch {
                    project_id: project.id.clone(),
                    module_id: module.id,
                    parent_id: module.parent_id,
                    name: module.name,
                    slug: module.slug,
                    summary: module.summary,
                    status: module.status,
                });
            }
        }
    }
    Ok(SearchResults {
        projects: matched_projects,
        modules: matched_modules,
    })
}

fn contains(query: &str, fields: &[&str]) -> bool {
    fields
        .iter()
        .any(|field| field.to_lowercase().contains(query))
}

fn locate_source(
    database: &InspectorDatabase,
    allowed: &HashSet<String>,
    input: SourceLocateInput,
) -> Result<SourceLocateResult, AppError> {
    if input.path.trim().is_empty() {
        return Err(AppError::Validation("path is required".to_string()));
    }
    if input.end_line.is_some() && input.start_line.is_none() {
        return Err(AppError::Validation(
            "startLine is required when endLine is supplied".to_string(),
        ));
    }
    if let (Some(start), Some(end)) = (input.start_line, input.end_line) {
        if start == 0 || end < start {
            return Err(AppError::InvalidLineRange { start, end });
        }
    }
    let (project, relative_path) = match &input.project_id {
        Some(project_id) => {
            let project = database.project(project_id)?;
            let relative = relative_to_project_root(&project.root_path, &input.path)?;
            (project, relative)
        }
        None => {
            let candidate = Path::new(&input.path);
            if !candidate.is_absolute() {
                return Err(AppError::Validation(
                    "an absolute path is required when projectId is omitted".to_string(),
                ));
            }
            let absolute = canonical_or_lexical(candidate);
            database
                .list_projects(false)?
                .into_iter()
                .filter(|project| allowed.contains(&project.id))
                .filter_map(|project| {
                    let root = canonical_root(Path::new(&project.root_path)).ok()?;
                    let relative = absolute.strip_prefix(&root).ok()?.to_path_buf();
                    Some((root.components().count(), project, relative))
                })
                .max_by_key(|(depth, _, _)| *depth)
                .map(|(_, project, relative)| (project, relative))
                .ok_or_else(|| AppError::NotFound(format!("project for path {}", input.path)))?
        }
    };
    let relative_path = validated_relative_path(&relative_path.to_string_lossy())?;
    let snapshot = database.load_inspector_snapshot(&project.id)?;
    let line_range = input
        .start_line
        .map(|start| (start, input.end_line.unwrap_or(start)));
    let modules = snapshot
        .modules
        .into_iter()
        .filter_map(|mut module| {
            module.flows.retain(|flow| {
                flow.source_refs.iter().any(|source| {
                    Path::new(&source.path) == relative_path
                        && line_range.is_none_or(|(start, end)| {
                            source.start_line <= end && source.end_line >= start
                        })
                })
            });
            (!module.flows.is_empty()).then_some(module)
        })
        .collect::<Vec<_>>();
    let related = related_files(
        Path::new(&project.root_path),
        &[relative_path.to_string_lossy().into_owned()],
    )?;
    Ok(SourceLocateResult {
        project,
        relative_path: relative_path.to_string_lossy().into_owned(),
        modules,
        related_files: related,
    })
}

fn relative_to_project_root(root_path: &str, path: &str) -> Result<PathBuf, AppError> {
    let candidate = Path::new(path);
    if !candidate.is_absolute() {
        return validated_relative_path(path);
    }
    let root = canonical_root(Path::new(root_path))?;
    let absolute = canonical_or_lexical(candidate);
    let relative = absolute
        .strip_prefix(&root)
        .map_err(|_| AppError::PathTraversal(path.to_string()))?;
    validated_relative_path(&relative.to_string_lossy())
}

fn canonical_or_lexical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        let mut clean = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => clean.push(prefix.as_os_str()),
                Component::RootDir => clean.push(component.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    clean.pop();
                }
                Component::Normal(part) => clean.push(part),
            }
        }
        clean
    })
}

fn tool_error(error: AppError) -> String {
    serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::inspector::InspectorGraph;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "vibe-inspector-mcp-{label}-{}-{unique}",
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

    fn fixture(label: &str) -> (TestDirectory, InspectorDatabase, String, PathBuf) {
        let directory = TestDirectory::new(label);
        let root = directory.path().join("project");
        fs::create_dir(&root).expect("create root");
        let database = InspectorDatabase::new(directory.path().join("inspector.sqlite3"));
        database.initialize().expect("initialize database");
        let project = database
            .create_project(
                "Fixture",
                root.to_string_lossy().as_ref(),
                "test",
                "mcp",
                "create-project",
            )
            .expect("create project");
        (directory, database, project.id, root)
    }

    fn seed_module_with_source(
        database: &InspectorDatabase,
        project_id: &str,
        root: &Path,
    ) -> String {
        fs::write(root.join("main.rs"), "one\ntwo\nthree\n").expect("write source");
        let module = crud::module_create(
            database,
            crud::ModuleCreateInput {
                request_id: "module-create".to_string(),
                project_id: project_id.to_string(),
                expected_project_revision: 1,
                parent_id: None,
                name: "Inspector".to_string(),
                slug: "inspector".to_string(),
                summary: "Inspector domain.".to_string(),
                status: EntityStatus::Ready,
            },
            "test",
        )
        .expect("module");
        let flow = crud::flow_create(
            database,
            crud::FlowCreateInput {
                request_id: "flow-create".to_string(),
                project_id: project_id.to_string(),
                expected_project_revision: 2,
                module_id: module.entity_id.clone(),
                name: "Read".to_string(),
                summary: "Read evidence.".to_string(),
                status: EntityStatus::Ready,
                graph: InspectorGraph::default(),
            },
            "test",
        )
        .expect("flow");
        crud::source_ref_upsert(
            database,
            crud::SourceRefUpsertInput {
                request_id: "source-create".to_string(),
                project_id: project_id.to_string(),
                expected_project_revision: 3,
                flow_id: flow.entity_id,
                source_ref_id: None,
                expected_revision: None,
                path: "main.rs".to_string(),
                start_line: 1,
                end_line: 2,
                start_column: None,
                end_column: None,
                symbol: None,
                label: None,
            },
            "test",
        )
        .expect("source ref");
        module.entity_id
    }

    #[test]
    fn authorization_requires_an_allow_list() {
        let error = McpAuthorization::new("session", Vec::<String>::new())
            .expect_err("empty allow-list must fail");
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn authorization_rejects_projects_outside_the_session() {
        let authorization =
            McpAuthorization::new("session", ["project-1".to_string()]).expect("authorization");
        assert!(authorization.authorize("project-1").is_ok());
        assert!(matches!(
            authorization.authorize("project-2"),
            Err(AppError::Unauthorized(_))
        ));
    }

    #[test]
    fn local_tray_can_start_before_projects_are_registered() {
        let authorization =
            McpAuthorization::for_local_tray("tray-http:session", Vec::<String>::new())
                .expect("empty local allow-list");
        assert!(authorization
            .allowed_project_ids
            .read()
            .expect("project allow-list lock")
            .is_empty());
    }

    #[test]
    fn local_tray_allows_project_create_for_auto_import() {
        let authorization =
            McpAuthorization::for_local_tray("tray-http:session", Vec::<String>::new())
                .expect("empty local allow-list");
        assert!(authorization.authorize_project_create().is_ok());
    }

    #[test]
    fn auto_registered_projects_join_the_session_allow_list() {
        let authorization =
            McpAuthorization::new("session", ["project-1".to_string()]).expect("authorization");
        assert!(matches!(
            authorization.authorize("project-2"),
            Err(AppError::Unauthorized(_))
        ));
        authorization.allow_project("project-2");
        assert!(authorization.authorize("project-2").is_ok());
    }

    #[test]
    fn search_matches_projects_and_modules_within_the_allow_list() {
        let (_directory, database, project_id, root) = fixture("search");
        seed_module_with_source(&database, &project_id, &root);
        let allowed = HashSet::from([project_id.clone()]);

        let by_module = search_projects_modules(
            &database,
            &allowed,
            SearchInput {
                query: "domain".to_string(),
                project_id: None,
            },
        )
        .expect("search by module");
        assert!(by_module.projects.is_empty());
        assert_eq!(by_module.modules.len(), 1);
        assert_eq!(by_module.modules[0].slug, "inspector");

        let by_project = search_projects_modules(
            &database,
            &allowed,
            SearchInput {
                query: "fixture".to_string(),
                project_id: None,
            },
        )
        .expect("search by project");
        assert_eq!(by_project.projects.len(), 1);
        assert_eq!(by_project.projects[0].id, project_id);

        let scoped = search_projects_modules(
            &database,
            &allowed,
            SearchInput {
                query: "inspector".to_string(),
                project_id: Some(project_id.clone()),
            },
        )
        .expect("scoped search");
        assert_eq!(scoped.modules.len(), 1);

        let not_allowed = search_projects_modules(
            &database,
            &HashSet::new(),
            SearchInput {
                query: "inspector".to_string(),
                project_id: None,
            },
        )
        .expect("search outside the allow-list");
        assert!(not_allowed.projects.is_empty());
        assert!(not_allowed.modules.is_empty());

        let invalid = search_projects_modules(
            &database,
            &allowed,
            SearchInput {
                query: "   ".to_string(),
                project_id: None,
            },
        )
        .expect_err("blank query");
        assert!(matches!(invalid, AppError::Validation(_)));
    }

    #[test]
    fn locate_resolves_a_file_to_its_project_and_modules() {
        let (_directory, database, project_id, root) = fixture("locate");
        let module_id = seed_module_with_source(&database, &project_id, &root);
        let allowed = HashSet::from([project_id.clone()]);
        let absolute = root.join("main.rs").to_string_lossy().into_owned();

        let located = locate_source(
            &database,
            &allowed,
            SourceLocateInput {
                project_id: None,
                path: absolute.clone(),
                start_line: None,
                end_line: None,
            },
        )
        .expect("locate by absolute path");
        assert_eq!(located.project.id, project_id);
        assert_eq!(located.relative_path, "main.rs");
        assert_eq!(located.modules.len(), 1);
        assert_eq!(located.modules[0].id, module_id);
        assert!(located.related_files.is_empty());

        let missed = locate_source(
            &database,
            &allowed,
            SourceLocateInput {
                project_id: None,
                path: absolute,
                start_line: Some(3),
                end_line: Some(3),
            },
        )
        .expect("locate outside the recorded range");
        assert!(missed.modules.is_empty());

        let scoped = locate_source(
            &database,
            &allowed,
            SourceLocateInput {
                project_id: Some(project_id.clone()),
                path: "main.rs".to_string(),
                start_line: None,
                end_line: None,
            },
        )
        .expect("locate by project-relative path");
        assert_eq!(scoped.modules.len(), 1);

        let outside = locate_source(
            &database,
            &allowed,
            SourceLocateInput {
                project_id: None,
                path: "/etc/hosts".to_string(),
                start_line: None,
                end_line: None,
            },
        )
        .expect_err("path outside every allowed root");
        assert!(matches!(outside, AppError::NotFound(_)));

        let traversal = locate_source(
            &database,
            &allowed,
            SourceLocateInput {
                project_id: Some(project_id),
                path: "/etc/hosts".to_string(),
                start_line: None,
                end_line: None,
            },
        )
        .expect_err("absolute path escaping the project root");
        assert!(matches!(traversal, AppError::PathTraversal(_)));

        let missing_start = locate_source(
            &database,
            &allowed,
            SourceLocateInput {
                project_id: None,
                path: "main.rs".to_string(),
                start_line: None,
                end_line: Some(4),
            },
        )
        .expect_err("endLine without startLine");
        assert!(matches!(missing_start, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn version_tools_return_recorded_versions_after_a_module_update() {
        let (_directory, database, project_id, root) = fixture("versions");
        let module_id = seed_module_with_source(&database, &project_id, &root);
        crud::module_update(
            &database,
            crud::ModuleUpdateInput {
                request_id: "module-update".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 4,
                module_id: module_id.clone(),
                expected_revision: 1,
                parent_id: None,
                clear_parent: false,
                name: None,
                slug: None,
                summary: Some("Updated Inspector domain.".to_string()),
                status: None,
            },
            "test",
        )
        .expect("update module");
        let server = VibeInspectorMcp::new(
            database.clone(),
            McpAuthorization::new("session", [project_id.clone()]).expect("authorization"),
        );

        let listed = server
            .version_list(Parameters(VersionListInput {
                request_id: None,
                project_id: project_id.clone(),
                entity_type: Some("module".to_string()),
                entity_id: Some(module_id.clone()),
                limit: None,
            }))
            .await
            .expect("list versions");
        assert_eq!(listed.0.versions.len(), 1);
        let summary = &listed.0.versions[0];
        assert_eq!(summary.entity_id, module_id);
        assert_eq!(summary.revision, 1, "the update records the old snapshot");

        let fetched = server
            .version_get(Parameters(VersionGetInput {
                request_id: None,
                project_id: project_id.clone(),
                version_id: summary.id.clone(),
            }))
            .await
            .expect("get version");
        assert_eq!(fetched.0.version.revision, 1);
        assert_eq!(
            fetched.0.version.snapshot_json["summary"],
            serde_json::json!("Inspector domain."),
            "the stored snapshot predates the update"
        );

        let missing = server
            .version_get(Parameters(VersionGetInput {
                request_id: None,
                project_id,
                version_id: "does-not-exist".to_string(),
            }))
            .await
            .err()
            .expect("unknown version");
        assert!(missing.contains("\"code\":\"not_found\""), "{missing}");
    }

    #[tokio::test]
    async fn version_get_hides_versions_from_other_projects() {
        let (_directory, database, project_id, root) = fixture("version-scope");
        let module_id = seed_module_with_source(&database, &project_id, &root);
        crud::module_update(
            &database,
            crud::ModuleUpdateInput {
                request_id: "module-update".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 4,
                module_id: module_id.clone(),
                expected_revision: 1,
                parent_id: None,
                clear_parent: false,
                name: None,
                slug: None,
                summary: Some("Updated Inspector domain.".to_string()),
                status: None,
            },
            "test",
        )
        .expect("update module");
        let version_id = database
            .list_entity_versions(&project_id, Some("module"), Some(&module_id), 10)
            .expect("versions")[0]
            .id
            .clone();
        let other_root = root.join("other");
        fs::create_dir(&other_root).expect("create other root");
        let other = database
            .create_project(
                "Other",
                other_root.to_string_lossy().as_ref(),
                "test",
                "mcp",
                "create-other",
            )
            .expect("create other project");
        let server = VibeInspectorMcp::new(
            database,
            McpAuthorization::new("session", [project_id.clone(), other.id.clone()])
                .expect("authorization"),
        );

        let hidden = server
            .version_get(Parameters(VersionGetInput {
                request_id: None,
                project_id: other.id,
                version_id,
            }))
            .await
            .err()
            .expect("cross-project version");
        assert!(hidden.contains("\"code\":\"not_found\""), "{hidden}");
    }

    #[tokio::test]
    async fn issue_tools_reject_projects_outside_the_allow_list() {
        let (_directory, database, project_id, _root) = fixture("issue-scope");
        let server = VibeInspectorMcp::new(
            database,
            McpAuthorization::new("session", ["other-project".to_string()])
                .expect("authorization"),
        );

        let denied = server
            .issue_list(Parameters(IssueListInput {
                request_id: None,
                project_id: project_id.clone(),
                module_id: None,
                status: None,
                limit: None,
            }))
            .await
            .err()
            .expect("unauthorized project");
        assert!(denied.contains("\"code\":\"unauthorized\""), "{denied}");

        let denied = server
            .issue_create(Parameters(crud::IssueCreateInput {
                request_id: "issue-create".to_string(),
                project_id,
                module_id: "module-1".to_string(),
                flow_id: None,
                title: "Denied".to_string(),
                description: None,
                severity: crate::storage::issues::IssueSeverity::P1,
                status: None,
                path: None,
                start_line: None,
                end_line: None,
                source: None,
            }))
            .await
            .err()
            .expect("unauthorized project");
        assert!(denied.contains("\"code\":\"unauthorized\""), "{denied}");
    }

    #[test]
    fn tool_router_exposes_the_complete_public_contract() {
        let authorization =
            McpAuthorization::new("session", ["project-1".to_string()]).expect("authorization");
        let server = VibeInspectorMcp::new(
            InspectorDatabase::new(std::env::temp_dir().join("unused-inspector.sqlite3")),
            authorization,
        );
        let names = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect::<HashSet<_>>();
        assert_eq!(names.len(), 30);
        for required in [
            "inspector.project.list",
            "inspector.project.create",
            "inspector.module.archive",
            "inspector.flow.update",
            "inspector.sourceRef.upsert",
            "inspector.explanation.upsert",
            "inspector.command.run",
            "inspector.search",
            "inspector.source.locate",
            "inspector.source.related",
            "inspector.version.list",
            "inspector.version.get",
            "inspector.issue.list",
            "inspector.issue.create",
            "inspector.issue.update",
            "inspector.issue.archive",
        ] {
            assert!(names.contains(required), "missing MCP tool {required}");
        }
    }
}
