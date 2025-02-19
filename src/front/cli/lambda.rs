use anyhow::bail;
use clap::Args;
use lambda_http::run;

use crate::{
    cli::{CommandResult, GlobalArgs, SUCCESS},
    event_queue_client::{AwsEventBusClient, AwsEventBusConfig},
    front::{config::FrontConfig, routes::build_app},
    github_client::OctorustClient,
    github_config::{GithubApiConfig, GithubAppConfig},
    trace::init_fmt_with_json,
};

#[derive(Debug, Clone, Args)]
pub struct LambdaArgs {
    #[command(flatten)]
    event_bus_config: AwsEventBusConfig,
    #[command(flatten)]
    github_app_config: GithubAppConfig,
    #[command(flatten)]
    github_config: GithubApiConfig,
    #[command(flatten)]
    config: FrontConfig,
}

#[allow(clippy::no_effect_underscore_binding)]
pub async fn lambda(global: GlobalArgs, args: LambdaArgs) -> CommandResult {
    init_fmt_with_json(&global.verbose);

    let github_client = OctorustClient::new(args.github_config, args.github_app_config.clone())?;
    let app = build_app(
        args.config,
        AwsEventBusClient::new(args.event_bus_config).await,
        github_client,
        args.github_app_config,
    );
    if let Err(e) = run(app).await {
        bail!("failed to run lambda: {e}");
    }
    SUCCESS
}
