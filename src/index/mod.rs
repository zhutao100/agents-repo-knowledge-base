mod artifacts;
mod check;
mod deps;
mod git_inputs;
mod meta;
mod symbols;
mod tree;

use std::path::Path;

use clap::ValueEnum;

use crate::error::KbError;
use crate::repo::diff_source::DiffSource;
use crate::repo::root::discover_repo_root;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum IndexScope {
    All,
    Changed,
}

pub fn index_regen(diff_source: &DiffSource, scope: IndexScope) -> Result<(), KbError> {
    index_regen_at(&discover_repo_root()?, diff_source, scope)
}

pub fn index_check(diff_source: &DiffSource) -> Result<(), KbError> {
    index_check_at(&discover_repo_root()?, diff_source)
}

pub fn index_regen_at(
    repo_root: &Path,
    diff_source: &DiffSource,
    scope: IndexScope,
) -> Result<(), KbError> {
    check::index_regen_at(repo_root, diff_source, scope)
}

pub fn index_check_at(repo_root: &Path, diff_source: &DiffSource) -> Result<(), KbError> {
    check::index_check_at(repo_root, diff_source)
}
