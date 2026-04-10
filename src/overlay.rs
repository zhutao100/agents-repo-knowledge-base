use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::obligations::{ObligationRule, ObligationsConfig};
use crate::config::tags::{load_tags_config_at, tags_toml_path, validate_tags_at};
use crate::error::KbError;
use crate::repo::path::RepoPath;
use crate::repo::prefix::normalize_path_prefix;
use crate::repo::root::discover_repo_root;

pub fn tags_upsert(tag_id: String, description: Option<String>) -> Result<(), KbError> {
    tags_upsert_at(&discover_repo_root()?, tag_id, description)
}

pub fn tags_upsert_at(
    repo_root: &Path,
    tag_id: String,
    description: Option<String>,
) -> Result<(), KbError> {
    let tag_id = tag_id.trim().to_string();
    validate_tag_id(&tag_id)?;
    if let Some(desc) = description.as_deref() {
        validate_single_line_string("description", desc)?;
    }

    let mut entries_by_id: BTreeMap<String, Option<String>> = BTreeMap::new();
    if let Some(cfg) = load_tags_config_at(repo_root)? {
        for t in cfg.tag {
            entries_by_id.insert(t.id, t.description);
        }
    }

    let existing_desc = entries_by_id.get(&tag_id).cloned().unwrap_or(None);
    entries_by_id.insert(
        tag_id,
        description.or(existing_desc).map(|s| s.trim().to_string()),
    );

    let mut out = String::new();
    for (id, desc) in entries_by_id {
        out.push_str("[[tag]]\n");
        out.push_str(&format!("id = {}\n", render_toml_string(&id)));
        if let Some(d) = desc {
            out.push_str(&format!("description = {}\n", render_toml_string(d.trim())));
        }
        out.push('\n');
    }

    write_text_file(&tags_toml_path(repo_root), &out)?;
    Ok(())
}

pub fn module_init(
    module_id: String,
    title: Option<String>,
    owners: Vec<String>,
    tags: Vec<String>,
    entrypoints: Vec<String>,
    edit_points: Vec<String>,
    related_facts: Vec<String>,
) -> Result<(), KbError> {
    module_write_at(
        &discover_repo_root()?,
        ModuleWriteInput {
            module_id,
            title,
            owners,
            tags,
            entrypoints,
            edit_points,
            related_facts,
        },
        true,
    )
}

pub fn module_upsert(
    module_id: String,
    title: Option<String>,
    owners: Vec<String>,
    tags: Vec<String>,
    entrypoints: Vec<String>,
    edit_points: Vec<String>,
    related_facts: Vec<String>,
) -> Result<(), KbError> {
    module_write_at(
        &discover_repo_root()?,
        ModuleWriteInput {
            module_id,
            title,
            owners,
            tags,
            entrypoints,
            edit_points,
            related_facts,
        },
        false,
    )
}

pub struct ModuleWriteInput {
    pub module_id: String,
    pub title: Option<String>,
    pub owners: Vec<String>,
    pub tags: Vec<String>,
    pub entrypoints: Vec<String>,
    pub edit_points: Vec<String>,
    pub related_facts: Vec<String>,
}

pub fn module_write_at(
    repo_root: &Path,
    input: ModuleWriteInput,
    create_new: bool,
) -> Result<(), KbError> {
    let ModuleWriteInput {
        module_id,
        title,
        owners,
        tags,
        entrypoints,
        edit_points,
        related_facts,
    } = input;

    let module_id = module_id.trim().to_string();
    validate_module_id(&module_id)?;

    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| module_id.clone());
    validate_single_line_string("title", &title)?;

    let mut owners = normalize_unique_strings(owners);
    for owner in &owners {
        validate_single_line_string("owner", owner)?;
    }

    let mut tags = normalize_unique_strings(tags);
    for tag in &tags {
        validate_tag_id(tag)?;
    }
    validate_tags_at(repo_root, &tags)?;

    let mut entrypoints = normalize_unique_paths(entrypoints, true)?;
    let mut edit_points = normalize_unique_paths(edit_points, false)?;
    let mut related_facts = normalize_unique_strings(related_facts);
    for fact_id in &related_facts {
        validate_fact_id(fact_id)?;
    }

    // Deterministic, stable ordering in-file.
    owners.sort();
    tags.sort();
    entrypoints.sort();
    edit_points.sort();
    related_facts.sort();

    let mut text = String::new();
    text.push_str(&format!("id = {}\n", render_toml_string(&module_id)));
    text.push_str(&format!("title = {}\n", render_toml_string(&title)));

    if !owners.is_empty() {
        text.push_str(&format!("owners = {}\n", render_toml_string_array(&owners)));
    }
    if !tags.is_empty() {
        text.push_str(&format!("tags = {}\n", render_toml_string_array(&tags)));
    }
    if !entrypoints.is_empty() {
        text.push_str(&format!(
            "entrypoints = {}\n",
            render_toml_string_array(&entrypoints)
        ));
    }
    if !edit_points.is_empty() {
        text.push_str(&format!(
            "edit_points = {}\n",
            render_toml_string_array(&edit_points)
        ));
    }
    if !related_facts.is_empty() {
        text.push_str(&format!(
            "related_facts = {}\n",
            render_toml_string_array(&related_facts)
        ));
    }

    let rel = format!("kb/atlas/modules/{module_id}.toml");
    let path = repo_root.join(&rel);
    write_text_file_create_mode(&path, &text, create_new).map_err(|err| {
        err.with_detail("path", rel)
            .with_detail("module_id", module_id.clone())
    })?;
    Ok(())
}

pub fn facts_upsert(
    fact_id: String,
    fact_type: String,
    tags: Vec<String>,
    paths: Vec<String>,
    data_json: Option<String>,
) -> Result<(), KbError> {
    facts_upsert_at(
        &discover_repo_root()?,
        fact_id,
        fact_type,
        tags,
        paths,
        data_json,
    )
}

pub fn facts_upsert_at(
    repo_root: &Path,
    fact_id: String,
    fact_type: String,
    tags: Vec<String>,
    paths: Vec<String>,
    data_json: Option<String>,
) -> Result<(), KbError> {
    let fact_id = fact_id.trim().to_string();
    validate_fact_id(&fact_id)?;

    let fact_type = fact_type.trim().to_string();
    validate_fact_type(&fact_type)?;

    let mut tags = normalize_unique_strings(tags);
    for tag in &tags {
        validate_tag_id(tag)?;
    }
    validate_tags_at(repo_root, &tags)?;
    tags.sort();

    let mut paths = normalize_unique_paths(paths, true)?;
    paths.sort();

    let data = match data_json {
        Some(raw) => {
            let v: serde_json::Value = serde_json::from_str(&raw).map_err(|err| {
                KbError::invalid_argument("failed to parse data_json")
                    .with_detail("cause", err.to_string())
            })?;
            Some(v)
        }
        None => None,
    };

    let mut facts_by_id = load_facts_by_id(repo_root)?;
    let mut record = facts_by_id
        .remove(&fact_id)
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(obj) = record.as_object_mut() else {
        return Err(KbError::invalid_argument(
            "fact record must be a JSON object",
        ));
    };

    obj.insert(
        "fact_id".to_string(),
        serde_json::Value::String(fact_id.clone()),
    );
    obj.insert(
        "type".to_string(),
        serde_json::Value::String(fact_type.clone()),
    );

    if tags.is_empty() {
        obj.remove("tags");
    } else {
        obj.insert(
            "tags".to_string(),
            serde_json::Value::Array(tags.into_iter().map(serde_json::Value::String).collect()),
        );
    }

    if paths.is_empty() {
        obj.remove("paths");
    } else {
        obj.insert(
            "paths".to_string(),
            serde_json::Value::Array(paths.into_iter().map(serde_json::Value::String).collect()),
        );
    }

    if let Some(v) = data {
        obj.insert("data".to_string(), v);
    }

    facts_by_id.insert(fact_id.clone(), serde_json::Value::Object(obj.clone()));
    write_facts_file(repo_root, &facts_by_id)?;

    Ok(())
}

pub fn obligations_upsert_rule(
    rule_id: String,
    when_path_prefix: String,
    require_module_card: Option<String>,
    require_fact_types: Vec<String>,
    require_session_capsule: Option<bool>,
) -> Result<(), KbError> {
    obligations_upsert_rule_at(
        &discover_repo_root()?,
        rule_id,
        when_path_prefix,
        require_module_card,
        require_fact_types,
        require_session_capsule,
    )
}

pub fn obligations_upsert_rule_at(
    repo_root: &Path,
    rule_id: String,
    when_path_prefix: String,
    require_module_card: Option<String>,
    require_fact_types: Vec<String>,
    require_session_capsule: Option<bool>,
) -> Result<(), KbError> {
    let rule_id = rule_id.trim().to_string();
    validate_rule_id(&rule_id)?;

    let when_path_prefix = normalize_path_prefix(&when_path_prefix)?;

    let require_module_card = require_module_card
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(card) = require_module_card.as_deref() {
        validate_module_id(card)?;
    }

    let mut fact_types = normalize_unique_strings(require_fact_types);
    for t in &fact_types {
        validate_fact_type(t)?;
    }
    fact_types.sort();
    fact_types.dedup();

    let require_fact_types = if fact_types.is_empty() {
        None
    } else {
        Some(fact_types)
    };

    let require_session_capsule = require_session_capsule.filter(|v| *v);

    let path = obligations_toml_path(repo_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(KbError::internal(err, "failed to read obligations.toml")),
    };

    let cfg: ObligationsConfig = if text.trim().is_empty() {
        ObligationsConfig { rule: Vec::new() }
    } else {
        toml::from_str(&text).map_err(|err| {
            KbError::invalid_argument("failed to parse obligations.toml")
                .with_detail("cause", err.to_string())
        })?
    };

    let mut by_id: BTreeMap<String, ObligationRule> = BTreeMap::new();
    for r in cfg.rule {
        by_id.insert(r.id.clone(), r);
    }

    by_id.insert(
        rule_id.clone(),
        ObligationRule {
            id: rule_id,
            when_path_prefix,
            require_module_card,
            require_fact_types,
            require_session_capsule,
        },
    );

    let mut out = String::new();
    for (_id, r) in by_id {
        out.push_str("[[rule]]\n");
        out.push_str(&format!("id = {}\n", render_toml_string(&r.id)));
        out.push_str(&format!(
            "when_path_prefix = {}\n",
            render_toml_string(&r.when_path_prefix)
        ));
        if let Some(card) = r.require_module_card.as_deref() {
            out.push_str(&format!(
                "require_module_card = {}\n",
                render_toml_string(card)
            ));
        }
        if let Some(types) = r.require_fact_types.as_deref() {
            let mut t = types.to_vec();
            t.sort();
            t.dedup();
            out.push_str(&format!(
                "require_fact_types = {}\n",
                render_toml_string_array(&t)
            ));
        }
        if r.require_session_capsule.unwrap_or(false) {
            out.push_str("require_session_capsule = true\n");
        }
        out.push('\n');
    }

    write_text_file(&path, &out)?;
    Ok(())
}

fn obligations_toml_path(repo_root: &Path) -> PathBuf {
    repo_root.join("kb/config/obligations.toml")
}

fn load_facts_by_id(repo_root: &Path) -> Result<BTreeMap<String, serde_json::Value>, KbError> {
    let path = repo_root.join("kb/facts/facts.jsonl");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(KbError::internal(err, "failed to read facts.jsonl")),
    };

    let mut out: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).map_err(|err| {
            KbError::invalid_argument("failed to parse facts.jsonl")
                .with_detail("cause", err.to_string())
        })?;
        let Some(obj) = v.as_object() else {
            return Err(KbError::invalid_argument(
                "fact record must be a JSON object",
            ));
        };
        let Some(id) = obj.get("fact_id").and_then(|x| x.as_str()) else {
            return Err(KbError::invalid_argument("fact record missing fact_id"));
        };
        out.insert(id.to_string(), v);
    }
    Ok(out)
}

fn write_facts_file(
    repo_root: &Path,
    facts_by_id: &BTreeMap<String, serde_json::Value>,
) -> Result<(), KbError> {
    let path = repo_root.join("kb/facts/facts.jsonl");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| KbError::internal(err, "failed to create kb/facts directory"))?;
    }

    let file = std::fs::File::create(&path)
        .map_err(|err| KbError::internal(err, "failed to write facts.jsonl"))?;
    let mut writer = std::io::BufWriter::new(file);
    for v in facts_by_id.values() {
        serde_json::to_writer(&mut writer, v)
            .map_err(|err| KbError::internal(err, "failed to serialize fact"))?;
        writer
            .write_all(b"\n")
            .map_err(|err| KbError::internal(err, "failed to write facts.jsonl"))?;
    }
    writer
        .flush()
        .map_err(|err| KbError::internal(err, "failed to flush facts.jsonl"))?;
    Ok(())
}

fn normalize_unique_strings(values: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = values
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn normalize_unique_paths(values: Vec<String>, allow_dirs: bool) -> Result<Vec<String>, KbError> {
    let mut out = Vec::new();
    for v in values {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            continue;
        }
        let is_dir = trimmed.ends_with('/');
        if is_dir && !allow_dirs {
            return Err(KbError::invalid_argument("path must not end with '/'")
                .with_detail("path", trimmed));
        }
        let normalized = RepoPath::parse(trimmed.trim_end_matches('/'))?
            .as_str()
            .to_string();
        if is_dir {
            out.push(format!("{normalized}/"));
        } else {
            out.push(normalized);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn validate_single_line_string(field: &str, value: &str) -> Result<(), KbError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(
            KbError::invalid_argument("value must not be empty").with_detail("field", field)
        );
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(
            KbError::invalid_argument("value contains invalid characters")
                .with_detail("field", field),
        );
    }
    Ok(())
}

fn validate_tag_id(tag_id: &str) -> Result<(), KbError> {
    validate_ascii_id("tag_id", tag_id, false)
}

fn validate_module_id(module_id: &str) -> Result<(), KbError> {
    validate_ascii_id("module_id", module_id, false)
}

fn validate_rule_id(rule_id: &str) -> Result<(), KbError> {
    validate_ascii_id("rule_id", rule_id, false)
}

fn validate_fact_id(fact_id: &str) -> Result<(), KbError> {
    validate_ascii_id("fact_id", fact_id, true)
}

fn validate_fact_type(fact_type: &str) -> Result<(), KbError> {
    let trimmed = fact_type.trim();
    if trimmed.is_empty() {
        return Err(KbError::invalid_argument("fact type must not be empty"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(KbError::invalid_argument("invalid fact type").with_detail("type", trimmed));
    }
    Ok(())
}

fn validate_ascii_id(field: &str, value: &str, allow_colon: bool) -> Result<(), KbError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(KbError::invalid_argument("id must not be empty").with_detail(field, value));
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(
            KbError::invalid_argument("id contains invalid characters").with_detail(field, value)
        );
    }
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return Err(KbError::invalid_argument("id must not be empty").with_detail(field, value));
    };
    if !first.is_ascii_alphanumeric() {
        return Err(KbError::invalid_argument("invalid id").with_detail(field, trimmed));
    }
    for c in chars {
        if c.is_ascii_alphanumeric()
            || c == '_'
            || c == '.'
            || c == '-'
            || (allow_colon && c == ':')
        {
            continue;
        }
        return Err(KbError::invalid_argument("invalid id").with_detail(field, trimmed));
    }
    Ok(())
}

fn write_text_file(path: &Path, text: &str) -> Result<(), KbError> {
    write_text_file_create_mode(path, text, false)
}

fn write_text_file_create_mode(path: &Path, text: &str, create_new: bool) -> Result<(), KbError> {
    let normalized = text.trim_end_matches('\n');
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| KbError::internal(err, "failed to create parent directory"))?;
    }

    if create_new {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|err| {
                KbError::invalid_argument("file already exists")
                    .with_detail("cause", err.to_string())
            })?;
        file.write_all(normalized.as_bytes())
            .map_err(|err| KbError::internal(err, "failed to write file"))?;
        file.write_all(b"\n")
            .map_err(|err| KbError::internal(err, "failed to write file"))?;
        return Ok(());
    }

    std::fs::write(path, format!("{normalized}\n"))
        .map_err(|err| KbError::internal(err, "failed to write file"))?;
    Ok(())
}

fn render_toml_string(value: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn render_toml_string_array(values: &[String]) -> String {
    let inner = values
        .iter()
        .map(|v| render_toml_string(v))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{inner}]")
}
