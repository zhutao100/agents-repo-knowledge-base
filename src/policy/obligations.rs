use std::collections::BTreeSet;
use std::path::Path;

use crate::error::{ErrorCode, KbError};
use crate::query::plan::{plan_diff_at, Policy};
use crate::repo::diff::{list_changed_paths, ChangeKind};
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::reader::DiffSourceReader;
use crate::repo::root::discover_repo_root;

pub fn obligations_check(diff_source: &DiffSource) -> Result<(), KbError> {
    obligations_check_at(&discover_repo_root()?, diff_source)
}

pub fn obligations_check_at(repo_root: &Path, diff_source: &DiffSource) -> Result<(), KbError> {
    let plan = plan_diff_at(repo_root, diff_source, Policy::Default)?;
    let changes = list_changed_paths(repo_root, diff_source)?;
    let reader = DiffSourceReader::new_at_root(repo_root.to_path_buf(), diff_source.clone());

    let mut missing_module_cards = BTreeSet::new();
    let mut missing_module_card_updates = BTreeSet::new();

    for module_id in &plan.required.module_cards {
        let rel = format!("kb/atlas/modules/{module_id}.toml");

        let exists = match reader.read_bytes(&RepoPath::parse(&rel)?) {
            Ok(_) => true,
            Err(err) if err.code == ErrorCode::NotFound => false,
            Err(err) => return Err(err),
        };
        if !exists {
            missing_module_cards.insert(module_id.clone());
        }

        if !path_updated_in_diff(&changes, &rel) {
            missing_module_card_updates.insert(module_id.clone());
        }
    }

    let mut missing_fact_types = BTreeSet::new();
    let mut missing_fact_updates = BTreeSet::new();
    if !plan.required.fact_types.is_empty() {
        let facts_rel = "kb/facts/facts.jsonl";

        if !path_updated_in_diff(&changes, facts_rel) {
            for t in &plan.required.fact_types {
                missing_fact_updates.insert(t.clone());
            }
        }

        let facts_text = match reader.read_to_string(&RepoPath::parse(facts_rel)?) {
            Ok(s) => Some(s),
            Err(err) if err.code == ErrorCode::NotFound => None,
            Err(err) => return Err(err),
        };

        match facts_text.as_deref() {
            Some(text) => {
                let present_types = parse_fact_types(text)?;
                for t in &plan.required.fact_types {
                    if !present_types.contains(t.as_str()) {
                        missing_fact_types.insert(t.clone());
                    }
                }
            }
            None => {
                for t in &plan.required.fact_types {
                    missing_fact_types.insert(t.clone());
                    missing_fact_updates.insert(t.clone());
                }
            }
        }
    }

    let mut missing_session_capsule = false;
    if plan.required.session_capsule {
        let ok = changes.iter().any(|c| {
            (c.change_kind == ChangeKind::Add
                || c.change_kind == ChangeKind::Modify
                || c.change_kind == ChangeKind::Rename)
                && c.path.starts_with("kb/sessions/")
        });
        if !ok {
            missing_session_capsule = true;
        }
    }

    if missing_module_cards.is_empty()
        && missing_module_card_updates.is_empty()
        && missing_fact_types.is_empty()
        && missing_fact_updates.is_empty()
        && !missing_session_capsule
    {
        return Ok(());
    }

    let mut err = KbError::invalid_argument("obligations unmet")
        .with_detail("diff_source", diff_source.as_display());

    if !missing_module_cards.is_empty() {
        err = err.with_detail(
            "missing_module_cards",
            serde_json::to_string(&missing_module_cards.into_iter().collect::<Vec<_>>())
                .unwrap_or_else(|_| "[]".to_string()),
        );
    }
    if !missing_module_card_updates.is_empty() {
        err = err.with_detail(
            "missing_module_card_updates",
            serde_json::to_string(&missing_module_card_updates.into_iter().collect::<Vec<_>>())
                .unwrap_or_else(|_| "[]".to_string()),
        );
    }
    if !missing_fact_types.is_empty() {
        err = err.with_detail(
            "missing_fact_types",
            serde_json::to_string(&missing_fact_types.into_iter().collect::<Vec<_>>())
                .unwrap_or_else(|_| "[]".to_string()),
        );
    }
    if !missing_fact_updates.is_empty() {
        err = err.with_detail(
            "missing_fact_updates",
            serde_json::to_string(&missing_fact_updates.into_iter().collect::<Vec<_>>())
                .unwrap_or_else(|_| "[]".to_string()),
        );
    }
    if missing_session_capsule {
        err = err.with_detail("missing_session_capsule", "true");
    }

    Err(err)
}

fn path_updated_in_diff(changes: &[crate::repo::diff::DiffPathChange], rel_path: &str) -> bool {
    changes.iter().any(|c| {
        c.path == rel_path
            && (c.change_kind == ChangeKind::Add
                || c.change_kind == ChangeKind::Modify
                || c.change_kind == ChangeKind::Rename)
    })
}

fn parse_fact_types(text: &str) -> Result<BTreeSet<String>, KbError> {
    let mut types = BTreeSet::new();
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
        let Some(t) = obj.get("type").and_then(|v| v.as_str()) else {
            return Err(KbError::invalid_argument("fact record missing type"));
        };
        let Some(_id) = obj.get("fact_id").and_then(|v| v.as_str()) else {
            return Err(KbError::invalid_argument("fact record missing fact_id"));
        };
        types.insert(t.to_string());
    }
    Ok(types)
}
