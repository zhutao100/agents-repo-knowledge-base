use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::KbError;

pub fn git_command(repo_root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("--no-pager")
        .current_dir(repo_root)
        .env("LC_ALL", "C");
    cmd
}

pub fn git_output(repo_root: &Path, args: &[&str]) -> Result<Vec<u8>, KbError> {
    let output = git_command(repo_root).args(args).output().map_err(|err| {
        KbError::backend_missing("git is required").with_detail("cause", err.to_string())
    })?;

    if !output.status.success() {
        return Err(KbError::backend_failed("git command failed")
            .with_detail("args", args.join(" "))
            .with_detail(
                "stderr",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
    }

    Ok(output.stdout)
}

pub fn git_output_with_input(
    repo_root: &Path,
    args: &[&str],
    stdin_bytes: &[u8],
) -> Result<Vec<u8>, KbError> {
    let mut child = git_command(repo_root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            KbError::backend_missing("git is required").with_detail("cause", err.to_string())
        })?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| KbError::internal("missing", "failed to open git stdin"))?;
        stdin
            .write_all(stdin_bytes)
            .map_err(|err| KbError::internal(err, "failed to write git stdin"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| KbError::internal(err, "failed to read git output"))?;

    if !output.status.success() {
        return Err(KbError::backend_failed("git command failed")
            .with_detail("args", args.join(" "))
            .with_detail(
                "stderr",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
    }

    Ok(output.stdout)
}
