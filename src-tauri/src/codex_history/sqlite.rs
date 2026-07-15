use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{Connection, OpenFlags};

use crate::{
    error::AppError,
    project_discovery::{resolve_project_root, ProjectCandidate},
};

use super::{
    CodexHistoryProvider, CodexHistoryScanResult, CodexProjectProposal, ScanCodexHistoryInput,
    MAX_HISTORY_THREADS, MAX_PROJECT_PROPOSALS, PARSER_VERSION,
};

#[derive(Debug, Clone)]
pub struct SqliteCodexHistoryProvider {
    path: PathBuf,
}

#[derive(Debug)]
struct ThreadRecord {
    id: String,
    cwd: String,
    updated_at: i64,
}

#[derive(Debug)]
struct ProposalAccumulator {
    project: ProjectCandidate,
    conversation_ids: Vec<String>,
    latest_activity: i64,
}

impl SqliteCodexHistoryProvider {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn connect(&self) -> Result<Connection, AppError> {
        if !self.path.is_file() {
            return Err(AppError::CodexHistory(format!(
                "Codex state database is unavailable at {}",
                path_string(&self.path)
            )));
        }
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(history_error)?;
        connection
            .busy_timeout(Duration::from_millis(750))
            .map_err(history_error)?;
        Ok(connection)
    }
}

impl CodexHistoryProvider for SqliteCodexHistoryProvider {
    fn scan(&self, input: &ScanCodexHistoryInput) -> Result<CodexHistoryScanResult, AppError> {
        let connection = self.connect()?;
        let archived_filter = if input.include_archived {
            ""
        } else {
            "AND archived = 0"
        };
        let sql = format!(
            "SELECT id, cwd, COALESCE(updated_at_ms, updated_at, 0)
             FROM threads
             WHERE cwd <> '' {archived_filter}
             ORDER BY COALESCE(updated_at_ms, updated_at, 0) DESC
             LIMIT {}",
            MAX_HISTORY_THREADS + 1
        );
        let mut statement = connection.prepare(&sql).map_err(history_error)?;
        let mut threads = statement
            .query_map([], |row| {
                Ok(ThreadRecord {
                    id: row.get(0)?,
                    cwd: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            })
            .map_err(history_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(history_error)?;
        let truncated_by_threads = threads.len() > MAX_HISTORY_THREADS;
        threads.truncate(MAX_HISTORY_THREADS);

        let mut proposals = BTreeMap::<String, ProposalAccumulator>::new();
        let mut roots_by_cwd = HashMap::<String, Option<ProjectCandidate>>::new();
        let mut ignored_threads = 0usize;
        let mut truncated_by_projects = false;

        for thread in &threads {
            let project = roots_by_cwd
                .entry(thread.cwd.clone())
                .or_insert_with(|| resolve_project_root(Path::new(&thread.cwd)));
            let Some(project) = project.clone() else {
                ignored_threads += 1;
                continue;
            };
            let key = project.root_path.clone();
            if let Some(existing) = proposals.get_mut(&key) {
                existing.conversation_ids.push(thread.id.clone());
                existing.latest_activity = existing.latest_activity.max(thread.updated_at);
                continue;
            }
            if proposals.len() >= MAX_PROJECT_PROPOSALS {
                truncated_by_projects = true;
                ignored_threads += 1;
                continue;
            }
            proposals.insert(
                key,
                ProposalAccumulator {
                    project,
                    conversation_ids: vec![thread.id.clone()],
                    latest_activity: thread.updated_at,
                },
            );
        }

        let mut proposals = proposals
            .into_values()
            .map(|mut proposal| {
                proposal.conversation_ids.sort();
                CodexProjectProposal {
                    project: proposal.project,
                    conversation_ids: proposal.conversation_ids,
                    latest_activity: proposal.latest_activity,
                    parser_version: PARSER_VERSION.to_string(),
                }
            })
            .collect::<Vec<_>>();
        proposals.sort_by(|left, right| {
            right
                .latest_activity
                .cmp(&left.latest_activity)
                .then(left.project.name.cmp(&right.project.name))
        });

        Ok(CodexHistoryScanResult {
            provider: "codex-state-sqlite".to_string(),
            source_path: path_string(&self.path),
            proposals,
            scanned_threads: threads.len(),
            ignored_threads,
            truncated: truncated_by_threads || truncated_by_projects,
        })
    }
}

fn history_error(error: impl std::fmt::Display) -> AppError {
    AppError::CodexHistory(error.to_string())
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

    use rusqlite::params;

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "vibe-inspector-history-{label}-{}-{unique}",
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

    fn create_history_database(path: &Path) -> Connection {
        let connection = Connection::open(path).expect("open history database");
        connection
            .execute_batch(
                "CREATE TABLE threads (
                    id TEXT PRIMARY KEY,
                    cwd TEXT NOT NULL,
                    updated_at INTEGER NOT NULL,
                    updated_at_ms INTEGER,
                    archived INTEGER NOT NULL DEFAULT 0
                );",
            )
            .expect("create threads table");
        connection
    }

    #[test]
    fn groups_nested_conversations_by_repository_root() {
        let directory = TestDirectory::new("grouping");
        let repository = directory.path().join("repository");
        let nested = repository.join("packages/api/src");
        fs::create_dir_all(&nested).expect("create repository");
        fs::create_dir(repository.join(".git")).expect("create git marker");
        fs::write(
            repository.join("package.json"),
            r#"{"name":"fixture","workspaces":["packages/*"]}"#,
        )
        .expect("write manifest");

        let state_path = directory.path().join("state.sqlite");
        let connection = create_history_database(&state_path);
        connection
            .execute(
                "INSERT INTO threads (id, cwd, updated_at, updated_at_ms, archived)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["thread-1", nested.to_string_lossy(), 10, 1000, 0],
            )
            .expect("insert first thread");
        connection
            .execute(
                "INSERT INTO threads (id, cwd, updated_at, updated_at_ms, archived)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["thread-2", repository.to_string_lossy(), 20, 2000, 0],
            )
            .expect("insert second thread");
        connection
            .execute(
                "INSERT INTO threads (id, cwd, updated_at, updated_at_ms, archived)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["archived", repository.to_string_lossy(), 30, 3000, 1],
            )
            .expect("insert archived thread");
        drop(connection);

        let provider = SqliteCodexHistoryProvider::new(state_path);
        let result = provider
            .scan(&ScanCodexHistoryInput {
                include_archived: false,
            })
            .expect("scan history");

        assert_eq!(result.scanned_threads, 2);
        assert_eq!(result.proposals.len(), 1);
        assert_eq!(result.proposals[0].project.name, "repository");
        assert_eq!(result.proposals[0].conversation_ids.len(), 2);
        assert_eq!(result.proposals[0].latest_activity, 2000);
    }

    #[test]
    fn ignores_missing_and_unrecognized_working_directories() {
        let directory = TestDirectory::new("ignored");
        let empty = directory.path().join("empty");
        fs::create_dir(&empty).expect("create empty directory");
        let state_path = directory.path().join("state.sqlite");
        let connection = create_history_database(&state_path);
        for (id, cwd) in [
            ("missing", directory.path().join("missing")),
            ("empty", empty),
        ] {
            connection
                .execute(
                    "INSERT INTO threads (id, cwd, updated_at, archived) VALUES (?1, ?2, 1, 0)",
                    params![id, cwd.to_string_lossy()],
                )
                .expect("insert ignored thread");
        }
        drop(connection);

        let result = SqliteCodexHistoryProvider::new(state_path)
            .scan(&ScanCodexHistoryInput {
                include_archived: false,
            })
            .expect("scan history");
        assert!(result.proposals.is_empty());
        assert_eq!(result.ignored_threads, 2);
    }
}
