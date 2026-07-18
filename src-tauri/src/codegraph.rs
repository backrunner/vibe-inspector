use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags};

use crate::{
    error::AppError,
    fs_broker::{canonical_root, validated_relative_path, validated_source_path},
};

const CODEGRAPH_DATABASE: &str = ".codegraph/codegraph.db";
const MAX_INPUT_FILES: usize = 64;
const MAX_RELATED_FILES: usize = 80;

const DEPENDENTS_SQL: &str = "
    SELECT DISTINCT src.file_path
    FROM edges e
    JOIN nodes tgt ON tgt.id = e.target
    JOIN nodes src ON src.id = e.source
    WHERE tgt.file_path = ?1
      AND e.kind != 'contains'
      AND src.file_path != ?2
";

const DEPENDENCIES_SQL: &str = "
    SELECT DISTINCT tgt.file_path
    FROM edges e
    JOIN nodes src ON src.id = e.source
    JOIN nodes tgt ON tgt.id = e.target
    WHERE src.file_path = ?1
      AND e.kind != 'contains'
      AND tgt.file_path != ?2
";

pub fn related_files(root: &Path, source_paths: &[String]) -> Result<Vec<String>, AppError> {
    let canonical_root = canonical_root(root)?;
    let database_path = canonical_root.join(CODEGRAPH_DATABASE);
    if !database_path.is_file() {
        return Ok(Vec::new());
    }
    let canonical_database = match fs::canonicalize(&database_path) {
        Ok(path) if path.starts_with(&canonical_root) => path,
        _ => return Ok(Vec::new()),
    };
    let connection = match Connection::open_with_flags(
        canonical_database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(connection) => connection,
        Err(_) => return Ok(Vec::new()),
    };

    let normalized_sources = source_paths
        .iter()
        .take(MAX_INPUT_FILES)
        .filter_map(|path| normalize_relative_path(path).ok())
        .collect::<BTreeSet<_>>();
    if normalized_sources.is_empty() {
        return Ok(Vec::new());
    }

    let candidates = match query_related_files(&connection, &normalized_sources) {
        Ok(paths) => paths,
        Err(_) => return Ok(Vec::new()),
    };
    let related = candidates
        .into_iter()
        .filter(|path| !normalized_sources.contains(path))
        .filter(|path| validated_source_path(&canonical_root, path).is_ok())
        .take(MAX_RELATED_FILES)
        .collect();

    Ok(related)
}

fn query_related_files(
    connection: &Connection,
    source_paths: &BTreeSet<String>,
) -> rusqlite::Result<BTreeSet<String>> {
    let mut dependents = connection.prepare(DEPENDENTS_SQL)?;
    let mut dependencies = connection.prepare(DEPENDENCIES_SQL)?;
    let mut related = BTreeSet::new();

    for path in source_paths {
        for statement in [&mut dependents, &mut dependencies] {
            let rows = statement.query_map(params![path, path], |row| row.get::<_, String>(0))?;
            for row in rows {
                if let Ok(path) = row.and_then(|value| {
                    normalize_relative_path(&value)
                        .map_err(|_| rusqlite::Error::InvalidPath(PathBuf::from(value)))
                }) {
                    related.insert(path);
                }
            }
        }
    }

    Ok(related)
}

fn normalize_relative_path(value: &str) -> Result<String, AppError> {
    let normalized = value.strip_prefix("./").unwrap_or(value);
    validated_relative_path(normalized).map(|path| path.to_string_lossy().into_owned())
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
                "vibe-inspector-codegraph-{label}-{}-{unique}",
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
    fn returns_dependencies_and_dependents_from_an_existing_index() {
        let directory = TestDirectory::new("related");
        fs::create_dir_all(directory.path().join("src")).expect("create sources");
        fs::create_dir_all(directory.path().join(".codegraph")).expect("create index directory");
        for file in ["a.rs", "b.rs", "c.rs"] {
            fs::write(directory.path().join("src").join(file), file).expect("write source");
        }
        let connection = Connection::open(directory.path().join(CODEGRAPH_DATABASE))
            .expect("open codegraph fixture");
        connection
            .execute_batch(
                "
                CREATE TABLE nodes (id INTEGER PRIMARY KEY, file_path TEXT NOT NULL);
                CREATE TABLE edges (source INTEGER NOT NULL, target INTEGER NOT NULL, kind TEXT NOT NULL);
                INSERT INTO nodes (id, file_path) VALUES
                  (1, 'src/a.rs'), (2, 'src/b.rs'), (3, 'src/c.rs'), (4, '../outside.rs');
                INSERT INTO edges (source, target, kind) VALUES
                  (2, 1, 'calls'), (1, 3, 'imports'), (1, 4, 'imports'), (3, 1, 'contains');
                ",
            )
            .expect("seed codegraph fixture");
        drop(connection);

        let related = related_files(directory.path(), &["src/a.rs".to_string()])
            .expect("query related files");

        assert_eq!(related, vec!["src/b.rs", "src/c.rs"]);
    }

    #[test]
    fn missing_or_incompatible_indexes_do_not_break_source_evidence() {
        let directory = TestDirectory::new("fallback");
        assert!(related_files(directory.path(), &["src/a.rs".to_string()])
            .expect("missing index fallback")
            .is_empty());

        fs::create_dir_all(directory.path().join(".codegraph")).expect("create index directory");
        Connection::open(directory.path().join(CODEGRAPH_DATABASE))
            .expect("open incompatible database");
        assert!(related_files(directory.path(), &["src/a.rs".to_string()])
            .expect("incompatible index fallback")
            .is_empty());
    }
}
