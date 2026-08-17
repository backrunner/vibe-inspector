pub mod agent_integrations;
pub mod cli;
mod codegraph;
mod codex_history;
pub mod command_service;
pub mod error;
pub mod fs_broker;
pub mod inspector;
pub mod mcp;
mod project_discovery;
pub mod project_registry;
pub mod storage;
pub mod tray_runtime;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use agent_integrations::{AgentIntegration, AgentIntegrationResult, InstallAgentIntegrationsInput};
use codex_history::{scan_default_codex_history, CodexHistoryScanResult, ScanCodexHistoryInput};
use error::AppError;
use fs_broker::{
    read_source_file, read_source_range, ReadSourceFileInput, ReadSourceInput, SourceRange,
};
use inspector::InspectorSnapshot;
use mcp::ActivityList;
use project_discovery::{DiscoverProjectsInput, ProjectDiscoveryResult};
use project_registry::{ProjectRegistry, RegisterProjectsInput};
use storage::issues::{Issue, IssuePatch, IssueStatus};
use storage::versions::{EntityVersion, EntityVersionSummary};
use storage::{InspectorDatabase, StoredProject};
use tray_runtime::{
    clear_quit_intent, ensure_tray_running, install_local_panic_hook, record_diagnostic,
    set_tray_login_item, tray_health, DiagnosticEvent, DiagnosticProcess, TrayPaths,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeHealth {
    app: &'static str,
    tray: String,
    mcp: String,
    endpoint: Option<String>,
    error: Option<String>,
    login_item_enabled: bool,
    pid: Option<u32>,
    started_at: Option<String>,
    checked_at: String,
}

#[tauri::command]
async fn runtime_health() -> RuntimeHealth {
    tauri::async_runtime::spawn_blocking(|| {
        let paths = TrayPaths::for_macos_user();
        match paths.and_then(|paths| tray_health(&paths)) {
            Ok(health) => RuntimeHealth {
                app: "desktop",
                tray: health.tray,
                mcp: health.mcp,
                endpoint: health.endpoint,
                error: health.error,
                login_item_enabled: health.login_item_enabled,
                pid: Some(health.pid),
                started_at: Some(health.started_at),
                checked_at: health.checked_at,
            },
            Err(error) => RuntimeHealth {
                app: "desktop",
                tray: "offline".to_string(),
                mcp: "offline".to_string(),
                endpoint: None,
                error: Some(error.to_string()),
                login_item_enabled: false,
                pid: None,
                started_at: None,
                checked_at: unix_epoch_millis(),
            },
        }
    })
    .await
    .unwrap_or_else(|error| RuntimeHealth {
        app: "desktop",
        tray: "offline".to_string(),
        mcp: "offline".to_string(),
        endpoint: None,
        error: Some(error.to_string()),
        login_item_enabled: false,
        pid: None,
        started_at: None,
        checked_at: unix_epoch_millis(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetLoginItemInput {
    enabled: bool,
}

#[tauri::command]
async fn set_login_item(input: SetLoginItemInput) -> Result<RuntimeHealth, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths =
            TrayPaths::for_macos_user().map_err(|error| AppError::Storage(error.to_string()))?;
        let health = set_tray_login_item(&paths, input.enabled)
            .map_err(|error| AppError::Storage(error.to_string()))?;
        Ok(RuntimeHealth {
            app: "desktop",
            tray: health.tray,
            mcp: health.mcp,
            endpoint: health.endpoint,
            error: health.error,
            login_item_enabled: health.login_item_enabled,
            pid: Some(health.pid),
            started_at: Some(health.started_at),
            checked_at: health.checked_at,
        })
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

fn unix_epoch_millis() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[tauri::command]
async fn discover_projects(
    input: DiscoverProjectsInput,
) -> Result<ProjectDiscoveryResult, AppError> {
    tauri::async_runtime::spawn_blocking(move || project_discovery::discover_projects(input))
        .await
        .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn scan_codex_history(
    input: ScanCodexHistoryInput,
) -> Result<CodexHistoryScanResult, AppError> {
    tauri::async_runtime::spawn_blocking(move || scan_default_codex_history(input))
        .await
        .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn load_registered_projects(app: tauri::AppHandle) -> Result<ProjectRegistry, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let registry = project_registry::load_project_registry(&app)?;
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        database.import_registered_projects(&registry.projects)?;
        Ok(registry)
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn register_projects(
    app: tauri::AppHandle,
    input: RegisterProjectsInput,
) -> Result<ProjectRegistry, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        // Reuse the IDs of projects that already exist in SQLite (for example
        // auto-imported through MCP) so registry and database share one
        // project identity per root path.
        let existing_ids = database
            .list_projects(false)?
            .into_iter()
            .filter_map(|project| {
                uuid::Uuid::parse_str(&project.id)
                    .ok()
                    .map(|id| (project.root_path, id))
            })
            .collect::<std::collections::HashMap<_, _>>();
        let registry = project_registry::register_projects(&app, input, &existing_ids)?;
        database.import_registered_projects(&registry.projects)?;
        let registry_ids = registry
            .projects
            .iter()
            .map(|project| project.id.to_string())
            .collect::<Vec<_>>();
        let project_ids =
            agent_integrations::merged_project_ids(&registry_ids, &database.list_projects(false)?);
        agent_integrations::sync_agent_project_allowlist(&project_ids);
        Ok(registry)
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn detect_agent_integrations() -> Result<Vec<AgentIntegration>, AppError> {
    tauri::async_runtime::spawn_blocking(agent_integrations::detect_integrations)
        .await
        .map_err(|error| AppError::DiscoveryTask(error.to_string()))
}

#[tauri::command]
async fn install_agent_integrations(
    app: tauri::AppHandle,
    input: InstallAgentIntegrationsInput,
) -> Result<Vec<AgentIntegrationResult>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let registry = project_registry::load_project_registry(&app)?;
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        let registry_ids = registry
            .projects
            .iter()
            .map(|project| project.id.to_string())
            .collect::<Vec<_>>();
        let project_ids =
            agent_integrations::merged_project_ids(&registry_ids, &database.list_projects(false)?);
        Ok(agent_integrations::install_integrations(
            &project_ids,
            input,
        ))
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadProjectSourceInput {
    project_id: String,
    relative_path: String,
    start_line: usize,
    end_line: usize,
    expected_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadProjectFileInput {
    project_id: String,
    relative_path: String,
    expected_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListCodegraphRelatedFilesInput {
    project_id: String,
    relative_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadInspectorSnapshotInput {
    project_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListInspectorActivityInput {
    project_id: String,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListEntityVersionsInput {
    project_id: String,
    entity_type: Option<String>,
    entity_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetEntityVersionInput {
    project_id: String,
    version_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListProjectIssuesInput {
    project_id: String,
    module_id: Option<String>,
    status: Option<IssueStatus>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetIssueStatusInput {
    project_id: String,
    issue_id: String,
    expected_revision: u64,
    status: IssueStatus,
}

#[tauri::command]
async fn list_inspector_projects(app: tauri::AppHandle) -> Result<Vec<StoredProject>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        database.refresh_project_availability("desktop")
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn load_inspector_snapshot(
    app: tauri::AppHandle,
    input: LoadInspectorSnapshotInput,
) -> Result<InspectorSnapshot, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        database.load_inspector_snapshot(&input.project_id)
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn list_inspector_activity(
    app: tauri::AppHandle,
    input: ListInspectorActivityInput,
) -> Result<ActivityList, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        let limit = input.limit.unwrap_or(50).clamp(1, 200);
        Ok(ActivityList {
            command_runs: database.list_command_runs(&input.project_id, limit)?,
            audit_events: database.list_mcp_audit(&input.project_id, limit)?,
        })
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn list_entity_versions(
    app: tauri::AppHandle,
    input: ListEntityVersionsInput,
) -> Result<Vec<EntityVersionSummary>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        database.project(&input.project_id)?;
        let limit = input.limit.unwrap_or(50).clamp(1, 200);
        database.list_entity_versions(
            &input.project_id,
            input.entity_type.as_deref(),
            input.entity_id.as_deref(),
            limit,
        )
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn get_entity_version(
    app: tauri::AppHandle,
    input: GetEntityVersionInput,
) -> Result<EntityVersion, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        let version = database.get_entity_version(&input.version_id)?;
        if version.project_id != input.project_id {
            return Err(AppError::NotFound(format!(
                "entity version {}",
                input.version_id
            )));
        }
        Ok(version)
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn list_project_issues(
    app: tauri::AppHandle,
    input: ListProjectIssuesInput,
) -> Result<Vec<Issue>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        database.project(&input.project_id)?;
        let limit = input.limit.unwrap_or(100).clamp(1, 200);
        database.list_issues(
            &input.project_id,
            input.module_id.as_deref(),
            input.status,
            limit,
        )
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn set_issue_status(
    app: tauri::AppHandle,
    input: SetIssueStatusInput,
) -> Result<Issue, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        let issue = database.get_issue(&input.issue_id)?;
        if issue.project_id != input.project_id {
            return Err(AppError::NotFound(format!("issue {}", input.issue_id)));
        }
        database.update_issue(
            &input.issue_id,
            input.expected_revision,
            &IssuePatch {
                status: Some(input.status),
                ..IssuePatch::default()
            },
            "desktop",
            "manual",
            None,
        )
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn read_project_source(
    app: tauri::AppHandle,
    input: ReadProjectSourceInput,
) -> Result<SourceRange, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        let project = database.project(&input.project_id)?;
        read_source_range(
            std::path::Path::new(&project.root_path),
            ReadSourceInput {
                relative_path: input.relative_path,
                start_line: input.start_line,
                end_line: input.end_line,
                expected_hash: input.expected_hash,
            },
        )
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn read_project_file(
    app: tauri::AppHandle,
    input: ReadProjectFileInput,
) -> Result<SourceRange, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        let project = database.project(&input.project_id)?;
        read_source_file(
            std::path::Path::new(&project.root_path),
            ReadSourceFileInput {
                relative_path: input.relative_path,
                expected_hash: input.expected_hash,
            },
        )
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[tauri::command]
async fn list_codegraph_related_files(
    app: tauri::AppHandle,
    input: ListCodegraphRelatedFilesInput,
) -> Result<Vec<codegraph::RelatedFile>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = InspectorDatabase::for_app(&app)?;
        database.initialize()?;
        let project = database.project(&input.project_id)?;
        codegraph::related_files(
            std::path::Path::new(&project.root_path),
            &input.relative_paths,
        )
    })
    .await
    .map_err(|error| AppError::DiscoveryTask(error.to_string()))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let tray_paths = TrayPaths::for_macos_user().ok();
    if let Some(paths) = tray_paths.clone() {
        install_local_panic_hook(paths, DiagnosticProcess::Desktop);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _cwd| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            },
        ))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            InspectorDatabase::for_app(app.handle())
                .and_then(|database| database.initialize())
                .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;
            if let Some(paths) = tray_paths.clone() {
                let _ =
                    record_diagnostic(&paths, DiagnosticProcess::Desktop, DiagnosticEvent::Started);
                std::thread::spawn(move || {
                    // Opening the desktop app is an explicit user action, so a
                    // previously recorded tray quit intent no longer applies.
                    let _ = clear_quit_intent(&paths);
                    let event = if ensure_tray_running(&paths).is_ok() {
                        DiagnosticEvent::TrayHelperReady
                    } else {
                        DiagnosticEvent::TrayHelperUnavailable
                    };
                    let _ = record_diagnostic(&paths, DiagnosticProcess::Desktop, event);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runtime_health,
            set_login_item,
            discover_projects,
            scan_codex_history,
            load_registered_projects,
            register_projects,
            detect_agent_integrations,
            install_agent_integrations,
            list_inspector_projects,
            load_inspector_snapshot,
            list_inspector_activity,
            list_entity_versions,
            get_entity_version,
            list_project_issues,
            set_issue_status,
            read_project_source,
            read_project_file,
            list_codegraph_related_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running VibeInspector");
}
