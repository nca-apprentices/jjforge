//! The options and commands `jf` adds to jj's.

use std::io::Write as _;

use clap::Args;
use clap::FromArgMatches as _;
use clap::Subcommand;
use jj_cli::cli_util::CliRunner;
use jj_cli::cli_util::CommandHelper;
use jj_cli::command_error::CommandError;
use jj_cli::command_error::cli_error;
use jj_cli::command_error::user_error;
use jj_cli::ui::Ui;
use reqwest::Url;

use crate::forge;

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
}

/// Adds the options and commands of `jf` to `runner`.
pub(crate) fn add(runner: CliRunner<'_>) -> CliRunner<'_> {
    // jj-cli hands a global option to its own callback, before the command
    // runs, so `run` reads `Forge` from the matches instead.
    runner
        .add_global_args(|_: &mut Ui, _: Forge| Ok(()))
        .add_subcommand(run)
}

async fn run(ui: &mut Ui, helper: &CommandHelper, command: Command) -> Result<(), CommandError> {
    let forge = Forge::from_arg_matches(helper.matches()).map_err(cli_error)?;
    let Command::Echo { words } = command;
    let client = forge::Client::new(forge.endpoint).map_err(user_error)?;

    // jj-cli runs commands on its own executor, and reqwest needs tokio. The
    // trace ID goes to stderr either way, so a person can quote it.
    let answer = tokio::runtime::Runtime::new()?.block_on(client.echo(words.join(" ")));
    writeln!(ui.stderr(), "trace id: {}", client.trace())?;
    writeln!(ui.stdout(), "{}", answer.map_err(user_error)?)?;
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
        let (_, Command::Echo { words }) = parse(&["jf", "echo", "hello", "there"]).unwrap();

        assert_eq!(words.join(" "), "hello there");
    }
}
