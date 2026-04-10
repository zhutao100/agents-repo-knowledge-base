use std::path::Path;

use kb::index::{index_regen_at, IndexScope};
use kb::query::pack::{pack_diff_at, pack_selectors_at, SelectorInputs};
use kb::query::plan::{plan_diff_at, Policy};
use kb::repo::diff_source::DiffSource;

mod support;

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

#[test]
fn dp0003_plan_diff_and_pack_diff_work_end_to_end() {
    let repo_root = support::TempRepo::new("kb-tool-test-plan-pack-");
    run(std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(&repo_root));
    run(std::process::Command::new("git")
        .args(["config", "user.email", "kb-tool@test.invalid"])
        .current_dir(&repo_root));
    run(std::process::Command::new("git")
        .args(["config", "user.name", "kb-tool"])
        .current_dir(&repo_root));

    write_file(repo_root.join("src/lib.rs").as_path(), "pub fn foo() {}\n");
    run(std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&repo_root));
    run(std::process::Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(&repo_root));

    write_file(
        repo_root.join("kb/config/obligations.toml").as_path(),
        r#"
[[rule]]
id = "core"
when_path_prefix = "src/"
require_module_card = "core"
"#,
    );
    write_file(
        repo_root.join("kb/atlas/modules/core.toml").as_path(),
        r#"
id = "core"
title = "Core"
"#,
    );

    // Make a change so HEAD vs worktree diff is non-empty.
    write_file(
        repo_root.join("src/lib.rs").as_path(),
        "pub fn foo() { println!(\"x\"); }\n",
    );

    // Index must exist for pack to work.
    let diff_source = DiffSource::Worktree;
    index_regen_at(&repo_root, &diff_source, IndexScope::All).expect("index regen");

    let plan = plan_diff_at(&repo_root, &diff_source, Policy::Default).expect("plan diff");
    assert!(plan.changed_paths.iter().any(|c| c.path == "src/lib.rs"));
    assert!(plan.required.module_cards.iter().any(|m| m == "core"));

    let pack = pack_diff_at(&repo_root, &diff_source, 0, 200_000, 10).expect("pack diff");
    assert_eq!(pack.diff_source, "worktree");
    assert!(!pack.tree.is_empty());
    assert!(!pack.symbols.is_empty());
    assert!(!pack.modules.is_empty());

    let selectors = SelectorInputs {
        paths: vec!["src/lib.rs".to_string()],
        modules: vec!["core".to_string()],
        symbols: vec![],
        facts: vec![],
    };
    let pack_sel = pack_selectors_at(&repo_root, &selectors, 200_000, 10).expect("pack selectors");
    assert!(pack_sel.tree.iter().any(|r| r.path == "src/lib.rs"));
    assert!(pack_sel.modules.iter().any(|m| m.module_id == "core"));
}

#[test]
fn dp0003_pack_selectors_expands_modules_to_paths_and_facts() {
    let repo_root = support::TempRepo::new("kb-tool-test-plan-pack-");
    run(std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(&repo_root));
    run(std::process::Command::new("git")
        .args(["config", "user.email", "kb-tool@test.invalid"])
        .current_dir(&repo_root));
    run(std::process::Command::new("git")
        .args(["config", "user.name", "kb-tool"])
        .current_dir(&repo_root));

    write_file(repo_root.join("src/lib.rs").as_path(), "pub fn foo() {}\n");
    run(std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&repo_root));
    run(std::process::Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(&repo_root));

    write_file(
        repo_root.join("kb/atlas/modules/core.toml").as_path(),
        r#"
id = "core"
title = "Core"
entrypoints = ["src/"]
edit_points = ["src/lib.rs"]
related_facts = ["fact:core:contract:api"]
"#,
    );
    write_file(
        repo_root.join("kb/facts/facts.jsonl").as_path(),
        r#"{"fact_id":"fact:core:contract:api","type":"contract","tags":["api"]}"#,
    );

    let diff_source = DiffSource::Worktree;
    index_regen_at(&repo_root, &diff_source, IndexScope::All).expect("index regen");

    let selectors = SelectorInputs {
        paths: vec![],
        modules: vec!["core".to_string()],
        symbols: vec![],
        facts: vec![],
    };
    let pack_sel = pack_selectors_at(&repo_root, &selectors, 200_000, 10).expect("pack selectors");

    assert!(pack_sel.tree.iter().any(|r| r.path == "src/lib.rs"));
    assert!(pack_sel.modules.iter().any(|m| m.module_id == "core"));
    assert!(pack_sel
        .facts
        .iter()
        .any(|f| f.get("fact_id").and_then(|v| v.as_str()) == Some("fact:core:contract:api")));
    assert!(pack_sel.snippets.iter().any(|s| s.path == "src/lib.rs"));
}
