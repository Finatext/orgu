use clap::Args;

use crate::{
    checkout::{CheckoutConfig, Libgit2Checkout},
    cli::{Cli, CommandResult, SUCCESS},
    events::{CheckRequest, User},
    github_client::{NullClient, OctorustClient},
    github_config::{GithubApiConfig, GithubAppConfig},
    github_token::{DefaultTokenFetcher, TokenFetcher as _},
    runner::handler::{Config, Handler},
    trace::init_fmt_with_pretty,
};

#[derive(Debug, Clone, Args)]
pub struct OneshotArgs {
    #[command(flatten)]
    github_app_config: GithubAppConfig,
    #[command(flatten)]
    github_config: GithubApiConfig,
    #[command(flatten)]
    checkout_config: CheckoutConfig,
    #[command(flatten)]
    handler_config: Config,
    /// GitHub repository owner name. e.g. `octocat/helloworld` -> `octocat`.
    #[arg(env, long, short = 'o')]
    repo_owner: String,
    /// GitHub repository name. e.g. `octocat/helloworld` -> `helloworld`.
    #[arg(env, long, short = 'r')]
    repo_name: String,
    /// SHA of the commit to be checked out. If none, remote HEAD will be checked-out.
    #[arg(env, long)]
    head_sha: Option<String>,
}

pub async fn oneshot(cli: Cli, args: OneshotArgs) -> CommandResult {
    init_fmt_with_pretty(&cli.verbose);

    let checkout = Libgit2Checkout::new(args.checkout_config);
    let fetcher =
        DefaultTokenFetcher::new(args.github_config.clone(), args.github_app_config.clone())?;
    let handler = Handler::new(args.handler_config, NullClient, checkout, fetcher.clone());

    let token = fetcher.fetch_token().await?;
    let github_client = OctorustClient::new_with_token(args.github_config, token.clone())?;

    let head_sha = match args.head_sha {
        Some(sha) => sha,
        None => {
            github_client
                .fetch_head_sha(&args.repo_owner, &args.repo_name)
                .await?
        }
    };
    let repo = github_client
        .get_repo(&token, &args.repo_owner, &args.repo_name)
        .await?;

    let req = CheckRequest {
        request_id: "oneshot".to_owned(),
        delivery_id: "oneshot".to_owned(),
        event_name: "pull_request".to_owned(),
        action: "synchronize".to_owned(),
        head_sha: head_sha.clone(),
        base_sha: None,
        base_ref: None,
        before: None,
        after: Some(head_sha.clone()),
        pull_request_number: None,
        repository: repo,
        sender: User {
            login: "octocat".to_owned(),
        },
    };

    handler.handle_event(req).await?;

    SUCCESS
}
