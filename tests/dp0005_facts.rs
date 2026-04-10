use std::path::Path;

use kb::query::describe::describe_fact_at;
use kb::query::list::list_facts_at;

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
    dir.push(format!("kb-tool-test-facts-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp repo dir");
    dir
}

#[test]
fn dp0005_facts_are_listable_and_describable() {
    let repo_root = temp_repo_dir();

    write_file(
        repo_root.join("kb/config/tags.toml").as_path(),
        r#"
[[tag]]
id = "python"

[[tag]]
id = "rust"
"#,
    );
    write_file(
        repo_root.join("kb/facts/facts.jsonl").as_path(),
        r#"
{"fact_id":"fact:a","type":"contract","tags":["rust","x"]}
{"fact_id":"fact:b","type":"determinism","tags":["rust"]}
{"fact_id":"fact:c","type":"contract","tags":["python"]}
"#
        .trim_start(),
    );

    let all = list_facts_at(&repo_root, None, None).expect("list facts");
    assert_eq!(all.facts.len(), 3);
    assert_eq!(all.facts[0].fact_id, "fact:a");
    assert_eq!(all.facts[1].fact_id, "fact:b");
    assert_eq!(all.facts[2].fact_id, "fact:c");

    let contracts =
        list_facts_at(&repo_root, Some("contract".to_string()), None).expect("list contract facts");
    assert_eq!(contracts.facts.len(), 2);
    assert_eq!(contracts.facts[0].fact_id, "fact:a");
    assert_eq!(contracts.facts[1].fact_id, "fact:c");

    let rust_facts =
        list_facts_at(&repo_root, None, Some("rust".to_string())).expect("list rust facts");
    assert_eq!(rust_facts.facts.len(), 2);
    assert_eq!(rust_facts.facts[0].fact_id, "fact:a");
    assert_eq!(rust_facts.facts[1].fact_id, "fact:b");

    let fact_b = describe_fact_at(&repo_root, "fact:b".to_string()).expect("describe fact");
    assert_eq!(fact_b.fact_id, "fact:b");
    assert_eq!(
        fact_b
            .fact
            .as_object()
            .unwrap()
            .get("type")
            .unwrap()
            .as_str()
            .unwrap(),
        "determinism"
    );

    assert!(describe_fact_at(&repo_root, "fact:nope".to_string()).is_err());

    let _ = std::fs::remove_dir_all(repo_root);
}
