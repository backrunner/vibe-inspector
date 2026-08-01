use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    error::AppError,
    project_discovery::{
        discover_projects, DiscoverProjectsInput, DiscoveryMode, ProjectKind, ProjectNameSource,
    },
};

const REGISTRY_SCHEMA_VERSION: u32 = 2;
const REGISTRY_FILE: &str = "projects.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterProjectsInput {
    pub root_paths: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<RegisterProjectEvidence>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterProjectEvidence {
    pub root_path: String,
    pub source: RegistrationSource,
    #[serde(default)]
    pub conversation_ids: Vec<String>,
    pub parser_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RegistrationSource {
    Manual,
    CodexHistory,
}

impl RegistrationSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::CodexHistory => "codex-history",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredProject {
    pub id: Uuid,
    pub name: String,
    pub root_path: String,
    pub kind: ProjectKind,
    pub name_source: ProjectNameSource,
    pub markers: Vec<String>,
    pub source: RegistrationSource,
    #[serde(default)]
    pub source_evidence: Vec<RegisteredSourceEvidence>,
    pub revision: u64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredSourceEvidence {
    pub id: Uuid,
    pub kind: RegistrationSource,
    pub conversation_id: Option<String>,
    pub observed_path: String,
    pub observed_at: String,
    pub parser_version: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistry {
    pub schema_version: u32,
    pub revision: u64,
    pub updated_at: String,
    pub projects: Vec<RegisteredProject>,
}

impl ProjectRegistry {
    fn empty() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            revision: 0,
            updated_at: timestamp(),
            projects: Vec::new(),
        }
    }
}

pub fn load_project_registry(app: &AppHandle) -> Result<ProjectRegistry, AppError> {
    let path = registry_path(app)?;
    let persisted_version = persisted_schema_version(&path)?;
    let registry = load_registry_from_path(&path)?;
    if persisted_version.is_some_and(|version| version < REGISTRY_SCHEMA_VERSION) {
        write_registry_to_path(&path, &registry)?;
    }
    Ok(registry)
}

fn persisted_schema_version(path: &Path) -> Result<Option<u32>, AppError> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(storage_error)?;
    let document = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(storage_error)?;
    let version = document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    Ok(version)
}

fn load_registry_from_path(path: &Path) -> Result<ProjectRegistry, AppError> {
    if !path.exists() {
        return Ok(ProjectRegistry::empty());
    }

    let bytes = fs::read(path).map_err(storage_error)?;
    let document = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(storage_error)?;
    let schema_version = document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| AppError::Storage("project registry schema is missing".to_string()))?;
    if schema_version == 1 {
        return serde_json::from_value::<LegacyProjectRegistry>(document)
            .map(LegacyProjectRegistry::migrate)
            .map_err(storage_error);
    }
    if schema_version != u64::from(REGISTRY_SCHEMA_VERSION) {
        return Err(AppError::Storage(format!(
            "unsupported project registry schema {schema_version}"
        )));
    }
    serde_json::from_value(document).map_err(storage_error)
}

/// Registers discovered project roots into the JSON registry. `existing_ids`
/// maps canonical root paths to Inspector project IDs that already exist in
/// SQLite (for example projects auto-imported through MCP); new registry
/// entries reuse those IDs so both stores share one project identity.
pub fn register_projects(
    app: &AppHandle,
    input: RegisterProjectsInput,
    existing_ids: &HashMap<String, Uuid>,
) -> Result<ProjectRegistry, AppError> {
    let mut registry = load_project_registry(app)?;
    let mut changed = false;
    let evidence = input
        .evidence
        .into_iter()
        .map(|item| (item.root_path.clone(), item))
        .collect::<HashMap<_, _>>();

    for root_path in input.root_paths.into_iter().take(200) {
        let discovery = discover_projects(DiscoverProjectsInput {
            root_path,
            mode: DiscoveryMode::SingleProject,
        })?;
        let Some(candidate) = discovery.candidates.into_iter().next() else {
            continue;
        };
        let source = evidence
            .get(&candidate.root_path)
            .map(|item| item.source)
            .unwrap_or(RegistrationSource::Manual);
        let source_evidence = build_source_evidence(
            &candidate.root_path,
            source,
            evidence.get(&candidate.root_path),
        );
        if let Some(existing) = registry
            .projects
            .iter_mut()
            .find(|project| project.root_path == candidate.root_path)
        {
            let before = existing.source_evidence.len();
            for item in source_evidence {
                let duplicate = existing.source_evidence.iter().any(|current| {
                    current.kind == item.kind
                        && current.conversation_id == item.conversation_id
                        && current.observed_path == item.observed_path
                        && current.parser_version == item.parser_version
                });
                if !duplicate {
                    existing.source_evidence.push(item);
                }
            }
            if existing.source_evidence.len() != before {
                existing.revision += 1;
                existing.updated_at = timestamp();
                changed = true;
            }
            continue;
        }

        registry.projects.push(RegisteredProject {
            id: existing_ids
                .get(&candidate.root_path)
                .copied()
                .unwrap_or_else(Uuid::now_v7),
            name: candidate.name,
            root_path: candidate.root_path,
            kind: candidate.kind,
            name_source: candidate.name_source,
            markers: candidate.markers,
            source,
            source_evidence,
            revision: 1,
            updated_at: timestamp(),
        });
        changed = true;
    }

    if changed {
        registry.revision += 1;
        registry.schema_version = REGISTRY_SCHEMA_VERSION;
        registry.updated_at = timestamp();
        registry.projects.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.root_path.cmp(&right.root_path))
        });
        write_registry(app, &registry)?;
    }

    Ok(registry)
}

fn build_source_evidence(
    root_path: &str,
    source: RegistrationSource,
    evidence: Option<&RegisterProjectEvidence>,
) -> Vec<RegisteredSourceEvidence> {
    let observed_at = timestamp();
    let parser_version = evidence
        .and_then(|item| item.parser_version.clone())
        .unwrap_or_else(|| "manual-v1".to_string());
    let conversation_ids: Vec<String> = evidence
        .map(|item| item.conversation_ids.iter().take(256).cloned().collect())
        .unwrap_or_default();
    if conversation_ids.is_empty() {
        return vec![RegisteredSourceEvidence {
            id: Uuid::now_v7(),
            kind: source,
            conversation_id: None,
            observed_path: root_path.to_string(),
            observed_at,
            parser_version,
        }];
    }
    conversation_ids
        .into_iter()
        .map(|conversation_id| RegisteredSourceEvidence {
            id: Uuid::now_v7(),
            kind: source,
            conversation_id: Some(conversation_id),
            observed_path: root_path.to_string(),
            observed_at: observed_at.clone(),
            parser_version: parser_version.clone(),
        })
        .collect()
}

fn registry_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let directory = app.path().app_data_dir().map_err(storage_error)?;
    fs::create_dir_all(&directory).map_err(storage_error)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).map_err(storage_error)?;
    Ok(directory.join(REGISTRY_FILE))
}

fn write_registry(app: &AppHandle, registry: &ProjectRegistry) -> Result<(), AppError> {
    write_registry_to_path(&registry_path(app)?, registry)
}

fn write_registry_to_path(path: &Path, registry: &ProjectRegistry) -> Result<(), AppError> {
    let temporary_path = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(registry).map_err(storage_error)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(storage_error)?;
    file.write_all(&bytes).map_err(storage_error)?;
    file.sync_all().map_err(storage_error)?;
    fs::rename(&temporary_path, path).map_err(storage_error)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(storage_error)
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn storage_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(error.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProjectRegistry {
    revision: u64,
    updated_at: String,
    projects: Vec<LegacyRegisteredProject>,
}

impl LegacyProjectRegistry {
    fn migrate(self) -> ProjectRegistry {
        ProjectRegistry {
            schema_version: REGISTRY_SCHEMA_VERSION,
            revision: self.revision,
            updated_at: self.updated_at,
            projects: self
                .projects
                .into_iter()
                .map(LegacyRegisteredProject::migrate)
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyRegisteredProject {
    id: Uuid,
    name: String,
    root_path: String,
    kind: ProjectKind,
    name_source: ProjectNameSource,
    markers: Vec<String>,
    revision: u64,
    updated_at: String,
}

impl LegacyRegisteredProject {
    fn migrate(self) -> RegisteredProject {
        let source_evidence =
            build_source_evidence(&self.root_path, RegistrationSource::Manual, None);
        RegisteredProject {
            id: self.id,
            name: self.name,
            root_path: self.root_path,
            kind: self.kind,
            name_source: self.name_source,
            markers: self.markers,
            source: RegistrationSource::Manual,
            source_evidence,
            revision: self.revision,
            updated_at: self.updated_at,
        }
    }
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
                "vibe-inspector-registry-{label}-{}-{unique}",
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

    #[test]
    fn registry_round_trips_through_atomic_file() {
        let directory = TestDirectory::new("round-trip");
        let path = directory.path().join(REGISTRY_FILE);
        let mut registry = ProjectRegistry::empty();
        registry.revision = 1;
        registry.projects.push(RegisteredProject {
            id: Uuid::now_v7(),
            name: "VibeInspector".to_string(),
            root_path: "/tmp/vibe-inspector".to_string(),
            kind: ProjectKind::Repository,
            name_source: ProjectNameSource::Directory,
            markers: vec!["git".to_string()],
            source: RegistrationSource::Manual,
            source_evidence: build_source_evidence(
                "/tmp/vibe-inspector",
                RegistrationSource::Manual,
                None,
            ),
            revision: 1,
            updated_at: timestamp(),
        });

        write_registry_to_path(&path, &registry).expect("write registry");
        let loaded = load_registry_from_path(&path).expect("load registry");

        assert_eq!(loaded.schema_version, REGISTRY_SCHEMA_VERSION);
        assert_eq!(loaded.projects[0].source_evidence.len(), 1);
        assert_eq!(loaded.projects[0].id, registry.projects[0].id);
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn registry_rejects_unknown_schema_versions() {
        let directory = TestDirectory::new("schema-version");
        let path = directory.path().join(REGISTRY_FILE);
        fs::write(
            &path,
            r#"{"schemaVersion":3,"revision":0,"updatedAt":"0","projects":[]}"#,
        )
        .expect("write incompatible registry");

        let error = load_registry_from_path(&path).expect_err("reject incompatible registry");
        assert!(matches!(error, AppError::Storage(_)));
    }

    #[test]
    fn registry_migrates_schema_one_with_manual_evidence() {
        let directory = TestDirectory::new("schema-one");
        let path = directory.path().join(REGISTRY_FILE);
        let project_id = Uuid::now_v7();
        fs::write(
            &path,
            format!(
                r#"{{"schemaVersion":1,"revision":1,"updatedAt":"1","projects":[{{"id":"{project_id}","name":"Fixture","rootPath":"/tmp/fixture","kind":"repository","nameSource":"directory","markers":["git"],"source":"manual","revision":1,"updatedAt":"1"}}]}}"#
            ),
        )
        .expect("write legacy registry");

        let registry = load_registry_from_path(&path).expect("migrate registry");
        assert_eq!(registry.schema_version, REGISTRY_SCHEMA_VERSION);
        assert_eq!(registry.projects[0].source, RegistrationSource::Manual);
        assert_eq!(registry.projects[0].source_evidence.len(), 1);
    }
}
