use std::collections::BTreeSet;
use std::path::Path;

use crate::error::KbError;
use crate::repo::diff::parse_name_status_z;
use crate::repo::diff_source::DiffSource;
use crate::repo::git::git_output;

const EXCLUDED_PREFIXES: [&str; 3] = ["kb/gen/", "kb/cache/", "kb/.tmp/"];

pub fn list_tracked_file_paths(
    repo_root: &Path,
    diff_source: &DiffSource,
) -> Result<Vec<String>, KbError> {
    let paths = match diff_source {
        DiffSource::Staged | DiffSource::Worktree => git_output(repo_root, &["ls-files", "-z"])?,
        DiffSource::Commit(sha) => {
            git_output(repo_root, &["ls-tree", "-r", "--name-only", "-z", sha])?
        }
    };

    let mut out = split_nul_terminated_paths(&paths)?;
    out.retain(|path| !is_excluded_path(path));

    if matches!(diff_source, DiffSource::Worktree) {
        let deleted = deleted_paths_in_worktree(repo_root)?;
        out.retain(|path| !deleted.contains(path));
    }

    out.sort();
    out.dedup();
    Ok(out)
}

fn deleted_paths_in_worktree(repo_root: &Path) -> Result<BTreeSet<String>, KbError> {
    let bytes = git_output(
        repo_root,
        &["diff", "--name-status", "-z", "--find-renames"],
    )?;
    let entries = parse_name_status_z(&bytes)?;

    let mut deleted = BTreeSet::new();
    for (status, paths) in entries {
        if status.starts_with('D') || status.starts_with('R') {
            deleted.insert(paths[0].clone());
        }
    }

    Ok(deleted)
}

fn split_nul_terminated_paths(bytes: &[u8]) -> Result<Vec<String>, KbError> {
    let mut paths = split_nul(bytes)?;
    if paths.last().is_some_and(|p| p.is_empty()) {
        paths.pop();
    }
    Ok(paths.into_iter().map(|s| s.to_string()).collect())
}

fn split_nul(bytes: &[u8]) -> Result<Vec<&str>, KbError> {
    let text = std::str::from_utf8(bytes).map_err(|err| {
        KbError::backend_failed("git output is not valid utf-8")
            .with_detail("cause", err.to_string())
    })?;
    Ok(text.split('\0').collect())
}

fn is_excluded_path(path: &str) -> bool {
    EXCLUDED_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}
