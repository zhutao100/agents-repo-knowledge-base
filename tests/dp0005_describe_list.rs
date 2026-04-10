use std::path::Path;

use kb::index::{index_regen_at, IndexScope};
use kb::query::describe::{
    describe_path_at, describe_symbol_at, DescribePathInclude, DescribeSymbolInclude,
};
use kb::query::list::{list_modules_at, list_symbols_at, list_tags_at};
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
fn dp0005_list_and_describe_are_deterministic_and_typed() {
    let repo_root = support::TempRepo::new("kb-tool-test-describe-list-");
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

    // Module cards (intentionally out of order to test stable sorting).
    write_file(
        repo_root.join("kb/atlas/modules/b.toml").as_path(),
        "id = \"b\"\ntitle = \"B\"\n",
    );
    write_file(
        repo_root.join("kb/atlas/modules/a.toml").as_path(),
        "id = \"a\"\ntitle = \"A\"\n",
    );

    // Tags config for validation.
    write_file(
        repo_root.join("kb/config/tags.toml").as_path(),
        r#"
[[tag]]
id = "ok"
description = "ok"
"#,
    );

    // Index required for list/describe symbols and path.
    let diff_source = DiffSource::Worktree;
    index_regen_at(&repo_root, &diff_source, IndexScope::All).expect("index regen");

    // list tags stable ordering
    let tags = list_tags_at(&repo_root).expect("list tags");
    assert_eq!(tags.tags, vec!["ok".to_string()]);

    // list modules stable ordering by id
    let modules = list_modules_at(&repo_root, None, None).expect("list modules");
    assert_eq!(modules.modules[0].module_id, "a");
    assert_eq!(modules.modules[1].module_id, "b");

    // Unknown tag should fail when tags.toml is present.
    assert!(list_modules_at(&repo_root, Some("nope".to_string()), None).is_err());

    // describe path determinism
    let includes = vec![DescribePathInclude::Dirs, DescribePathInclude::Files];
    let out1 = describe_path_at(&repo_root, "src/".to_string(), 1, includes.clone()).unwrap();
    let out2 = describe_path_at(&repo_root, "src/".to_string(), 1, includes).unwrap();
    assert_eq!(
        serde_json::to_string(&out1).unwrap(),
        serde_json::to_string(&out2).unwrap()
    );

    // describe symbol returns empty uses when xrefs missing
    let list_syms = list_symbols_at(&repo_root, "src/lib.rs".to_string(), None).unwrap();
    let sym_id = list_syms.symbols[0].symbol_id.clone();
    let sym = describe_symbol_at(
        &repo_root,
        sym_id,
        vec![DescribeSymbolInclude::Def, DescribeSymbolInclude::Uses],
    )
    .unwrap();
    assert!(sym.uses.unwrap().is_empty());

    // module card filename/id mismatch rejected
    write_file(
        repo_root.join("kb/atlas/modules/mismatch.toml").as_path(),
        "id = \"other\"\ntitle = \"x\"\n",
    );
    assert!(list_modules_at(&repo_root, None, None).is_err());
}
