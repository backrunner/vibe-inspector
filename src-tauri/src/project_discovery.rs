use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

const MAX_SCAN_DEPTH: usize = 3;
const MAX_VISITED_DIRECTORIES: usize = 4_096;
const MAX_CANDIDATES: usize = 200;
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".next",
    ".svelte-kit",
    ".turbo",
    "build",
    "coverage",
    "DerivedData",
    "dist",
    "node_modules",
    "out",
    "Pods",
    "target",
    "vendor",
];

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiscoveryMode {
    Collection,
    SingleProject,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverProjectsInput {
    pub root_path: String,
    pub mode: DiscoveryMode,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectKind {
    Monorepo,
    Repository,
    Project,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectNameSource {
    PackageJson,
    CargoToml,
    PyprojectToml,
    DenoJson,
    ComposerJson,
    GoModule,
    Directory,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCandidate {
    pub name: String,
    pub root_path: String,
    pub relative_path: String,
    pub kind: ProjectKind,
    pub name_source: ProjectNameSource,
    pub markers: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiscoveryResult {
    pub selected_root: String,
    pub mode: DiscoveryMode,
    pub candidates: Vec<ProjectCandidate>,
    pub visited_directories: usize,
    pub truncated: bool,
}

pub fn discover_projects(input: DiscoverProjectsInput) -> Result<ProjectDiscoveryResult, AppError> {
    let selected_root = fs::canonicalize(&input.root_path)
        .map_err(|_| AppError::InvalidPath(input.root_path.clone()))?;
    if !selected_root.is_dir() {
        return Err(AppError::NotDirectory(path_string(&selected_root)));
    }

    let mut candidates = Vec::new();
    let mut visited_directories = 1;
    let mut truncated = false;

    match input.mode {
        DiscoveryMode::SingleProject => {
            let candidate = inspect_project_root(&selected_root, &selected_root)
                .ok_or_else(|| AppError::NotProjectRoot(path_string(&selected_root)))?;
            candidates.push(candidate);
        }
        DiscoveryMode::Collection => {
            if let Some(candidate) = inspect_project_root(&selected_root, &selected_root) {
                candidates.push(candidate);
            } else {
                let mut queue = VecDeque::new();
                enqueue_directories(&selected_root, 1, &mut queue);

                while let Some((directory, depth)) = queue.pop_front() {
                    if visited_directories >= MAX_VISITED_DIRECTORIES
                        || candidates.len() >= MAX_CANDIDATES
                    {
                        truncated = true;
                        break;
                    }
                    visited_directories += 1;

                    if let Some(candidate) = inspect_project_root(&directory, &selected_root) {
                        candidates.push(candidate);
                        continue;
                    }

                    if depth < MAX_SCAN_DEPTH {
                        enqueue_directories(&directory, depth + 1, &mut queue);
                    }
                }
            }
        }
    }

    candidates.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    Ok(ProjectDiscoveryResult {
        selected_root: path_string(&selected_root),
        mode: input.mode,
        candidates,
        visited_directories,
        truncated,
    })
}

pub fn resolve_project_root(start: &Path) -> Option<ProjectCandidate> {
    let canonical = fs::canonicalize(start).ok()?;
    let starting_directory = if canonical.is_dir() {
        canonical
    } else {
        canonical.parent()?.to_path_buf()
    };
    let mut fallback = None;
    let mut workspace = None;

    for ancestor in starting_directory.ancestors().take(16) {
        let Some(candidate) = inspect_project_root_shallow(ancestor) else {
            continue;
        };
        if candidate.markers.iter().any(|marker| marker == "git") {
            return Some(candidate);
        }
        if candidate.kind == ProjectKind::Monorepo && workspace.is_none() {
            workspace = Some(candidate.clone());
        }
        if fallback.is_none() {
            fallback = Some(candidate);
        }
    }

    workspace.or(fallback)
}

fn inspect_project_root_shallow(path: &Path) -> Option<ProjectCandidate> {
    let mut markers = Vec::new();
    let mut workspace = false;

    if marker_exists(&path.join(".git")) {
        markers.push("git".to_string());
    }
    for marker in [
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "deno.json",
        "composer.json",
        "go.mod",
    ] {
        if marker_is_file(&path.join(marker)) {
            markers.push(marker.to_string());
        }
    }
    for marker in ["pnpm-workspace.yaml", "lerna.json", "nx.json", "turbo.json"] {
        if marker_is_file(&path.join(marker)) {
            markers.push(marker.to_string());
            workspace = true;
        }
    }
    if markers.is_empty() {
        return None;
    }

    let name = path
        .file_name()
        .and_then(|name| clean_name(&name.to_string_lossy()))?;
    let kind = if workspace {
        ProjectKind::Monorepo
    } else if markers.iter().any(|marker| marker == "git") {
        ProjectKind::Repository
    } else {
        ProjectKind::Project
    };
    Some(ProjectCandidate {
        name,
        root_path: path_string(path),
        relative_path: ".".to_string(),
        kind,
        name_source: ProjectNameSource::Directory,
        markers,
    })
}

fn marker_exists(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn marker_is_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn enqueue_directories(root: &Path, depth: usize, queue: &mut VecDeque<(PathBuf, usize)>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut directories = entries
        .flatten()
        .filter_map(|entry| {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if should_skip_directory(&name) {
                return None;
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).ok()?;
            (metadata.is_dir() && !metadata.file_type().is_symlink()).then_some(path)
        })
        .collect::<Vec<_>>();
    directories.sort();
    queue.extend(directories.into_iter().map(|path| (path, depth)));
}

fn should_skip_directory(name: &str) -> bool {
    name.starts_with('.') || SKIPPED_DIRECTORIES.contains(&name)
}

fn inspect_project_root(path: &Path, selection_root: &Path) -> Option<ProjectCandidate> {
    let mut markers = Vec::new();
    let mut workspace = false;
    let mut preferred_name: Option<(String, ProjectNameSource)> = None;

    if path.join(".git").exists() {
        markers.push("git".to_string());
    }

    let package_json = path.join("package.json");
    if package_json.is_file() {
        markers.push("package.json".to_string());
        preferred_name =
            json_manifest_name(&package_json).map(|name| (name, ProjectNameSource::PackageJson));
    }

    let deno_json = path.join("deno.json");
    if deno_json.is_file() {
        markers.push("deno.json".to_string());
        if preferred_name.is_none() {
            preferred_name =
                json_manifest_name(&deno_json).map(|name| (name, ProjectNameSource::DenoJson));
        }
    }

    let composer_json = path.join("composer.json");
    if composer_json.is_file() {
        markers.push("composer.json".to_string());
        if preferred_name.is_none() {
            preferred_name = json_manifest_name(&composer_json)
                .map(|name| (name, ProjectNameSource::ComposerJson));
        }
    }

    let cargo_toml = path.join("Cargo.toml");
    if cargo_toml.is_file() {
        markers.push("Cargo.toml".to_string());
        if let Some(value) = toml_manifest(&cargo_toml) {
            workspace = value.get("workspace").is_some();
            if preferred_name.is_none() {
                preferred_name = value
                    .get("package")
                    .and_then(|package| package.get("name"))
                    .and_then(|name| name.as_str())
                    .and_then(clean_name)
                    .map(|name| (name, ProjectNameSource::CargoToml));
            }
        }
    }

    let pyproject_toml = path.join("pyproject.toml");
    if pyproject_toml.is_file() {
        markers.push("pyproject.toml".to_string());
        if preferred_name.is_none() {
            preferred_name = toml_manifest(&pyproject_toml)
                .and_then(|value| {
                    value
                        .get("project")
                        .and_then(|project| project.get("name"))
                        .and_then(|name| name.as_str())
                        .and_then(clean_name)
                })
                .map(|name| (name, ProjectNameSource::PyprojectToml));
        }
    }

    let go_mod = path.join("go.mod");
    if go_mod.is_file() {
        markers.push("go.mod".to_string());
        if preferred_name.is_none() {
            preferred_name =
                go_module_name(&go_mod).map(|name| (name, ProjectNameSource::GoModule));
        }
    }

    for manifest in ["Package.swift", "Gemfile", "mix.exs"] {
        if path.join(manifest).is_file() {
            markers.push(manifest.to_string());
        }
    }

    for marker in [
        "pnpm-workspace.yaml",
        "pnpm-workspace.yml",
        "turbo.json",
        "nx.json",
        "lerna.json",
        "rush.json",
    ] {
        if path.join(marker).is_file() {
            workspace = true;
            markers.push(marker.to_string());
        }
    }

    if markers.is_empty() {
        return None;
    }

    markers.sort();
    let directory_name = path.file_name()?.to_string_lossy().trim().to_string();
    let (name, name_source) =
        preferred_name.unwrap_or_else(|| (directory_name.clone(), ProjectNameSource::Directory));
    let relative_path = path
        .strip_prefix(selection_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(path_string)
        .unwrap_or_else(|| ".".to_string());
    let kind = if workspace {
        ProjectKind::Monorepo
    } else if path.join(".git").exists() {
        ProjectKind::Repository
    } else {
        ProjectKind::Project
    };

    Some(ProjectCandidate {
        name,
        root_path: path_string(path),
        relative_path,
        kind,
        name_source,
        markers,
    })
}

fn json_manifest_name(path: &Path) -> Option<String> {
    let text = read_small_text(path)?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    value.get("name")?.as_str().and_then(clean_name)
}

fn toml_manifest(path: &Path) -> Option<toml::Value> {
    read_small_text(path)?.parse::<toml::Value>().ok()
}

fn go_module_name(path: &Path) -> Option<String> {
    let text = read_small_text(path)?;
    let module = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("module "))?;
    let name = module.trim().trim_end_matches('/').rsplit('/').next()?;
    clean_name(name)
}

fn read_small_text(path: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn clean_name(value: &str) -> Option<String> {
    let name = value.trim();
    (!name.is_empty() && name.len() <= 160).then(|| name.to_string())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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
                "vibe-inspector-{label}-{}-{unique}",
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

    fn write(path: impl AsRef<Path>, value: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, value).expect("write fixture");
    }

    #[test]
    fn collection_scan_prunes_projects_before_nested_packages() {
        let root = TestDirectory::new("collection");
        write(
            root.path().join("commerce/package.json"),
            r#"{"name":"commerce-suite","private":true}"#,
        );
        write(
            root.path().join("commerce/pnpm-workspace.yaml"),
            "packages:\n  - packages/*\n",
        );
        write(
            root.path().join("commerce/packages/payments/package.json"),
            r#"{"name":"@commerce/payments"}"#,
        );
        write(
            root.path().join("teams/platform/worker/Cargo.toml"),
            "[package]\nname = \"worker-service\"\nversion = \"0.1.0\"\n",
        );
        write(root.path().join("notes/readme.txt"), "not a project");

        let result = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::Collection,
        })
        .expect("discover collection");

        let names = result
            .candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["commerce-suite", "worker-service"]);
        assert!(result
            .candidates
            .iter()
            .all(|candidate| !candidate.root_path.contains("packages/payments")));
        assert_eq!(result.candidates[0].kind, ProjectKind::Monorepo);
    }

    #[test]
    fn single_project_does_not_promote_nested_manifests() {
        let root = TestDirectory::new("single-invalid");
        write(
            root.path().join("nested/package.json"),
            r#"{"name":"nested-package"}"#,
        );

        let error = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::SingleProject,
        })
        .expect_err("selected root should be rejected");

        assert!(matches!(error, AppError::NotProjectRoot(_)));
    }

    #[test]
    fn manifest_name_precedes_directory_name() {
        let root = TestDirectory::new("manifest-name");
        write(
            root.path().join("package.json"),
            r#"{"name":"@acme/inspector"}"#,
        );

        let result = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::SingleProject,
        })
        .expect("discover project");

        assert_eq!(result.candidates[0].name, "@acme/inspector");
        assert_eq!(
            result.candidates[0].name_source,
            ProjectNameSource::PackageJson
        );
    }

    #[test]
    fn git_root_falls_back_to_directory_name() {
        let root = TestDirectory::new("git-name");
        fs::create_dir(root.path().join(".git")).expect("create git marker");

        let result = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::SingleProject,
        })
        .expect("discover git root");

        assert_eq!(
            result.candidates[0].name_source,
            ProjectNameSource::Directory
        );
    }

    #[test]
    fn collection_scan_respects_maximum_depth() {
        let root = TestDirectory::new("max-depth");
        write(
            root.path().join("one/two/visible/package.json"),
            r#"{"name":"visible-project"}"#,
        );
        write(
            root.path().join("one/two/three/hidden/package.json"),
            r#"{"name":"too-deep"}"#,
        );

        let result = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::Collection,
        })
        .expect("discover bounded collection");

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "visible-project");
    }

    #[test]
    fn collection_scan_skips_hidden_and_generated_directories() {
        let root = TestDirectory::new("ignored-directories");
        write(
            root.path().join("apps/visible/package.json"),
            r#"{"name":"visible-project"}"#,
        );
        for directory in [".hidden", "dist", "node_modules", "target"] {
            write(
                root.path().join(directory).join("ignored/package.json"),
                r#"{"name":"ignored-project"}"#,
            );
        }

        let result = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::Collection,
        })
        .expect("discover collection without generated output");

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "visible-project");
    }

    #[cfg(unix)]
    #[test]
    fn collection_scan_does_not_follow_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let root = TestDirectory::new("symlink-root");
        let external = TestDirectory::new("symlink-external");
        write(
            external.path().join("package.json"),
            r#"{"name":"external-project"}"#,
        );
        symlink(external.path(), root.path().join("linked-project")).expect("create symlink");

        let result = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::Collection,
        })
        .expect("discover collection without following symlinks");

        assert!(result.candidates.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn manifest_reader_rejects_file_symlinks() {
        use std::os::unix::fs::symlink;

        let root = TestDirectory::new("manifest-symlink");
        fs::create_dir(root.path().join(".git")).expect("create git marker");
        let external = root.path().join("external.json");
        write(&external, r#"{"name":"linked-name"}"#);
        symlink(&external, root.path().join("package.json")).expect("create manifest symlink");

        let result = discover_projects(DiscoverProjectsInput {
            root_path: path_string(root.path()),
            mode: DiscoveryMode::SingleProject,
        })
        .expect("discover root without following manifest symlink");

        assert_eq!(
            result.candidates[0].name_source,
            ProjectNameSource::Directory
        );
    }
}
