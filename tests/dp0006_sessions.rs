use std::path::Path;

use chrono::Datelike;
use kb::error::ErrorCode;
use kb::query::session::{
    session_check_at, session_finalize_at, session_init_at, SessionCapsule, VerificationKind,
};
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
    dir.push(format!(
        "kb-tool-test-sessions-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp repo dir");
    dir
}

#[test]
fn dp0006_session_init_creates_expected_schema_and_path() {
    let repo_root = temp_repo_dir();
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

    let now = chrono::Local::now();
    let yyyy = now.year();
    let mm = now.month();

    let id = "PR-1234".to_string();
    let created = session_init_at(&repo_root, id.clone(), vec![]).expect("session init");
    assert!(created.is_file(), "capsule file created");

    let rel = created
        .strip_prefix(&repo_root)
        .expect("strip prefix")
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(rel, format!("kb/sessions/{yyyy:04}/{mm:02}/{id}.json"));

    let text = std::fs::read_to_string(&created).expect("read capsule");
    let capsule: SessionCapsule = serde_json::from_str(&text).expect("parse capsule");
    assert_eq!(capsule.session_id, id);
    assert!(capsule.tags.is_empty());
    assert_eq!(capsule.summary, "");
    assert!(capsule.decisions.is_empty());
    assert!(capsule.pitfalls.is_empty());
    assert!(capsule.verification.is_empty());
    assert!(capsule.refs.is_empty());

    let _ = std::fs::remove_dir_all(repo_root);
}

#[test]
fn dp0006_session_init_fails_when_file_exists() {
    let repo_root = temp_repo_dir();
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

    let id = "PR-1234".to_string();
    session_init_at(&repo_root, id.clone(), vec![]).expect("session init");
    let err = session_init_at(&repo_root, id, vec![]).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);

    let _ = std::fs::remove_dir_all(repo_root);
}

#[test]
fn dp0006_session_finalize_merges_verification_and_appends_refs() {
    let repo_root = temp_repo_dir();
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

    let id = "PR-1234".to_string();
    let capsule_path = session_init_at(&repo_root, id.clone(), vec![]).expect("session init");

    // Make a worktree change so finalize can derive refs from the diff.
    write_file(
        repo_root.join("src/lib.rs").as_path(),
        "pub fn foo() {}\npub fn bar() {}\n",
    );

    let diff_source = DiffSource::Worktree;
    session_finalize_at(
        &repo_root,
        id.clone(),
        &diff_source,
        vec![
            VerificationKind::Tests,
            VerificationKind::Lint,
            VerificationKind::Tests,
        ],
    )
    .expect("session finalize");
    let capsule: SessionCapsule =
        serde_json::from_str(&std::fs::read_to_string(&capsule_path).unwrap()).unwrap();
    assert_eq!(
        capsule.verification,
        vec!["lint".to_string(), "tests".to_string()]
    );
    assert_eq!(capsule.refs, vec!["src/lib.rs".to_string()]);

    session_finalize_at(&repo_root, id, &diff_source, vec![VerificationKind::Bench])
        .expect("session finalize again");
    let capsule: SessionCapsule =
        serde_json::from_str(&std::fs::read_to_string(&capsule_path).unwrap()).unwrap();
    assert_eq!(
        capsule.verification,
        vec!["bench".to_string(), "lint".to_string(), "tests".to_string()]
    );
    assert_eq!(capsule.refs, vec!["src/lib.rs".to_string()]);

    let _ = std::fs::remove_dir_all(repo_root);
}

#[test]
fn dp0006_session_check_rejects_missing_keys_and_absolute_paths() {
    let repo_root = temp_repo_dir();
    run(std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(&repo_root));

    let id = "PR-1234".to_string();
    let capsule_path = session_init_at(&repo_root, id.clone(), vec![]).expect("session init");

    // Missing required key.
    write_file(
        &capsule_path,
        r#"{"session_id":"PR-1234","tags":[],"summary":"","decisions":[],"pitfalls":[],"verification":[]}"#,
    );
    let err = session_check_at(&repo_root, id.clone()).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);

    // Absolute path should be rejected.
    write_file(
        &capsule_path,
        r#"{"session_id":"PR-1234","tags":[],"summary":"","decisions":[],"pitfalls":[],"verification":[],"refs":["/Users/example/file"]}"#,
    );
    let err = session_check_at(&repo_root, id).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);

    let _ = std::fs::remove_dir_all(repo_root);
}
