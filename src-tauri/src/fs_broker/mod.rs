use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::AppError;

const MAX_SOURCE_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RETURNED_BYTES: usize = 512 * 1024;
const MAX_RETURNED_LINES: usize = 500;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSourceInput {
    pub relative_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub expected_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSourceFileInput {
    pub relative_path: String,
    pub expected_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub relative_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub content_hash: String,
    pub status: SourceRangeStatus,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SourceRangeStatus {
    Current,
    Stale,
}

pub fn read_source_range(root: &Path, input: ReadSourceInput) -> Result<SourceRange, AppError> {
    validate_line_range(input.start_line, input.end_line)?;
    let (relative_path, bytes) = read_validated_source(root, &input.relative_path)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|_| AppError::SourceUnreadable(input.relative_path.clone()))?;
    let content_hash = sha256_hex(&bytes);
    let mut selected = String::new();
    let mut actual_end = input.start_line.saturating_sub(1);

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        if line_number < input.start_line {
            continue;
        }
        if line_number > input.end_line {
            break;
        }
        if !selected.is_empty() {
            selected.push('\n');
        }
        selected.push_str(line);
        actual_end = line_number;
        if selected.len() > MAX_RETURNED_BYTES {
            return Err(AppError::SourceRangeTooLarge {
                bytes: selected.len(),
                limit: MAX_RETURNED_BYTES,
            });
        }
    }

    if actual_end < input.start_line {
        return Err(AppError::InvalidLineRange {
            start: input.start_line,
            end: input.end_line,
        });
    }
    let status = match input.expected_hash {
        Some(expected) if expected != content_hash => SourceRangeStatus::Stale,
        _ => SourceRangeStatus::Current,
    };

    Ok(SourceRange {
        relative_path: path_string(&relative_path),
        start_line: input.start_line,
        end_line: actual_end,
        text: selected,
        content_hash,
        status,
    })
}

pub fn read_source_file(root: &Path, input: ReadSourceFileInput) -> Result<SourceRange, AppError> {
    let (relative_path, bytes) = read_validated_source(root, &input.relative_path)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|_| AppError::SourceUnreadable(input.relative_path.clone()))?;
    let content_hash = sha256_hex(&bytes);
    let status = match input.expected_hash {
        Some(expected) if expected != content_hash => SourceRangeStatus::Stale,
        _ => SourceRangeStatus::Current,
    };

    Ok(SourceRange {
        relative_path: path_string(&relative_path),
        start_line: 1,
        end_line: source.split('\n').count(),
        text: source.to_string(),
        content_hash,
        status,
    })
}

fn validate_line_range(start_line: usize, end_line: usize) -> Result<(), AppError> {
    let line_count = end_line.saturating_sub(start_line).saturating_add(1);
    if start_line == 0 || end_line < start_line || line_count > MAX_RETURNED_LINES {
        return Err(AppError::InvalidLineRange {
            start: start_line,
            end: end_line,
        });
    }
    Ok(())
}

pub(crate) fn canonical_root(root: &Path) -> Result<PathBuf, AppError> {
    let canonical = fs::canonicalize(root).map_err(|_| AppError::InvalidPath(path_string(root)))?;
    if !canonical.is_dir() {
        return Err(AppError::NotDirectory(path_string(&canonical)));
    }
    Ok(canonical)
}

pub(crate) fn validated_relative_path(value: &str) -> Result<PathBuf, AppError> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        return Err(AppError::PathTraversal(value.to_string()));
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::PathTraversal(value.to_string()));
            }
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(AppError::PathTraversal(value.to_string()));
    }
    Ok(clean)
}

pub(crate) fn validated_source_path(root: &Path, value: &str) -> Result<PathBuf, AppError> {
    let canonical_root = canonical_root(root)?;
    let relative_path = validated_relative_path(value)?;
    let canonical_file = fs::canonicalize(canonical_root.join(&relative_path))
        .map_err(|_| AppError::SourceUnreadable(value.to_string()))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(AppError::PathTraversal(value.to_string()));
    }
    if !fs::metadata(&canonical_file)
        .map_err(|_| AppError::SourceUnreadable(value.to_string()))?
        .is_file()
    {
        return Err(AppError::SourceUnreadable(value.to_string()));
    }
    Ok(relative_path)
}

fn read_validated_source(root: &Path, value: &str) -> Result<(PathBuf, Vec<u8>), AppError> {
    let canonical_root = canonical_root(root)?;
    let relative_path = validated_relative_path(value)?;
    let canonical_file = fs::canonicalize(canonical_root.join(&relative_path))
        .map_err(|_| AppError::SourceUnreadable(value.to_string()))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(AppError::PathTraversal(value.to_string()));
    }
    let metadata =
        fs::metadata(&canonical_file).map_err(|_| AppError::SourceUnreadable(value.to_string()))?;
    if !metadata.is_file() {
        return Err(AppError::SourceUnreadable(value.to_string()));
    }
    if metadata.len() > MAX_SOURCE_FILE_BYTES {
        return Err(AppError::SourceTooLarge {
            path: value.to_string(),
            bytes: metadata.len(),
            limit: MAX_SOURCE_FILE_BYTES,
        });
    }
    let bytes =
        fs::read(&canonical_file).map_err(|_| AppError::SourceUnreadable(value.to_string()))?;
    Ok((relative_path, bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
                "vibe-inspector-fs-{label}-{}-{unique}",
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
    fn reads_a_bounded_range_and_detects_stale_hashes() {
        let directory = TestDirectory::new("range");
        fs::write(
            directory.path().join("source.rs"),
            "one\ntwo\nthree\nfour\n",
        )
        .expect("write source");
        let current = read_source_range(
            directory.path(),
            ReadSourceInput {
                relative_path: "source.rs".to_string(),
                start_line: 2,
                end_line: 3,
                expected_hash: None,
            },
        )
        .expect("read range");
        assert_eq!(current.text, "two\nthree");
        assert_eq!(current.end_line, 3);
        assert_eq!(current.status, SourceRangeStatus::Current);

        let stale = read_source_range(
            directory.path(),
            ReadSourceInput {
                relative_path: "source.rs".to_string(),
                start_line: 1,
                end_line: 1,
                expected_hash: Some("outdated".to_string()),
            },
        )
        .expect("read stale range");
        assert_eq!(stale.status, SourceRangeStatus::Stale);
        assert_eq!(stale.content_hash.len(), 64);
    }

    #[test]
    fn reads_a_complete_file_and_preserves_trailing_lines() {
        let directory = TestDirectory::new("complete-file");
        fs::write(directory.path().join("source.rs"), "one\ntwo\nthree\n").expect("write source");

        let source = read_source_file(
            directory.path(),
            ReadSourceFileInput {
                relative_path: "source.rs".to_string(),
                expected_hash: None,
            },
        )
        .expect("read complete file");

        assert_eq!(source.start_line, 1);
        assert_eq!(source.end_line, 4);
        assert_eq!(source.text, "one\ntwo\nthree\n");
        assert_eq!(source.status, SourceRangeStatus::Current);
    }

    #[test]
    fn complete_file_reads_handle_empty_text_and_stale_hashes() {
        let directory = TestDirectory::new("empty-file");
        fs::write(directory.path().join("empty.txt"), "").expect("write empty source");

        let source = read_source_file(
            directory.path(),
            ReadSourceFileInput {
                relative_path: "empty.txt".to_string(),
                expected_hash: Some("outdated".to_string()),
            },
        )
        .expect("read empty file");

        assert_eq!(source.end_line, 1);
        assert!(source.text.is_empty());
        assert_eq!(source.status, SourceRangeStatus::Stale);
    }

    #[test]
    fn rejects_parent_traversal_and_large_ranges() {
        let directory = TestDirectory::new("validation");
        assert!(matches!(
            read_source_range(
                directory.path(),
                ReadSourceInput {
                    relative_path: "../secret".to_string(),
                    start_line: 1,
                    end_line: 1,
                    expected_hash: None,
                }
            ),
            Err(AppError::PathTraversal(_))
        ));
        assert!(matches!(
            read_source_range(
                directory.path(),
                ReadSourceInput {
                    relative_path: "source.rs".to_string(),
                    start_line: 1,
                    end_line: MAX_RETURNED_LINES + 1,
                    expected_hash: None,
                }
            ),
            Err(AppError::InvalidLineRange { .. })
        ));
        assert!(matches!(
            read_source_file(
                directory.path(),
                ReadSourceFileInput {
                    relative_path: "../secret".to_string(),
                    expected_hash: None,
                }
            ),
            Err(AppError::PathTraversal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_registered_root() {
        use std::os::unix::fs::symlink;

        let root = TestDirectory::new("symlink-root");
        let outside = TestDirectory::new("symlink-outside");
        let secret = outside.path().join("secret.txt");
        fs::write(&secret, "secret").expect("write secret");
        symlink(&secret, root.path().join("escape.txt")).expect("create symlink");

        let result = read_source_range(
            root.path(),
            ReadSourceInput {
                relative_path: "escape.txt".to_string(),
                start_line: 1,
                end_line: 1,
                expected_hash: None,
            },
        );
        assert!(matches!(result, Err(AppError::PathTraversal(_))));

        let complete_result = read_source_file(
            root.path(),
            ReadSourceFileInput {
                relative_path: "escape.txt".to_string(),
                expected_hash: None,
            },
        );
        assert!(matches!(
            complete_result,
            Err(AppError::PathTraversal(_))
        ));
    }

    #[test]
    fn rejects_files_above_the_byte_limit() {
        let directory = TestDirectory::new("large-file");
        let path = directory.path().join("large.txt");
        let file = fs::File::create(&path).expect("create large file");
        file.set_len(MAX_SOURCE_FILE_BYTES + 1)
            .expect("size large file");

        let result = read_source_range(
            directory.path(),
            ReadSourceInput {
                relative_path: "large.txt".to_string(),
                start_line: 1,
                end_line: 1,
                expected_hash: None,
            },
        );
        assert!(matches!(result, Err(AppError::SourceTooLarge { .. })));

        let complete_result = read_source_file(
            directory.path(),
            ReadSourceFileInput {
                relative_path: "large.txt".to_string(),
                expected_hash: None,
            },
        );
        assert!(matches!(
            complete_result,
            Err(AppError::SourceTooLarge { .. })
        ));
    }
}
