use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::error::KbError;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct TagsConfig {
    #[serde(default)]
    pub tag: Vec<TagEntry>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagEntry {
    pub id: String,

    #[serde(default)]
    pub description: Option<String>,
}

pub fn tags_toml_path(repo_root: &Path) -> PathBuf {
    repo_root.join("kb/config/tags.toml")
}

pub fn load_tags_config_at(repo_root: &Path) -> Result<Option<TagsConfig>, KbError> {
    let path = tags_toml_path(repo_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(KbError::internal(err, "failed to read tags.toml")),
    };

    let cfg: TagsConfig = toml::from_str(&text).map_err(|err| {
        KbError::invalid_argument("failed to parse tags.toml").with_detail("cause", err.to_string())
    })?;

    Ok(Some(cfg))
}

pub fn known_tag_ids_at(repo_root: &Path) -> Result<Option<BTreeSet<String>>, KbError> {
    let Some(cfg) = load_tags_config_at(repo_root)? else {
        return Ok(None);
    };

    let mut known = BTreeSet::new();
    for entry in cfg.tag {
        let _ = entry.description;
        known.insert(entry.id);
    }
    Ok(Some(known))
}

pub fn validate_tag_at(repo_root: &Path, tag: &str) -> Result<(), KbError> {
    let Some(known) = known_tag_ids_at(repo_root)? else {
        return Ok(());
    };
    if known.contains(tag) {
        return Ok(());
    }
    Err(KbError::invalid_argument("unknown tag").with_detail("tag", tag))
}

pub fn validate_tags_at(repo_root: &Path, tags: &[String]) -> Result<(), KbError> {
    let Some(known) = known_tag_ids_at(repo_root)? else {
        return Ok(());
    };
    for tag in tags {
        if known.contains(tag) {
            continue;
        }
        return Err(KbError::invalid_argument("unknown tag").with_detail("tag", tag));
    }
    Ok(())
}
