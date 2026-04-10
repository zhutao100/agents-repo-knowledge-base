use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use clap::ValueEnum;

use crate::error::{ErrorCode, KbError};
use crate::index::artifacts::{DepEdge, SymbolRecord, TreeRecord};
use crate::query::module_card::ModuleCardToml;
use crate::query::read::{read_jsonl, reader_for, try_read_text};
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::root::discover_repo_root;

const TOP_SYMBOLS_PER_FILE: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
pub enum DescribePathInclude {
    Dirs,
    Files,
    TopSymbols,
    Entrypoints,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DescribePathOutput {
    pub path: String,
    pub depth: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirs: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<TreeRecord>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoints: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
pub enum DescribeModuleInclude {
    All,
    Card,
    Entrypoints,
    EditPoints,
    RelatedFacts,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DescribeModuleOutput {
    pub module_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<ModuleCardOut>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoints: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_points: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_facts: Option<Vec<String>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ModuleCardOut {
    pub id: String,
    pub title: String,
    pub owners: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DescribeFactOutput {
    pub fact_id: String,
    pub fact: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
pub enum DescribeSymbolInclude {
    Def,
    Signature,
    Uses,
    Deps,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DescribeSymbolOutput {
    pub symbol_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub def: Option<SymbolDef>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses: Option<Vec<XrefEdge>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub deps: Option<Vec<DepEdge>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SymbolDef {
    pub path: String,
    pub line: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,

    pub kind: String,
    pub name: String,
    pub qualified_name: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct XrefEdge {
    pub from_symbol_id: String,
    pub kind: String,
    pub to_symbol_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

pub fn describe_path(
    path: String,
    depth: u32,
    include: Vec<DescribePathInclude>,
) -> Result<DescribePathOutput, KbError> {
    describe_path_at(&discover_repo_root()?, path, depth, include)
}

pub fn describe_module(
    module_id: String,
    include: Vec<DescribeModuleInclude>,
) -> Result<DescribeModuleOutput, KbError> {
    describe_module_at(&discover_repo_root()?, module_id, include)
}

pub fn describe_symbol(
    symbol_id: String,
    include: Vec<DescribeSymbolInclude>,
) -> Result<DescribeSymbolOutput, KbError> {
    describe_symbol_at(&discover_repo_root()?, symbol_id, include)
}

pub fn describe_fact(fact_id: String) -> Result<DescribeFactOutput, KbError> {
    describe_fact_at(&discover_repo_root()?, fact_id)
}

pub fn describe_path_at(
    repo_root: &Path,
    path: String,
    depth: u32,
    include: Vec<DescribePathInclude>,
) -> Result<DescribePathOutput, KbError> {
    let include_set: BTreeSet<DescribePathInclude> = include.into_iter().collect();
    let want_dirs = include_set.contains(&DescribePathInclude::Dirs);
    let want_files = include_set.contains(&DescribePathInclude::Files)
        || include_set.contains(&DescribePathInclude::TopSymbols);
    let want_top_symbols = include_set.contains(&DescribePathInclude::TopSymbols);
    let want_entrypoints = include_set.contains(&DescribePathInclude::Entrypoints);

    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let tree: Vec<TreeRecord> = read_jsonl(&reader, "kb/gen/tree.jsonl")?;
    let tree_by_path: BTreeMap<String, TreeRecord> =
        tree.into_iter().map(|r| (r.path.clone(), r)).collect();

    let (normalized, is_dir, is_root) = normalize_describe_path_input(&path)?;
    let target = if is_dir && !normalized.is_empty() {
        format!("{normalized}/")
    } else {
        normalized.clone()
    };

    if !is_root {
        let Some(record) = tree_by_path.get(&target) else {
            return Err(KbError::not_found("path not found").with_detail("path", &path));
        };
        if is_dir && record.kind != "dir" {
            return Err(KbError::not_found("directory not found").with_detail("path", &path));
        }
        if !is_dir && record.kind != "file" {
            return Err(KbError::not_found("file not found").with_detail("path", &path));
        }
    }

    let (dirs, mut files) = if is_dir {
        let (base_prefix, base_segments) = if is_root {
            ("".to_string(), 0usize)
        } else {
            (target.clone(), count_segments(&target))
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for (p, r) in &tree_by_path {
            if !p.starts_with(&base_prefix) {
                continue;
            }
            let segs = count_segments(p);
            if segs > base_segments + depth as usize {
                continue;
            }
            if r.kind == "dir" {
                if want_dirs {
                    dirs.push(p.clone());
                }
            } else if r.kind == "file" && want_files {
                files.push(r.clone());
            }
        }
        (Some(dirs), Some(files))
    } else {
        let file = tree_by_path
            .get(&target)
            .cloned()
            .ok_or_else(|| KbError::not_found("file not found").with_detail("path", &path))?;
        (
            if want_dirs { Some(Vec::new()) } else { None },
            if want_files { Some(vec![file]) } else { None },
        )
    };

    if let Some(ref mut fs) = files {
        fs.sort_by(|a, b| a.path.cmp(&b.path));
        fs.dedup_by(|a, b| a.path == b.path);

        if want_top_symbols {
            let symbols: Vec<SymbolRecord> = read_jsonl(&reader, "kb/gen/symbols.jsonl")?;
            let mut by_path: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for s in symbols {
                by_path.entry(s.path).or_default().push(s.symbol_id);
            }
            for ids in by_path.values_mut() {
                ids.sort();
                ids.dedup();
            }

            for f in fs.iter_mut() {
                if let Some(ids) = by_path.get(&f.path) {
                    f.top_symbols = Some(ids.iter().take(TOP_SYMBOLS_PER_FILE).cloned().collect());
                } else {
                    f.top_symbols = Some(Vec::new());
                }
            }
        }
    }

    let entrypoints = if want_entrypoints && is_dir {
        let mut eps = Vec::new();
        let modules_dir = repo_root.join("kb/atlas/modules");
        if modules_dir.is_dir() {
            let mut files = std::fs::read_dir(&modules_dir)
                .map_err(|err| KbError::internal(err, "failed to read kb/atlas/modules"))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("toml"))
                .collect::<Vec<_>>();
            files.sort();

            for file in files {
                let text = std::fs::read_to_string(&file)
                    .map_err(|err| KbError::internal(err, "failed to read module card"))?;
                let card: ModuleCardToml = toml::from_str(&text).map_err(|err| {
                    KbError::invalid_argument("failed to parse module card")
                        .with_detail("cause", err.to_string())
                })?;
                for ep in card.entrypoints {
                    if is_root || ep.starts_with(&target) {
                        eps.push(ep);
                    }
                }
            }
        }

        eps.sort();
        eps.dedup();
        Some(eps)
    } else {
        None
    };

    Ok(DescribePathOutput {
        path,
        depth,
        dirs: if want_dirs { dirs } else { None },
        files: if want_files { files } else { None },
        entrypoints,
    })
}

pub fn describe_module_at(
    repo_root: &Path,
    module_id: String,
    include: Vec<DescribeModuleInclude>,
) -> Result<DescribeModuleOutput, KbError> {
    let include_set: BTreeSet<DescribeModuleInclude> = include.into_iter().collect();
    let want_all = include_set.contains(&DescribeModuleInclude::All);
    let want_card = want_all || include_set.contains(&DescribeModuleInclude::Card);
    let want_entrypoints = want_all || include_set.contains(&DescribeModuleInclude::Entrypoints);
    let want_edit_points = want_all || include_set.contains(&DescribeModuleInclude::EditPoints);
    let want_related_facts = want_all || include_set.contains(&DescribeModuleInclude::RelatedFacts);

    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let rel = format!("kb/atlas/modules/{module_id}.toml");
    let Some(text) = try_read_text(&reader, &rel)? else {
        return Err(
            KbError::not_found("module card not found").with_detail("module_id", &module_id)
        );
    };
    let card: ModuleCardToml = toml::from_str(&text).map_err(|err| {
        KbError::invalid_argument("failed to parse module card")
            .with_detail("cause", err.to_string())
    })?;

    if card.id != module_id {
        return Err(
            KbError::invalid_argument("module card id must match filename")
                .with_detail("module_id", module_id)
                .with_detail("id", card.id),
        );
    }

    let mut entrypoints = card.entrypoints;
    entrypoints.sort();
    entrypoints.dedup();
    let mut edit_points = card.edit_points;
    edit_points.sort();
    edit_points.dedup();
    let mut related_facts = card.related_facts;
    related_facts.sort();
    related_facts.dedup();

    Ok(DescribeModuleOutput {
        module_id: module_id.clone(),
        card: if want_card {
            Some(ModuleCardOut {
                id: module_id,
                title: card.title,
                owners: card.owners,
                tags: card.tags,
            })
        } else {
            None
        },
        entrypoints: if want_entrypoints {
            Some(entrypoints)
        } else {
            None
        },
        edit_points: if want_edit_points {
            Some(edit_points)
        } else {
            None
        },
        related_facts: if want_related_facts {
            Some(related_facts)
        } else {
            None
        },
    })
}

pub fn describe_fact_at(repo_root: &Path, fact_id: String) -> Result<DescribeFactOutput, KbError> {
    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let facts = match read_jsonl::<serde_json::Value>(&reader, "kb/facts/facts.jsonl") {
        Ok(v) => v,
        Err(err) if err.code == ErrorCode::NotFound => {
            return Err(KbError::not_found("fact not found").with_detail("fact_id", &fact_id));
        }
        Err(err) => return Err(err),
    };

    for fact in facts {
        let Some(obj) = fact.as_object() else {
            return Err(KbError::invalid_argument(
                "fact record must be a JSON object",
            ));
        };
        if obj.get("type").and_then(|v| v.as_str()).is_none() {
            return Err(KbError::invalid_argument("fact record missing type"));
        }
        let Some(id) = obj.get("fact_id").and_then(|v| v.as_str()) else {
            return Err(KbError::invalid_argument("fact record missing fact_id"));
        };

        if id == fact_id.as_str() {
            return Ok(DescribeFactOutput { fact_id, fact });
        }
    }

    Err(KbError::not_found("fact not found").with_detail("fact_id", &fact_id))
}

pub fn describe_symbol_at(
    repo_root: &Path,
    symbol_id: String,
    include: Vec<DescribeSymbolInclude>,
) -> Result<DescribeSymbolOutput, KbError> {
    let include_set: BTreeSet<DescribeSymbolInclude> = include.into_iter().collect();
    let want_def = include_set.contains(&DescribeSymbolInclude::Def);
    let want_signature = include_set.contains(&DescribeSymbolInclude::Signature);
    let want_uses = include_set.contains(&DescribeSymbolInclude::Uses);
    let want_deps = include_set.contains(&DescribeSymbolInclude::Deps);

    let reader = reader_for(repo_root, &DiffSource::Worktree);
    let symbols: Vec<SymbolRecord> = read_jsonl(&reader, "kb/gen/symbols.jsonl")?;
    let sym = symbols
        .into_iter()
        .find(|s| s.symbol_id == symbol_id)
        .ok_or_else(|| KbError::not_found("symbol not found"))?;

    let def = if want_def {
        Some(SymbolDef {
            path: sym.path.clone(),
            line: sym.line,
            end_line: sym.end_line,
            kind: sym.kind.clone(),
            name: sym.name.clone(),
            qualified_name: sym.qualified_name.clone(),
        })
    } else {
        None
    };

    let signature = if want_signature {
        sym.signature.clone()
    } else {
        None
    };

    let deps = if want_deps {
        let edges: Vec<DepEdge> = read_jsonl(&reader, "kb/gen/deps.jsonl")?;
        let mut out: Vec<DepEdge> = edges
            .into_iter()
            .filter(|e| e.from_path == sym.path)
            .collect();
        out.sort_by(dep_edge_cmp);
        Some(out)
    } else {
        None
    };

    let uses = if want_uses {
        let Some(text) = try_read_text(&reader, "kb/gen/xrefs.jsonl")? else {
            return Ok(DescribeSymbolOutput {
                symbol_id: sym.symbol_id,
                def,
                signature,
                uses: Some(Vec::new()),
                deps,
            });
        };

        let mut out = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let edge: XrefEdge = serde_json::from_str(line).map_err(|err| {
                KbError::invalid_argument("failed to parse xrefs.jsonl")
                    .with_detail("cause", err.to_string())
            })?;
            if edge.to_symbol_id == sym.symbol_id {
                out.push(edge);
            }
        }
        out.sort_by(|a, b| match a.from_symbol_id.cmp(&b.from_symbol_id) {
            std::cmp::Ordering::Equal => match a.kind.cmp(&b.kind) {
                std::cmp::Ordering::Equal => match a.to_symbol_id.cmp(&b.to_symbol_id) {
                    std::cmp::Ordering::Equal => match a.path.cmp(&b.path) {
                        std::cmp::Ordering::Equal => a.line.cmp(&b.line),
                        other => other,
                    },
                    other => other,
                },
                other => other,
            },
            other => other,
        });
        Some(out)
    } else {
        None
    };

    Ok(DescribeSymbolOutput {
        symbol_id: sym.symbol_id,
        def,
        signature,
        uses,
        deps,
    })
}

fn dep_edge_cmp(a: &DepEdge, b: &DepEdge) -> std::cmp::Ordering {
    let a_to = a
        .to_path
        .as_deref()
        .or(a.to_external.as_deref())
        .unwrap_or("");
    let b_to = b
        .to_path
        .as_deref()
        .or(b.to_external.as_deref())
        .unwrap_or("");

    match a.from_path.cmp(&b.from_path) {
        std::cmp::Ordering::Equal => match a.kind.cmp(&b.kind) {
            std::cmp::Ordering::Equal => match a_to.cmp(b_to) {
                std::cmp::Ordering::Equal => match a.to_path.is_some().cmp(&b.to_path.is_some()) {
                    std::cmp::Ordering::Equal => a.raw.cmp(&b.raw),
                    other => other,
                },
                other => other,
            },
            other => other,
        },
        other => other,
    }
}

fn normalize_describe_path_input(input: &str) -> Result<(String, bool, bool), KbError> {
    let trimmed = input.trim();
    if trimmed == "." {
        return Ok((String::new(), true, true));
    }
    let is_dir = trimmed.ends_with('/');
    let raw = trimmed.trim_end_matches('/');
    let normalized = RepoPath::parse(raw)?.as_str().to_string();
    Ok((normalized, is_dir, false))
}

fn count_segments(path: &str) -> usize {
    let p = path.trim_end_matches('/');
    if p.is_empty() {
        return 0;
    }
    p.split('/').count()
}

pub fn describe_path_text(out: &DescribePathOutput) -> String {
    let mut lines = Vec::new();
    lines.push(format!("path: {}", out.path));
    lines.push(format!("depth: {}", out.depth));
    if let Some(dirs) = out.dirs.as_deref() {
        lines.push("dirs:".to_string());
        for d in dirs {
            lines.push(format!("- {d}"));
        }
    }
    if let Some(files) = out.files.as_deref() {
        lines.push("files:".to_string());
        for f in files {
            lines.push(format!("- {}", f.path));
        }
    }
    if let Some(eps) = out.entrypoints.as_deref() {
        lines.push("entrypoints:".to_string());
        for ep in eps {
            lines.push(format!("- {ep}"));
        }
    }
    lines.join("\n")
}

pub fn describe_module_text(out: &DescribeModuleOutput) -> String {
    let mut lines = Vec::new();
    lines.push(format!("module_id: {}", out.module_id));
    if let Some(card) = out.card.as_ref() {
        lines.push(format!("title: {}", card.title));
    }
    if let Some(eps) = out.entrypoints.as_deref() {
        lines.push("entrypoints:".to_string());
        for ep in eps {
            lines.push(format!("- {ep}"));
        }
    }
    if let Some(eps) = out.edit_points.as_deref() {
        lines.push("edit_points:".to_string());
        for ep in eps {
            lines.push(format!("- {ep}"));
        }
    }
    if let Some(rf) = out.related_facts.as_deref() {
        lines.push("related_facts:".to_string());
        for id in rf {
            lines.push(format!("- {id}"));
        }
    }
    lines.join("\n")
}

pub fn describe_symbol_text(out: &DescribeSymbolOutput) -> String {
    let mut lines = Vec::new();
    lines.push(format!("symbol_id: {}", out.symbol_id));
    if let Some(def) = out.def.as_ref() {
        lines.push(format!("def: {}:{}", def.path, def.line));
    }
    if let Some(sig) = out.signature.as_deref() {
        lines.push(format!("signature: {sig}"));
    }
    if let Some(deps) = out.deps.as_deref() {
        lines.push(format!("deps: {}", deps.len()));
    }
    if let Some(uses) = out.uses.as_deref() {
        lines.push(format!("uses: {}", uses.len()));
    }
    lines.join("\n")
}

pub fn describe_fact_text(out: &DescribeFactOutput) -> String {
    let text = serde_json::to_string_pretty(&out.fact)
        .unwrap_or_else(|_| "{\"error\":\"failed to format fact\"}".to_string());
    format!("fact_id: {}\n{}", out.fact_id, text)
}
