//! `jf` runs jj's own commands, as ADR 0001 decides.

use std::io;
use std::path::Path;
use std::process::Command;
use std::process::Output;

/// Runs `jf` in `dir` with no user configuration.
fn jf(dir: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_jf"))
        .current_dir(dir)
        .args(args)
        .env("JJ_CONFIG", dir.join("config.toml"))
        .env("JJ_USER", "Ada")
        .env("JJ_EMAIL", "ada@example.com")
        .output()
}

#[test]
fn jj_commands_work_without_git() {
    let dir = tempfile::tempdir().unwrap();

    let init = jf(dir.path(), &["debug", "init-simple", "repo"]).unwrap();
    assert!(init.status.success(), "{init:?}");

    let repo = dir.path().join("repo");
    let new = jf(&repo, &["new", "--message", "first"]).unwrap();
    assert!(new.status.success(), "{new:?}");

    let log = jf(&repo, &["log", "--no-graph", "--template", "description"]).unwrap();
    assert!(log.status.success(), "{log:?}");
    assert_eq!(String::from_utf8(log.stdout).unwrap(), "first\n");
}

#[test]
fn jj_commands_work_on_the_jjforge_store() {
    let dir = tempfile::tempdir().unwrap();

    let init = jf(dir.path(), &["init", "repo"]).unwrap();
    assert!(init.status.success(), "{init:?}");
    let repo = dir.path().join("repo");
    for store in ["store", "op_store", "op_heads"] {
        let kind = std::fs::read_to_string(repo.join(".jj/repo").join(store).join("type")).unwrap();
        assert_eq!(kind, "jjforge", "{store}");
    }

    std::fs::write(repo.join("README"), "hello\n").unwrap();
    for args in [
        &["describe", "--message", "first"][..],
        &["new", "--message", "second"],
        &["new", "--message", "side", "root()"],
        &[
            "rebase",
            "--revisions",
            "description(substring:side)",
            "--destination",
            "description(substring:first)",
        ],
    ] {
        let output = jf(&repo, args).unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
    }

    let log = jf(
        &repo,
        &[
            "log",
            "--no-graph",
            "--revisions",
            "description(substring:side)-",
            "--template",
            "description",
        ],
    )
    .unwrap();
    assert_eq!(String::from_utf8(log.stdout).unwrap(), "first\n");
    let op_log = jf(
        &repo,
        &[
            "op",
            "log",
            "--no-graph",
            "--template",
            "description ++ \"\\n\"",
        ],
    )
    .unwrap();
    let operations = String::from_utf8(op_log.stdout).unwrap();
    assert!(operations.contains("rebase"), "{operations}");
}

#[test]
fn a_failed_clone_leaves_an_empty_destination_empty() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("empty")).unwrap();

    let clone = jf(
        dir.path(),
        &["clone", "http://127.0.0.1:1/sync/v1/acme/demo", "empty"],
    )
    .unwrap();

    assert!(!clone.status.success(), "{clone:?}");
    assert_eq!(
        std::fs::read_dir(dir.path().join("empty")).unwrap().count(),
        0
    );
}
