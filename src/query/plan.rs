use std::collections::BTreeSet;
use std::path::Path;

use clap::ValueEnum;

use crate::config::obligations::ObligationsConfig;
use crate::error::KbError;
use crate::query::read::{read_text, reader_for};
use crate::repo::diff::{list_changed_paths, DiffPathChange};
use crate::repo::diff_source::DiffSource;
use crate::repo::prefix::normalize_path_prefix;
use crate::repo::root::discover_repo_root;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Policy {
    Default,
    Strict,
}

impl Policy {
    pub fn as_str(self) -> &'static str {
        match self {
            Policy::Default => "default",
            Policy::Strict => "strict",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlanDiffOutput {
    pub diff_source: String,
    pub policy: String,
    pub changed_paths: Vec<ChangedPath>,
    pub affected_modules: Vec<String>,
    pub triggered_rules: Vec<TriggeredRule>,
    pub required: Required,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
pub struct ChangedPath {
    pub path: String,
    pub change_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
pub struct TriggeredRule {
    pub id: String,
    pub when_path_prefix: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Required {
    pub module_cards: Vec<String>,
    pub fact_types: Vec<String>,
    pub session_capsule: bool,
}

pub fn plan_diff(diff_source: &DiffSource, policy: Policy) -> Result<PlanDiffOutput, KbError> {
    plan_diff_at(&discover_repo_root()?, diff_source, policy)
}

pub fn plan_diff_at(
    repo_root: &Path,
    diff_source: &DiffSource,
    policy: Policy,
) -> Result<PlanDiffOutput, KbError> {
    let reader = reader_for(repo_root, diff_source);
    let obligations_toml = read_text(&reader, "kb/config/obligations.toml")?;
    let config: ObligationsConfig = toml::from_str(&obligations_toml).map_err(|err| {
        KbError::invalid_argument("failed to parse obligations.toml")
            .with_detail("cause", err.to_string())
    })?;

    let changes = list_changed_paths(repo_root, diff_source)?;
    let changed_paths = changes
        .iter()
        .map(|c| ChangedPath {
            path: c.path.clone(),
            change_kind: c.change_kind.as_str().to_string(),
        })
        .collect::<Vec<_>>();

    let (triggered_rules, required) = evaluate_obligations(&config, &changes)?;

    let mut affected_modules = required.module_cards.clone();
    affected_modules.sort();
    affected_modules.dedup();

    Ok(PlanDiffOutput {
        diff_source: diff_source.as_display(),
        policy: policy.as_str().to_string(),
        changed_paths,
        affected_modules,
        triggered_rules,
        required,
    })
}

fn evaluate_obligations(
    config: &ObligationsConfig,
    changes: &[DiffPathChange],
) -> Result<(Vec<TriggeredRule>, Required), KbError> {
    let changed_paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();

    let mut triggered = Vec::new();
    let mut required_module_cards = BTreeSet::new();
    let mut required_fact_types = BTreeSet::new();
    let mut require_session = false;

    for rule in &config.rule {
        let prefix = normalize_path_prefix(&rule.when_path_prefix)?;
        if !changed_paths.iter().any(|p| p.starts_with(prefix.as_str())) {
            continue;
        }

        triggered.push(TriggeredRule {
            id: rule.id.clone(),
            when_path_prefix: prefix.clone(),
        });

        if let Some(card) = rule.require_module_card.as_deref() {
            required_module_cards.insert(card.to_string());
        }

        if let Some(types) = rule.require_fact_types.as_deref() {
            for t in types {
                required_fact_types.insert(t.to_string());
            }
        }

        if rule.require_session_capsule.unwrap_or(false) {
            require_session = true;
        }
    }

    triggered.sort_by(|a, b| a.id.cmp(&b.id));
    triggered.dedup();

    Ok((
        triggered,
        Required {
            module_cards: required_module_cards.into_iter().collect(),
            fact_types: required_fact_types.into_iter().collect(),
            session_capsule: require_session,
        },
    ))
}

pub fn plan_diff_text(out: &PlanDiffOutput) -> String {
    let mut lines = Vec::new();
    lines.push(format!("diff_source: {}", out.diff_source));
    lines.push(format!("policy: {}", out.policy));
    lines.push("changed_paths:".to_string());
    for c in &out.changed_paths {
        lines.push(format!("- {} ({})", c.path, c.change_kind));
    }
    lines.push("triggered_rules:".to_string());
    for r in &out.triggered_rules {
        lines.push(format!("- {} (prefix={})", r.id, r.when_path_prefix));
    }
    lines.push("required:".to_string());
    lines.push(format!(
        "- module_cards: {}",
        out.required.module_cards.join(", ")
    ));
    lines.push(format!(
        "- fact_types: {}",
        out.required.fact_types.join(", ")
    ));
    lines.push(format!(
        "- session_capsule: {}",
        out.required.session_capsule
    ));
    lines.join("\n")
}
