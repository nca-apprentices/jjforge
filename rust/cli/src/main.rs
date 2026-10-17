//! `jf` is jj, built on jj-cli's `CliRunner`, with the forge's commands and
//! the store type `jjforge` added, as ADR 0001 decides. `jf clone`, `jf
//! fetch`, and `jf push` sync with vcs over `sync/v1`. `jf echo <message>`
//! proves the client reaches the server, which passes the message through
//! vcs.

mod commands;
mod credential;
mod forge;

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use jj_cli::cli_util::CliRunner;
use jjforge_store::client;
use jjforge_store::client::Fetch;
use jjforge_store::client::TransportError;

fn main() -> ExitCode {
    // The about text comes last, because each addition replaces it with its
    // doc comment.
    commands::add(CliRunner::init())
        .add_store_factories(client::factories(Arc::new(find_forge)))
        .add_extra_config(client::config())
        .name("jf")
        .about("The jjforge command-line client")
        .version(env!("CARGO_PKG_VERSION"))
        .run()
        .into()
}

/// The forge of a clone, so its backend fetches the trees and files it
/// lacks, or `None` for a repository without one.
fn find_forge(repo_path: &Path) -> Result<Option<Arc<dyn Fetch>>, TransportError> {
    let Some(url) = client::forge_url(repo_path)? else {
        return Ok(None);
    };
    let credential = credential::Credential::from_env();
    Ok(Some(Arc::new(forge::SyncClient::new(
        url.parse()?,
        credential,
    )?)))
}
