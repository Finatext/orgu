mod lambda;
mod oneshot;
mod server;

use clap::Subcommand;

use crate::cli::{CommandResult, GlobalArgs};

#[derive(Debug, Clone, Subcommand)]
pub enum RunnerCommands {
    /// Run runner server. This will be called from custom queue relay service or local front server.
    Server(server::ServerArgs),
    /// Run CI job as oneshot task. Use this to develop CI job locally.
    Oneshot(oneshot::OneshotArgs),
    /// Run runner in AWS Lambda function. Triggered by EventBridge events.
    Lambda(lambda::LambdaArgs),
}

pub async fn run(global: GlobalArgs, c: RunnerCommands) -> CommandResult {
    match c {
        RunnerCommands::Server(args) => server::server(global, args).await,
        RunnerCommands::Oneshot(args) => oneshot::oneshot(global, args).await,
        RunnerCommands::Lambda(args) => lambda::lambda(global, args).await,
    }
}
