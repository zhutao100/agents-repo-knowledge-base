use std::collections::BTreeSet;
use std::path::Path;

use crate::error::KbError;
use crate::index::artifacts::TreeRecord;
use crate::io::jsonl::write_jsonl_file;
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::reader::DiffSourceReader;

pub fn write_tree_jsonl(
    repo_root: &Path,
    diff_source: &DiffSource,
    tracked_files: &[String],
    gen_dir: &Path,
) -> Result<(), KbError> {
    let reader = DiffSourceReader::new_at_root(repo_root.to_path_buf(), diff_source.clone());

    let mut dir_paths = BTreeSet::new();
    let mut records = Vec::with_capacity(tracked_files.len());

    for file_path in tracked_files {
        if file_path.ends_with('/') {
            continue;
        }
        for dir in parent_dirs(file_path) {
            dir_paths.insert(dir);
        }

        let repo_path = RepoPath::parse(file_path)?;
        let bytes = reader.read_bytes(&repo_path)?;
        let lines = count_lines(&bytes);

        records.push(TreeRecord {
            path: file_path.to_string(),
            kind: "file".to_string(),
            bytes: Some(bytes.len() as u64),
            lines: Some(lines),
            lang: Some(lang_for_path(file_path)),
            top_symbols: None,
        });
    }

    for dir in dir_paths {
        records.push(TreeRecord {
            path: dir,
            kind: "dir".to_string(),
            bytes: None,
            lines: None,
            lang: None,
            top_symbols: None,
        });
    }

    records.sort_by(|a, b| match a.path.cmp(&b.path) {
        std::cmp::Ordering::Equal => kind_weight(&a.kind).cmp(&kind_weight(&b.kind)),
        other => other,
    });

    std::fs::create_dir_all(gen_dir)
        .map_err(|err| KbError::internal(err, "failed to create kb/gen"))?;
    write_jsonl_file(&gen_dir.join("tree.jsonl"), &records)
        .map_err(|err| KbError::internal(err, "failed to write tree.jsonl"))?;
    Ok(())
}

fn kind_weight(kind: &str) -> u8 {
    match kind {
        "dir" => 0,
        _ => 1,
    }
}

fn parent_dirs(file_path: &str) -> Vec<String> {
    let trimmed = file_path.trim_end_matches('/');
    let mut out = Vec::new();

    let mut parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() <= 1 {
        return out;
    }
    parts.pop();

    let mut current = String::new();
    for part in parts {
        current.push_str(part);
        current.push('/');
        out.push(current.clone());
    }

    out
}

fn count_lines(bytes: &[u8]) -> u64 {
    if bytes.is_empty() {
        return 0;
    }

    let newlines = bytes.iter().filter(|b| **b == b'\n').count() as u64;
    if bytes.last() == Some(&b'\n') {
        newlines
    } else {
        newlines + 1
    }
}

fn lang_for_path(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "go" => "go",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "swift" => "swift",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "sh" => "shell",
        _ => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_dirs_use_trailing_slash() {
        assert_eq!(parent_dirs("src/lib.rs"), vec!["src/".to_string()]);
        assert_eq!(
            parent_dirs("src/repo/path.rs"),
            vec!["src/".to_string(), "src/repo/".to_string()]
        );
        assert_eq!(parent_dirs("Cargo.toml"), Vec::<String>::new());
    }

    #[test]
    fn count_lines_matches_spec() {
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"a"), 1);
        assert_eq!(count_lines(b"a\n"), 1);
        assert_eq!(count_lines(b"a\nb"), 2);
        assert_eq!(count_lines(b"a\nb\n"), 2);
    }
}
