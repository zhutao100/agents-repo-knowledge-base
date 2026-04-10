use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Datelike;
use clap::ValueEnum;

use crate::config::tags::validate_tags_at;
use crate::error::KbError;
use crate::io::json::write_json_to_writer;
use crate::repo::diff::list_changed_paths;
use crate::repo::diff_source::DiffSource;
use crate::repo::root::discover_repo_root;

const SESSION_REFS_MAX: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum VerificationKind {
    Tests,
    Bench,
    Repro,
    Lint,
}

impl VerificationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            VerificationKind::Tests => "tests",
            VerificationKind::Bench => "bench",
            VerificationKind::Repro => "repro",
            VerificationKind::Lint => "lint",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionCapsule {
    pub session_id: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub decisions: Vec<String>,
    pub pitfalls: Vec<String>,
    pub verification: Vec<String>,
    pub refs: Vec<String>,
}

pub fn session_init(session_id: String, tags: Vec<String>) -> Result<(), KbError> {
    session_init_at(&discover_repo_root()?, session_id, tags)?;
    Ok(())
}

pub fn session_init_at(
    repo_root: &Path,
    session_id: String,
    tags: Vec<String>,
) -> Result<PathBuf, KbError> {
    validate_session_id(&session_id)?;
    validate_tags_at(repo_root, &tags)?;

    let mut tags = tags;
    tags.sort();
    tags.dedup();

    let template = read_session_template(repo_root)?;
    let capsule = SessionCapsule {
        session_id: session_id.clone(),
        tags,
        summary: template.summary,
        decisions: template.decisions,
        pitfalls: template.pitfalls,
        verification: Vec::new(),
        refs: Vec::new(),
    };
    validate_capsule(&capsule, Some(&session_id))?;

    let out_rel = session_init_rel_path(&session_id)?;
    let out_path = repo_root.join(&out_rel);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| KbError::internal(err, "failed to create kb/sessions directory"))?;
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&out_path)
        .map_err(|err| {
            KbError::invalid_argument("session capsule already exists")
                .with_detail("path", out_rel.as_str())
                .with_detail("cause", err.to_string())
        })?;
    write_json_to_writer(&mut file, &capsule)
        .map_err(|err| KbError::internal(err, "failed to write session capsule"))?;
    file.flush()
        .map_err(|err| KbError::internal(err, "failed to flush session capsule"))?;

    Ok(out_path)
}

pub fn session_finalize(
    session_id: String,
    diff_source: &DiffSource,
    verification: Vec<VerificationKind>,
) -> Result<(), KbError> {
    session_finalize_at(
        &discover_repo_root()?,
        session_id,
        diff_source,
        verification,
    )?;
    Ok(())
}

pub fn session_finalize_at(
    repo_root: &Path,
    session_id: String,
    diff_source: &DiffSource,
    verification: Vec<VerificationKind>,
) -> Result<PathBuf, KbError> {
    validate_session_id(&session_id)?;

    let rel = locate_unique_capsule_rel_path(repo_root, &session_id)?;
    let abs_path = repo_root.join(&rel);
    let text = std::fs::read_to_string(&abs_path).map_err(|err| {
        KbError::not_found("session capsule not found").with_detail("cause", err.to_string())
    })?;

    let mut capsule: SessionCapsule = serde_json::from_str(&text).map_err(|err| {
        KbError::invalid_argument("failed to parse session capsule")
            .with_detail("cause", err.to_string())
    })?;
    if capsule.session_id != session_id {
        return Err(
            KbError::invalid_argument("session_id does not match requested id")
                .with_detail("expected", &session_id)
                .with_detail("found", &capsule.session_id),
        );
    }
    validate_session_id(&capsule.session_id)?;

    // Merge verification.
    let mut merged_verification: BTreeSet<String> = capsule.verification.iter().cloned().collect();
    for v in verification {
        merged_verification.insert(v.as_str().to_string());
    }
    capsule.verification = merged_verification.into_iter().collect();

    // Append refs derived from the diff.
    let mut derived_refs: BTreeSet<String> = capsule.refs.iter().cloned().collect();
    for c in list_changed_paths(repo_root, diff_source)? {
        if c.path.starts_with("kb/gen/")
            || c.path.starts_with("kb/cache/")
            || c.path.starts_with("kb/.tmp/")
            || c.path.starts_with("kb/sessions/")
        {
            continue;
        }
        derived_refs.insert(c.path);
    }
    capsule.refs = derived_refs.into_iter().take(SESSION_REFS_MAX).collect();

    normalize_set(&mut capsule.tags);
    validate_tags_at(repo_root, &capsule.tags)?;
    validate_capsule(&capsule, Some(&session_id))?;

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&abs_path)
        .map_err(|err| KbError::internal(err, "failed to open session capsule for write"))?;
    write_json_to_writer(&mut file, &capsule)
        .map_err(|err| KbError::internal(err, "failed to write session capsule"))?;
    file.flush()
        .map_err(|err| KbError::internal(err, "failed to flush session capsule"))?;

    Ok(abs_path)
}

pub fn session_check(session_id: String) -> Result<(), KbError> {
    session_check_at(&discover_repo_root()?, session_id)?;
    Ok(())
}

pub fn session_check_at(repo_root: &Path, session_id: String) -> Result<PathBuf, KbError> {
    validate_session_id(&session_id)?;
    let rel = locate_unique_capsule_rel_path(repo_root, &session_id)?;
    let abs_path = repo_root.join(&rel);
    let text = std::fs::read_to_string(&abs_path).map_err(|err| {
        KbError::not_found("session capsule not found").with_detail("cause", err.to_string())
    })?;
    let capsule: SessionCapsule = serde_json::from_str(&text).map_err(|err| {
        KbError::invalid_argument("failed to parse session capsule")
            .with_detail("cause", err.to_string())
    })?;
    validate_capsule(&capsule, Some(&session_id))?;
    validate_tags_at(repo_root, &capsule.tags)?;
    Ok(abs_path)
}

fn normalize_set(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn validate_session_id(session_id: &str) -> Result<(), KbError> {
    let mut chars = session_id.chars();
    let Some(first) = chars.next() else {
        return Err(KbError::invalid_argument("session id must not be empty"));
    };
    if !first.is_ascii_alphanumeric() {
        return Err(KbError::invalid_argument("invalid session id").with_detail("id", session_id));
    }
    for c in chars {
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
            continue;
        }
        return Err(KbError::invalid_argument("invalid session id").with_detail("id", session_id));
    }
    Ok(())
}

fn validate_capsule(capsule: &SessionCapsule, expected_id: Option<&str>) -> Result<(), KbError> {
    if let Some(expected) = expected_id {
        if capsule.session_id != expected {
            return Err(
                KbError::invalid_argument("session_id does not match requested id")
                    .with_detail("expected", expected)
                    .with_detail("found", &capsule.session_id),
            );
        }
    }

    validate_session_id(&capsule.session_id)?;
    validate_sorted_unique("tags", &capsule.tags)?;
    validate_sorted_unique("verification", &capsule.verification)?;
    validate_sorted_unique("refs", &capsule.refs)?;

    for v in &capsule.verification {
        if !matches!(v.as_str(), "tests" | "bench" | "repro" | "lint") {
            return Err(KbError::invalid_argument("unknown verification kind")
                .with_detail("verification", v));
        }
    }

    if capsule.refs.len() > SESSION_REFS_MAX {
        return Err(KbError::invalid_argument("too many refs")
            .with_detail("max_refs", SESSION_REFS_MAX.to_string()));
    }

    validate_no_absolute_paths(capsule)?;

    Ok(())
}

fn validate_sorted_unique(field: &str, values: &[String]) -> Result<(), KbError> {
    let mut expected = values.to_vec();
    expected.sort();
    expected.dedup();
    if expected != values {
        return Err(
            KbError::invalid_argument("field must be stable-sorted unique")
                .with_detail("field", field),
        );
    }
    Ok(())
}

fn validate_no_absolute_paths(capsule: &SessionCapsule) -> Result<(), KbError> {
    for t in &capsule.tags {
        reject_if_absolute_path(t, "tags")?;
    }
    for line in capsule.summary.lines() {
        reject_if_absolute_path(line, "summary")?;
    }
    for d in &capsule.decisions {
        reject_if_absolute_path(d, "decisions")?;
    }
    for p in &capsule.pitfalls {
        reject_if_absolute_path(p, "pitfalls")?;
    }
    for v in &capsule.verification {
        reject_if_absolute_path(v, "verification")?;
    }
    for r in &capsule.refs {
        reject_if_absolute_path(r, "refs")?;
    }
    Ok(())
}

fn reject_if_absolute_path(value: &str, field: &str) -> Result<(), KbError> {
    let trimmed = value.trim_start();
    if trimmed.starts_with('/')
        || looks_like_windows_abs_path(trimmed)
        || trimmed.starts_with("\\\\")
    {
        return Err(KbError::invalid_argument(
            "absolute paths are not allowed in session capsules",
        )
        .with_detail("field", field)
        .with_detail("value", value));
    }
    Ok(())
}

fn looks_like_windows_abs_path(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(second) = chars.next() else {
        return false;
    };
    let Some(third) = chars.next() else {
        return false;
    };

    first.is_ascii_alphabetic() && second == ':' && (third == '\\' || third == '/')
}

fn session_init_rel_path(session_id: &str) -> Result<String, KbError> {
    let now = chrono::Local::now();
    let yyyy = now.year();
    let mm = now.month();
    Ok(format!("kb/sessions/{yyyy:04}/{mm:02}/{session_id}.json"))
}

fn read_session_template(repo_root: &Path) -> Result<SessionCapsule, KbError> {
    let path = repo_root.join("kb/templates/session.json");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(built_in_session_template())
        }
        Err(err) => return Err(KbError::internal(err, "failed to read session template")),
    };

    let text = std::str::from_utf8(&bytes).map_err(|err| {
        KbError::invalid_argument("session template is not valid utf-8")
            .with_detail("cause", err.to_string())
    })?;
    let template: SessionCapsule = serde_json::from_str(text).map_err(|err| {
        KbError::invalid_argument("failed to parse session template")
            .with_detail("cause", err.to_string())
    })?;

    Ok(template)
}

fn built_in_session_template() -> SessionCapsule {
    SessionCapsule {
        session_id: String::new(),
        tags: Vec::new(),
        summary: String::new(),
        decisions: Vec::new(),
        pitfalls: Vec::new(),
        verification: Vec::new(),
        refs: Vec::new(),
    }
}

fn locate_unique_capsule_rel_path(repo_root: &Path, session_id: &str) -> Result<String, KbError> {
    let candidates = locate_capsule_candidates(repo_root, session_id)?;
    if candidates.is_empty() {
        return Err(KbError::not_found("session capsule not found").with_detail("id", session_id));
    }
    if candidates.len() > 1 {
        return Err(KbError::invalid_argument("multiple session capsules found")
            .with_detail("id", session_id)
            .with_detail(
                "candidates",
                serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string()),
            ));
    }
    Ok(candidates[0].clone())
}

fn locate_capsule_candidates(repo_root: &Path, session_id: &str) -> Result<Vec<String>, KbError> {
    let sessions_root = repo_root.join("kb/sessions");
    if !sessions_root.is_dir() {
        return Ok(Vec::new());
    }

    let filename = format!("{session_id}.json");
    let mut out: Vec<String> = Vec::new();
    walk_dir_sorted(&sessions_root, repo_root, &filename, &mut out)?;
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk_dir_sorted(
    dir: &Path,
    repo_root: &Path,
    filename: &str,
    out: &mut Vec<String>,
) -> Result<(), KbError> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|err| KbError::internal(err, "failed to read kb/sessions directory"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();

    for path in entries {
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|err| KbError::internal(err, "failed to stat kb/sessions path"))?;
        if meta.file_type().is_symlink() {
            return Err(
                KbError::invalid_argument("symlinks are not allowed under kb/sessions")
                    .with_detail(
                        "path",
                        path.strip_prefix(repo_root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string(),
                    ),
            );
        }

        if meta.is_dir() {
            walk_dir_sorted(&path, repo_root, filename, out)?;
            continue;
        }

        if !meta.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name != filename {
            continue;
        }

        let rel = path.strip_prefix(repo_root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        out.push(rel_str);
    }

    Ok(())
}
