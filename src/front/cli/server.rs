use axum::{body::Body, serve, Router, ServiceExt};
use clap::Args;
use http::Request;
use tokio::net::TcpListener;
use tower_http::normalize_path::NormalizePath;
use url::Url;

use crate::{
    cli::{Cli, CommandResult, SUCCESS},
    event_queue_client::{
        AwsEventBusClient, AwsEventBusConfig, EventQueueRelayClient, EventQueueRelayConfig,
    },
    front::{config::FrontConfig, routes::build_app},
    github_client::OctorustClient,
    github_config::{GithubApiConfig, GithubAppConfig},
    trace::init_fmt_with_pretty,
};

#[derive(Debug, Clone, Args)]
pub struct ServerArgs {
    #[command(flatten)]
    github_app_config: GithubAppConfig,
    #[command(flatten)]
    github_config: GithubApiConfig,
    #[command(flatten)]
    config: FrontConfig,
    #[arg(long, default_value = "http://127.0.0.1:3001/run")]
    event_queue_relay_endpoint: String,
    /// The address to listen on.
    #[arg(long, default_value = "127.0.0.1")]
    address: String,
    /// The port to listen on.
    #[arg(long, default_value = "3000")]
    port: u16,
    /// Switch to use AWS EventBus as event bus.
    #[arg(long, env, default_value = "false")]
    use_aws_event_bus: bool,
    #[command(flatten)]
    event_bus_config: AwsEventBusConfig,
}

pub async fn server(cli: Cli, args: ServerArgs) -> CommandResult {
    init_fmt_with_pretty(&cli.verbose);

    let github_client = OctorustClient::new(args.github_config, args.github_app_config)?;

    let app = if args.use_aws_event_bus {
        build_app(
            args.config,
            AwsEventBusClient::new(args.event_bus_config).await,
            github_client,
        )
    } else {
        let config = EventQueueRelayConfig {
            endpoint: Url::parse(&args.event_queue_relay_endpoint)?,
        };
        build_app(
            args.config,
            EventQueueRelayClient::new(config),
            github_client,
        )
    };
    let app = <NormalizePath<Router> as ServiceExt<Request<Body>>>::into_make_service(app);

    let listener = TcpListener::bind([args.address, args.port.to_string()].join(":")).await?;
    println!("listening on {}", listener.local_addr()?);
    serve(listener, app).await?;

    SUCCESS
}
