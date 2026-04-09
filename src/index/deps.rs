use std::collections::BTreeSet;
use std::path::Path;

use crate::error::KbError;
use crate::index::artifacts::DepEdge;
use crate::io::jsonl::write_jsonl_file;
use crate::repo::diff_source::DiffSource;
use crate::repo::path::RepoPath;
use crate::repo::reader::DiffSourceReader;

pub fn write_deps_jsonl(
    repo_root: &Path,
    diff_source: &DiffSource,
    tracked_files: &[String],
    tracked_set: &BTreeSet<String>,
    gen_dir: &Path,
) -> Result<(), KbError> {
    let reader = DiffSourceReader::new_at_root(repo_root.to_path_buf(), diff_source.clone());

    let mut edges = Vec::new();
    for from_path in tracked_files {
        let lang = lang_for_path(from_path);
        if lang == "unknown" {
            continue;
        }

        let repo_path = RepoPath::parse(from_path)?;
        let bytes = match reader.read_bytes(&repo_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };

        match lang.as_str() {
            "rust" => edges.extend(parse_rust_deps(from_path, text)),
            "javascript" | "typescript" => {
                edges.extend(parse_js_deps(from_path, text, tracked_set));
            }
            _ => {}
        }
    }

    edges.sort_by(dep_edge_cmp);
    edges.dedup();

    std::fs::create_dir_all(gen_dir)
        .map_err(|err| KbError::internal(err, "failed to create kb/gen"))?;
    write_jsonl_file(&gen_dir.join("deps.jsonl"), &edges)
        .map_err(|err| KbError::internal(err, "failed to write deps.jsonl"))?;
    Ok(())
}

fn parse_rust_deps(from_path: &str, text: &str) -> Vec<DepEdge> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("use ") else {
            continue;
        };

        let raw = rest.trim_end_matches(';').trim();
        let Some(first) = raw.split("::").next() else {
            continue;
        };

        let first = first.trim();
        if first.is_empty() {
            continue;
        }

        out.push(DepEdge {
            from_path: from_path.to_string(),
            kind: "import".to_string(),
            to_path: None,
            to_external: Some(first.to_string()),
            raw: None,
        });
    }
    out
}

fn parse_js_deps(from_path: &str, text: &str, tracked_set: &BTreeSet<String>) -> Vec<DepEdge> {
    let mut out = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("import ") {
            if let Some(module) = extract_import_module(trimmed) {
                out.push(make_js_edge(from_path, "import", &module, tracked_set));
            }
            continue;
        }

        if let Some(module) = extract_call_module(trimmed, "require") {
            out.push(make_js_edge(from_path, "require", &module, tracked_set));
            continue;
        }

        if let Some(module) = extract_call_module(trimmed, "import") {
            out.push(make_js_edge(from_path, "dynamic", &module, tracked_set));
            continue;
        }
    }

    out
}

fn make_js_edge(
    from_path: &str,
    kind: &str,
    module: &str,
    tracked_set: &BTreeSet<String>,
) -> DepEdge {
    if module.starts_with("./") || module.starts_with("../") {
        if let Some(to_path) = resolve_js_relative(from_path, module, tracked_set) {
            return DepEdge {
                from_path: from_path.to_string(),
                kind: kind.to_string(),
                to_path: Some(to_path),
                to_external: None,
                raw: None,
            };
        }
    }

    DepEdge {
        from_path: from_path.to_string(),
        kind: kind.to_string(),
        to_path: None,
        to_external: Some(module.to_string()),
        raw: None,
    }
}

fn extract_import_module(line: &str) -> Option<String> {
    // import ... from "module";
    if let Some(idx) = line.rfind(" from ") {
        return extract_first_quoted(&line[idx..]);
    }

    // import "module";
    extract_first_quoted(line)
}

fn extract_call_module(line: &str, func: &str) -> Option<String> {
    let needle = format!("{func}(");
    let idx = line.find(&needle)?;
    let rest = &line[idx + needle.len()..];
    extract_first_quoted(rest)
}

fn extract_first_quoted(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'"' || *b == b'\'' {
            let quote = *b;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == quote {
                    return Some(String::from_utf8_lossy(&bytes[i + 1..j]).to_string());
                }
                j += 1;
            }
            return None;
        }
    }
    None
}

fn resolve_js_relative(
    from_path: &str,
    module: &str,
    tracked_set: &BTreeSet<String>,
) -> Option<String> {
    let base_dir = from_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut components: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };

    let target = module.trim_start_matches("./");
    for part in target.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            components.pop()?;
            continue;
        }
        components.push(part);
    }

    let joined = components.join("/");

    let candidates = if joined
        .rsplit_once('/')
        .map(|(_, f)| f)
        .unwrap_or(&joined)
        .contains('.')
    {
        vec![joined]
    } else {
        vec![
            format!("{joined}.ts"),
            format!("{joined}.tsx"),
            format!("{joined}.js"),
            format!("{joined}.jsx"),
            format!("{joined}/index.ts"),
            format!("{joined}/index.tsx"),
            format!("{joined}/index.js"),
            format!("{joined}/index.jsx"),
        ]
    };

    let matches: Vec<String> = candidates
        .into_iter()
        .filter(|candidate| tracked_set.contains(candidate))
        .collect();

    if matches.len() == 1 {
        return Some(matches[0].clone());
    }

    None
}

fn dep_edge_cmp(a: &DepEdge, b: &DepEdge) -> std::cmp::Ordering {
    let a_to = a
        .to_path
        .as_deref()
        .unwrap_or_else(|| a.to_external.as_deref().unwrap_or(""));
    let b_to = b
        .to_path
        .as_deref()
        .unwrap_or_else(|| b.to_external.as_deref().unwrap_or(""));

    match a.from_path.cmp(&b.from_path) {
        std::cmp::Ordering::Equal => match a.kind.cmp(&b.kind) {
            std::cmp::Ordering::Equal => match a_to.cmp(b_to) {
                std::cmp::Ordering::Equal => a.raw.cmp(&b.raw),
                other => other,
            },
            other => other,
        },
        other => other,
    }
}

fn lang_for_path(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        _ => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_import_when_unambiguous() {
        let tracked = BTreeSet::from(["src/util.ts".to_string(), "src/index.ts".to_string()]);
        assert_eq!(
            resolve_js_relative("src/index.ts", "./util", &tracked),
            Some("src/util.ts".to_string())
        );
    }

    #[test]
    fn rejects_relative_import_when_ambiguous() {
        let tracked = BTreeSet::from([
            "src/util.ts".to_string(),
            "src/util.js".to_string(),
            "src/index.ts".to_string(),
        ]);
        assert_eq!(
            resolve_js_relative("src/index.ts", "./util", &tracked),
            None
        );
    }
}
