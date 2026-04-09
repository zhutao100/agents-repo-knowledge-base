use std::path::Path;
use std::process::Command;

use crate::error::KbError;
use crate::repo::diff_source::DiffSource;
use crate::repo::git::git_output;

const EMPTY_TREE_SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ChangeKind {
    Add,
    Modify,
    Delete,
    Rename,
    Unknown,
}

impl ChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeKind::Add => "add",
            ChangeKind::Modify => "modify",
            ChangeKind::Delete => "delete",
            ChangeKind::Rename => "rename",
            ChangeKind::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct DiffPathChange {
    pub path: String,
    pub change_kind: ChangeKind,
}

pub fn list_changed_paths(
    repo_root: &Path,
    diff_source: &DiffSource,
) -> Result<Vec<DiffPathChange>, KbError> {
    let entries = match diff_source {
        DiffSource::Staged => {
            if !head_exists(repo_root)? {
                return Ok(Vec::new());
            }
            git_output(
                repo_root,
                &["diff", "--cached", "--name-status", "-z", "--find-renames"],
            )?
        }
        DiffSource::Worktree => {
            if !head_exists(repo_root)? {
                return Ok(Vec::new());
            }
            git_output(
                repo_root,
                &["diff", "HEAD", "--name-status", "-z", "--find-renames"],
            )?
        }
        DiffSource::Commit(sha) => {
            let base = commit_base(repo_root, sha)?;
            git_output(
                repo_root,
                &["diff", &base, sha, "--name-status", "-z", "--find-renames"],
            )?
        }
    };

    let parsed = parse_name_status_z(&entries)?;

    let mut out = Vec::new();
    for (status, paths) in parsed {
        let kind = change_kind_from_status(&status);
        if kind == ChangeKind::Rename && paths.len() == 2 {
            out.push(DiffPathChange {
                path: paths[0].clone(),
                change_kind: ChangeKind::Rename,
            });
            out.push(DiffPathChange {
                path: paths[1].clone(),
                change_kind: ChangeKind::Rename,
            });
        } else if !paths.is_empty() {
            out.push(DiffPathChange {
                path: paths[0].clone(),
                change_kind: kind,
            });
        }
    }

    out.sort_by(|a, b| match a.path.cmp(&b.path) {
        std::cmp::Ordering::Equal => a.change_kind.cmp(&b.change_kind),
        other => other,
    });
    out.dedup();
    Ok(out)
}

pub fn parse_name_status_z(bytes: &[u8]) -> Result<Vec<(String, Vec<String>)>, KbError> {
    let mut parts = split_nul(bytes)?;
    if parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }

    let mut out = Vec::new();
    let mut idx = 0;
    while idx < parts.len() {
        let status = parts[idx].to_string();
        idx += 1;

        let paths_needed = if status.starts_with('R') || status.starts_with('C') {
            2
        } else {
            1
        };

        if idx + paths_needed > parts.len() {
            return Err(KbError::backend_failed(
                "unexpected git diff --name-status output",
            ));
        }

        let mut paths = Vec::with_capacity(paths_needed);
        for _ in 0..paths_needed {
            paths.push(parts[idx].to_string());
            idx += 1;
        }

        out.push((status, paths));
    }

    Ok(out)
}

fn split_nul(bytes: &[u8]) -> Result<Vec<&str>, KbError> {
    let text = std::str::from_utf8(bytes).map_err(|err| {
        KbError::backend_failed("git output is not valid utf-8")
            .with_detail("cause", err.to_string())
    })?;
    Ok(text.split('\0').collect())
}

fn change_kind_from_status(status: &str) -> ChangeKind {
    if status.starts_with('A') {
        return ChangeKind::Add;
    }
    if status.starts_with('M') {
        return ChangeKind::Modify;
    }
    if status.starts_with('D') {
        return ChangeKind::Delete;
    }
    if status.starts_with('R') {
        return ChangeKind::Rename;
    }
    ChangeKind::Unknown
}

fn head_exists(repo_root: &Path) -> Result<bool, KbError> {
    let output = Command::new("git")
        .arg("--no-pager")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(repo_root)
        .env("LC_ALL", "C")
        .output()
        .map_err(|err| {
            KbError::backend_missing("git is required").with_detail("cause", err.to_string())
        })?;

    Ok(output.status.success())
}

fn commit_base(repo_root: &Path, sha: &str) -> Result<String, KbError> {
    let output = Command::new("git")
        .arg("--no-pager")
        .args(["rev-list", "--parents", "-n", "1", sha])
        .current_dir(repo_root)
        .env("LC_ALL", "C")
        .output()
        .map_err(|err| {
            KbError::backend_missing("git is required").with_detail("cause", err.to_string())
        })?;

    if !output.status.success() {
        return Err(KbError::not_found("commit not found").with_detail("commit", sha));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.split_whitespace().collect();
    if parts.len() <= 1 {
        return Ok(EMPTY_TREE_SHA.to_string());
    }
    Ok(format!("{}^", sha))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_status_handles_renames() {
        let bytes = b"R100\0old.txt\0new.txt\0M\0a.txt\0";
        let parsed = parse_name_status_z(bytes).unwrap();
        assert_eq!(
            parsed,
            vec![
                (
                    "R100".to_string(),
                    vec!["old.txt".to_string(), "new.txt".to_string()]
                ),
                ("M".to_string(), vec!["a.txt".to_string()]),
            ]
        );
    }
}
