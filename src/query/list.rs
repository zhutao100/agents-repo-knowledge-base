use std::path::Path;

use crate::error::{ErrorCode, KbError};
use crate::index::artifacts::SymbolRecord;
use crate::query::module_card::ModuleCardToml;
use crate::query::read::{read_jsonl, reader_for, try_read_text};
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::root::discover_repo_root;

#[derive(Clone, Debug, serde::Serialize)]
pub struct ListTagsOutput {
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ListModulesOutput {
    pub modules: Vec<ModuleListEntry>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ModuleListEntry {
    pub module_id: String,
    pub title: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ListFactsOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    pub facts: Vec<FactSummary>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct FactSummary {
    pub fact_id: String,
    pub r#type: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ListSymbolsOutput {
    pub path: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    pub symbols: Vec<SymbolListEntry>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SymbolListEntry {
    pub symbol_id: String,
    pub name: String,
    pub qualified_name: String,
}

#[derive(Debug, serde::Deserialize)]
struct TagsConfig {
    #[serde(default)]
    tag: Vec<TagEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TagEntry {
    id: String,
    #[serde(default)]
    description: Option<String>,
}

pub fn list_tags() -> Result<ListTagsOutput, KbError> {
    list_tags_at(&discover_repo_root()?)
}

pub fn list_modules(
    tag: Option<String>,
    owner: Option<String>,
) -> Result<ListModulesOutput, KbError> {
    list_modules_at(&discover_repo_root()?, tag, owner)
}

pub fn list_facts(
    fact_type: Option<String>,
    tag: Option<String>,
) -> Result<ListFactsOutput, KbError> {
    list_facts_at(&discover_repo_root()?, fact_type, tag)
}

pub fn list_symbols(path: String, kind: Option<String>) -> Result<ListSymbolsOutput, KbError> {
    list_symbols_at(&discover_repo_root()?, path, kind)
}

pub fn list_tags_text(out: &ListTagsOutput) -> String {
    out.tags.join("\n")
}

pub fn list_modules_text(out: &ListModulesOutput) -> String {
    out.modules
        .iter()
        .map(|m| format!("{}: {}", m.module_id, m.title))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn list_facts_text(out: &ListFactsOutput) -> String {
    out.facts
        .iter()
        .map(|f| f.fact_id.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn list_symbols_text(out: &ListSymbolsOutput) -> String {
    out.symbols
        .iter()
        .map(|s| format!("{} {}", s.symbol_id, s.qualified_name))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn list_tags_at(repo_root: &Path) -> Result<ListTagsOutput, KbError> {
    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let text = match try_read_text(&reader, "kb/config/tags.toml")? {
        Some(t) => t,
        None => return Ok(ListTagsOutput { tags: Vec::new() }),
    };

    let cfg: TagsConfig = toml::from_str(&text).map_err(|err| {
        KbError::invalid_argument("failed to parse tags.toml").with_detail("cause", err.to_string())
    })?;
    let mut tags: Vec<String> = cfg
        .tag
        .into_iter()
        .map(|t| {
            let _ = t.description;
            t.id
        })
        .collect();
    tags.sort();
    tags.dedup();
    Ok(ListTagsOutput { tags })
}

pub fn list_modules_at(
    repo_root: &Path,
    tag: Option<String>,
    owner: Option<String>,
) -> Result<ListModulesOutput, KbError> {
    if let Some(t) = tag.as_deref() {
        validate_tag(repo_root, t)?;
    }

    let mut out = Vec::new();
    let modules_dir = repo_root.join("kb/atlas/modules");
    if !modules_dir.is_dir() {
        return Ok(ListModulesOutput { modules: out });
    }

    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&modules_dir)
        .map_err(|err| KbError::internal(err, "failed to read kb/atlas/modules"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("toml"))
        .collect();
    files.sort();

    for path in files {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| KbError::invalid_argument("invalid module card filename"))?;
        let text = std::fs::read_to_string(&path)
            .map_err(|err| KbError::internal(err, "failed to read module card"))?;
        let card: ModuleCardToml = toml::from_str(&text).map_err(|err| {
            KbError::invalid_argument("failed to parse module card")
                .with_detail("cause", err.to_string())
        })?;

        if card.id != stem {
            return Err(
                KbError::invalid_argument("module card id must match filename")
                    .with_detail("file", format!("kb/atlas/modules/{stem}.toml"))
                    .with_detail("id", card.id),
            );
        }

        if let Some(ref t) = tag {
            if !card.tags.iter().any(|x| x == t) {
                continue;
            }
        }
        if let Some(ref o) = owner {
            if !card.owners.iter().any(|x| x == o) {
                continue;
            }
        }

        out.push(ModuleListEntry {
            module_id: card.id,
            title: card.title,
        });
    }

    out.sort_by(|a, b| a.module_id.cmp(&b.module_id));
    out.dedup_by(|a, b| a.module_id == b.module_id);
    Ok(ListModulesOutput { modules: out })
}

pub fn list_facts_at(
    repo_root: &Path,
    fact_type: Option<String>,
    tag: Option<String>,
) -> Result<ListFactsOutput, KbError> {
    if let Some(t) = tag.as_deref() {
        validate_tag(repo_root, t)?;
    }

    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let facts = match read_jsonl::<serde_json::Value>(&reader, "kb/facts/facts.jsonl") {
        Ok(v) => v,
        Err(err) if err.code == ErrorCode::NotFound => Vec::new(),
        Err(err) => return Err(err),
    };

    let mut out = Vec::new();
    for fact in facts {
        let Some(obj) = fact.as_object() else {
            return Err(KbError::invalid_argument(
                "fact record must be a JSON object",
            ));
        };
        let Some(t) = obj.get("type").and_then(|v| v.as_str()) else {
            return Err(KbError::invalid_argument("fact record missing type"));
        };
        let Some(id) = obj.get("fact_id").and_then(|v| v.as_str()) else {
            return Err(KbError::invalid_argument("fact record missing fact_id"));
        };

        if let Some(ref type_filter) = fact_type {
            if t != type_filter {
                continue;
            }
        }

        let mut tags = obj
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .map(str::to_string)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        tags.sort();
        tags.dedup();

        if let Some(ref tag_filter) = tag {
            if !tags.iter().any(|x| x == tag_filter) {
                continue;
            }
        }

        out.push(FactSummary {
            fact_id: id.to_string(),
            r#type: t.to_string(),
            tags,
        });
    }

    out.sort_by(|a, b| a.fact_id.cmp(&b.fact_id));
    out.dedup_by(|a, b| a.fact_id == b.fact_id);

    Ok(ListFactsOutput {
        r#type: fact_type,
        tag,
        facts: out,
    })
}

pub fn list_symbols_at(
    repo_root: &Path,
    path: String,
    kind: Option<String>,
) -> Result<ListSymbolsOutput, KbError> {
    let normalized = RepoPath::parse(&path)?.as_str().to_string();
    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let symbols = read_jsonl::<SymbolRecord>(&reader, "kb/gen/symbols.jsonl")?;

    let mut out = Vec::new();
    for s in symbols {
        if s.path != normalized {
            continue;
        }
        if let Some(ref k) = kind {
            if &s.kind != k {
                continue;
            }
        }
        out.push(SymbolListEntry {
            symbol_id: s.symbol_id,
            name: s.name,
            qualified_name: s.qualified_name,
        });
    }

    out.sort_by(|a, b| a.symbol_id.cmp(&b.symbol_id));
    out.dedup_by(|a, b| a.symbol_id == b.symbol_id);

    Ok(ListSymbolsOutput {
        path: normalized,
        kind,
        symbols: out,
    })
}

fn validate_tag(repo_root: &Path, tag: &str) -> Result<(), KbError> {
    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let Some(text) = try_read_text(&reader, "kb/config/tags.toml")? else {
        return Ok(());
    };

    let cfg: TagsConfig = toml::from_str(&text).map_err(|err| {
        KbError::invalid_argument("failed to parse tags.toml").with_detail("cause", err.to_string())
    })?;
    if cfg.tag.iter().any(|t| t.id == tag) {
        return Ok(());
    }
    Err(KbError::invalid_argument("unknown tag").with_detail("tag", tag))
}
