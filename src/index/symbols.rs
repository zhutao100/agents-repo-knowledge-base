use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use crate::error::KbError;
use crate::index::artifacts::SymbolRecord;
use crate::io::jsonl::write_jsonl_file;
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::reader::DiffSourceReader;

#[derive(Debug, serde::Deserialize)]
struct CtagsRecord {
    #[serde(rename = "_type")]
    ty: String,
    name: Option<String>,
    path: Option<String>,
    pattern: Option<String>,
    language: Option<String>,
    line: Option<u64>,
    end: Option<u64>,
    kind: Option<String>,
    signature: Option<String>,
    scope: Option<String>,
}

pub fn write_symbols_jsonl(
    repo_root: &Path,
    diff_source: &DiffSource,
    tracked_files: &[String],
    gen_dir: &Path,
) -> Result<(), KbError> {
    let input_root = materialize_input_root(repo_root, diff_source, tracked_files)?;
    let records = run_ctags(&input_root, tracked_files)?;

    let mut out = Vec::new();
    for record in records {
        if record.ty != "tag" {
            continue;
        }

        let name = record
            .name
            .ok_or_else(|| KbError::backend_failed("ctags record missing name"))?;
        let path = record
            .path
            .ok_or_else(|| KbError::backend_failed("ctags record missing path"))?;
        let kind = record
            .kind
            .ok_or_else(|| KbError::backend_failed("ctags record missing kind"))?;
        let line = record
            .line
            .ok_or_else(|| KbError::backend_failed("ctags record missing line"))?;

        let lang = record
            .language
            .unwrap_or_else(|| "unknown".to_string())
            .to_lowercase();
        let repo_path = RepoPath::parse(&path)?;
        let path = repo_path.as_str().to_string();

        let qualified_name = if let Some(scope) = record.scope.as_deref().filter(|s| !s.is_empty())
        {
            format!("{scope}::{name}")
        } else {
            name.clone()
        };

        let disambiguator =
            if let Some(pattern) = record.pattern.as_deref().filter(|p| !p.is_empty()) {
                format!("pat_sha256={}", sha256_hex(pattern.as_bytes()))
            } else if let Some(signature) = record.signature.as_deref().filter(|s| !s.is_empty()) {
                format!("sig_sha256={}", sha256_hex(signature.as_bytes()))
            } else {
                format!("line={line}")
            };

        let symbol_id = format!(
            "sym:v1:{}",
            sha256_hex(
                canonical_symbol_key(&lang, &path, &kind, &qualified_name, &disambiguator)
                    .as_bytes()
            )
        );

        out.push(SymbolRecord {
            symbol_id,
            lang,
            path,
            kind,
            name,
            qualified_name,
            line,
            end_line: record.end,
            signature: record.signature,
            scope: record.scope,
        });
    }

    out.sort_by(|a, b| a.symbol_id.cmp(&b.symbol_id));
    out.dedup_by(|a, b| a.symbol_id == b.symbol_id);

    std::fs::create_dir_all(gen_dir)
        .map_err(|err| KbError::internal(err, "failed to create kb/gen"))?;
    write_jsonl_file(&gen_dir.join("symbols.jsonl"), &out)
        .map_err(|err| KbError::internal(err, "failed to write symbols.jsonl"))?;

    if input_root != repo_root {
        let _ = std::fs::remove_dir_all(input_root);
    }

    Ok(())
}

fn canonical_symbol_key(
    lang: &str,
    path: &str,
    kind: &str,
    qualified_name: &str,
    disambiguator: &str,
) -> String {
    format!(
        "lang={lang}\npath={path}\nkind={kind}\nqualified_name={qualified_name}\ndisambiguator={disambiguator}\n"
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn materialize_input_root(
    repo_root: &Path,
    diff_source: &DiffSource,
    tracked_files: &[String],
) -> Result<PathBuf, KbError> {
    if matches!(diff_source, DiffSource::Worktree) {
        return Ok(repo_root.to_path_buf());
    }

    let input_root = repo_root.join("kb/.tmp/ctags_input");
    if input_root.exists() {
        std::fs::remove_dir_all(&input_root)
            .map_err(|err| KbError::internal(err, "failed to clear kb/.tmp/ctags_input"))?;
    }
    std::fs::create_dir_all(&input_root)
        .map_err(|err| KbError::internal(err, "failed to create kb/.tmp/ctags_input"))?;

    let reader = DiffSourceReader::new_at_root(repo_root.to_path_buf(), diff_source.clone());
    for file_path in tracked_files {
        if file_path.ends_with('/') {
            continue;
        }
        let repo_path = RepoPath::parse(file_path)?;
        let bytes = reader.read_bytes(&repo_path)?;

        let out_path = input_root.join(file_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| KbError::internal(err, "failed to create ctags input dir"))?;
        }
        let mut file = File::create(&out_path)
            .map_err(|err| KbError::internal(err, "failed to write ctags input"))?;
        file.write_all(&bytes)
            .map_err(|err| KbError::internal(err, "failed to write ctags input"))?;
    }

    Ok(input_root)
}

fn run_ctags(repo_root: &Path, tracked_files: &[String]) -> Result<Vec<CtagsRecord>, KbError> {
    let mut cmd = Command::new("ctags");
    cmd.current_dir(repo_root)
        .env("LC_ALL", "C")
        .arg("--output-format=json")
        .arg("--sort=no")
        .arg("--quiet=yes")
        .arg("-f")
        .arg("-")
        .arg("-L")
        .arg("-")
        .arg("--fields=+n+e+S+l+z")
        .arg("--fields=-T")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|err| {
        KbError::backend_missing("ctags is required").with_detail("cause", err.to_string())
    })?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| KbError::internal("missing", "failed to open ctags stdin"))?;
        for path in tracked_files {
            stdin
                .write_all(path.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
                .map_err(|err| KbError::internal(err, "failed to write ctags stdin"))?;
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|err| KbError::internal(err, "failed to read ctags output"))?;

    if !output.status.success() {
        return Err(KbError::backend_failed("ctags failed").with_detail(
            "stderr",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let mut out = Vec::new();
    let reader = BufReader::new(&output.stdout[..]);
    for line in reader.lines() {
        let line =
            line.map_err(|err| KbError::internal(err, "failed to read ctags output line"))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: CtagsRecord = serde_json::from_str(&line).map_err(|err| {
            KbError::backend_failed("failed to parse ctags json")
                .with_detail("cause", err.to_string())
        })?;
        out.push(record);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_id_is_stable() {
        let key = canonical_symbol_key(
            "rust",
            "src/lib.rs",
            "function",
            "crate::parse_thing",
            "pat_sha256=abc",
        );
        assert_eq!(sha256_hex(key.as_bytes()), sha256_hex(key.as_bytes()));
    }
}
