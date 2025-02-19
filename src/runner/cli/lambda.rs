use std::error::Error;

use anyhow::bail;
use aws_lambda_events::eventbridge::EventBridgeEvent;
use clap::Args;
use lambda_runtime::{run, service_fn, LambdaEvent};
use tracing::error;

use crate::{
    checkout::{CheckoutConfig, Libgit2Checkout},
    cli::{CommandResult, GlobalArgs, FAILURE},
    events::CheckRequest,
    github_client::OctorustClient,
    github_config::{GithubApiConfig, GithubAppConfig},
    github_token::DefaultTokenFetcher,
    runner::handler::{Config, Handler},
    trace::init_fmt_with_json,
};

#[derive(Debug, Clone, Args)]
pub struct LambdaArgs {
    #[command(flatten)]
    github_app_config: GithubAppConfig,
    #[command(flatten)]
    github_config: GithubApiConfig,
    #[command(flatten)]
    checkout_config: CheckoutConfig,
    #[command(flatten)]
    handler_config: Config,
}

pub async fn lambda(global: GlobalArgs, args: LambdaArgs) -> CommandResult {
    init_fmt_with_json(&global.verbose);

    let client = OctorustClient::new(args.github_config.clone(), args.github_app_config.clone())?;
    let checkout = Libgit2Checkout::new(args.checkout_config);
    let fetcher =
        DefaultTokenFetcher::new(args.github_config.clone(), args.github_app_config.clone())?;
    let handler = Handler::new(
        args.handler_config,
        args.github_app_config,
        client,
        checkout,
        fetcher,
    );

    let service = service_fn(|event: LambdaEvent<EventBridgeEvent<CheckRequest>>| {
        let h = &handler;
        async move {
            h.handle_event(event.payload.detail)
                .await
                .map_err(Into::<Box<dyn Error>>::into)
        }
    });

    // Use bail! because run returns unmachable type.
    if let Err(e) = run(service).await {
        bail!("lambda_runtime::run error: {:?}", e);
    }

    error!("lambda_runtime::run returned unexpectedly");
    FAILURE
}
