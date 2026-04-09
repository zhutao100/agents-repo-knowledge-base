use std::path::Path;

use serde::de::DeserializeOwned;

use crate::error::{ErrorCode, KbError};
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::reader::DiffSourceReader;

pub fn reader_for(repo_root: &Path, diff_source: &DiffSource) -> DiffSourceReader {
    DiffSourceReader::new_at_root(repo_root.to_path_buf(), diff_source.clone())
}

pub fn read_text(reader: &DiffSourceReader, rel_path: &str) -> Result<String, KbError> {
    let repo_path = RepoPath::parse(rel_path)?;
    reader.read_to_string(&repo_path)
}

pub fn try_read_text(reader: &DiffSourceReader, rel_path: &str) -> Result<Option<String>, KbError> {
    let repo_path = RepoPath::parse(rel_path)?;
    match reader.read_to_string(&repo_path) {
        Ok(s) => Ok(Some(s)),
        Err(err) if err.code == ErrorCode::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

pub fn read_jsonl<T: DeserializeOwned>(
    reader: &DiffSourceReader,
    rel_path: &str,
) -> Result<Vec<T>, KbError> {
    let repo_path = RepoPath::parse(rel_path)?;
    let bytes = reader.read_bytes(&repo_path)?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        KbError::invalid_argument("jsonl is not valid utf-8")
            .with_detail("path", rel_path)
            .with_detail("cause", err.to_string())
    })?;

    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let record: T = serde_json::from_str(line).map_err(|err| {
            KbError::invalid_argument("failed to parse jsonl").with_detail("cause", err.to_string())
        })?;
        out.push(record);
    }
    Ok(out)
}
