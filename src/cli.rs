mod checkout;
mod pattern;

use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};

use crate::{front::cli as front, runner::cli as runner, ssmenv::with_replaced_env};

pub type CommandResult = anyhow::Result<ExitCode>;

pub const SUCCESS: CommandResult = Ok(ExitCode::SUCCESS);
// Indicates domain failures, not errors.
pub const FAILURE: CommandResult = Ok(ExitCode::FAILURE);

#[allow(clippy::partial_pub_fields)] // To use global options.
#[derive(Debug, Clone, Parser)]
#[command(version, about, args_override_self(true))]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

#[derive(Debug, Clone, Subcommand)]
enum Commands {
    #[command(subcommand)]
    /// Run front.
    Front(front::FrontCommands),
    #[command(subcommand)]
    /// Run runner.
    Runner(runner::RunnerCommands),
    #[command(subcommand)]
    /// Support tools to help EventBridge event pattern development.
    Pattern(pattern::PatternCommands),
    /// Clone and checkout GitHub repository.
    /// Use this command inside CI job in which GitHub Installation Access Token is available.
    Checkout(checkout::CheckoutArgs),
}

pub async fn run() -> CommandResult {
    // FIXME(taiki45): Set up tracing subscriber, before calling with_replaced_env.
    //   The promlem is: Setting proper formatter can be determined by subcommand,
    //   but to get subcommand, we need parsed Cli which requires with_replaced_env.
    let cli = with_replaced_env(Cli::parse)
        .await
        .with_context(|| "fetching from AWS SSM failed")?;
    let cli_clone = cli.clone();
    match cli.command {
        // Pass Cli to use global options. Is there a better way?
        Commands::Front(c) => front::run(cli_clone, c).await,
        Commands::Runner(c) => runner::run(cli_clone, c).await,
        Commands::Pattern(c) => pattern::run(cli_clone, c).await,
        Commands::Checkout(c) => checkout::checkout(cli_clone, c).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}
