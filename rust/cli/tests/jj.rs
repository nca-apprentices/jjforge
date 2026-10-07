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
