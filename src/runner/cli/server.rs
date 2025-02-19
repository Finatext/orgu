use std::{sync::Arc, time::Duration};

use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use axum::{serve, Json};
use clap::{Args, ValueEnum};
use strum::Display;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    normalize_path::NormalizePathLayer,
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::{info, Level};

use crate::{
    app_error::AppError,
    checkout::{CheckoutConfig, Libgit2Checkout},
    cli::{CommandResult, GlobalArgs, SUCCESS},
    events::CheckRequest,
    github_client::OctorustClient,
    github_config::{GithubApiConfig, GithubAppConfig},
    github_token::DefaultTokenFetcher,
    runner::handler::{Config, Handler},
    trace::init_fmt_with_pretty,
};

#[derive(Debug, Clone, Args)]
pub struct ServerArgs {
    #[command(flatten)]
    github_app_config: GithubAppConfig,
    #[command(flatten)]
    github_config: GithubApiConfig,
    #[command(flatten)]
    checkout_config: CheckoutConfig,
    #[command(flatten)]
    handler_config: Config,
    /// Filter events to process.
    #[arg(short, long, default_value = "pull_request")]
    select: Selection,
    /// The address to listen on.
    #[arg(long, default_value = "127.0.0.1")]
    address: String,
    /// The port to listen on.
    #[arg(long, default_value = "3001")]
    port: u16,
}

#[derive(Debug, Clone, ValueEnum, Display)]
#[strum(serialize_all = "snake_case")]
#[clap(rename_all = "snake_case")]
enum Selection {
    PullRequest,
    CheckSuite,
}

impl Selection {
    fn matches(&self, req: &CheckRequest) -> bool {
        match self {
            Self::PullRequest => {
                req.event_name == "pull_request"
                    || (req.event_name == "check_suite" && req.action == "rerequested")
                    || (req.event_name == "check_run" && req.action == "rerequested")
            }
            Self::CheckSuite => {
                req.event_name == "check_suite"
                    || (req.event_name == "check_run" && req.action == "rerequested")
            }
        }
    }
}

struct AppState {
    handler: Handler<OctorustClient, Libgit2Checkout, DefaultTokenFetcher>,
    selection: Selection,
}

pub async fn server(global: GlobalArgs, args: ServerArgs) -> CommandResult {
    init_fmt_with_pretty(&global.verbose);

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
    let app = build_app(handler, args.select);

    let listener = TcpListener::bind([args.address, args.port.to_string()].join(":")).await?;
    println!("listening on {}", listener.local_addr()?);
    serve(listener, app).await?;

    SUCCESS
}

fn build_app(
    handler: Handler<OctorustClient, Libgit2Checkout, DefaultTokenFetcher>,
    selection: Selection,
) -> Router {
    let shared_state = Arc::new(AppState { handler, selection });

    let router = Router::new()
        .route("/", get(|| async { "ok" }))
        .route("/run", post(handle))
        .with_state(shared_state);

    apply_middleware(router)
}

fn apply_middleware(router: Router) -> Router {
    let middleware = ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::DEBUG))
                .on_response(
                    DefaultOnResponse::new()
                        .latency_unit(LatencyUnit::Millis)
                        .level(Level::INFO),
                ),
        )
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(TimeoutLayer::new(Duration::from_secs(60 * 15)));
    router.layer(middleware)
}

async fn handle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CheckRequest>,
) -> Result<&'static str, AppError> {
    if !state.selection.matches(&req) {
        info!(
            "skipping event: selection={}, event={}, action={}",
            state.selection, req.event_name, req.action
        );
        return Ok("skipped");
    }

    state.handler.handle_event(req).await?;
    Ok("ok")
}
