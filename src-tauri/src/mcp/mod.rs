mod crud;

use std::{collections::HashSet, time::Instant};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, Json, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    command_service::{run_inspector_command, RunInspectorCommandInput, RunInspectorCommandResult},
    error::AppError,
    fs_broker::{read_source_range, ReadSourceInput, SourceRange},
    inspector::{InspectorFlow, InspectorModule, InspectorSnapshot},
    storage::{commands::StoredCommandRun, mcp::McpAuditEvent, InspectorDatabase, StoredProject},
};

#[derive(Debug, Clone)]
pub struct McpAuthorization {
    session_id: String,
    allowed_project_ids: HashSet<String>,
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
            allowed_project_ids,
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
            allowed_project_ids: allowed_project_ids.into_iter().collect(),
            allow_project_create: false,
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
        if self.allowed_project_ids.contains(project_id) {
            Ok(())
        } else {
            Err(AppError::Unauthorized(format!("project {project_id}")))
        }
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

#[tool_router(router = tool_router)]
impl VibeInspectorMcp {
    #[tool(
        name = "inspector.project.list",
        description = "List Inspector projects allowed for this authenticated MCP session"
    )]
    async fn project_list(&self) -> Result<Json<ProjectList>, String> {
        let allowed = self.authorization.allowed_project_ids.clone();
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
        description = "Get one active Inspector module"
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
        description = "Register one recognized local project root when this session has project-create capability"
    )]
    async fn project_create(
        &self,
        Parameters(input): Parameters<crud::ProjectCreateInput>,
    ) -> Result<Json<crud::ProjectMutationResult>, String> {
        self.authorization
            .authorize_project_create()
            .map_err(tool_error)?;
        let actor = format!("mcp:{}", self.authorization.session_id);
        self.execute(
            "inspector.project.create",
            None,
            Vec::new(),
            move |database| crud::project_create(database, input, &actor),
        )
        .await
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
        description = "Create a module with a server-generated UUIDv7 ID"
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
                "Local-only repository inspection. All project reads require session allow-list authorization; source files are read-only and bounded.",
            )
    }
}

fn tool_error(error: AppError) -> String {
    serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(authorization.allowed_project_ids.is_empty());
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
        assert_eq!(names.len(), 21);
        for required in [
            "inspector.project.list",
            "inspector.project.create",
            "inspector.module.archive",
            "inspector.flow.update",
            "inspector.sourceRef.upsert",
            "inspector.explanation.upsert",
            "inspector.command.run",
        ] {
            assert!(names.contains(required), "missing MCP tool {required}");
        }
    }
}
