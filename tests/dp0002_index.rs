use std::path::Path;

use kb::index::{index_check_at, index_regen_at, IndexScope};
use kb::repo::diff_source::DiffSource;

fn run(cmd: &mut std::process::Command) {
    let status = cmd.status().expect("spawn");
    assert!(status.success(), "command failed: {cmd:?}");
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(path, content).expect("write file");
}

fn temp_repo_dir() -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    dir.push(format!("kb-tool-test-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp repo dir");
    dir
}

#[test]
fn dp0002_index_regen_is_deterministic_and_check_works() {
    let repo_root = temp_repo_dir();
    run(std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(&repo_root));

    write_file(repo_root.join("src/lib.rs").as_path(), "pub fn foo() {}\n");
    run(std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&repo_root));

    let diff_source = DiffSource::Worktree;
    index_regen_at(&repo_root, &diff_source, IndexScope::All).expect("regen");

    let kb_meta_1 = std::fs::read(repo_root.join("kb/gen/kb_meta.json")).expect("kb_meta");
    let tree_1 = std::fs::read(repo_root.join("kb/gen/tree.jsonl")).expect("tree");
    let symbols_1 = std::fs::read(repo_root.join("kb/gen/symbols.jsonl")).expect("symbols");
    let deps_1 = std::fs::read(repo_root.join("kb/gen/deps.jsonl")).expect("deps");

    index_regen_at(&repo_root, &diff_source, IndexScope::All).expect("regen again");

    assert_eq!(
        kb_meta_1,
        std::fs::read(repo_root.join("kb/gen/kb_meta.json")).unwrap()
    );
    assert_eq!(
        tree_1,
        std::fs::read(repo_root.join("kb/gen/tree.jsonl")).unwrap()
    );
    assert_eq!(
        symbols_1,
        std::fs::read(repo_root.join("kb/gen/symbols.jsonl")).unwrap()
    );
    assert_eq!(
        deps_1,
        std::fs::read(repo_root.join("kb/gen/deps.jsonl")).unwrap()
    );

    // Basic schema sanity: directory paths must end with `/`.
    for line in String::from_utf8(tree_1).unwrap().lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if v.get("kind").and_then(|k| k.as_str()) == Some("dir") {
            let path = v.get("path").and_then(|p| p.as_str()).unwrap();
            assert!(path.ends_with('/'), "dir path must end with '/': {path}");
        }
    }

    // No timestamp-like fields should be present in symbols output.
    let symbols_text = String::from_utf8(symbols_1).unwrap();
    assert!(!symbols_text.contains("timestamp"));
    assert!(!symbols_text.contains("epoch"));

    index_check_at(&repo_root, &diff_source).expect("check passes after regen");

    // Mutate an artifact and ensure check fails.
    let tree_path = repo_root.join("kb/gen/tree.jsonl");
    let mut tree = std::fs::read_to_string(&tree_path).unwrap();
    tree.push_str(
        "{\"path\":\"x\",\"kind\":\"file\",\"bytes\":1,\"lines\":1,\"lang\":\"unknown\"}\n",
    );
    std::fs::write(&tree_path, tree).unwrap();
    assert!(index_check_at(&repo_root, &diff_source).is_err());

    // Regen fixes it.
    index_regen_at(&repo_root, &diff_source, IndexScope::All).expect("regen fixes");
    index_check_at(&repo_root, &diff_source).expect("check passes again");

    let _ = std::fs::remove_dir_all(repo_root);
}
