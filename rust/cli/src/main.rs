//! `jf` is jj, built on jj-cli's `CliRunner`, with the forge's commands added,
//! as ADR 0001 decides. `jf echo <message>` sends the message through the
//! forge's REST API and prints the answer. It proves the client reaches the
//! server, which passes the message through vcs, and nothing more.

mod commands;
mod forge;

use std::process::ExitCode;

use jj_cli::cli_util::CliRunner;

fn main() -> ExitCode {
    // The about text comes last, because each addition replaces it with its
    // doc comment.
    commands::add(CliRunner::init())
        .name("jf")
        .about("The jjforge command-line client")
        .version(env!("CARGO_PKG_VERSION"))
        .run()
        .into()
}
