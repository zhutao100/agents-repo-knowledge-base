use std::path::PathBuf;
use std::process::Command;

use crate::error::KbError;
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::root::discover_repo_root;

#[derive(Clone, Debug)]
pub struct DiffSourceReader {
    repo_root: PathBuf,
    diff_source: DiffSource,
}

impl DiffSourceReader {
    pub fn new(diff_source: DiffSource) -> Result<Self, KbError> {
        Ok(Self::new_at_root(discover_repo_root()?, diff_source))
    }

    pub fn new_at_root(repo_root: PathBuf, diff_source: DiffSource) -> Self {
        Self {
            repo_root,
            diff_source,
        }
    }

    pub fn read_bytes(&self, path: &RepoPath) -> Result<Vec<u8>, KbError> {
        match &self.diff_source {
            DiffSource::Worktree => {
                let file_path = self.repo_root.join(path.as_str());
                std::fs::read(&file_path).map_err(|err| {
                    KbError::not_found("file not found")
                        .with_detail("path", path.as_str())
                        .with_detail("cause", err.to_string())
                })
            }
            DiffSource::Staged => git_show_bytes(&self.repo_root, &format!(":{}", path.as_str())),
            DiffSource::Commit(sha) => {
                git_show_bytes(&self.repo_root, &format!("{}:{}", sha, path.as_str()))
            }
        }
    }

    pub fn read_to_string(&self, path: &RepoPath) -> Result<String, KbError> {
        let bytes = self.read_bytes(path)?;
        String::from_utf8(bytes).map_err(|err| {
            KbError::invalid_argument("file is not valid utf-8")
                .with_detail("path", path.as_str())
                .with_detail("cause", err.to_string())
        })
    }
}

fn git_show_bytes(repo_root: &PathBuf, spec: &str) -> Result<Vec<u8>, KbError> {
    let output = Command::new("git")
        .arg("--no-pager")
        .arg("show")
        .arg(spec)
        .current_dir(repo_root)
        .env("LC_ALL", "C")
        .output()
        .map_err(|err| KbError::internal(err, "failed to run git"))?;

    if !output.status.success() {
        return Err(KbError::not_found("git object not found").with_detail("spec", spec));
    }

    Ok(output.stdout)
}
