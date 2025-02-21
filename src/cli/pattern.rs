mod generate;
mod test;

use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use strum::Display;

use crate::cli::{CommandResult, GlobalArgs};

#[derive(Debug, Clone, Subcommand)]
pub enum PatternCommands {
    /// Generate example event to test EventBridge event pattern.
    Test(test::TestArgs),
    /// Generate event pattern.
    Generate(generate::GenerateArgs),
}

pub async fn run(global: GlobalArgs, c: PatternCommands) -> CommandResult {
    match c {
        PatternCommands::Test(args) => test::test(global, args).await,
        PatternCommands::Generate(args) => generate::generate(global, args),
    }
}

#[derive(Debug, Clone, Args)]
struct CustomPropsConfig {
    #[arg(short, long, value_parser = parse_key_val)]
    /// GitHub Custom Properties for the example event. Pass each pair as `key=value` format.
    custom_props: Vec<(String, String)>,
}

#[derive(Debug, Clone, ValueEnum, Display)]
#[clap(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
enum EventType {
    PullRequest,
    CheckSuite,
}

#[derive(Debug, Clone, ValueEnum, Display)]
#[clap(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
enum EventAction {
    Opened,
    Cloned,
    Synchronize,
    Reopened,
    ReadyForReview,
    Requested,
    Rerequested,
}

fn parse_key_val(s: &str) -> Result<(String, String)> {
    match s.split_once('=') {
        Some((key, value)) => Ok((key.to_owned(), value.to_owned())),
        None => bail!("invalid key=value pair: no `=` found in `{}`", s),
    }
}
