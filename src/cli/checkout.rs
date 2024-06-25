use std::{env::current_dir, path::PathBuf};

use anyhow::Context;
use clap::Args;
use tokio::fs::create_dir_all;

use crate::{
    checkout::{Checkout as _, CheckoutConfig, CheckoutInput, Libgit2Checkout},
    cli::{Cli, CommandResult, SUCCESS},
    github_client::OctorustClient,
    github_config::GithubApiConfig,
    trace::init_fmt_with_full,
};

#[derive(Debug, Clone, Args)]
pub struct CheckoutArgs {
    /// Target SHA to checkout. If none, remote HEAD will be used.
    #[arg(long, short)]
    sha: Option<String>,
    /// GitHub App installation token. Or GitHub Personal Access Token.
    #[arg(env = "GITHUB_TOKEN", hide_env_values = true, long)]
    token: String,
    /// Checkout given repository under this path. If none, checkout in
    /// the current working direcotry.
    #[arg(long)]
    under: Option<PathBuf>,
    /// GitHub owner.
    owner: String,
    /// GitHub repository name.
    repo: String,
    #[command(flatten)]
    checkout_config: CheckoutConfig,
    #[command(flatten)]
    github_config: GithubApiConfig,
}

pub async fn checkout(cli: Cli, args: CheckoutArgs) -> CommandResult {
    init_fmt_with_full(&cli.verbose);

    let under = match args.under {
        Some(p) => p,
        None => current_dir().with_context(|| "could not get current working directory")?,
    };
    create_dir_all(&under)
        .await
        .with_context(|| format!("could not create directory: {}", under.to_string_lossy()))?;
    let sha = match args.sha {
        Some(sha) => sha,
        None => {
            let github_client =
                OctorustClient::new_with_token(args.github_config, args.token.clone())?;
            github_client
                .fetch_head_sha(&args.owner, &args.repo)
                .await?
        }
    };

    let input = CheckoutInput {
        owner: args.owner,
        repo: args.repo,
        sha,
        token: args.token.clone(),
    };
    let checkout = Libgit2Checkout::new(args.checkout_config);
    checkout.checkout_under(&input, &under).await?;

    SUCCESS
}
