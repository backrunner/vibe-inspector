pub(crate) mod diff;
mod normalize;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    error::AppError,
    flow_graph::{validate_renderable_snapshot, GraphDiagnostic, GraphValidationReport},
    inspector::{InspectorSnapshot, ProvenanceSource},
    storage::InspectorDatabase,
};

use diff::{analyze_structure, ensure_scope_unchanged, validate_stable_ids};
use normalize::normalize_proposal;

const COMMAND_SCHEMA_VERSION: u32 = 1;
const LARGE_INIT_THRESHOLD: usize = 25;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InspectorCommand {
    InspectorInit,
    InspectorSync,
    InspectorClarify,
}

impl InspectorCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InspectorInit => "inspector-init",
            Self::InspectorSync => "inspector-sync",
            Self::InspectorClarify => "inspector-clarify",
        }
    }

    fn provenance_source(self) -> ProvenanceSource {
        match self {
            Self::InspectorInit => ProvenanceSource::InspectorInit,
            Self::InspectorSync => ProvenanceSource::InspectorSync,
            Self::InspectorClarify => ProvenanceSource::InspectorClarify,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InspectorScopeKind {
    Project,
    Module,
    Flow,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorCommandScope {
    pub kind: InspectorScopeKind,
    pub entity_id: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunInspectorCommandInput {
    pub request_id: String,
    pub project_id: String,
    pub command: InspectorCommand,
    pub expected_revision: u64,
    pub scope: InspectorCommandScope,
    pub proposal: InspectorSnapshot,
    #[serde(default)]
    pub accept_structural_changes: bool,
    #[serde(default)]
    pub accept_large_change: bool,
    pub conversation_id: Option<String>,
    pub model: Option<String>,
    pub prompt_hash: Option<String>,
    #[serde(default)]
    pub coverage: Option<InspectionCoverage>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionCoverage {
    pub total_files: u64,
    pub inspected_files: u64,
    #[serde(default)]
    pub ignored_files: u64,
    #[serde(default)]
    pub generated_files: u64,
    #[serde(default)]
    pub unreadable_files: Vec<String>,
    pub source_ref_count: u64,
    pub completed: bool,
    pub inventory_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CommandRunStatus {
    Committed,
    NeedsReview,
}

impl CommandRunStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::NeedsReview => "needs-review",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructuralSummary {
    pub added_modules: Vec<String>,
    pub removed_modules: Vec<String>,
    pub renamed_modules: Vec<String>,
    pub moved_modules: Vec<String>,
    pub added_flows: Vec<String>,
    pub removed_flows: Vec<String>,
    pub renamed_flows: Vec<String>,
    pub moved_flows: Vec<String>,
    #[serde(default)]
    pub changed_graphs: Vec<String>,
    #[serde(default)]
    pub changed_evidence: Vec<String>,
}

impl StructuralSummary {
    pub fn has_changes(&self) -> bool {
        !self.added_modules.is_empty()
            || !self.removed_modules.is_empty()
            || !self.renamed_modules.is_empty()
            || !self.moved_modules.is_empty()
            || !self.added_flows.is_empty()
            || !self.removed_flows.is_empty()
            || !self.renamed_flows.is_empty()
            || !self.moved_flows.is_empty()
            || !self.changed_graphs.is_empty()
            || !self.changed_evidence.is_empty()
    }

    fn added_count(&self) -> usize {
        self.added_modules.len() + self.added_flows.len()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunInspectorCommandResult {
    pub schema_version: u32,
    pub request_id: String,
    pub project_id: String,
    pub project_revision: u64,
    pub command: InspectorCommand,
    pub status: CommandRunStatus,
    pub scope_fingerprint: String,
    pub review_reason: Option<String>,
    pub structural_summary: StructuralSummary,
    pub coverage: Option<InspectionCoverage>,
    #[serde(default)]
    pub graph_warnings: Vec<GraphDiagnostic>,
    pub snapshot: InspectorSnapshot,
}

pub fn run_inspector_command(
    database: &InspectorDatabase,
    input: RunInspectorCommandInput,
    actor: &str,
) -> Result<RunInspectorCommandResult, AppError> {
    let graph_warnings = validate_request(&input, actor)?;
    if let Some(stored) = database.command_run(&input.request_id)? {
        if stored.project_id != input.project_id || stored.command != input.command.as_str() {
            return Err(AppError::Validation(format!(
                "requestId {} was already used for another command",
                input.request_id
            )));
        }
        let result = stored
            .result_json
            .ok_or_else(|| AppError::Database("command result is missing".to_string()))?;
        return serde_json::from_str(&result)
            .map_err(|error| AppError::Database(error.to_string()));
    }

    let current = database.load_inspector_snapshot(&input.project_id)?;
    if current.project_revision != input.expected_revision {
        return Err(AppError::RevisionConflict {
            entity_id: input.project_id.clone(),
            expected: input.expected_revision,
            actual: current.project_revision,
        });
    }
    validate_scope(&current, &input)?;
    if input.command == InspectorCommand::InspectorSync {
        validate_stable_ids(&current, &input.proposal)?;
    }
    if matches!(
        input.command,
        InspectorCommand::InspectorSync | InspectorCommand::InspectorClarify
    ) {
        ensure_scope_unchanged(&current, &input.proposal, &input.scope)?;
    }

    let scope_fingerprint = scope_fingerprint(&input)?;
    let structural_summary = analyze_structure(&current, &input.proposal);
    let review_reason = review_reason(&input, &structural_summary);
    if let Some(reason) = review_reason {
        let mut snapshot = input.proposal.clone();
        snapshot.project_revision = current.project_revision;
        let result = RunInspectorCommandResult {
            schema_version: COMMAND_SCHEMA_VERSION,
            request_id: input.request_id.clone(),
            project_id: input.project_id.clone(),
            project_revision: current.project_revision,
            command: input.command,
            status: CommandRunStatus::NeedsReview,
            scope_fingerprint: scope_fingerprint.clone(),
            review_reason: Some(reason),
            structural_summary,
            coverage: input.coverage.clone(),
            graph_warnings,
            snapshot,
        };
        save_result(database, &input, actor, &scope_fingerprint, &result)?;
        return Ok(result);
    }

    let normalized = normalize_proposal(&current, input.proposal.clone(), &input)?;
    let committed = database.commit_inspector_snapshot(
        &normalized,
        input.expected_revision,
        actor,
        input.command.as_str(),
        &input.request_id,
    )?;
    let result = RunInspectorCommandResult {
        schema_version: COMMAND_SCHEMA_VERSION,
        request_id: input.request_id.clone(),
        project_id: input.project_id.clone(),
        project_revision: committed.project_revision,
        command: input.command,
        status: CommandRunStatus::Committed,
        scope_fingerprint: scope_fingerprint.clone(),
        review_reason: None,
        structural_summary,
        coverage: input.coverage.clone(),
        graph_warnings,
        snapshot: committed,
    };
    save_result(database, &input, actor, &scope_fingerprint, &result)?;
    Ok(result)
}

fn validate_request(
    input: &RunInspectorCommandInput,
    actor: &str,
) -> Result<Vec<GraphDiagnostic>, AppError> {
    if input.request_id.trim().is_empty() || input.request_id.len() > 128 {
        return Err(AppError::Validation("invalid requestId".to_string()));
    }
    if actor.trim().is_empty() || actor.len() > 128 {
        return Err(AppError::Validation("invalid command actor".to_string()));
    }
    if input.proposal.project_id != input.project_id
        || input.proposal.project_revision != input.expected_revision
    {
        return Err(AppError::Validation(
            "proposal project identity or revision does not match the request".to_string(),
        ));
    }
    if input.proposal.modules.is_empty() {
        return Err(AppError::Validation(
            "an Inspector proposal must contain at least one module".to_string(),
        ));
    }
    validate_coverage(input.coverage.as_ref())?;
    let graph_report = validate_renderable_snapshot(&input.proposal)?;
    if input.command == InspectorCommand::InspectorInit {
        let parent_ids = input
            .proposal
            .modules
            .iter()
            .filter_map(|module| module.parent_id.as_deref())
            .collect::<std::collections::HashSet<_>>();
        if let Some(module) = input
            .proposal
            .modules
            .iter()
            .find(|module| !parent_ids.contains(module.id.as_str()) && module.flows.is_empty())
        {
            return Err(AppError::RenderError(GraphValidationReport::single(
                &module.id,
                "module.flow.missing",
                &format!("leaf module {} ({}) has no business flow. Add at least one renderable flow and retry", module.name, module.id),
            )));
        }
    }
    Ok(graph_report.diagnostics)
}

fn validate_coverage(coverage: Option<&InspectionCoverage>) -> Result<(), AppError> {
    let Some(coverage) = coverage else {
        return Ok(());
    };
    let unique_unreadable = coverage
        .unreadable_files
        .iter()
        .collect::<std::collections::HashSet<_>>();
    let unreadable_count = u64::try_from(unique_unreadable.len())
        .map_err(|_| AppError::Validation("coverage unreadable file count overflow".to_string()))?;
    let accounted = coverage
        .inspected_files
        .checked_add(unreadable_count)
        .and_then(|value| value.checked_add(coverage.ignored_files))
        .and_then(|value| value.checked_add(coverage.generated_files))
        .ok_or_else(|| AppError::Validation("coverage file count overflow".to_string()))?;
    if accounted > coverage.total_files {
        return Err(AppError::Validation(
            "coverage accounted files (inspected + ignored + generated + unreadable) exceed totalFiles"
                .to_string(),
        ));
    }
    if coverage.completed && accounted != coverage.total_files {
        return Err(AppError::Validation(
            "completed coverage must account for every in-scope file".to_string(),
        ));
    }
    if coverage.unreadable_files.iter().any(|path| {
        path.trim().is_empty()
            || std::path::Path::new(path).is_absolute()
            || std::path::Path::new(path)
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
    }) {
        return Err(AppError::Validation(
            "coverage unreadableFiles must contain repository-relative paths".to_string(),
        ));
    }
    if coverage
        .inventory_fingerprint
        .as_deref()
        .is_some_and(|value| value.trim().is_empty() || value.len() > 256)
    {
        return Err(AppError::Validation(
            "invalid coverage inventoryFingerprint".to_string(),
        ));
    }
    Ok(())
}

fn validate_scope(
    current: &InspectorSnapshot,
    input: &RunInspectorCommandInput,
) -> Result<(), AppError> {
    let entity_id = input.scope.entity_id.as_deref();
    match input.scope.kind {
        InspectorScopeKind::Project => {
            if entity_id.is_some() {
                return Err(AppError::Validation(
                    "project scope cannot include entityId".to_string(),
                ));
            }
        }
        InspectorScopeKind::Module => {
            let id = entity_id.ok_or_else(|| {
                AppError::Validation("module scope requires entityId".to_string())
            })?;
            if !current.modules.iter().any(|module| module.id == id) {
                return Err(AppError::NotFound(format!("module {id}")));
            }
        }
        InspectorScopeKind::Flow => {
            let id = entity_id
                .ok_or_else(|| AppError::Validation("flow scope requires entityId".to_string()))?;
            if !current
                .modules
                .iter()
                .flat_map(|module| &module.flows)
                .any(|flow| flow.id == id)
            {
                return Err(AppError::NotFound(format!("flow {id}")));
            }
        }
    }
    if input.command == InspectorCommand::InspectorClarify
        && input.scope.kind == InspectorScopeKind::Project
    {
        return Err(AppError::Validation(
            "inspector-clarify requires a module or flow target".to_string(),
        ));
    }
    Ok(())
}

fn review_reason(input: &RunInspectorCommandInput, summary: &StructuralSummary) -> Option<String> {
    if input.command == InspectorCommand::InspectorInit
        && summary.added_count() > LARGE_INIT_THRESHOLD
        && !input.accept_large_change
    {
        return Some(format!(
            "initialization adds {} modules and flows; confirmation is required",
            summary.added_count()
        ));
    }
    if summary.has_changes()
        && input.command != InspectorCommand::InspectorInit
        && !input.accept_structural_changes
    {
        return Some("structural changes require explicit acceptance".to_string());
    }
    None
}

fn scope_fingerprint(input: &RunInspectorCommandInput) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(&(
        &input.project_id,
        input.command,
        &input.scope,
        input.expected_revision,
    ))
    .map_err(|error| AppError::Validation(error.to_string()))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn save_result(
    database: &InspectorDatabase,
    input: &RunInspectorCommandInput,
    actor: &str,
    scope_fingerprint: &str,
    result: &RunInspectorCommandResult,
) -> Result<(), AppError> {
    let result_json =
        serde_json::to_string(result).map_err(|error| AppError::Database(error.to_string()))?;
    let coverage_json = input
        .coverage
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| AppError::Database(error.to_string()))?;
    database.save_command_run(
        &input.request_id,
        &input.project_id,
        input.command.as_str(),
        scope_fingerprint,
        actor,
        result.status.as_str(),
        &result_json,
        coverage_json.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use uuid::Uuid;

    use crate::{
        inspector::{
            EntityStatus, FlowNodeKind, InspectorExplanation, InspectorFlow, InspectorFlowEdge,
            InspectorFlowNode, InspectorGraph, InspectorModule, InspectorSourceRef, Provenance,
            SourceRefStatus,
        },
        project_discovery::{ProjectKind, ProjectNameSource},
        project_registry::{RegisteredProject, RegisteredSourceEvidence, RegistrationSource},
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
                "vibe-inspector-command-{label}-{}-{unique}",
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

    fn database(label: &str) -> (TestDirectory, InspectorDatabase, String) {
        let directory = TestDirectory::new(label);
        let project_root = directory.path().join("project");
        fs::create_dir(&project_root).expect("create project root");
        let project = RegisteredProject {
            id: Uuid::now_v7(),
            name: "Fixture".to_string(),
            root_path: project_root.to_string_lossy().into_owned(),
            kind: ProjectKind::Repository,
            name_source: ProjectNameSource::Directory,
            markers: vec!["git".to_string()],
            source: RegistrationSource::Manual,
            source_evidence: vec![RegisteredSourceEvidence {
                id: Uuid::now_v7(),
                kind: RegistrationSource::Manual,
                conversation_id: None,
                observed_path: project_root.to_string_lossy().into_owned(),
                observed_at: "1".to_string(),
                parser_version: "test".to_string(),
            }],
            revision: 1,
            updated_at: "1".to_string(),
        };
        let project_id = project.id.to_string();
        let database = InspectorDatabase::new(directory.path().join("inspector.sqlite3"));
        database.initialize().expect("initialize database");
        database
            .import_registered_projects(&[project])
            .expect("import project");
        (directory, database, project_id)
    }

    fn proposal(project_id: &str, revision: u64) -> InspectorSnapshot {
        let module_id = "new:module:desktop".to_string();
        let flow = new_flow(project_id, &module_id, "refresh");
        InspectorSnapshot {
            project_id: project_id.to_string(),
            project_revision: revision,
            modules: vec![InspectorModule {
                id: module_id,
                project_id: project_id.to_string(),
                parent_id: None,
                name: "Desktop".to_string(),
                slug: "desktop".to_string(),
                summary: "Desktop inspection features.".to_string(),
                status: EntityStatus::Ready,
                revision: 999,
                created_at: "client".to_string(),
                updated_at: "client".to_string(),
                flows: vec![flow],
            }],
        }
    }

    fn new_flow(project_id: &str, module_id: &str, suffix: &str) -> InspectorFlow {
        let flow_id = format!("new:flow:{suffix}");
        let source_id = format!("new:source:{suffix}");
        let start_id = format!("new:node:{suffix}:start");
        let end_id = format!("new:node:{suffix}:end");
        InspectorFlow {
            id: flow_id.clone(),
            project_id: project_id.to_string(),
            module_id: module_id.to_string(),
            name: format!("{suffix} flow"),
            summary: "Refreshes Inspector evidence.".to_string(),
            status: EntityStatus::Ready,
            revision: 999,
            created_at: "client".to_string(),
            updated_at: "client".to_string(),
            graph: InspectorGraph {
                kind: crate::inspector::DiagramKind::Workflow,
                nodes: vec![
                    InspectorFlowNode {
                        id: start_id.clone(),
                        label: "Read evidence".to_string(),
                        kind: FlowNodeKind::Start,
                        description: None,
                        source_ref_ids: vec![source_id.clone()],
                        position: 0,
                    },
                    InspectorFlowNode {
                        id: end_id.clone(),
                        label: "Commit snapshot".to_string(),
                        kind: FlowNodeKind::End,
                        description: None,
                        source_ref_ids: vec![source_id.clone()],
                        position: 1,
                    },
                ],
                edges: vec![InspectorFlowEdge {
                    id: format!("new:edge:{suffix}"),
                    from: start_id,
                    to: end_id,
                    kind: crate::inspector::FlowEdgeKind::Control,
                    mechanism: None,
                    source_ref_ids: vec![],
                    label: None,
                }],
            },
            explanation: Some(InspectorExplanation {
                id: format!("new:explanation:{suffix}"),
                project_id: project_id.to_string(),
                flow_id: flow_id.clone(),
                headline: "Evidence before assertion.".to_string(),
                body: "The flow ties each claim to a bounded source range.".to_string(),
                decisions: vec!["Keep repository files read-only.".to_string()],
                confidence: Some(0.9),
                provenance: Provenance {
                    source: ProvenanceSource::Manual,
                    conversation_id: None,
                    model: None,
                    prompt_hash: None,
                    generated_at: "client".to_string(),
                    confidence: Some(0.9),
                    evidence_ids: vec![source_id.clone()],
                },
                revision: 999,
                created_at: "client".to_string(),
                updated_at: "client".to_string(),
            }),
            source_refs: vec![InspectorSourceRef {
                id: source_id,
                project_id: project_id.to_string(),
                flow_id,
                path: format!("src/{suffix}.rs"),
                start_line: 1,
                end_line: 12,
                start_column: None,
                end_column: None,
                symbol: Some(format!("{suffix}_inspector")),
                label: Some("Inspector boundary".to_string()),
                content_hash: "hash".to_string(),
                status: SourceRefStatus::Current,
                revision: 999,
                created_at: "client".to_string(),
                updated_at: "client".to_string(),
            }],
        }
    }

    fn command_input(
        project_id: &str,
        command: InspectorCommand,
        revision: u64,
        proposal: InspectorSnapshot,
        request_id: &str,
    ) -> RunInspectorCommandInput {
        RunInspectorCommandInput {
            request_id: request_id.to_string(),
            project_id: project_id.to_string(),
            command,
            expected_revision: revision,
            scope: InspectorCommandScope {
                kind: InspectorScopeKind::Project,
                entity_id: None,
                description: None,
            },
            proposal,
            accept_structural_changes: false,
            accept_large_change: false,
            conversation_id: Some("thread-1".to_string()),
            model: Some("fixture-model".to_string()),
            prompt_hash: Some("prompt-hash".to_string()),
            coverage: None,
        }
    }

    #[test]
    fn init_assigns_uuid_v7_metadata_and_replays_idempotently() {
        let (_directory, database, project_id) = database("init");
        let mut input = command_input(
            &project_id,
            InspectorCommand::InspectorInit,
            1,
            proposal(&project_id, 1),
            "request-init",
        );
        input.coverage = Some(InspectionCoverage {
            total_files: 4,
            inspected_files: 2,
            ignored_files: 1,
            generated_files: 1,
            unreadable_files: vec![],
            source_ref_count: 1,
            completed: true,
            inventory_fingerprint: Some("sha256:fixture".to_string()),
        });
        let committed =
            run_inspector_command(&database, input.clone(), "codex").expect("commit init");
        assert_eq!(committed.status, CommandRunStatus::Committed);
        assert_eq!(committed.project_revision, 2);
        let module = &committed.snapshot.modules[0];
        assert_eq!(
            Uuid::parse_str(&module.id)
                .expect("module UUID")
                .get_version_num(),
            7
        );
        assert_eq!(module.revision, 1);
        let stored = database
            .command_run("request-init")
            .expect("stored command")
            .expect("command run");
        assert!(stored
            .coverage_json
            .as_deref()
            .is_some_and(|coverage| coverage.contains("inventoryFingerprint")));
        assert_ne!(module.created_at, "client");
        let explanation = module.flows[0].explanation.as_ref().expect("explanation");
        assert_eq!(
            explanation.provenance.source,
            ProvenanceSource::InspectorInit
        );
        assert_eq!(
            explanation.provenance.conversation_id.as_deref(),
            Some("thread-1")
        );

        let replayed = run_inspector_command(&database, input, "codex").expect("replay init");
        assert_eq!(replayed, committed);
        assert_eq!(database.project(&project_id).expect("project").revision, 2);
    }

    #[test]
    fn repeat_init_reuses_existing_entities_and_adds_missing_modules() {
        let (_directory, database, project_id) = database("repeat-init");
        let initial = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorInit,
                1,
                proposal(&project_id, 1),
                "request-init",
            ),
            "codex",
        )
        .expect("initial init");
        let existing_module_id = initial.snapshot.modules[0].id.clone();

        let mut repeated = initial.snapshot;
        repeated.project_revision = 2;
        repeated.modules[0].summary = "More precise desktop inspection features.".to_string();
        let module_id = "new:module:services".to_string();
        repeated.modules.push(InspectorModule {
            id: module_id.clone(),
            project_id: project_id.clone(),
            parent_id: None,
            name: "Services".to_string(),
            slug: "services".to_string(),
            summary: "Long-running local services.".to_string(),
            status: EntityStatus::Ready,
            revision: 1,
            created_at: "client".to_string(),
            updated_at: "client".to_string(),
            flows: vec![new_flow(&project_id, &module_id, "service-health")],
        });

        let committed = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorInit,
                2,
                repeated,
                "request-repeat-init",
            ),
            "codex",
        )
        .expect("repeat init");

        assert_eq!(committed.snapshot.modules.len(), 2);
        assert_eq!(committed.snapshot.modules[0].id, existing_module_id);
        assert_eq!(committed.snapshot.modules[0].revision, 2);
        assert_eq!(committed.structural_summary.added_modules, vec!["Services"]);
    }

    #[test]
    fn init_rejects_missing_or_unrenderable_business_flows() {
        let (_directory, database, project_id) = database("init-flow-validation");
        let mut missing = proposal(&project_id, 1);
        missing.modules[0].flows.clear();
        assert!(matches!(
            run_inspector_command(
                &database,
                command_input(
                    &project_id,
                    InspectorCommand::InspectorInit,
                    1,
                    missing,
                    "request-missing-flow",
                ),
                "codex",
            ),
            Err(AppError::RenderError(report))
                if report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("no business flow"))
        ));

        let mut disconnected = proposal(&project_id, 1);
        disconnected.modules[0].flows[0].graph.edges.clear();
        assert!(matches!(
            run_inspector_command(
                &database,
                command_input(
                    &project_id,
                    InspectorCommand::InspectorInit,
                    1,
                    disconnected,
                    "request-unrenderable-flow",
                ),
                "codex",
            ),
            Err(AppError::RenderError(report))
                if report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("connected"))
        ));
    }

    #[test]
    fn sync_structural_changes_require_review_and_are_idempotent() {
        let (_directory, database, project_id) = database("sync-review");
        let init = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorInit,
                1,
                proposal(&project_id, 1),
                "request-init",
            ),
            "codex",
        )
        .expect("commit init");
        let mut proposed = init.snapshot.clone();
        proposed.project_revision = 2;
        let module_id = proposed.modules[0].id.clone();
        proposed.modules[0]
            .flows
            .push(new_flow(&project_id, &module_id, "second"));
        let input = command_input(
            &project_id,
            InspectorCommand::InspectorSync,
            2,
            proposed,
            "request-sync",
        );

        let review = run_inspector_command(&database, input.clone(), "codex").expect("review sync");
        assert_eq!(review.status, CommandRunStatus::NeedsReview);
        assert_eq!(review.structural_summary.added_flows, vec!["second flow"]);
        assert_eq!(database.project(&project_id).expect("project").revision, 2);
        assert_eq!(
            run_inspector_command(&database, input, "codex")
                .expect("replay review")
                .status,
            CommandRunStatus::NeedsReview
        );
    }

    #[test]
    fn sync_rejects_replacing_a_stable_module_id() {
        let (_directory, database, project_id) = database("sync-id");
        let init = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorInit,
                1,
                proposal(&project_id, 1),
                "request-init",
            ),
            "codex",
        )
        .expect("commit init");
        let mut proposed = init.snapshot;
        proposed.project_revision = 2;
        proposed.modules[0].id = "new:module:replacement".to_string();
        let error = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorSync,
                2,
                proposed,
                "request-sync",
            ),
            "codex",
        )
        .expect_err("ID churn must fail");
        assert!(matches!(error, AppError::Validation(message) if message.contains("preserve")));
    }

    #[test]
    fn clarify_requires_a_module_or_flow_target() {
        let (_directory, database, project_id) = database("clarify-target");
        let error = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorClarify,
                1,
                proposal(&project_id, 1),
                "request-clarify",
            ),
            "codex",
        )
        .expect_err("clarify target must be required");
        assert!(matches!(error, AppError::Validation(message) if message.contains("requires")));
    }

    #[test]
    fn sync_records_entity_versions_for_changed_content() {
        let (_directory, database, project_id) = database("sync-versions");
        let init = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorInit,
                1,
                proposal(&project_id, 1),
                "request-init",
            ),
            "codex",
        )
        .expect("commit init");
        assert!(database
            .list_entity_versions(&project_id, None, None, 10)
            .expect("versions")
            .is_empty());

        let mut proposed = init.snapshot.clone();
        proposed.project_revision = 2;
        proposed.modules[0].summary = "Refined summary.".to_string();
        proposed.modules[0].flows[0]
            .explanation
            .as_mut()
            .expect("explanation")
            .body = "Refined body.".to_string();
        let committed = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorSync,
                2,
                proposed,
                "request-sync",
            ),
            "codex",
        )
        .expect("commit sync");
        assert_eq!(committed.status, CommandRunStatus::Committed);

        let versions = database
            .list_entity_versions(&project_id, None, None, 10)
            .expect("versions");
        assert_eq!(versions.len(), 2);
        let module_version = versions
            .iter()
            .find(|version| version.entity_type == "module")
            .expect("module version");
        assert_eq!(module_version.revision, 1);
        assert_eq!(module_version.entity_id, init.snapshot.modules[0].id);
        assert_eq!(module_version.actor, "codex");
        assert_eq!(module_version.source, "inspector-sync");
        let flow_version = versions
            .iter()
            .find(|version| version.entity_type == "flow")
            .expect("flow version");
        let flow_detail = database
            .get_entity_version(&flow_version.id)
            .expect("flow version detail");
        assert_eq!(
            flow_detail.snapshot_json["explanation"]["body"],
            "The flow ties each claim to a bounded source range."
        );
        assert_eq!(
            flow_detail.snapshot_json["graph"]["nodes"][0]["label"],
            "Read evidence"
        );

        let mut noop = committed.snapshot.clone();
        noop.project_revision = 3;
        run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorSync,
                3,
                noop,
                "request-sync-noop",
            ),
            "codex",
        )
        .expect("commit no-op sync");
        assert_eq!(
            database
                .list_entity_versions(&project_id, None, None, 10)
                .expect("versions")
                .len(),
            2,
            "a no-op sync must not record versions"
        );
    }

    #[test]
    fn new_entities_require_temporary_ids() {
        let (_directory, database, project_id) = database("temporary-id");
        let mut proposed = proposal(&project_id, 1);
        proposed.modules[0].id = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let error = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorInit,
                1,
                proposed,
                "request-init",
            ),
            "codex",
        )
        .expect_err("client UUID must be rejected");
        assert!(matches!(error, AppError::Validation(message) if message.contains("new:")));
    }

    #[test]
    fn init_remaps_temporary_edge_evidence_ids() {
        let (_directory, database, project_id) = database("edge-evidence");
        let mut proposed = proposal(&project_id, 1);
        let temporary_source_id = proposed.modules[0].flows[0].source_refs[0].id.clone();
        assert!(temporary_source_id.starts_with("new:source:"));
        proposed.modules[0].flows[0].graph.edges[0].source_ref_ids =
            vec![temporary_source_id.clone()];

        let committed = run_inspector_command(
            &database,
            command_input(
                &project_id,
                InspectorCommand::InspectorInit,
                1,
                proposed,
                "request-edge-evidence",
            ),
            "codex",
        )
        .expect("edge evidence with temporary IDs must commit");

        let flow = &committed.snapshot.modules[0].flows[0];
        let source_id = flow.source_refs[0].id.clone();
        assert!(Uuid::parse_str(&source_id).is_ok());
        assert_eq!(flow.graph.edges[0].source_ref_ids, vec![source_id]);
    }

    #[test]
    fn structural_summary_deserializes_legacy_command_results() {
        let legacy = r#"{
            "addedModules": [],
            "removedModules": [],
            "renamedModules": [],
            "movedModules": [],
            "addedFlows": [],
            "removedFlows": [],
            "renamedFlows": [],
            "movedFlows": []
        }"#;
        let summary: StructuralSummary =
            serde_json::from_str(legacy).expect("legacy result without changedGraphs fields");
        assert_eq!(summary, StructuralSummary::default());
        assert!(!summary.has_changes());
    }

    #[test]
    fn graph_and_evidence_changes_count_as_structural_changes() {
        let (_directory, _database, project_id) = database("graph-change-review");
        let input = command_input(
            &project_id,
            InspectorCommand::InspectorSync,
            1,
            proposal(&project_id, 1),
            "request-graph-change",
        );

        let mut graph_only = StructuralSummary::default();
        assert!(!graph_only.has_changes());
        graph_only.changed_graphs.push("refresh flow".to_string());
        assert!(graph_only.has_changes());
        assert_eq!(
            review_reason(&input, &graph_only).as_deref(),
            Some("structural changes require explicit acceptance")
        );

        let mut evidence_only = StructuralSummary::default();
        evidence_only.changed_evidence.push("refresh flow".to_string());
        assert!(evidence_only.has_changes());
        assert!(review_reason(&input, &evidence_only).is_some());
    }

    #[test]
    fn committed_results_include_structured_graph_warnings() {
        let (_directory, database, project_id) = database("graph-warnings");
        let input = command_input(
            &project_id,
            InspectorCommand::InspectorInit,
            1,
            proposal(&project_id, 1),
            "request-warnings",
        );
        let committed =
            run_inspector_command(&database, input.clone(), "codex").expect("commit init");
        assert!(committed.graph_warnings.iter().any(|diagnostic| {
            diagnostic.code == "graph.edge.evidence-unverified"
                && diagnostic.severity == crate::flow_graph::GraphDiagnosticSeverity::Warning
        }));
        let serialized = serde_json::to_string(&committed).expect("serialize result");
        assert!(serialized.contains("\"graphWarnings\""));

        let replayed = run_inspector_command(&database, input, "codex").expect("replay init");
        assert_eq!(replayed.graph_warnings, committed.graph_warnings);
    }

    #[test]
    fn coverage_deduplicates_unreadable_files_before_counting() {
        let coverage = InspectionCoverage {
            total_files: 2,
            inspected_files: 1,
            ignored_files: 0,
            generated_files: 0,
            unreadable_files: vec!["src/locked.rs".to_string(), "src/locked.rs".to_string()],
            source_ref_count: 0,
            completed: true,
            inventory_fingerprint: None,
        };
        validate_coverage(Some(&coverage)).expect("duplicate unreadable paths count once");
    }

    #[test]
    fn coverage_overflow_error_describes_all_accounted_components() {
        let coverage = InspectionCoverage {
            total_files: 2,
            inspected_files: 1,
            ignored_files: 1,
            generated_files: 1,
            unreadable_files: vec![],
            source_ref_count: 0,
            completed: false,
            inventory_fingerprint: None,
        };
        let error = validate_coverage(Some(&coverage)).expect_err("accounted exceeds total");
        assert!(
            matches!(error, AppError::Validation(message) if message.contains("inspected + ignored + generated + unreadable"))
        );
    }
}
