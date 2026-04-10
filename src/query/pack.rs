use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use crate::error::{ErrorCode, KbError};
use crate::index::artifacts::{DepEdge, SymbolRecord, TreeRecord};
use crate::query::module_card::ModuleCardToml;
use crate::query::plan::{
    plan_diff_at, ChangedPath, PlanDiffOutput, Policy, Required, TriggeredRule,
};
use crate::query::read::{read_jsonl, reader_for, try_read_text};
use crate::repo::diff::ChangeKind;
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::reader::DiffSourceReader;
use crate::repo::root::discover_repo_root;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Budgets {
    pub max_bytes: u64,
    pub snippet_lines: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ModuleCard {
    pub module_id: String,
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Snippet {
    pub path: String,
    pub symbol_id: String,
    pub start_line: u64,
    pub end_line: u64,
    pub text: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PackDiffOutput {
    pub diff_source: String,
    pub radius: u32,
    pub budgets: Budgets,
    pub plan: PlanSummary,
    pub modules: Vec<ModuleCard>,
    pub facts: Vec<serde_json::Value>,
    pub tree: Vec<TreeRecord>,
    pub symbols: Vec<SymbolRecord>,
    pub deps: Vec<DepEdge>,
    pub snippets: Vec<Snippet>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlanSummary {
    pub policy: String,
    pub changed_paths: Vec<ChangedPath>,
    pub affected_modules: Vec<String>,
    pub triggered_rules: Vec<TriggeredRule>,
    pub required: Required,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PackSelectorsOutput {
    pub selectors: SelectorsSummary,
    pub budgets: Budgets,
    pub modules: Vec<ModuleCard>,
    pub facts: Vec<serde_json::Value>,
    pub tree: Vec<TreeRecord>,
    pub symbols: Vec<SymbolRecord>,
    pub deps: Vec<DepEdge>,
    pub snippets: Vec<Snippet>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SelectorsSummary {
    pub paths: Vec<String>,
    pub modules: Vec<String>,
    pub symbols: Vec<String>,
    pub facts: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SelectorInputs {
    pub paths: Vec<String>,
    pub modules: Vec<String>,
    pub symbols: Vec<String>,
    pub facts: Vec<String>,
}

const TOP_SYMBOLS_PER_FILE: usize = 5;

pub fn pack_diff(
    diff_source: &DiffSource,
    radius: u32,
    max_bytes: u64,
    snippet_lines: u64,
) -> Result<PackDiffOutput, KbError> {
    pack_diff_at(
        &discover_repo_root()?,
        diff_source,
        radius,
        max_bytes,
        snippet_lines,
    )
}

pub fn pack_selectors(
    selectors: &SelectorInputs,
    max_bytes: u64,
    snippet_lines: u64,
) -> Result<PackSelectorsOutput, KbError> {
    pack_selectors_at(&discover_repo_root()?, selectors, max_bytes, snippet_lines)
}

pub fn pack_diff_at(
    repo_root: &Path,
    diff_source: &DiffSource,
    radius: u32,
    max_bytes: u64,
    snippet_lines: u64,
) -> Result<PackDiffOutput, KbError> {
    let plan = plan_diff_at(repo_root, diff_source, Policy::Default)?;
    let reader = reader_for(repo_root, diff_source);

    let mut modules = read_required_modules(&reader, &plan.required.module_cards)?;
    let mut facts = read_required_facts(&reader, &plan.required.fact_types)?;

    let tree_records: Vec<TreeRecord> = read_jsonl(&reader, "kb/gen/tree.jsonl")?;
    let symbols_records: Vec<SymbolRecord> = read_jsonl(&reader, "kb/gen/symbols.jsonl")?;
    let deps_records: Vec<DepEdge> = read_jsonl(&reader, "kb/gen/deps.jsonl")?;

    let tree_files_by_path: BTreeMap<String, TreeRecord> = tree_records
        .into_iter()
        .filter(|r| r.kind == "file")
        .map(|r| (r.path.clone(), r))
        .collect();
    let file_set: BTreeSet<String> = tree_files_by_path.keys().cloned().collect();

    let seed_paths = seed_paths_from_plan(&plan, &file_set);
    let included_paths = expand_paths(&seed_paths, &deps_records, &file_set, radius);

    let mut tree: Vec<TreeRecord> = included_paths
        .iter()
        .filter_map(|p| tree_files_by_path.get(p).cloned())
        .collect();
    tree.sort_by(|a, b| a.path.cmp(&b.path));
    tree.dedup_by(|a, b| a.path == b.path);

    let mut symbols: Vec<SymbolRecord> = symbols_records
        .into_iter()
        .filter(|s| included_paths.contains(&s.path))
        .collect();
    symbols.sort_by(|a, b| a.symbol_id.cmp(&b.symbol_id));
    symbols.dedup_by(|a, b| a.symbol_id == b.symbol_id);

    apply_top_symbols(&mut tree, &symbols);

    let mut deps: Vec<DepEdge> = deps_records
        .into_iter()
        .filter(|e| {
            if !included_paths.contains(&e.from_path) {
                return false;
            }
            if e.to_external.is_some() {
                return true;
            }
            e.to_path
                .as_deref()
                .is_some_and(|p| included_paths.contains(p))
        })
        .collect();
    deps.sort_by(dep_edge_cmp);
    deps.dedup();

    let mut snippets = build_snippets(
        repo_root,
        diff_source,
        snippet_lines,
        &included_paths,
        &symbols,
    )?;
    snippets.sort_by(|a, b| match a.path.cmp(&b.path) {
        std::cmp::Ordering::Equal => a.symbol_id.cmp(&b.symbol_id),
        other => other,
    });
    snippets.dedup_by(|a, b| a.path == b.path && a.symbol_id == b.symbol_id);

    modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));
    modules.dedup_by(|a, b| a.module_id == b.module_id);

    #[allow(clippy::unnecessary_sort_by)]
    facts.sort_by(|a, b| fact_id_str(a).cmp(fact_id_str(b)));
    facts.dedup_by(|a, b| fact_id_str(a) == fact_id_str(b));

    let mut out = PackDiffOutput {
        diff_source: plan.diff_source.clone(),
        radius,
        budgets: Budgets {
            max_bytes,
            snippet_lines,
        },
        plan: PlanSummary {
            policy: plan.policy.clone(),
            changed_paths: plan.changed_paths.clone(),
            affected_modules: plan.affected_modules.clone(),
            triggered_rules: plan.triggered_rules.clone(),
            required: plan.required.clone(),
        },
        modules,
        facts,
        tree,
        symbols,
        deps,
        snippets,
    };

    apply_budget_pack_diff(&mut out)?;
    Ok(out)
}

pub fn pack_selectors_at(
    repo_root: &Path,
    selectors: &SelectorInputs,
    max_bytes: u64,
    snippet_lines: u64,
) -> Result<PackSelectorsOutput, KbError> {
    let diff_source = DiffSource::Worktree;
    let reader = reader_for(repo_root, &diff_source);

    let tree_records: Vec<TreeRecord> = read_jsonl(&reader, "kb/gen/tree.jsonl")?;
    let symbols_records: Vec<SymbolRecord> = read_jsonl(&reader, "kb/gen/symbols.jsonl")?;
    let deps_records: Vec<DepEdge> = read_jsonl(&reader, "kb/gen/deps.jsonl")?;

    let tree_by_path: BTreeMap<String, TreeRecord> = tree_records
        .into_iter()
        .map(|r| (r.path.clone(), r))
        .collect();
    let symbols_by_id: BTreeMap<String, SymbolRecord> = symbols_records
        .iter()
        .cloned()
        .map(|s| (s.symbol_id.clone(), s))
        .collect();

    let selector_paths = normalize_unique(selectors.paths.iter().map(String::as_str))?;
    let selector_modules = normalize_unique(selectors.modules.iter().map(String::as_str))?;
    let selector_symbols = normalize_unique(selectors.symbols.iter().map(String::as_str))?;
    let selector_facts = normalize_unique(selectors.facts.iter().map(String::as_str))?;

    let mut included_tree = Vec::new();
    let mut included_symbols = Vec::new();
    let mut included_deps = Vec::new();
    let mut included_snippets = Vec::new();
    let mut included_modules = Vec::new();
    let mut included_facts = Vec::new();

    // Expand module selectors into additional typed selectors (entrypoints/edit-points/related-facts).
    let mut expanded_paths_raw: Vec<String> = selector_paths.clone();
    let mut expanded_fact_ids_raw: Vec<String> = selector_facts.clone();

    for module_id in &selector_modules {
        let rel = format!("kb/atlas/modules/{module_id}.toml");
        if let Some(text) = try_read_text(&reader, &rel)? {
            included_modules.push(ModuleCard {
                module_id: module_id.clone(),
                path: rel,
                text: normalize_text(&text),
            });

            let card: ModuleCardToml = toml::from_str(&text).map_err(|err| {
                KbError::invalid_argument("failed to parse module card")
                    .with_detail("module_id", module_id.clone())
                    .with_detail("cause", err.to_string())
            })?;
            expanded_paths_raw.extend(card.entrypoints);
            expanded_paths_raw.extend(card.edit_points);
            expanded_fact_ids_raw.extend(card.related_facts);
        }
    }

    let expanded_paths = normalize_unique(expanded_paths_raw.iter().map(String::as_str))?;
    let expanded_fact_ids = normalize_unique(expanded_fact_ids_raw.iter().map(String::as_str))?;

    let mut symbol_paths: BTreeSet<String> = BTreeSet::new();

    for path in &expanded_paths {
        let (normalized, is_dir) = resolve_selector_path(&tree_by_path, path)?;
        let key = if is_dir && !normalized.ends_with('/') {
            format!("{normalized}/")
        } else {
            normalized.clone()
        };

        if let Some(record) = tree_by_path.get(&key) {
            included_tree.push(record.clone());
        }

        if is_dir {
            let (tree_more, symbol_more) = expand_dir_tree(&tree_by_path, &key);
            included_tree.extend(tree_more);
            for p in symbol_more {
                symbol_paths.insert(p);
            }
        } else {
            symbol_paths.insert(key.clone());
        }
    }

    for symbol_id in &selector_symbols {
        if let Some(symbol) = symbols_by_id.get(symbol_id).cloned() {
            included_symbols.push(symbol.clone());
            if let Some(record) = tree_by_path.get(&symbol.path) {
                included_tree.push(record.clone());
            }
            if let Some(snippet) =
                build_symbol_snippet(repo_root, &diff_source, snippet_lines, &symbol)?
            {
                included_snippets.push(snippet);
            }
        }
    }

    for path in &symbol_paths {
        included_symbols.extend(symbols_records.iter().filter(|s| s.path == *path).cloned());
        included_deps.extend(
            deps_records
                .iter()
                .filter(|e| e.from_path == *path)
                .cloned(),
        );
    }

    let mut file_snippets = build_snippets(
        repo_root,
        &diff_source,
        snippet_lines,
        &symbol_paths,
        &included_symbols,
    )?;
    included_snippets.append(&mut file_snippets);

    if !expanded_fact_ids.is_empty() {
        let facts = match read_jsonl::<serde_json::Value>(&reader, "kb/facts/facts.jsonl") {
            Ok(v) => v,
            Err(err) if err.code == ErrorCode::NotFound => Vec::new(),
            Err(err) => return Err(err),
        };

        for fact in &facts {
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

        for fact_id_sel in &expanded_fact_ids {
            if let Some(fact) = facts
                .iter()
                .find(|v| fact_id_str(v) == fact_id_sel.as_str())
                .cloned()
            {
                included_facts.push(fact);
            }
        }
    }

    included_tree.sort_by(|a, b| a.path.cmp(&b.path));
    included_tree.dedup_by(|a, b| a.path == b.path);

    included_symbols.sort_by(|a, b| a.symbol_id.cmp(&b.symbol_id));
    included_symbols.dedup_by(|a, b| a.symbol_id == b.symbol_id);

    included_deps.sort_by(dep_edge_cmp);
    included_deps.dedup();

    included_modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));
    included_modules.dedup_by(|a, b| a.module_id == b.module_id);

    #[allow(clippy::unnecessary_sort_by)]
    included_facts.sort_by(|a, b| fact_id_str(a).cmp(fact_id_str(b)));
    included_facts.dedup_by(|a, b| fact_id_str(a) == fact_id_str(b));

    included_snippets.sort_by(|a, b| match a.path.cmp(&b.path) {
        std::cmp::Ordering::Equal => a.symbol_id.cmp(&b.symbol_id),
        other => other,
    });
    included_snippets.dedup_by(|a, b| a.path == b.path && a.symbol_id == b.symbol_id);

    apply_top_symbols(&mut included_tree, &symbols_records);

    let mut out = PackSelectorsOutput {
        selectors: SelectorsSummary {
            paths: selector_paths,
            modules: selector_modules,
            symbols: selector_symbols,
            facts: selector_facts,
        },
        budgets: Budgets {
            max_bytes,
            snippet_lines,
        },
        modules: included_modules,
        facts: included_facts,
        tree: included_tree,
        symbols: included_symbols,
        deps: included_deps,
        snippets: included_snippets,
    };

    apply_budget_pack_selectors(&mut out)?;
    Ok(out)
}

pub fn pack_diff_text(out: &PackDiffOutput) -> String {
    let mut lines = Vec::new();
    lines.push(format!("diff_source: {}", out.diff_source));
    lines.push(format!("radius: {}", out.radius));
    lines.push(format!("budgets.max_bytes: {}", out.budgets.max_bytes));
    lines.push(format!(
        "budgets.snippet_lines: {}",
        out.budgets.snippet_lines
    ));

    lines.push("changed_paths:".to_string());
    for c in &out.plan.changed_paths {
        lines.push(format!("- {} ({})", c.path, c.change_kind));
    }

    lines.push("required:".to_string());
    lines.push(format!(
        "- module_cards: {}",
        out.plan.required.module_cards.join(", ")
    ));
    lines.push(format!(
        "- fact_types: {}",
        out.plan.required.fact_types.join(", ")
    ));
    lines.push(format!(
        "- session_capsule: {}",
        out.plan.required.session_capsule
    ));

    lines.push("included_files:".to_string());
    for f in &out.tree {
        lines.push(format!("- {}", f.path));
    }

    lines.push(format!("symbols: {}", out.symbols.len()));
    lines.push(format!("deps: {}", out.deps.len()));

    if !out.snippets.is_empty() {
        lines.push("snippets:".to_string());
        for snip in &out.snippets {
            lines.push(format!("--- {}:{}", snip.path, snip.start_line));
            lines.push("```".to_string());
            lines.push(snip.text.clone());
            lines.push("```".to_string());
        }
    }

    lines.join("\n")
}

pub fn pack_selectors_text(out: &PackSelectorsOutput) -> String {
    let mut lines = Vec::new();
    lines.push(format!("budgets.max_bytes: {}", out.budgets.max_bytes));
    lines.push(format!(
        "budgets.snippet_lines: {}",
        out.budgets.snippet_lines
    ));

    lines.push("selectors:".to_string());
    lines.push(format!("- paths: {}", out.selectors.paths.join(", ")));
    lines.push(format!("- modules: {}", out.selectors.modules.join(", ")));
    lines.push(format!("- symbols: {}", out.selectors.symbols.join(", ")));
    lines.push(format!("- facts: {}", out.selectors.facts.join(", ")));

    lines.push("included_files:".to_string());
    for f in &out.tree {
        lines.push(format!("- {}", f.path));
    }

    lines.push(format!("symbols: {}", out.symbols.len()));
    lines.push(format!("deps: {}", out.deps.len()));

    if !out.snippets.is_empty() {
        lines.push("snippets:".to_string());
        for snip in &out.snippets {
            lines.push(format!("--- {}:{}", snip.path, snip.start_line));
            lines.push("```".to_string());
            lines.push(snip.text.clone());
            lines.push("```".to_string());
        }
    }

    lines.join("\n")
}

fn read_required_modules(
    reader: &DiffSourceReader,
    module_ids: &[String],
) -> Result<Vec<ModuleCard>, KbError> {
    let mut out = Vec::new();
    for module_id in module_ids {
        let rel = format!("kb/atlas/modules/{module_id}.toml");
        if let Some(text) = try_read_text(reader, &rel)? {
            out.push(ModuleCard {
                module_id: module_id.clone(),
                path: rel,
                text: normalize_text(&text),
            });
        }
    }
    out.sort_by(|a, b| a.module_id.cmp(&b.module_id));
    out.dedup_by(|a, b| a.module_id == b.module_id);
    Ok(out)
}

fn read_required_facts(
    reader: &DiffSourceReader,
    required_types: &[String],
) -> Result<Vec<serde_json::Value>, KbError> {
    if required_types.is_empty() {
        return Ok(Vec::new());
    }

    let types: BTreeSet<&str> = required_types.iter().map(String::as_str).collect();
    let facts = match read_jsonl::<serde_json::Value>(reader, "kb/facts/facts.jsonl") {
        Ok(v) => v,
        Err(err) if err.code == ErrorCode::NotFound => return Ok(Vec::new()),
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
        let Some(_id) = obj.get("fact_id").and_then(|v| v.as_str()) else {
            return Err(KbError::invalid_argument("fact record missing fact_id"));
        };
        if types.contains(t) {
            out.push(fact);
        }
    }

    #[allow(clippy::unnecessary_sort_by)]
    out.sort_by(|a, b| fact_id_str(a).cmp(fact_id_str(b)));
    out.dedup_by(|a, b| fact_id_str(a) == fact_id_str(b));
    Ok(out)
}

fn seed_paths_from_plan(plan: &PlanDiffOutput, file_set: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for c in &plan.changed_paths {
        if c.change_kind == ChangeKind::Delete.as_str() {
            continue;
        }
        if file_set.contains(&c.path) {
            out.push(c.path.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn expand_paths(
    seeds: &[String],
    deps: &[DepEdge],
    file_set: &BTreeSet<String>,
    radius: u32,
) -> BTreeSet<String> {
    if radius == 0 {
        return seeds.iter().cloned().collect();
    }

    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in deps {
        let Some(to) = edge.to_path.as_deref() else {
            continue;
        };
        if !file_set.contains(&edge.from_path) || !file_set.contains(to) {
            continue;
        }
        adjacency
            .entry(edge.from_path.clone())
            .or_default()
            .insert(to.to_string());
        adjacency
            .entry(to.to_string())
            .or_default()
            .insert(edge.from_path.clone());
    }

    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    for seed in seeds {
        if visited.insert(seed.clone()) {
            queue.push_back((seed.clone(), 0));
        }
    }

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= radius {
            continue;
        }
        let Some(neighbors) = adjacency.get(&node) else {
            continue;
        };
        for n in neighbors.iter() {
            if visited.insert(n.clone()) {
                queue.push_back((n.clone(), depth + 1));
            }
        }
    }

    visited
}

fn build_snippets(
    repo_root: &Path,
    diff_source: &DiffSource,
    snippet_lines: u64,
    included_paths: &BTreeSet<String>,
    symbols: &[SymbolRecord],
) -> Result<Vec<Snippet>, KbError> {
    let reader = reader_for(repo_root, diff_source);

    let mut by_path: BTreeMap<&str, Vec<&SymbolRecord>> = BTreeMap::new();
    for sym in symbols {
        by_path.entry(sym.path.as_str()).or_default().push(sym);
    }

    let mut out = Vec::new();
    for path in included_paths.iter() {
        let Some(mut syms) = by_path.get_mut(path.as_str()).map(|v| v.clone()) else {
            continue;
        };
        syms.sort_by(|a, b| match a.line.cmp(&b.line) {
            std::cmp::Ordering::Equal => a.symbol_id.cmp(&b.symbol_id),
            other => other,
        });
        let Some(sym) = syms.first() else {
            continue;
        };

        if let Some(snippet) = build_symbol_snippet_with_reader(&reader, snippet_lines, sym)? {
            out.push(snippet);
        }
    }

    Ok(out)
}

fn build_symbol_snippet(
    repo_root: &Path,
    diff_source: &DiffSource,
    snippet_lines: u64,
    sym: &SymbolRecord,
) -> Result<Option<Snippet>, KbError> {
    let reader = reader_for(repo_root, diff_source);
    build_symbol_snippet_with_reader(&reader, snippet_lines, sym)
}

const SNIPPET_HARD_LINE_CHARS: usize = 2000;
const SNIPPET_HEAD_CHARS: usize = 160;
const SNIPPET_TAIL_CHARS: usize = 80;

fn gate_snippet_line(line: &str) -> String {
    let trimmed = line.trim_end_matches([' ', '\t']);
    let len_chars = trimmed.chars().count();
    if len_chars <= SNIPPET_HARD_LINE_CHARS {
        return trimmed.to_string();
    }

    let head: String = trimmed.chars().take(SNIPPET_HEAD_CHARS).collect();
    let tail_rev: String = trimmed.chars().rev().take(SNIPPET_TAIL_CHARS).collect();
    let tail: String = tail_rev.chars().rev().collect();

    format!(
        "[kb truncated chars={} head='{}' tail='{}']",
        len_chars,
        escape_snippet_marker_field(&head),
        escape_snippet_marker_field(&tail)
    )
}

fn escape_snippet_marker_field(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

fn build_symbol_snippet_with_reader(
    reader: &DiffSourceReader,
    snippet_lines: u64,
    sym: &SymbolRecord,
) -> Result<Option<Snippet>, KbError> {
    if snippet_lines == 0 {
        return Ok(None);
    }
    let repo_path = RepoPath::parse(&sym.path)?;
    let text = match reader.read_to_string(&repo_path) {
        Ok(s) => s,
        Err(err) if err.code == ErrorCode::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }

    let total = lines.len() as u64;
    if total == 0 {
        return Ok(None);
    }

    let start = sym.line.max(1).min(total);
    let max_end = start
        .saturating_add(snippet_lines)
        .saturating_sub(1)
        .min(total);
    let end = sym.end_line.unwrap_or(max_end).min(max_end).min(total);
    if end < start {
        return Ok(None);
    }

    let selected = &lines[(start - 1) as usize..end as usize];
    let cleaned: Vec<String> = selected.iter().map(|l| gate_snippet_line(l)).collect();

    Ok(Some(Snippet {
        path: sym.path.clone(),
        symbol_id: sym.symbol_id.clone(),
        start_line: start,
        end_line: end,
        text: cleaned.join("\n"),
    }))
}

fn dep_edge_cmp(a: &DepEdge, b: &DepEdge) -> std::cmp::Ordering {
    match a.from_path.cmp(&b.from_path) {
        std::cmp::Ordering::Equal => match a.kind.cmp(&b.kind) {
            std::cmp::Ordering::Equal => match dep_edge_to_key(a).cmp(dep_edge_to_key(b)) {
                std::cmp::Ordering::Equal => {
                    match dep_edge_to_is_path(a).cmp(&dep_edge_to_is_path(b)) {
                        std::cmp::Ordering::Equal => a.raw.cmp(&b.raw),
                        other => other,
                    }
                }
                other => other,
            },
            other => other,
        },
        other => other,
    }
}

fn dep_edge_to_key(edge: &DepEdge) -> &str {
    edge.to_path
        .as_deref()
        .or(edge.to_external.as_deref())
        .unwrap_or("")
}

fn dep_edge_to_is_path(edge: &DepEdge) -> bool {
    edge.to_path.is_some()
}

fn fact_id_str(v: &serde_json::Value) -> &str {
    v.as_object()
        .and_then(|o| o.get("fact_id"))
        .and_then(|id| id.as_str())
        .unwrap_or("")
}

fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn resolve_selector_path(
    tree_by_path: &BTreeMap<String, TreeRecord>,
    input: &str,
) -> Result<(String, bool), KbError> {
    let (normalized, is_dir_hint) = normalize_selector_path(input)?;
    if is_dir_hint {
        return Ok((normalized, true));
    }
    let dir_key = format!("{normalized}/");
    if let Some(r) = tree_by_path.get(&dir_key) {
        if r.kind == "dir" {
            return Ok((normalized, true));
        }
    }
    Ok((normalized, false))
}

const DIR_EXPAND_DEPTH: usize = 2;
const DIR_EXPAND_MAX_DIRS: usize = 200;
const DIR_EXPAND_MAX_FILES: usize = 200;
const DIR_EXPAND_MAX_SYMBOL_FILES: usize = 40;

fn expand_dir_tree(
    tree_by_path: &BTreeMap<String, TreeRecord>,
    prefix: &str,
) -> (Vec<TreeRecord>, Vec<String>) {
    let base_segments = count_segments(prefix);

    let mut tree = Vec::new();
    let mut symbol_files = Vec::new();
    let mut dir_count = 0usize;
    let mut file_count = 0usize;

    for (p, r) in tree_by_path {
        if !p.starts_with(prefix) {
            continue;
        }
        let segs = count_segments(p);
        if segs > base_segments + DIR_EXPAND_DEPTH {
            continue;
        }
        if r.kind == "dir" {
            if dir_count >= DIR_EXPAND_MAX_DIRS {
                continue;
            }
            dir_count += 1;
            tree.push(r.clone());
            continue;
        }
        if r.kind == "file" {
            if file_count >= DIR_EXPAND_MAX_FILES {
                continue;
            }
            file_count += 1;
            tree.push(r.clone());
            if symbol_files.len() < DIR_EXPAND_MAX_SYMBOL_FILES {
                symbol_files.push(p.clone());
            }
        }
    }

    (tree, symbol_files)
}

fn count_segments(path: &str) -> usize {
    let p = path.trim_end_matches('/');
    if p.is_empty() {
        return 0;
    }
    p.split('/').count()
}

fn apply_top_symbols(tree: &mut [TreeRecord], symbols: &[SymbolRecord]) {
    let mut by_path: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for s in symbols {
        by_path
            .entry(s.path.as_str())
            .or_default()
            .push(s.symbol_id.as_str());
    }
    for ids in by_path.values_mut() {
        ids.sort();
        ids.dedup();
    }

    for r in tree {
        if r.kind != "file" {
            continue;
        }
        if let Some(ids) = by_path.get(r.path.as_str()) {
            r.top_symbols = Some(
                ids.iter()
                    .take(TOP_SYMBOLS_PER_FILE)
                    .map(|s| (*s).to_string())
                    .collect(),
            );
        } else {
            r.top_symbols = Some(Vec::new());
        }
    }
}

fn normalize_selector_path(input: &str) -> Result<(String, bool), KbError> {
    let is_dir = input.trim_end().ends_with('/');
    let trimmed = input.trim().trim_end_matches('/');
    let normalized = RepoPath::parse(trimmed)?.as_str().to_string();
    Ok((normalized, is_dir))
}

fn normalize_unique<'a>(inputs: impl Iterator<Item = &'a str>) -> Result<Vec<String>, KbError> {
    let mut out: Vec<String> = inputs
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

fn apply_budget_pack_diff(out: &mut PackDiffOutput) -> Result<(), KbError> {
    loop {
        let bytes = serde_json::to_vec(out)
            .map_err(|err| KbError::internal(err, "failed to serialize pack diff output"))?;
        if (bytes.len() as u64) <= out.budgets.max_bytes {
            return Ok(());
        }

        if !out.snippets.is_empty() {
            out.snippets.pop();
            continue;
        }
        if !out.deps.is_empty() {
            out.deps.pop();
            continue;
        }
        if !out.symbols.is_empty() {
            out.symbols.pop();
            continue;
        }
        if !out.tree.is_empty() {
            out.tree.pop();
            continue;
        }
        if !out.facts.is_empty() {
            out.facts.pop();
            continue;
        }
        if !out.modules.is_empty() {
            out.modules.pop();
            continue;
        }

        return Err(
            KbError::invalid_argument("max_bytes is too small to fit required metadata")
                .with_detail("max_bytes", out.budgets.max_bytes.to_string()),
        );
    }
}

fn apply_budget_pack_selectors(out: &mut PackSelectorsOutput) -> Result<(), KbError> {
    loop {
        let bytes = serde_json::to_vec(out)
            .map_err(|err| KbError::internal(err, "failed to serialize pack selectors output"))?;
        if (bytes.len() as u64) <= out.budgets.max_bytes {
            return Ok(());
        }

        if !out.snippets.is_empty() {
            out.snippets.pop();
            continue;
        }
        if !out.deps.is_empty() {
            out.deps.pop();
            continue;
        }
        if !out.symbols.is_empty() {
            out.symbols.pop();
            continue;
        }
        if !out.tree.is_empty() {
            out.tree.pop();
            continue;
        }
        if !out.facts.is_empty() {
            out.facts.pop();
            continue;
        }
        if !out.modules.is_empty() {
            out.modules.pop();
            continue;
        }

        return Err(
            KbError::invalid_argument("max_bytes is too small to fit required metadata")
                .with_detail("max_bytes", out.budgets.max_bytes.to_string()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_snippet_line_trims_whitespace() {
        assert_eq!(gate_snippet_line("hello\t \t"), "hello");
    }

    #[test]
    fn gate_snippet_line_keeps_reasonable_lines() {
        let line = "a".repeat(SNIPPET_HARD_LINE_CHARS);
        assert_eq!(gate_snippet_line(&line), line);
    }

    #[test]
    fn gate_snippet_line_truncates_pathological_lines() {
        let line = "a".repeat(SNIPPET_HARD_LINE_CHARS + 10);
        let out = gate_snippet_line(&line);
        assert!(out.starts_with("[kb truncated "));
        assert!(out.contains("head='"));
        assert!(out.contains("tail='"));
        assert!(out.len() < SNIPPET_HARD_LINE_CHARS);
    }
}
