mod lambda;
mod server;

use clap::Subcommand;

use crate::cli::{CommandResult, GlobalArgs};

#[derive(Debug, Clone, Subcommand)]
pub enum FrontCommands {
    /// Run front server. This will connect to relay or another local runner server.
    Server(server::ServerArgs),
    /// Run front server in AWS Lambda function.
    Lambda(lambda::LambdaArgs),
}

pub async fn run(global: GlobalArgs, c: FrontCommands) -> CommandResult {
    match c {
        FrontCommands::Server(args) => server::server(global, args).await,
        FrontCommands::Lambda(args) => lambda::lambda(global, args).await,
    }
}
