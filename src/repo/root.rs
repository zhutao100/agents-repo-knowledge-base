use std::path::PathBuf;
use std::process::Command;

use crate::error::KbError;

pub fn discover_repo_root() -> Result<PathBuf, KbError> {
    let cwd = std::env::current_dir().map_err(|err| KbError::internal(err, "failed to get cwd"))?;

    let output = Command::new("git")
        .arg("--no-pager")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(&cwd)
        .env("LC_ALL", "C")
        .output()
        .map_err(|err| KbError::internal(err, "failed to run git"))?;

    if !output.status.success() {
        return Err(KbError::not_found("git repo root not found"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.trim();
    if path.is_empty() {
        return Err(KbError::internal("empty", "git repo root not found"));
    }

    Ok(PathBuf::from(path))
}
