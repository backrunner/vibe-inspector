use std::{collections::HashSet, path::Path};

use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::AppError,
    fs_broker::{read_source_range, ReadSourceInput, SourceRangeStatus},
    inspector::{
        EntityStatus, InspectorExplanation, InspectorFlow, InspectorGraph, InspectorModule,
        InspectorSnapshot, InspectorSourceRef, Provenance, ProvenanceSource, SourceRefStatus,
    },
    project_discovery::{discover_projects, DiscoverProjectsInput, DiscoveryMode},
    storage::{
        issues::{Issue, IssuePatch, IssueSeverity, IssueStatus},
        InspectorDatabase, ProjectStatus, StoredProject,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMutationResult {
    pub request_id: String,
    pub project: StoredProject,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorMutationResult {
    pub request_id: String,
    pub project_revision: u64,
    pub entity_id: String,
    pub snapshot: InspectorSnapshot,
    /// True when the mutation matched an existing entity (for example a module
    /// whose slug already exists) and returned it unchanged instead of writing.
    #[serde(default)]
    pub reused_existing: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateInput {
    pub request_id: String,
    pub root_path: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdateInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_revision: u64,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectArchiveInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleCreateInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_project_revision: u64,
    pub parent_id: Option<String>,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub status: EntityStatus,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleUpdateInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_project_revision: u64,
    pub module_id: String,
    pub expected_revision: u64,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub clear_parent: bool,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub summary: Option<String>,
    pub status: Option<EntityStatus>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlowCreateInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_project_revision: u64,
    pub module_id: String,
    pub name: String,
    pub summary: String,
    pub status: EntityStatus,
    #[serde(default)]
    pub graph: InspectorGraph,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlowUpdateInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_project_revision: u64,
    pub flow_id: String,
    pub expected_revision: u64,
    pub module_id: Option<String>,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub status: Option<EntityStatus>,
    pub graph: Option<InspectorGraph>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityArchiveInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_project_revision: u64,
    pub entity_id: String,
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceRefUpsertInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_project_revision: u64,
    pub flow_id: String,
    pub source_ref_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_column: Option<usize>,
    pub end_column: Option<usize>,
    pub symbol: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationUpsertInput {
    pub request_id: String,
    pub project_id: String,
    pub expected_project_revision: u64,
    pub flow_id: String,
    pub explanation_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub headline: String,
    pub body: String,
    #[serde(default)]
    pub decisions: Vec<String>,
    pub confidence: Option<f64>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueMutationResult {
    pub request_id: String,
    pub issue_id: String,
    pub issue: Issue,
    /// Issues never reuse existing rows; this stays false and exists for
    /// parity with the other mutation results.
    #[serde(default)]
    pub reused_existing: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueCreateInput {
    pub request_id: String,
    pub project_id: String,
    pub module_id: String,
    pub flow_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub severity: IssueSeverity,
    pub status: Option<IssueStatus>,
    pub path: Option<String>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueUpdateInput {
    pub request_id: String,
    pub project_id: String,
    pub issue_id: String,
    pub expected_revision: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub severity: Option<IssueSeverity>,
    pub status: Option<IssueStatus>,
    /// Outer None leaves the field untouched; Some(None) clears it.
    #[serde(default)]
    pub flow_id: Option<Option<String>>,
    #[serde(default)]
    pub path: Option<Option<String>>,
    #[serde(default)]
    pub start_line: Option<Option<usize>>,
    #[serde(default)]
    pub end_line: Option<Option<usize>>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueArchiveInput {
    pub request_id: String,
    pub project_id: String,
    pub issue_id: String,
    pub expected_revision: u64,
}

pub fn project_create(
    database: &InspectorDatabase,
    input: ProjectCreateInput,
    actor: &str,
) -> Result<ProjectMutationResult, AppError> {
    if let Some(cached) = cached_result(
        database,
        &input.request_id,
        "inspector.project.create",
        None,
    )? {
        return Ok(cached);
    }
    let discovery = discover_projects(DiscoverProjectsInput {
        root_path: input.root_path,
        mode: DiscoveryMode::SingleProject,
    })?;
    let candidate = discovery
        .candidates
        .into_iter()
        .next()
        .ok_or(AppError::NotProjectRoot(discovery.selected_root))?;
    let name = input.name.as_deref().unwrap_or(&candidate.name);
    let project =
        database.create_project(name, &candidate.root_path, actor, "mcp", &input.request_id)?;
    let result = ProjectMutationResult {
        request_id: input.request_id.clone(),
        project,
    };
    save_result(
        database,
        &input.request_id,
        &result.project.id,
        "inspector.project.create",
        actor,
        &result,
    )?;
    Ok(result)
}

pub fn project_update(
    database: &InspectorDatabase,
    input: ProjectUpdateInput,
    actor: &str,
) -> Result<ProjectMutationResult, AppError> {
    if let Some(cached) = cached_result(
        database,
        &input.request_id,
        "inspector.project.update",
        Some(&input.project_id),
    )? {
        return Ok(cached);
    }
    let project = database.update_project(
        &input.project_id,
        input.expected_revision,
        input.name.as_deref(),
        None,
        actor,
        "mcp",
        &input.request_id,
    )?;
    let result = ProjectMutationResult {
        request_id: input.request_id.clone(),
        project,
    };
    save_result(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.project.update",
        actor,
        &result,
    )?;
    Ok(result)
}

pub fn project_archive(
    database: &InspectorDatabase,
    input: ProjectArchiveInput,
    actor: &str,
) -> Result<ProjectMutationResult, AppError> {
    if let Some(cached) = cached_result(
        database,
        &input.request_id,
        "inspector.project.archive",
        Some(&input.project_id),
    )? {
        return Ok(cached);
    }
    let project = database.update_project(
        &input.project_id,
        input.expected_revision,
        None,
        Some(ProjectStatus::Archived),
        actor,
        "mcp",
        &input.request_id,
    )?;
    let result = ProjectMutationResult {
        request_id: input.request_id.clone(),
        project,
    };
    save_result(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.project.archive",
        actor,
        &result,
    )?;
    Ok(result)
}

pub fn module_create(
    database: &InspectorDatabase,
    input: ModuleCreateInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.module.create",
        actor,
        || {
            validate_active_status(input.status)?;
            validate_slug(&input.slug)?;
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            if let Some(existing_id) = snapshot
                .modules
                .iter()
                .find(|module| module.slug == input.slug)
                .map(|module| module.id.clone())
            {
                return Ok(InspectorMutationResult {
                    request_id: input.request_id.clone(),
                    project_revision: snapshot.project_revision,
                    entity_id: existing_id,
                    snapshot,
                    reused_existing: true,
                });
            }
            if let Some(parent_id) = &input.parent_id {
                require_module(&snapshot, parent_id)?;
            }
            let id = Uuid::now_v7().to_string();
            let now = timestamp();
            snapshot.modules.push(InspectorModule {
                id: id.clone(),
                project_id: input.project_id.clone(),
                parent_id: input.parent_id,
                name: non_empty(input.name, "module name")?,
                slug: input.slug,
                summary: non_empty(input.summary, "module summary")?,
                status: input.status,
                revision: 1,
                created_at: now.clone(),
                updated_at: now,
                flows: Vec::new(),
            });
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.module.create",
                &input.request_id,
                id,
            )
        },
    )
}

pub fn module_update(
    database: &InspectorDatabase,
    input: ModuleUpdateInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.module.update",
        actor,
        || {
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            if let Some(parent_id) = &input.parent_id {
                require_module(&snapshot, parent_id)?;
                if parent_id == &input.module_id {
                    return Err(AppError::Validation(
                        "a module cannot parent itself".to_string(),
                    ));
                }
            }
            let module = require_module_mut(&mut snapshot, &input.module_id)?;
            require_revision(&input.module_id, input.expected_revision, module.revision)?;
            if let Some(status) = input.status {
                validate_active_status(status)?;
                module.status = status;
            }
            if let Some(name) = input.name {
                module.name = non_empty(name, "module name")?;
            }
            if let Some(slug) = input.slug {
                validate_slug(&slug)?;
                module.slug = slug;
            }
            if let Some(summary) = input.summary {
                module.summary = non_empty(summary, "module summary")?;
            }
            if input.clear_parent {
                module.parent_id = None;
            } else if input.parent_id.is_some() {
                module.parent_id = input.parent_id;
            }
            module.revision = next_revision(module.revision)?;
            module.updated_at = timestamp();
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.module.update",
                &input.request_id,
                input.module_id,
            )
        },
    )
}

pub fn module_archive(
    database: &InspectorDatabase,
    input: EntityArchiveInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.module.archive",
        actor,
        || {
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            let module = require_module(&snapshot, &input.entity_id)?;
            require_revision(&input.entity_id, input.expected_revision, module.revision)?;
            let mut removed = HashSet::from([input.entity_id.clone()]);
            loop {
                let before = removed.len();
                for module in &snapshot.modules {
                    if module
                        .parent_id
                        .as_ref()
                        .is_some_and(|parent| removed.contains(parent))
                    {
                        removed.insert(module.id.clone());
                    }
                }
                if removed.len() == before {
                    break;
                }
            }
            snapshot
                .modules
                .retain(|module| !removed.contains(&module.id));
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.module.archive",
                &input.request_id,
                input.entity_id,
            )
        },
    )
}

pub fn flow_create(
    database: &InspectorDatabase,
    input: FlowCreateInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.flow.create",
        actor,
        || {
            validate_active_status(input.status)?;
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            let id = Uuid::now_v7().to_string();
            let now = timestamp();
            let module = require_module_mut(&mut snapshot, &input.module_id)?;
            module.flows.push(InspectorFlow {
                id: id.clone(),
                project_id: input.project_id.clone(),
                module_id: input.module_id,
                name: non_empty(input.name, "flow name")?,
                summary: non_empty(input.summary, "flow summary")?,
                status: input.status,
                revision: 1,
                created_at: now.clone(),
                updated_at: now,
                graph: input.graph,
                explanation: None,
                source_refs: Vec::new(),
            });
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.flow.create",
                &input.request_id,
                id,
            )
        },
    )
}

pub fn flow_update(
    database: &InspectorDatabase,
    input: FlowUpdateInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.flow.update",
        actor,
        || {
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            if let Some(module_id) = &input.module_id {
                require_module(&snapshot, module_id)?;
            }
            let (module_index, flow_index) = flow_position(&snapshot, &input.flow_id)?;
            let mut flow = snapshot.modules[module_index].flows.remove(flow_index);
            require_revision(&input.flow_id, input.expected_revision, flow.revision)?;
            if let Some(status) = input.status {
                validate_active_status(status)?;
                flow.status = status;
            }
            if let Some(name) = input.name {
                flow.name = non_empty(name, "flow name")?;
            }
            if let Some(summary) = input.summary {
                flow.summary = non_empty(summary, "flow summary")?;
            }
            if let Some(graph) = input.graph {
                flow.graph = graph;
            }
            let destination = input.module_id.unwrap_or_else(|| flow.module_id.clone());
            flow.module_id = destination.clone();
            flow.revision = next_revision(flow.revision)?;
            flow.updated_at = timestamp();
            require_module_mut(&mut snapshot, &destination)?
                .flows
                .push(flow);
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.flow.update",
                &input.request_id,
                input.flow_id,
            )
        },
    )
}

pub fn flow_archive(
    database: &InspectorDatabase,
    input: EntityArchiveInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.flow.archive",
        actor,
        || {
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            let (module_index, flow_index) = flow_position(&snapshot, &input.entity_id)?;
            require_revision(
                &input.entity_id,
                input.expected_revision,
                snapshot.modules[module_index].flows[flow_index].revision,
            )?;
            snapshot.modules[module_index].flows.remove(flow_index);
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.flow.archive",
                &input.request_id,
                input.entity_id,
            )
        },
    )
}

pub fn source_ref_upsert(
    database: &InspectorDatabase,
    input: SourceRefUpsertInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.sourceRef.upsert",
        actor,
        || {
            let project = database.project(&input.project_id)?;
            let range = read_source_range(
                Path::new(&project.root_path),
                ReadSourceInput {
                    relative_path: input.path.clone(),
                    start_line: input.start_line,
                    end_line: input.end_line,
                    expected_hash: None,
                },
            )?;
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            let (_, _, flow) = require_flow_mut(&mut snapshot, &input.flow_id)?;
            let now = timestamp();
            let id = if let Some(id) = input.source_ref_id {
                let source = flow
                    .source_refs
                    .iter_mut()
                    .find(|source| source.id == id)
                    .ok_or_else(|| AppError::NotFound(format!("source reference {id}")))?;
                let expected = input.expected_revision.ok_or_else(|| {
                    AppError::Validation(
                        "expectedRevision is required for source updates".to_string(),
                    )
                })?;
                require_revision(&id, expected, source.revision)?;
                source.path = range.relative_path;
                source.start_line = range.start_line;
                source.end_line = range.end_line;
                source.start_column = input.start_column;
                source.end_column = input.end_column;
                source.symbol = input.symbol;
                source.label = input.label;
                source.content_hash = range.content_hash;
                source.status = source_status(range.status);
                source.revision = next_revision(source.revision)?;
                source.updated_at = now;
                id
            } else {
                if input.expected_revision.is_some() {
                    return Err(AppError::Validation(
                        "expectedRevision is only valid when sourceRefId is supplied".to_string(),
                    ));
                }
                let id = Uuid::now_v7().to_string();
                flow.source_refs.push(InspectorSourceRef {
                    id: id.clone(),
                    project_id: input.project_id.clone(),
                    flow_id: input.flow_id.clone(),
                    path: range.relative_path,
                    start_line: range.start_line,
                    end_line: range.end_line,
                    start_column: input.start_column,
                    end_column: input.end_column,
                    symbol: input.symbol,
                    label: input.label,
                    content_hash: range.content_hash,
                    status: source_status(range.status),
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now,
                });
                id
            };
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.sourceRef.upsert",
                &input.request_id,
                id,
            )
        },
    )
}

pub fn source_ref_archive(
    database: &InspectorDatabase,
    input: EntityArchiveInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.sourceRef.archive",
        actor,
        || {
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            let mut found = false;
            for flow in snapshot
                .modules
                .iter_mut()
                .flat_map(|module| &mut module.flows)
            {
                if let Some(index) = flow
                    .source_refs
                    .iter()
                    .position(|source| source.id == input.entity_id)
                {
                    require_revision(
                        &input.entity_id,
                        input.expected_revision,
                        flow.source_refs[index].revision,
                    )?;
                    flow.source_refs.remove(index);
                    found = true;
                    for node in &mut flow.graph.nodes {
                        node.source_ref_ids.retain(|id| id != &input.entity_id);
                    }
                    if let Some(explanation) = &mut flow.explanation {
                        explanation
                            .provenance
                            .evidence_ids
                            .retain(|id| id != &input.entity_id);
                    }
                    break;
                }
            }
            if !found {
                return Err(AppError::NotFound(format!(
                    "source reference {}",
                    input.entity_id
                )));
            }
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.sourceRef.archive",
                &input.request_id,
                input.entity_id,
            )
        },
    )
}

pub fn explanation_upsert(
    database: &InspectorDatabase,
    input: ExplanationUpsertInput,
    actor: &str,
) -> Result<InspectorMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.explanation.upsert",
        actor,
        || {
            let mut snapshot = database.load_inspector_snapshot(&input.project_id)?;
            let (_, _, flow) = require_flow_mut(&mut snapshot, &input.flow_id)?;
            let source_ids = flow
                .source_refs
                .iter()
                .map(|source| source.id.as_str())
                .collect::<HashSet<_>>();
            if input
                .evidence_ids
                .iter()
                .any(|id| !source_ids.contains(id.as_str()))
            {
                return Err(AppError::Validation(
                    "explanation evidence must belong to the selected flow".to_string(),
                ));
            }
            let now = timestamp();
            let id = if let Some(id) = input.explanation_id {
                let explanation = flow
                    .explanation
                    .as_mut()
                    .filter(|item| item.id == id)
                    .ok_or_else(|| AppError::NotFound(format!("explanation {id}")))?;
                let expected = input.expected_revision.ok_or_else(|| {
                    AppError::Validation(
                        "expectedRevision is required for explanation updates".to_string(),
                    )
                })?;
                require_revision(&id, expected, explanation.revision)?;
                explanation.headline = non_empty(input.headline, "explanation headline")?;
                explanation.body = non_empty(input.body, "explanation body")?;
                explanation.decisions = input.decisions;
                explanation.confidence = input.confidence;
                explanation.provenance = mcp_provenance(input.evidence_ids, &now);
                explanation.revision = next_revision(explanation.revision)?;
                explanation.updated_at = now;
                id
            } else {
                if input.expected_revision.is_some() {
                    return Err(AppError::Validation(
                        "expectedRevision is only valid when explanationId is supplied".to_string(),
                    ));
                }
                let id = Uuid::now_v7().to_string();
                flow.explanation = Some(InspectorExplanation {
                    id: id.clone(),
                    project_id: input.project_id.clone(),
                    flow_id: input.flow_id.clone(),
                    headline: non_empty(input.headline, "explanation headline")?,
                    body: non_empty(input.body, "explanation body")?,
                    decisions: input.decisions,
                    confidence: input.confidence,
                    provenance: mcp_provenance(input.evidence_ids, &now),
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now,
                });
                id
            };
            commit(
                database,
                snapshot,
                input.expected_project_revision,
                actor,
                "inspector.explanation.upsert",
                &input.request_id,
                id,
            )
        },
    )
}

pub fn issue_create(
    database: &InspectorDatabase,
    input: IssueCreateInput,
    actor: &str,
) -> Result<IssueMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.issue.create",
        actor,
        || {
            let module_revision = database
                .load_inspector_snapshot(&input.project_id)?
                .modules
                .iter()
                .find(|module| module.id == input.module_id)
                .map(|module| module.revision);
            let issue = database.create_issue(
                &input.project_id,
                &input.module_id,
                input.flow_id.as_deref(),
                module_revision,
                &input.title,
                input.description.as_deref().unwrap_or_default(),
                input.severity,
                input.status,
                input.path.as_deref(),
                input.start_line,
                input.end_line,
                actor,
                issue_source(input.source.as_deref()),
                Some(&input.request_id),
            )?;
            Ok(issue_result(&input.request_id, issue))
        },
    )
}

pub fn issue_update(
    database: &InspectorDatabase,
    input: IssueUpdateInput,
    actor: &str,
) -> Result<IssueMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.issue.update",
        actor,
        || {
            require_project_issue(database, &input.project_id, &input.issue_id)?;
            let patch = IssuePatch {
                title: input.title.clone(),
                description: input.description.clone(),
                severity: input.severity,
                status: input.status,
                flow_id: input.flow_id.clone(),
                path: input.path.clone(),
                start_line: input.start_line,
                end_line: input.end_line,
            };
            let issue = database.update_issue(
                &input.issue_id,
                input.expected_revision,
                &patch,
                actor,
                "mcp",
                Some(&input.request_id),
            )?;
            Ok(issue_result(&input.request_id, issue))
        },
    )
}

pub fn issue_archive(
    database: &InspectorDatabase,
    input: IssueArchiveInput,
    actor: &str,
) -> Result<IssueMutationResult, AppError> {
    mutation(
        database,
        &input.request_id,
        &input.project_id,
        "inspector.issue.archive",
        actor,
        || {
            require_project_issue(database, &input.project_id, &input.issue_id)?;
            let issue = database.archive_issue(
                &input.issue_id,
                input.expected_revision,
                actor,
                "mcp",
                Some(&input.request_id),
            )?;
            Ok(issue_result(&input.request_id, issue))
        },
    )
}

fn issue_result(request_id: &str, issue: Issue) -> IssueMutationResult {
    IssueMutationResult {
        request_id: request_id.to_string(),
        issue_id: issue.id.clone(),
        issue,
        reused_existing: false,
    }
}

fn issue_source(source: Option<&str>) -> &str {
    source
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp")
}

fn require_project_issue(
    database: &InspectorDatabase,
    project_id: &str,
    issue_id: &str,
) -> Result<(), AppError> {
    let issue = database.get_issue(issue_id)?;
    if issue.project_id == project_id {
        Ok(())
    } else {
        // Cross-project issues are hidden behind NotFound so one session
        // cannot probe another project's issue IDs.
        Err(AppError::NotFound(format!("issue {issue_id}")))
    }
}

fn mutation<T, F>(
    database: &InspectorDatabase,
    request_id: &str,
    project_id: &str,
    tool: &str,
    actor: &str,
    operation: F,
) -> Result<T, AppError>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Result<T, AppError>,
{
    if let Some(result) = cached_result(database, request_id, tool, Some(project_id))? {
        return Ok(result);
    }
    let result = operation()?;
    save_result(database, request_id, project_id, tool, actor, &result)?;
    Ok(result)
}

fn cached_result<T: DeserializeOwned>(
    database: &InspectorDatabase,
    request_id: &str,
    tool: &str,
    project_id: Option<&str>,
) -> Result<Option<T>, AppError> {
    let Some(stored) = database.command_run(request_id)? else {
        return Ok(None);
    };
    if stored.command != tool || project_id.is_some_and(|id| id != stored.project_id) {
        return Err(AppError::Validation(format!(
            "requestId {request_id} was already used for another mutation"
        )));
    }
    stored
        .result_json
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| AppError::Database(error.to_string()))
        })
        .transpose()
}

fn save_result<T: Serialize>(
    database: &InspectorDatabase,
    request_id: &str,
    project_id: &str,
    tool: &str,
    actor: &str,
    result: &T,
) -> Result<(), AppError> {
    database.save_command_run(
        request_id,
        project_id,
        tool,
        "mcp-crud",
        actor,
        "committed",
        &serde_json::to_string(result).map_err(|error| AppError::Database(error.to_string()))?,
        None,
    )
}

fn commit(
    database: &InspectorDatabase,
    snapshot: InspectorSnapshot,
    expected_revision: u64,
    actor: &str,
    _tool: &str,
    request_id: &str,
    entity_id: String,
) -> Result<InspectorMutationResult, AppError> {
    let committed = database.commit_inspector_snapshot(
        &snapshot,
        expected_revision,
        actor,
        "mcp",
        request_id,
    )?;
    Ok(InspectorMutationResult {
        request_id: request_id.to_string(),
        project_revision: committed.project_revision,
        entity_id,
        snapshot: committed,
        reused_existing: false,
    })
}

fn require_module<'a>(
    snapshot: &'a InspectorSnapshot,
    id: &str,
) -> Result<&'a InspectorModule, AppError> {
    snapshot
        .modules
        .iter()
        .find(|module| module.id == id)
        .ok_or_else(|| AppError::NotFound(format!("module {id}")))
}

fn require_module_mut<'a>(
    snapshot: &'a mut InspectorSnapshot,
    id: &str,
) -> Result<&'a mut InspectorModule, AppError> {
    snapshot
        .modules
        .iter_mut()
        .find(|module| module.id == id)
        .ok_or_else(|| AppError::NotFound(format!("module {id}")))
}

fn flow_position(snapshot: &InspectorSnapshot, id: &str) -> Result<(usize, usize), AppError> {
    for (module_index, module) in snapshot.modules.iter().enumerate() {
        if let Some(flow_index) = module.flows.iter().position(|flow| flow.id == id) {
            return Ok((module_index, flow_index));
        }
    }
    Err(AppError::NotFound(format!("flow {id}")))
}

fn require_flow_mut<'a>(
    snapshot: &'a mut InspectorSnapshot,
    id: &str,
) -> Result<(usize, usize, &'a mut InspectorFlow), AppError> {
    let (module_index, flow_index) = flow_position(snapshot, id)?;
    let flow = &mut snapshot.modules[module_index].flows[flow_index];
    Ok((module_index, flow_index, flow))
}

fn require_revision(id: &str, expected: u64, actual: u64) -> Result<(), AppError> {
    if expected == actual {
        Ok(())
    } else {
        Err(AppError::RevisionConflict {
            entity_id: id.to_string(),
            expected,
            actual,
        })
    }
}

fn validate_active_status(status: EntityStatus) -> Result<(), AppError> {
    if status == EntityStatus::Archived {
        Err(AppError::Validation(
            "use the archive tool instead of archived status".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_slug(slug: &str) -> Result<(), AppError> {
    if slug.is_empty()
        || slug.len() > 120
        || slug.starts_with('-')
        || slug.ends_with('-')
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Err(AppError::Validation(
            "module slug must use lowercase letters, digits, and internal hyphens".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn non_empty(value: String, field: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 20_000 {
        Err(AppError::Validation(format!("invalid {field}")))
    } else {
        Ok(value.to_string())
    }
}

fn next_revision(revision: u64) -> Result<u64, AppError> {
    revision
        .checked_add(1)
        .ok_or_else(|| AppError::Database("entity revision overflow".to_string()))
}

fn source_status(status: SourceRangeStatus) -> SourceRefStatus {
    match status {
        SourceRangeStatus::Current => SourceRefStatus::Current,
        SourceRangeStatus::Stale => SourceRefStatus::Stale,
    }
}

fn mcp_provenance(evidence_ids: Vec<String>, generated_at: &str) -> Provenance {
    Provenance {
        source: ProvenanceSource::Mcp,
        conversation_id: None,
        model: None,
        prompt_hash: None,
        generated_at: generated_at.to_string(),
        confidence: None,
        evidence_ids,
    }
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
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
                "vibe-inspector-mcp-crud-{label}-{}-{unique}",
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

    fn setup(label: &str) -> (TestDirectory, InspectorDatabase, String, PathBuf) {
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

    fn renderable_graph() -> InspectorGraph {
        let start_id = Uuid::now_v7().to_string();
        let end_id = Uuid::now_v7().to_string();
        InspectorGraph {
            kind: crate::inspector::DiagramKind::Workflow,
            nodes: vec![
                crate::inspector::InspectorFlowNode {
                    id: start_id.clone(),
                    label: "Start".to_string(),
                    kind: crate::inspector::FlowNodeKind::Start,
                    description: None,
                    source_ref_ids: vec![],
                    position: 0,
                },
                crate::inspector::InspectorFlowNode {
                    id: end_id.clone(),
                    label: "Complete".to_string(),
                    kind: crate::inspector::FlowNodeKind::End,
                    description: None,
                    source_ref_ids: vec![],
                    position: 1,
                },
            ],
            edges: vec![crate::inspector::InspectorFlowEdge {
                id: Uuid::now_v7().to_string(),
                from: start_id,
                to: end_id,
                label: None,
                kind: crate::inspector::FlowEdgeKind::Control,
                mechanism: None,
                source_ref_ids: vec![],
            }],
        }
    }

    #[test]
    fn project_create_is_idempotent_for_a_recognized_root() {
        let directory = TestDirectory::new("project-create");
        let root = directory.path().join("project");
        fs::create_dir(&root).expect("create root");
        fs::create_dir(root.join(".git")).expect("create git marker");
        let database = InspectorDatabase::new(directory.path().join("inspector.sqlite3"));
        database.initialize().expect("initialize database");
        let input = ProjectCreateInput {
            request_id: "project-request".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            name: Some("MCP Fixture".to_string()),
        };

        let created = project_create(&database, input.clone(), "mcp:test").expect("create");
        let replayed = project_create(&database, input, "mcp:test").expect("replay");

        assert_eq!(created.project.id, replayed.project.id);
        assert_eq!(database.list_projects(false).expect("projects").len(), 1);
        assert_eq!(
            database
                .list_command_runs(&created.project.id, 10)
                .expect("runs")
                .len(),
            1
        );
    }

    #[test]
    fn module_and_flow_crud_enforce_revisions_and_replay() {
        let (_directory, database, project_id, _root) = setup("entities");
        let module = module_create(
            &database,
            ModuleCreateInput {
                request_id: "module-create".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 1,
                parent_id: None,
                name: "Inspector".to_string(),
                slug: "inspector".to_string(),
                summary: "Inspector domain.".to_string(),
                status: EntityStatus::Ready,
            },
            "mcp:test",
        )
        .expect("create module");
        assert_eq!(module.project_revision, 2);
        let module_id = module.entity_id.clone();

        let update_input = ModuleUpdateInput {
            request_id: "module-update".to_string(),
            project_id: project_id.clone(),
            expected_project_revision: 2,
            module_id: module_id.clone(),
            expected_revision: 1,
            parent_id: None,
            clear_parent: false,
            name: None,
            slug: None,
            summary: Some("Versioned Inspector domain.".to_string()),
            status: None,
        };
        let updated =
            module_update(&database, update_input.clone(), "mcp:test").expect("update module");
        let replayed = module_update(&database, update_input, "mcp:test").expect("replay update");
        assert_eq!(updated.project_revision, 3);
        assert_eq!(replayed.project_revision, 3);
        assert_eq!(
            updated.snapshot.modules[0].revision, 2,
            "entity revision increments independently"
        );

        let conflict = module_update(
            &database,
            ModuleUpdateInput {
                request_id: "module-conflict".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 3,
                module_id: module_id.clone(),
                expected_revision: 1,
                parent_id: None,
                clear_parent: false,
                name: Some("Wrong".to_string()),
                slug: None,
                summary: None,
                status: None,
            },
            "mcp:test",
        )
        .expect_err("entity revision conflict");
        assert!(matches!(conflict, AppError::RevisionConflict { .. }));

        let flow = flow_create(
            &database,
            FlowCreateInput {
                request_id: "flow-create".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 3,
                module_id: module_id.clone(),
                name: "Refresh".to_string(),
                summary: "Refresh evidence.".to_string(),
                status: EntityStatus::Ready,
                graph: renderable_graph(),
            },
            "mcp:test",
        )
        .expect("create flow");
        assert_eq!(flow.project_revision, 4);
        let archived = flow_archive(
            &database,
            EntityArchiveInput {
                request_id: "flow-archive".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 4,
                entity_id: flow.entity_id,
                expected_revision: 1,
            },
            "mcp:test",
        )
        .expect("archive flow");
        assert!(archived.snapshot.modules[0].flows.is_empty());
    }

    #[test]
    fn project_create_revives_an_archived_project_with_the_same_root() {
        let (_directory, database, project_id, root) = setup("project-revive");
        let archived = database
            .update_project(
                &project_id,
                1,
                None,
                Some(ProjectStatus::Archived),
                "test",
                "mcp",
                "archive-project",
            )
            .expect("archive project");
        assert!(archived.archived_at.is_some());
        assert!(database.list_projects(false).expect("active").is_empty());

        let revived = database
            .create_project(
                "Fixture Revived",
                root.to_string_lossy().as_ref(),
                "test",
                "mcp",
                "revive-project",
            )
            .expect("revive archived project");

        assert_eq!(revived.id, project_id, "the same identity is kept");
        assert_eq!(revived.name, "Fixture Revived");
        assert_eq!(revived.revision, 3);
        assert!(revived.archived_at.is_none());
        assert_eq!(revived.status, ProjectStatus::Ready);
        assert_eq!(database.list_projects(false).expect("active").len(), 1);
    }

    #[test]
    fn module_create_reuses_the_active_module_with_the_same_slug() {
        let (_directory, database, project_id, _root) = setup("module-dedup");
        let input = ModuleCreateInput {
            request_id: "module-create".to_string(),
            project_id: project_id.clone(),
            expected_project_revision: 1,
            parent_id: None,
            name: "Inspector".to_string(),
            slug: "inspector".to_string(),
            summary: "Inspector domain.".to_string(),
            status: EntityStatus::Ready,
        };
        let created = module_create(&database, input, "mcp:test").expect("create module");
        assert!(!created.reused_existing);
        assert_eq!(created.project_revision, 2);

        let duplicate = module_create(
            &database,
            ModuleCreateInput {
                request_id: "module-create-again".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 2,
                parent_id: None,
                name: "Inspector duplicate".to_string(),
                slug: "inspector".to_string(),
                summary: "Accidental duplicate.".to_string(),
                status: EntityStatus::Ready,
            },
            "mcp:test",
        )
        .expect("duplicate slug reuses the existing module");

        assert!(duplicate.reused_existing);
        assert_eq!(duplicate.entity_id, created.entity_id);
        assert_eq!(
            duplicate.project_revision, created.project_revision,
            "a reused module must not bump the project revision"
        );
        assert_eq!(duplicate.snapshot.modules.len(), 1);
        assert_eq!(
            duplicate.snapshot.modules[0].summary, "Inspector domain.",
            "the existing module is returned unchanged"
        );

        // An invalid parent does not block slug reuse either.
        let with_bad_parent = module_create(
            &database,
            ModuleCreateInput {
                request_id: "module-create-bad-parent".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 2,
                parent_id: Some("does-not-exist".to_string()),
                name: "Inspector duplicate".to_string(),
                slug: "inspector".to_string(),
                summary: "Accidental duplicate.".to_string(),
                status: EntityStatus::Ready,
            },
            "mcp:test",
        )
        .expect("slug reuse skips the parent check");
        assert!(with_bad_parent.reused_existing);
        assert_eq!(with_bad_parent.entity_id, created.entity_id);
    }

    #[test]
    fn source_ref_upsert_uses_the_bounded_file_broker() {
        let (_directory, database, project_id, root) = setup("source");
        fs::write(root.join("source.rs"), "one\ntwo\nthree\nfour\n").expect("write source");
        let module = module_create(
            &database,
            ModuleCreateInput {
                request_id: "module-create".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 1,
                parent_id: None,
                name: "Inspector".to_string(),
                slug: "inspector".to_string(),
                summary: "Inspector domain.".to_string(),
                status: EntityStatus::Ready,
            },
            "mcp:test",
        )
        .expect("module");
        let flow = flow_create(
            &database,
            FlowCreateInput {
                request_id: "flow-create".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 2,
                module_id: module.entity_id,
                name: "Read source".to_string(),
                summary: "Reads bounded evidence.".to_string(),
                status: EntityStatus::Ready,
                graph: renderable_graph(),
            },
            "mcp:test",
        )
        .expect("flow");
        let source = source_ref_upsert(
            &database,
            SourceRefUpsertInput {
                request_id: "source-create".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 3,
                flow_id: flow.entity_id,
                source_ref_id: None,
                expected_revision: None,
                path: "source.rs".to_string(),
                start_line: 2,
                end_line: 3,
                start_column: None,
                end_column: None,
                symbol: None,
                label: Some("Evidence".to_string()),
            },
            "mcp:test",
        )
        .expect("source reference");
        let stored = &source.snapshot.modules[0].flows[0].source_refs[0];
        assert_eq!((stored.start_line, stored.end_line), (2, 3));
        assert_eq!(stored.content_hash.len(), 64);

        let traversal = source_ref_upsert(
            &database,
            SourceRefUpsertInput {
                request_id: "source-traversal".to_string(),
                project_id,
                expected_project_revision: 4,
                flow_id: source.snapshot.modules[0].flows[0].id.clone(),
                source_ref_id: None,
                expected_revision: None,
                path: "../secret".to_string(),
                start_line: 1,
                end_line: 1,
                start_column: None,
                end_column: None,
                symbol: None,
                label: None,
            },
            "mcp:test",
        )
        .expect_err("path traversal");
        assert!(matches!(traversal, AppError::PathTraversal(_)));
    }

    #[test]
    fn issue_create_is_idempotent_and_records_the_module_revision() {
        let (_directory, database, project_id, _root) = setup("issue-create");
        let module = module_create(
            &database,
            ModuleCreateInput {
                request_id: "module-create".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 1,
                parent_id: None,
                name: "Inspector".to_string(),
                slug: "inspector".to_string(),
                summary: "Inspector domain.".to_string(),
                status: EntityStatus::Ready,
            },
            "mcp:test",
        )
        .expect("module");
        let input = IssueCreateInput {
            request_id: "issue-create".to_string(),
            project_id: project_id.clone(),
            module_id: module.entity_id.clone(),
            flow_id: None,
            title: "Stale evidence".to_string(),
            description: Some("The source range moved.".to_string()),
            severity: IssueSeverity::P1,
            status: None,
            path: Some("src/main.rs".to_string()),
            start_line: Some(10),
            end_line: Some(12),
            source: Some("inspector-review".to_string()),
        };

        let created = issue_create(&database, input.clone(), "mcp:test").expect("create issue");
        assert_eq!(created.issue.revision, 1);
        assert_eq!(created.issue.status, IssueStatus::Open);
        assert_eq!(created.issue.module_revision, Some(1));
        assert_eq!(created.issue.source, "inspector-review");
        assert!(!created.reused_existing);

        let replayed = issue_create(&database, input, "mcp:test").expect("replay create");
        assert_eq!(replayed.issue_id, created.issue_id);
        assert_eq!(
            database
                .list_issues(&project_id, None, None, 50)
                .expect("issues")
                .len(),
            1,
            "a replayed requestId must not insert a second row"
        );
        assert_eq!(
            database
                .load_inspector_snapshot(&project_id)
                .expect("snapshot")
                .project_revision,
            2,
            "issues do not touch the project revision"
        );
    }

    #[test]
    fn issue_update_enforces_revisions_and_tracks_status_transitions() {
        let (_directory, database, project_id, _root) = setup("issue-update");
        let module = module_create(
            &database,
            ModuleCreateInput {
                request_id: "module-create".to_string(),
                project_id: project_id.clone(),
                expected_project_revision: 1,
                parent_id: None,
                name: "Inspector".to_string(),
                slug: "inspector".to_string(),
                summary: "Inspector domain.".to_string(),
                status: EntityStatus::Ready,
            },
            "mcp:test",
        )
        .expect("module");
        let created = issue_create(
            &database,
            IssueCreateInput {
                request_id: "issue-create".to_string(),
                project_id: project_id.clone(),
                module_id: module.entity_id,
                flow_id: None,
                title: "Missing flow".to_string(),
                description: None,
                severity: IssueSeverity::P2,
                status: None,
                path: None,
                start_line: None,
                end_line: None,
                source: None,
            },
            "mcp:test",
        )
        .expect("create issue");
        assert_eq!(created.issue.source, "mcp", "the default source is mcp");

        let resolved = issue_update(
            &database,
            IssueUpdateInput {
                request_id: "issue-resolve".to_string(),
                project_id: project_id.clone(),
                issue_id: created.issue_id.clone(),
                expected_revision: 1,
                title: None,
                description: None,
                severity: None,
                status: Some(IssueStatus::Resolved),
                flow_id: None,
                path: None,
                start_line: None,
                end_line: None,
            },
            "mcp:test",
        )
        .expect("resolve issue");
        assert_eq!(resolved.issue.status, IssueStatus::Resolved);
        assert_eq!(resolved.issue.revision, 2);
        assert!(resolved.issue.resolved_at.is_some());

        let replayed = issue_update(
            &database,
            IssueUpdateInput {
                request_id: "issue-resolve".to_string(),
                project_id: project_id.clone(),
                issue_id: created.issue_id.clone(),
                expected_revision: 1,
                title: None,
                description: None,
                severity: None,
                status: Some(IssueStatus::Resolved),
                flow_id: None,
                path: None,
                start_line: None,
                end_line: None,
            },
            "mcp:test",
        )
        .expect("replay resolve");
        assert_eq!(
            replayed.issue.revision, 2,
            "replay returns the cached result"
        );

        let conflict = issue_update(
            &database,
            IssueUpdateInput {
                request_id: "issue-conflict".to_string(),
                project_id: project_id.clone(),
                issue_id: created.issue_id.clone(),
                expected_revision: 1,
                title: None,
                description: None,
                severity: None,
                status: Some(IssueStatus::Wontfix),
                flow_id: None,
                path: None,
                start_line: None,
                end_line: None,
            },
            "mcp:test",
        )
        .expect_err("stale revision");
        assert!(matches!(conflict, AppError::RevisionConflict { .. }));

        let reopened = issue_update(
            &database,
            IssueUpdateInput {
                request_id: "issue-reopen".to_string(),
                project_id: project_id.clone(),
                issue_id: created.issue_id.clone(),
                expected_revision: 2,
                title: None,
                description: None,
                severity: None,
                status: Some(IssueStatus::Open),
                flow_id: None,
                path: None,
                start_line: None,
                end_line: None,
            },
            "mcp:test",
        )
        .expect("reopen issue");
        assert_eq!(reopened.issue.status, IssueStatus::Open);
        assert!(reopened.issue.resolved_at.is_none());

        let foreign = issue_update(
            &database,
            IssueUpdateInput {
                request_id: "issue-foreign".to_string(),
                project_id: "some-other-project".to_string(),
                issue_id: created.issue_id.clone(),
                expected_revision: 3,
                title: None,
                description: None,
                severity: None,
                status: Some(IssueStatus::Resolved),
                flow_id: None,
                path: None,
                start_line: None,
                end_line: None,
            },
            "mcp:test",
        )
        .expect_err("issue of another project");
        assert!(matches!(foreign, AppError::NotFound(_)));

        let archived = issue_archive(
            &database,
            IssueArchiveInput {
                request_id: "issue-archive".to_string(),
                project_id: project_id.clone(),
                issue_id: created.issue_id.clone(),
                expected_revision: 3,
            },
            "mcp:test",
        )
        .expect("archive issue");
        assert!(archived.issue.archived_at.is_some());
        assert!(database
            .list_issues(&project_id, None, None, 50)
            .expect("issues")
            .is_empty());

        let archived_replay = issue_archive(
            &database,
            IssueArchiveInput {
                request_id: "issue-archive".to_string(),
                project_id,
                issue_id: created.issue_id,
                expected_revision: 3,
            },
            "mcp:test",
        )
        .expect("replay archive");
        assert_eq!(archived_replay.issue.revision, archived.issue.revision);
    }
}
