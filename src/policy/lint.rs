use std::path::{Path, PathBuf};

use crate::error::KbError;
use crate::index::artifacts::{DepEdge, KbMeta, SymbolRecord, TreeRecord};
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::prefix::normalize_path_prefix;
use crate::repo::reader::DiffSourceReader;
use crate::repo::root::discover_repo_root;

pub fn lint_all() -> Result<(), KbError> {
    lint_all_at(&discover_repo_root()?)
}

pub fn lint_all_at(repo_root: &Path) -> Result<(), KbError> {
    lint_configs(repo_root)?;
    lint_generated_artifacts(repo_root)?;
    lint_overlays(repo_root)?;
    Ok(())
}

fn lint_configs(repo_root: &Path) -> Result<(), KbError> {
    // Required
    let obligations_path = repo_root.join("kb/config/obligations.toml");
    let obligations_text = std::fs::read_to_string(&obligations_path).map_err(|err| {
        KbError::not_found("kb/config/obligations.toml is required")
            .with_detail("cause", err.to_string())
    })?;

    let obligations: toml::Value = toml::from_str(&obligations_text).map_err(|err| {
        KbError::invalid_argument("failed to parse obligations.toml")
            .with_detail("cause", err.to_string())
    })?;

    // Repo-boundedness: validate when_path_prefix values.
    if let Some(rules) = obligations.get("rule").and_then(|v| v.as_array()) {
        for rule in rules {
            if let Some(prefix) = rule.get("when_path_prefix").and_then(|v| v.as_str()) {
                let normalized = normalize_path_prefix(prefix)?;
                if normalized != prefix {
                    return Err(
                        KbError::invalid_argument("when_path_prefix must be normalized")
                            .with_detail("when_path_prefix", prefix)
                            .with_detail("normalized", normalized),
                    );
                }
            }
        }
    }

    // Optional configs
    for rel in ["kb/config/tags.toml", "kb/config/kb.toml"] {
        let path = repo_root.join(rel);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|err| KbError::internal(err, "failed to read config"))?;
        let _: toml::Value = toml::from_str(&text).map_err(|err| {
            KbError::invalid_argument("failed to parse config")
                .with_detail("cause", err.to_string())
        })?;
    }

    Ok(())
}

fn lint_generated_artifacts(repo_root: &Path) -> Result<(), KbError> {
    let reader = DiffSourceReader::new_at_root(repo_root.to_path_buf(), DiffSource::Worktree);

    let kb_meta = read_json::<KbMeta>(&reader, "kb/gen/kb_meta.json")?;
    if kb_meta.kb_format_version != 1 {
        return Err(KbError::invalid_argument("kb_format_version must be 1"));
    }

    let tree = read_jsonl::<TreeRecord>(&reader, "kb/gen/tree.jsonl")?;
    validate_tree_records(&tree)?;

    let symbols = read_jsonl::<SymbolRecord>(&reader, "kb/gen/symbols.jsonl")?;
    validate_symbol_records(&symbols)?;

    let deps = read_jsonl::<DepEdge>(&reader, "kb/gen/deps.jsonl")?;
    validate_dep_edges(&deps)?;

    // Forbidden fields scan (best-effort).
    for rel in [
        "kb/gen/kb_meta.json",
        "kb/gen/tree.jsonl",
        "kb/gen/symbols.jsonl",
        "kb/gen/deps.jsonl",
    ] {
        let text = reader.read_to_string(&RepoPath::parse(rel)?)?;
        if text.contains("\"timestamp\"")
            || text.contains("\"epoch\"")
            || text.contains("\"mtime\"")
        {
            return Err(
                KbError::invalid_argument("forbidden timestamp-like fields found")
                    .with_detail("path", rel),
            );
        }
    }

    Ok(())
}

fn lint_overlays(repo_root: &Path) -> Result<(), KbError> {
    let modules_dir = repo_root.join("kb/atlas/modules");
    if modules_dir.is_dir() {
        let mut files = list_files_sorted(&modules_dir, "toml")?;
        for file in files.drain(..) {
            let text = std::fs::read_to_string(&file)
                .map_err(|err| KbError::internal(err, "failed to read module card"))?;
            let _: toml::Value = toml::from_str(&text).map_err(|err| {
                KbError::invalid_argument("failed to parse module card")
                    .with_detail("cause", err.to_string())
            })?;
        }
    }

    let facts_path = repo_root.join("kb/facts/facts.jsonl");
    if facts_path.is_file() {
        let reader = DiffSourceReader::new_at_root(repo_root.to_path_buf(), DiffSource::Worktree);
        let facts = read_jsonl::<serde_json::Value>(&reader, "kb/facts/facts.jsonl")?;
        for fact in facts {
            let Some(obj) = fact.as_object() else {
                return Err(KbError::invalid_argument(
                    "fact record must be a JSON object",
                ));
            };
            if obj.get("fact_id").and_then(|v| v.as_str()).is_none() {
                return Err(KbError::invalid_argument("fact record missing fact_id"));
            }
            if obj.get("type").and_then(|v| v.as_str()).is_none() {
                return Err(KbError::invalid_argument("fact record missing type"));
            }
        }
    }

    let sessions_dir = repo_root.join("kb/sessions");
    if sessions_dir.is_dir() {
        let mut json_files = Vec::new();
        collect_files_recursive(&sessions_dir, "json", &mut json_files)?;
        json_files.sort();
        for file in json_files {
            let text = std::fs::read_to_string(&file)
                .map_err(|err| KbError::internal(err, "failed to read session"))?;
            let _: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
                KbError::invalid_argument("failed to parse session json")
                    .with_detail("cause", err.to_string())
            })?;
        }
    }

    Ok(())
}

fn validate_tree_records(records: &[TreeRecord]) -> Result<(), KbError> {
    let mut last_path: Option<&str> = None;
    for r in records {
        validate_repo_path(&r.path, r.kind.as_str())?;

        if r.kind == "dir" {
            if !r.path.ends_with('/') {
                return Err(KbError::invalid_argument("dir path must end with '/'")
                    .with_detail("path", &r.path));
            }
        } else if r.kind == "file" {
            if r.path.ends_with('/') {
                return Err(KbError::invalid_argument("file path must not end with '/'")
                    .with_detail("path", &r.path));
            }
            if r.bytes.is_none() || r.lines.is_none() || r.lang.is_none() {
                return Err(
                    KbError::invalid_argument("file record missing required fields")
                        .with_detail("path", &r.path),
                );
            }
            if let Some(top) = r.top_symbols.as_deref() {
                let mut sorted = top.to_vec();
                sorted.sort();
                if sorted != top {
                    return Err(KbError::invalid_argument("top_symbols must be sorted")
                        .with_detail("path", &r.path));
                }
            }
        } else {
            return Err(
                KbError::invalid_argument("tree kind must be 'dir' or 'file'")
                    .with_detail("kind", &r.kind),
            );
        }

        if let Some(prev) = last_path {
            if r.path.as_str() < prev {
                return Err(KbError::invalid_argument("tree.jsonl is not sorted"));
            }
        }
        last_path = Some(r.path.as_str());
    }
    Ok(())
}

fn validate_symbol_records(records: &[SymbolRecord]) -> Result<(), KbError> {
    let mut last_id: Option<&str> = None;
    for r in records {
        if !is_valid_symbol_id(&r.symbol_id) {
            return Err(KbError::invalid_argument("invalid symbol_id")
                .with_detail("symbol_id", &r.symbol_id));
        }
        validate_repo_path(&r.path, "file")?;
        if r.line == 0 {
            return Err(KbError::invalid_argument("symbol line must be 1-based")
                .with_detail("symbol_id", &r.symbol_id));
        }
        if let Some(end) = r.end_line {
            if end < r.line {
                return Err(KbError::invalid_argument("end_line must be >= line")
                    .with_detail("symbol_id", &r.symbol_id));
            }
        }

        if let Some(prev) = last_id {
            if r.symbol_id.as_str() < prev {
                return Err(KbError::invalid_argument("symbols.jsonl is not sorted"));
            }
        }
        last_id = Some(r.symbol_id.as_str());
    }
    Ok(())
}

fn is_valid_symbol_id(symbol_id: &str) -> bool {
    const PREFIX: &str = "sym:v3:";
    const HEX_LEN: usize = 16;

    if !symbol_id.starts_with(PREFIX) {
        return false;
    }
    let suffix = &symbol_id[PREFIX.len()..];
    if suffix.len() != HEX_LEN {
        return false;
    }
    suffix
        .bytes()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_dep_edges(edges: &[DepEdge]) -> Result<(), KbError> {
    let mut last_key: Option<(String, String, String)> = None;
    for e in edges {
        validate_repo_path(&e.from_path, "file")?;
        let has_to_path = e.to_path.is_some();
        let has_to_external = e.to_external.is_some();
        if has_to_path == has_to_external {
            return Err(KbError::invalid_argument(
                "deps edge must have exactly one of to_path/to_external",
            ));
        }

        if let Some(raw) = e.raw.as_deref() {
            if raw.contains('\n') || raw.contains('\r') {
                return Err(KbError::invalid_argument("deps raw must be single-line"));
            }
        }

        match e.kind.as_str() {
            "import" | "include" | "require" | "dynamic" | "unknown" => {}
            _ => {
                return Err(
                    KbError::invalid_argument("invalid deps kind").with_detail("kind", &e.kind)
                )
            }
        }

        if let Some(to_path) = e.to_path.as_deref() {
            validate_repo_path(to_path, "file")?;
        }

        let to = e
            .to_path
            .as_deref()
            .or(e.to_external.as_deref())
            .unwrap_or("");

        let key = (e.from_path.clone(), e.kind.clone(), to.to_string());
        if let Some(prev) = last_key.as_ref() {
            if key < *prev {
                return Err(KbError::invalid_argument("deps.jsonl is not sorted"));
            }
        }
        last_key = Some(key);
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(
    reader: &DiffSourceReader,
    rel: &str,
) -> Result<T, KbError> {
    let repo_path = RepoPath::parse(rel)?;
    let text = reader.read_to_string(&repo_path)?;
    serde_json::from_str(&text).map_err(|err| {
        KbError::invalid_argument("failed to parse json").with_detail("cause", err.to_string())
    })
}

fn read_jsonl<T: serde::de::DeserializeOwned>(
    reader: &DiffSourceReader,
    rel: &str,
) -> Result<Vec<T>, KbError> {
    let repo_path = RepoPath::parse(rel)?;
    let bytes = reader.read_bytes(&repo_path)?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        KbError::invalid_argument("jsonl is not valid utf-8")
            .with_detail("path", rel)
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

fn validate_repo_path(path: &str, kind: &str) -> Result<(), KbError> {
    if path.starts_with('/') {
        return Err(
            KbError::invalid_argument("absolute paths are not allowed").with_detail("path", path)
        );
    }
    if path.contains('\\') {
        return Err(
            KbError::invalid_argument("paths must use '/' separators").with_detail("path", path)
        );
    }
    if path.contains("/../") || path.starts_with("../") || path == ".." {
        return Err(
            KbError::invalid_argument("path must not contain '..'").with_detail("path", path)
        );
    }
    if kind == "dir" && !path.ends_with('/') {
        // handled by caller
        return Ok(());
    }
    if kind == "file" && path.ends_with('/') {
        return Err(
            KbError::invalid_argument("file path must not end with '/'").with_detail("path", path)
        );
    }
    Ok(())
}

fn list_files_sorted(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, KbError> {
    let mut out = Vec::new();
    for entry in
        std::fs::read_dir(dir).map_err(|err| KbError::internal(err, "failed to read directory"))?
    {
        let entry = entry.map_err(|err| KbError::internal(err, "failed to read directory"))?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|e| e == extension)
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn collect_files_recursive(
    dir: &Path,
    extension: &str,
    out: &mut Vec<PathBuf>,
) -> Result<(), KbError> {
    for entry in
        std::fs::read_dir(dir).map_err(|err| KbError::internal(err, "failed to read directory"))?
    {
        let entry = entry.map_err(|err| KbError::internal(err, "failed to read directory"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, extension, out)?;
        } else if path.is_file()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|e| e == extension)
        {
            out.push(path);
        }
    }
    Ok(())
}
