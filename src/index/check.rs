use std::collections::BTreeSet;
use std::path::Path;

use crate::error::KbError;
use crate::index::deps::write_deps_jsonl;
use crate::index::git_inputs::list_tracked_file_paths;
use crate::index::meta::write_kb_meta;
use crate::index::symbols::write_symbols_jsonl;
use crate::index::tree::write_tree_jsonl;
use crate::index::IndexScope;
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::reader::DiffSourceReader;

const GEN_FILES: [&str; 4] = ["kb_meta.json", "tree.jsonl", "symbols.jsonl", "deps.jsonl"];

pub fn index_regen_at(
    repo_root: &Path,
    diff_source: &DiffSource,
    _scope: IndexScope,
) -> Result<(), KbError> {
    let tracked_files = list_tracked_file_paths(repo_root, diff_source)?;
    let tracked_set: BTreeSet<String> = tracked_files.iter().cloned().collect();

    let gen_dir = repo_root.join("kb/gen");
    write_kb_meta(&gen_dir)?;
    write_tree_jsonl(repo_root, diff_source, &tracked_files, &gen_dir)?;
    write_symbols_jsonl(repo_root, diff_source, &tracked_files, &gen_dir)?;
    write_deps_jsonl(
        repo_root,
        diff_source,
        &tracked_files,
        &tracked_set,
        &gen_dir,
    )?;

    Ok(())
}

pub fn index_check_at(repo_root: &Path, diff_source: &DiffSource) -> Result<(), KbError> {
    let tmp_root = repo_root.join("kb/.tmp/regen");
    let tmp_gen = tmp_root.join("gen");

    if tmp_root.exists() {
        std::fs::remove_dir_all(&tmp_root)
            .map_err(|err| KbError::internal(err, "failed to clear kb/.tmp/regen"))?;
    }
    std::fs::create_dir_all(&tmp_gen)
        .map_err(|err| KbError::internal(err, "failed to create kb/.tmp/regen"))?;

    let regen_result = index_regen_into(repo_root, diff_source, &tmp_gen);
    let compare_result = match regen_result {
        Ok(()) => compare_gen(repo_root, diff_source, &tmp_gen),
        Err(err) => Err(err),
    };

    let _ = std::fs::remove_dir_all(&tmp_root);
    compare_result
}

fn index_regen_into(
    repo_root: &Path,
    diff_source: &DiffSource,
    gen_dir: &Path,
) -> Result<(), KbError> {
    let tracked_files = list_tracked_file_paths(repo_root, diff_source)?;
    let tracked_set: BTreeSet<String> = tracked_files.iter().cloned().collect();

    write_kb_meta(gen_dir)?;
    write_tree_jsonl(repo_root, diff_source, &tracked_files, gen_dir)?;
    write_symbols_jsonl(repo_root, diff_source, &tracked_files, gen_dir)?;
    write_deps_jsonl(
        repo_root,
        diff_source,
        &tracked_files,
        &tracked_set,
        gen_dir,
    )?;

    Ok(())
}

fn compare_gen(
    repo_root: &Path,
    diff_source: &DiffSource,
    tmp_gen_dir: &Path,
) -> Result<(), KbError> {
    let reader = DiffSourceReader::new_at_root(repo_root.to_path_buf(), diff_source.clone());

    let mut diffs = Vec::new();
    for file in GEN_FILES {
        let tmp_bytes = std::fs::read(tmp_gen_dir.join(file))
            .map_err(|err| KbError::internal(err, "failed to read regenerated artifacts"))?;

        let rel = format!("kb/gen/{file}");
        let rel_path = RepoPath::parse(&rel)?;
        let expected = match diff_source {
            DiffSource::Worktree => std::fs::read(repo_root.join(&rel)).ok(),
            _ => reader.read_bytes(&rel_path).ok(),
        };

        let Some(expected_bytes) = expected else {
            diffs.push(rel);
            continue;
        };

        if tmp_bytes != expected_bytes {
            diffs.push(rel);
        }
    }

    if diffs.is_empty() {
        return Ok(());
    }

    let mut err = KbError::invalid_argument("kb/gen artifacts are stale");
    for path in diffs {
        err = err.with_detail("diff", path);
    }
    Err(err)
}
