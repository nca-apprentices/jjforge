//! The options and commands `jf` adds to jj's.

use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use clap::Args;
use clap::FromArgMatches as _;
use clap::Subcommand;
use jj_cli::cli_util::BoxedAsyncCliDispatch;
use jj_cli::cli_util::CliRunner;
use jj_cli::cli_util::CommandHelper;
use jj_cli::cli_util::short_operation_hash;
use jj_cli::command_error::CommandError;
use jj_cli::command_error::cli_error;
use jj_cli::command_error::user_error;
use jj_cli::ui::Ui;
use jjforge_store::ForgeDir;
use jjforge_store::Pushed;
use reqwest::Url;

use crate::forge;

/// Lets `jf undo` undo a fetch, and the fetch that ends a push, which jj
/// can't, as `jjforge_store::undo_fetch` says. The forge keeps what it
/// holds, and the next fetch brings it back.
async fn undo_fetch(
    ui: &mut Ui,
    helper: &CommandHelper,
    dispatch: BoxedAsyncCliDispatch<'_>,
) -> Result<(), CommandError> {
    if helper.matches().subcommand_name() != Some("undo") {
        return dispatch.call(ui, helper).await;
    }
    let mut workspace = helper.workspace_helper(ui).await?;
    let operation = workspace.repo().operation();
    let undone = format!(
        "{} {}",
        short_operation_hash(operation.id()),
        operation.metadata().description
    );
    let mut tx = workspace.start_transaction();
    let Some(description) = jjforge_store::undo_fetch(tx.repo_mut())
        .await
        .map_err(user_error)?
    else {
        drop(tx);
        drop(workspace);
        return dispatch.call(ui, helper).await;
    };
    tx.finish(ui, description).await?;
    writeln!(ui.status(), "Undid operation: {undone}")?;
    writeln!(
        ui.hint_default(),
        "The forge keeps what it holds, and the next `jf fetch` brings it back."
    )?;
    Ok(())
}

/// The forge to talk to, an option of every command.
#[derive(Args)]
struct Forge {
    /// The forge to talk to.
    #[arg(
        long,
        env = "JJFORGE_ENDPOINT",
        default_value = "http://localhost:8080",
        global = true
    )]
    endpoint: Url,
}

/// The commands `jf` adds to jj's. Bad usage exits with status 2, as
/// shared/docs/cli.md describes.
#[derive(Subcommand)]
enum Command {
    /// Sends a message through the forge and prints the answer.
    Echo {
        /// The message, joined with spaces.
        #[arg(required = true)]
        words: Vec<String>,
    },
    /// Clones a repository of the forge into a new workspace.
    ///
    /// The working copy starts a new change on `main@forge`. Trees and files
    /// arrive when jj first reads them.
    Clone {
        /// The repository's clone address, as the forge shows it.
        address: Url,
        /// The directory of the new workspace. The repository's ID by
        /// default.
        destination: Option<PathBuf>,
    },
    /// Fetches the forge's new operations and merges them into this
    /// repository.
    Fetch,
    /// Publishes bookmarks to the forge.
    ///
    /// When someone else published first, it fetches, merges, and tries
    /// again, or stops when a bookmark ends up conflicted.
    Push {
        /// A bookmark to publish.
        #[arg(long = "bookmark", short, required = true)]
        bookmarks: Vec<String>,
    },
    /// Creates a repository of the store type `jjforge` without a forge.
    #[command(hide = true)]
    Init {
        /// The directory of the new workspace.
        destination: PathBuf,
    },
}

/// Adds the options and commands of `jf` to `runner`.
pub(crate) fn add(runner: CliRunner<'_>) -> CliRunner<'_> {
    // jj-cli hands a global option to its own callback, before the command
    // runs, so `run` reads `Forge` from the matches instead.
    runner
        .add_global_args(|_: &mut Ui, _: Forge| Ok(()))
        .add_subcommand(run)
        .add_dispatch_hook(undo_fetch)
}

async fn run(ui: &mut Ui, helper: &CommandHelper, command: Command) -> Result<(), CommandError> {
    match command {
        Command::Echo { words } => echo(ui, helper, &words),
        Command::Clone {
            address,
            destination,
        } => clone(ui, helper, address, destination).await,
        Command::Fetch => fetch(ui, helper).await,
        Command::Push { bookmarks } => push(ui, helper, &bookmarks).await,
        Command::Init { destination } => {
            std::fs::create_dir_all(&destination)?;
            jjforge_store::init(helper.settings(), &destination)
                .await
                .map_err(user_error)?;
            Ok(())
        }
    }
}

fn echo(ui: &mut Ui, helper: &CommandHelper, words: &[String]) -> Result<(), CommandError> {
    let forge = Forge::from_arg_matches(helper.matches()).map_err(cli_error)?;
    let client = forge::Client::new(forge.endpoint).map_err(user_error)?;

    // jj-cli runs commands on its own executor, and reqwest needs tokio. The
    // trace ID goes to stderr either way, so a person can quote it.
    let answer = tokio::runtime::Runtime::new()?.block_on(client.echo(words.join(" ")));
    writeln!(ui.stderr(), "trace id: {}", client.trace())?;
    writeln!(ui.stdout(), "{}", answer.map_err(user_error)?)?;
    Ok(())
}

/// The token of `JJFORGE_TOKEN`, until `jf` signs people in itself.
fn token() -> Option<String> {
    std::env::var("JJFORGE_TOKEN").ok()
}

/// The repository's forge, as a clone recorded it.
fn remote(repo_path: &Path) -> Result<forge::SyncClient, CommandError> {
    let url = ForgeDir::new(repo_path)
        .url()?
        .ok_or_else(|| user_error("This repository has no forge to sync with"))?;
    let url = Url::parse(&url).map_err(user_error)?;
    forge::SyncClient::new(url, token()).map_err(user_error)
}

/// Clones into `destination`, and removes the directory again when the
/// clone fails, as #30 decides.
async fn clone(
    ui: &mut Ui,
    helper: &CommandHelper,
    address: Url,
    destination: Option<PathBuf>,
) -> Result<(), CommandError> {
    let destination = match destination {
        Some(destination) => destination,
        None => address
            .path_segments()
            .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
            .map(PathBuf::from)
            .ok_or_else(|| user_error("The address names no repository"))?,
    };
    let root = helper.cwd().join(&destination);
    if root.exists() && root.read_dir()?.next().is_some() {
        return Err(user_error(format!(
            "Destination {} is not empty",
            destination.display()
        )));
    }

    let created = !root.exists();
    std::fs::create_dir_all(&root)?;
    let cloned = clone_into(helper, &address, &root).await;
    if cloned.is_err() {
        // A directory that was there stays, but empty, as it was.
        let written = if created {
            root.clone()
        } else {
            root.join(".jj")
        };
        std::fs::remove_dir_all(&written).ok();
    }
    cloned?;
    writeln!(
        ui.status(),
        "Cloned {address} into {}",
        destination.display()
    )?;
    Ok(())
}

async fn clone_into(
    helper: &CommandHelper,
    address: &Url,
    root: &Path,
) -> Result<(), CommandError> {
    let remote = forge::SyncClient::new(address.clone(), token()).map_err(user_error)?;
    let (workspace, _) = jjforge_store::init(helper.settings(), root)
        .await
        .map_err(user_error)?;
    ForgeDir::new(workspace.repo_path()).set_url(address.as_str())?;

    // A workspace loaded through jf's factories reads what it lacks from the
    // forge it now records.
    let mut workspace = helper.load_workspace_at(root, helper.settings())?;
    let repo = workspace
        .repo_loader()
        .load_at_head()
        .await
        .map_err(user_error)?;
    let repo_path = workspace.repo_path().to_owned();
    let fetched = jjforge_store::fetch(&repo, &repo_path, &remote)
        .await
        .map_err(user_error)?;
    jjforge_store::start_on_main(&mut workspace, &fetched.repo)
        .await
        .map_err(user_error)?;
    Ok(())
}

async fn fetch(ui: &mut Ui, helper: &CommandHelper) -> Result<(), CommandError> {
    let workspace_helper = helper.workspace_helper(ui).await?;
    let old = workspace_helper.repo().clone();
    let repo_path = workspace_helper.repo_path().to_owned();
    drop(workspace_helper);

    let fetched = jjforge_store::fetch(&old, &repo_path, &remote(&repo_path)?)
        .await
        .map_err(user_error)?;
    if !fetched.changed {
        writeln!(ui.status(), "Nothing changed.")?;
        return Ok(());
    }
    let mut workspace = helper.load_workspace()?;
    jjforge_store::follow_working_copy(&mut workspace, &old, &fetched.repo)
        .await
        .map_err(user_error)?;
    writeln!(ui.status(), "Fetched from forge")?;
    Ok(())
}

async fn push(
    ui: &mut Ui,
    helper: &CommandHelper,
    bookmarks: &[String],
) -> Result<(), CommandError> {
    let workspace_helper = helper.workspace_helper(ui).await?;
    let old = workspace_helper.repo().clone();
    let repo_path = workspace_helper.repo_path().to_owned();
    drop(workspace_helper);

    let pushed = jjforge_store::push(&old, &repo_path, &remote(&repo_path)?, bookmarks)
        .await
        .map_err(user_error)?;
    let Pushed::Done(repo) = pushed else {
        writeln!(ui.status(), "Nothing changed.")?;
        return Ok(());
    };
    let mut workspace = helper.load_workspace()?;
    jjforge_store::follow_working_copy(&mut workspace, &old, &repo)
        .await
        .map_err(user_error)?;
    writeln!(ui.status(), "Pushed {} to forge", bookmarks.join(", "))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses `args` as jj-cli does for the options and commands `jf` adds.
    fn parse(args: &[&str]) -> Result<(Forge, Command), clap::Error> {
        let matches = Command::augment_subcommands(Forge::augment_args(clap::Command::new("jf")))
            .try_get_matches_from(args)?;
        Ok((
            Forge::from_arg_matches(&matches)?,
            Command::from_arg_matches(&matches)?,
        ))
    }

    #[test]
    fn echo_needs_a_message() {
        assert!(parse(&["jf", "echo"]).is_err());
    }

    #[test]
    fn endpoint_must_be_an_address() {
        assert!(parse(&["jf", "--endpoint", "not an address", "echo", "hi"]).is_err());
    }

    #[test]
    fn echo_joins_the_words() {
        let (_, Command::Echo { words }) = parse(&["jf", "echo", "hello", "there"]).unwrap() else {
            panic!("not echo");
        };

        assert_eq!(words.join(" "), "hello there");
    }

    #[test]
    fn push_needs_a_bookmark() {
        assert!(parse(&["jf", "push"]).is_err());
        let (_, Command::Push { bookmarks }) =
            parse(&["jf", "push", "-b", "main", "-b", "dev"]).unwrap()
        else {
            panic!("not push");
        };

        assert_eq!(bookmarks, ["main", "dev"]);
    }

    #[test]
    fn clone_needs_an_address() {
        assert!(parse(&["jf", "clone"]).is_err());
        assert!(parse(&["jf", "clone", "not an address"]).is_err());
    }
}
