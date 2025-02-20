use std::{collections::HashMap, env, future::Future, path::Path};

use anyhow::{Context as _, Result};
use clap::Args;
use tokio::{
    process::Command,
    time::{timeout, Instant},
};
use tracing::{error, info, info_span, instrument, trace, Instrument};

use crate::{
    checkout::{Checkout, CheckoutError, CheckoutInput},
    events::CheckRequest,
    github_client::GithubClient,
    github_config::GithubAppConfig,
    github_token::TokenFetcher,
    runner::hanlder_view::{fmt_cmd, CreateInput, UpdateInputBase},
};

#[derive(Debug, Clone, Args)]
pub struct Config {
    /// Job name to be used in the check run and reviewdog annotation.
    #[clap(long, env)]
    job_name: String,
    /// Command to run. To be executed without any shell.
    #[clap(required = true, last = true, env, num_args = 1.., value_delimiter = ' ')]
    command: Vec<String>,
    /// Wrap stdout and stderr with code block in the check run output.
    #[clap(long, env, default_value = "true")]
    wrap_stdout: bool,
    /// Timeout for the command execution.
    #[clap(long, env, default_value = "10m")]
    job_timeout: humantime::Duration,
}

#[derive(Debug)]
pub struct Handler<CL: GithubClient, CH: Checkout, F: TokenFetcher> {
    config: Config,
    runner_job_name: String,
    github_config: GithubAppConfig,
    client: CL,
    checkout: CH,
    token_fetcher: F,
}

impl<CL: GithubClient, CH: Checkout, F: TokenFetcher> Handler<CL, CH, F> {
    pub fn new(
        config: Config,
        github_config: GithubAppConfig,
        client: CL,
        checkout: CH,
        fetcher: F,
    ) -> Self {
        let runner_job_name = format!("run-{}", config.job_name);
        Self {
            config,
            runner_job_name,
            github_config,
            client,
            checkout,
            token_fetcher: fetcher,
        }
    }

    #[instrument(
        skip(self, req),
        fields(
            request_id = req.request_id,
            delivery_id = req.delivery_id,
            installation_id = req.installation_id,
            owner = req.repository.owner.login, repo = req.repository.name,
            head_sha = req.head_sha, pull_request_number = req.pull_request_number.unwrap_or_default(),
        ),
    )]
    pub async fn handle_event(&self, req: CheckRequest) -> Result<()> {
        with_event_logging(req.clone(), async move { self.do_handle_event(req).await }).await
    }

    async fn do_handle_event(&self, req: CheckRequest) -> Result<()> {
        // See `docs/re-run.md` for more details.
        match (
            req.event_name.as_str(),
            req.action.as_str(),
            req.installation_id,
        ) {
            // `rerequested` events are only accepted from the same installation ID.
            ("check_suite", "rerequested", id) | ("check_run", "rerequested", id)
                if id != self.github_config.installation_id =>
            {
                info!("skipping event from different installation");
                return Ok(());
            }
            // Other events are accepted regardless of the installation ID.
            (_, _, _) => {}
        }

        let create_input = CreateInput {
            req: req.clone(),
            name: self.runner_job_name.clone(),
            command: self.config.command.clone(),
        };
        let check_run = self
            .client
            .create_check_run(
                &req.repository.owner.login,
                &req.repository.name,
                &create_input.clone().into(),
            )
            .await?;
        let update_input = create_input.into_update_input(check_run.id, self.config.wrap_stdout);

        self.ensure_updating_check_run(update_input.clone(), async move {
            let owner = &req.repository.owner.login;
            let repo = &req.repository.name;

            let token = self.token_fetcher.fetch_token().await?;
            let checkout_input = CheckoutInput {
                owner: owner.clone(),
                repo: repo.clone(),
                sha: req.head_sha.to_owned(),
                token: token.to_owned(),
            };
            let cloned = match self.checkout.create_dir_and_checkout(&checkout_input).await {
                Ok(v) => v,
                Err(e) => {
                    match e.downcast_ref::<CheckoutError>() {
                        Some(CheckoutError::Timeout(d)) => {
                            info!(duration = %d, "checkout timed out");
                            self.client
                                .update_check_run(
                                    owner,
                                    repo,
                                    check_run.id,
                                    &update_input.into_checkout_timed_out(*d),
                                )
                                .await?;
                            // Checkout timeout is not orgu failure, so early return Ok.
                            return Ok(());
                        }
                        _ => return Err(e),
                    }
                }
            };

            let cmd = self.build_command(&cloned.path, &req, &token)?;
            let span =
                info_span!("run command", command = fmt_cmd(&cmd), path = %cloned.path.display());
            self.run_command(cmd, update_input).instrument(span).await
        })
        .await
    }

    // Execute the command and update the check-run status.
    // If the command fails to execute, it's likely due to a misconfiguration, and thus, an error is returned.
    // If the command executes but fails with an exit status, it's considered a domain failure, and thus, it's handled
    // as a normal outcome.
    async fn run_command(&self, mut cmd: Command, update_input: UpdateInputBase) -> Result<()> {
        info!("running command with timeout: {}", self.config.job_timeout);
        let start = Instant::now();
        // Without strong guarantee of killing the child process.
        // https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.kill_on_drop
        cmd.kill_on_drop(true);

        let out = match timeout(self.config.job_timeout.into(), cmd.output()).await {
            Ok(res) => res.with_context(|| format!("failed to run command: {}", fmt_cmd(&cmd)))?,
            Err(_) => {
                info!(elapsed = ?start.elapsed(), timeout_config = %self.config.job_timeout, "command timed out");
                self.client
                    .update_check_run(
                        update_input.owner(),
                        update_input.repo(),
                        update_input.check_run_id,
                        &update_input
                            .clone()
                            .into_command_timed_out(self.config.job_timeout, cmd),
                    )
                    .await?;
                // Timeout of command execution is not orgu failure, so early return an Ok.
                return Ok(());
            }
        };

        if out.status.success() {
            info!(elapsed = ?start.elapsed(), "command succeeded");
        } else {
            info!(status = out.status.to_string(), elapsed = ?start.elapsed(), "command failed");
        };
        // For pretty logging newlines, don't use structured logging here.
        trace!("stdout:\n{}", String::from_utf8_lossy(&out.stdout));
        trace!("stderr:\n{}", String::from_utf8_lossy(&out.stderr));

        let input = if out.status.success() {
            update_input.clone().into_command_succeeded(cmd, &out)
        } else {
            update_input.clone().into_command_failed(cmd, &out)
        };
        // Failure of given command is not orgu failure, so just report the failure and return Ok.
        self.client
            .update_check_run(
                update_input.owner(),
                update_input.repo(),
                update_input.check_run_id,
                &input,
            )
            .await?;
        Ok(())
    }

    fn build_command(&self, work_dir: &Path, req: &CheckRequest, token: &str) -> Result<Command> {
        let (program, args) = self
            .config
            .command
            .split_first()
            .with_context(|| "empty COMMAND arg given. See --help.")?;
        let mut c = Command::new(program);
        // Default to pipe stdin etc. Not to be piped, use `wait_with_output` instead of `output`.
        // https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.output
        //
        // Add reviewdog env vars: https://github.com/reviewdog/reviewdog?tab=readme-ov-file#jenkins-with-github-pull-request-builder-plugin
        c.args(args)
            .current_dir(work_dir)
            .env_clear()
            .env("GITHUB_TOKEN", token)
            // Reviewdog env vars.
            .env("REVIEWDOG_GITHUB_API_TOKEN", token)
            .env("REVIEWDOG_SKIP_DOGHOUSE", "true")
            .env("JOB_NAME", self.config.job_name.clone())
            .env("CI_COMMIT", req.head_sha.clone())
            .env("CI_REPO_OWNER", req.repository.owner.login.clone())
            .env("CI_REPO_NAME", req.repository.name.clone())
            .env(
                "CI_PULL_REQUEST",
                req.pull_request_number
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            )
            // Other useful env vars.
            .env("CI_DELIVERY_ID", req.delivery_id.clone())
            .env("CI_REQUEST_ID", req.request_id.clone())
            .env("CI_EVENT_NAME", req.event_name.clone())
            .env("CI_EVENT_ACTION", req.action.clone())
            .env("CI_HEAD", req.head_sha.clone())
            .env(
                "CI_HEAD_REF",
                req.pull_request_head_ref.clone().unwrap_or_default(),
            )
            .env("CI_BASE", req.base_sha.clone().unwrap_or_default())
            .env("CI_BASE_REF", req.base_ref.clone().unwrap_or_default())
            .env("CI_BEFORE", req.before.clone().unwrap_or_default())
            .env("CI_AFTER", req.after.clone().unwrap_or_default());
        if let Ok(v) = env::var("PATH") {
            c.env("PATH", v);
        }
        add_custom_props(&mut c, &req.repository.custom_properties);

        Ok(c)
    }

    // We already created GitHub check_run, so in case of error, we should mark the check_run as completed with failure.
    async fn ensure_updating_check_run(
        &self,
        input: UpdateInputBase,
        f: impl Future<Output = Result<()>>,
    ) -> Result<()> {
        match f.await {
            Ok(_) => Ok(()),
            Err(e) => {
                info!(original = ?e, "updating check run as failure due to error");
                self.client
                    .update_check_run(
                        input.owner(),
                        input.repo(),
                        input.check_run_id,
                        &input.clone().into_event_handle_failed(&e),
                    )
                    .await?;
                // After successfully updating the check run, return the original error.
                Err(e)
            }
        }
    }
}

// Log errors in the Handler layer to make easier to develop error reporting in local environment.
async fn with_event_logging(req: CheckRequest, f: impl Future<Output = Result<()>>) -> Result<()> {
    info!("handling event: {:?}", req);
    match f.await {
        Ok(_) => {
            info!("event handled successfully");
            Ok(())
        }
        Err(e) => {
            error!(error = ?e, "event handling failed");
            Err(e)
        }
    }
}

// Job can refer custom properties as env vars with `CUSTOM_PROP_` prefix with upcased key.
// e.g. `CUSTOM_PROP_TEAM=t-ferris`.
fn add_custom_props(c: &mut Command, custom_props: &HashMap<String, String>) {
    for (k, v) in custom_props {
        let upcased = k.to_uppercase();
        c.env(format!("CUSTOM_PROP_{upcased}"), v);
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;

    impl Default for Config {
        fn default() -> Self {
            Self {
                job_name: Default::default(),
                command: Default::default(),
                wrap_stdout: Default::default(),
                job_timeout: Duration::from_secs(10 * 60).into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::create_dir_all, time::Duration};

    use mockall::Sequence;
    use octorust::types::{ChecksCreateRequestConclusion, ChecksUpdateRequest};
    use pretty_assertions::assert_eq;

    use crate::{
        checkout::{MockCheckout, WorkDir},
        events::{GithubRepository, User},
        github_client::{empty_checkrun, MockGithubClient},
        github_token::MockTokenFetcher,
    };

    use super::*;

    fn build_checkrequest() -> CheckRequest {
        CheckRequest {
            event_name: "pull_request".to_owned(),
            action: "synchronize".to_owned(),
            head_sha: "testsha".to_owned(),
            pull_request_number: Some(55),
            pull_request_head_ref: Some("test-branch".to_owned()),
            repository: GithubRepository {
                full_name: "owner/repo".to_owned(),
                name: "repo".to_owned(),
                owner: User {
                    login: "owner".to_owned(),
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn config() -> Config {
        Config {
            command: vec!["echo".to_owned(), "hello".to_owned()],
            ..Default::default()
        }
    }

    fn work_dir() -> WorkDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_repo");
        // Blocking create_dir_all is ok in test.
        create_dir_all(&path).unwrap();
        WorkDir { path, _parent: dir }
    }

    #[tokio::test]
    async fn ok() {
        let mut fetcher = MockTokenFetcher::new();
        fetcher
            .expect_fetch_token()
            .once()
            .returning(|| Ok("test_token".to_owned()));

        let mut seq = Sequence::new();
        let mut client = MockGithubClient::new();
        client
            .expect_create_check_run()
            .once()
            .in_sequence(&mut seq)
            .returning(|_, _, _| Ok(empty_checkrun()));

        let mut checkout = MockCheckout::new();
        checkout
            .expect_create_dir_and_checkout()
            .once()
            .in_sequence(&mut seq)
            .returning(|_| Ok(work_dir()));

        fn check_env(input: &ChecksUpdateRequest) {
            assert!(input
                .output
                .as_ref()
                .unwrap()
                .summary
                .starts_with("Command succeeded"));
            let text = &input.output.as_ref().unwrap().text;

            assert!(text.contains("GITHUB_TOKEN=test_token"));

            assert!(text.contains("REVIEWDOG_GITHUB_API_TOKEN=test_token"));
            assert!(text.contains("REVIEWDOG_SKIP_DOGHOUSE=true"));
            assert!(text.contains("JOB_NAME=test_job"));
            assert!(text.contains("CI_COMMIT=testsha"));
            assert!(text.contains("CI_REPO_OWNER=owner"));
            assert!(text.contains("CI_REPO_NAME=repo"));
            assert!(text.contains("CI_PULL_REQUEST=55"));

            assert!(text.contains("CI_HEAD=testsha"));
            assert!(text.contains("CI_HEAD_REF=test-branch"));

            assert!(text.contains("PATH="));

            assert!(text.contains("CUSTOM_PROP_TEAM=t-platform"));
            assert!(text.contains("CUSTOM_PROP_DOMAIN=d-platform"));
        }

        client
            .expect_update_check_run()
            .once()
            .in_sequence(&mut seq)
            .withf(|_, _, _, input| {
                check_env(input);
                input.conclusion == Some(ChecksCreateRequestConclusion::Success)
            })
            .returning(|_, _, _, _| Ok(empty_checkrun()));

        let config = Config {
            job_name: "test_job".to_owned(),
            command: vec!["env".to_owned()],
            ..Default::default()
        };
        let handler = Handler::new(
            config,
            GithubAppConfig::default(),
            client,
            checkout,
            fetcher,
        );

        let mut req = build_checkrequest();
        let props = &mut req.repository.custom_properties;
        props.insert("team".to_owned(), "t-platform".to_owned());
        props.insert("domain".to_owned(), "d-platform".to_owned());
        let res = handler.handle_event(req).await;
        res.unwrap();
    }

    #[tokio::test]
    async fn command_failed() {
        let mut fetcher = MockTokenFetcher::new();
        fetcher
            .expect_fetch_token()
            .returning(|| Ok("test_token".to_owned()));
        let mut client = MockGithubClient::new();
        client
            .expect_create_check_run()
            .returning(|_, _, _| Ok(empty_checkrun()));
        let mut checkout = MockCheckout::new();
        checkout
            .expect_create_dir_and_checkout()
            .returning(|_| Ok(work_dir()));

        client
            .expect_update_check_run()
            .once()
            .withf(|_, _, _, input| {
                input
                    .output
                    .as_ref()
                    .unwrap()
                    .summary
                    .starts_with("Command failed with exit status: 1: `false`")
            })
            .returning(|_, _, _, _| Ok(empty_checkrun()));

        let config = Config {
            command: vec!["false".to_owned()],
            ..Default::default()
        };
        let handler = Handler::new(
            config,
            GithubAppConfig::default(),
            client,
            checkout,
            fetcher,
        );

        let res = handler.handle_event(Default::default()).await;
        res.unwrap();
    }

    #[tokio::test]
    async fn empty_command() {
        let mut fetcher = MockTokenFetcher::new();
        fetcher
            .expect_fetch_token()
            .returning(|| Ok("test_token".to_owned()));
        let mut client = MockGithubClient::new();
        client
            .expect_create_check_run()
            .returning(|_, _, _| Ok(empty_checkrun()));
        client
            .expect_update_check_run()
            .returning(|_, _, _, _| Ok(empty_checkrun()));
        let mut checkout = MockCheckout::new();
        checkout
            .expect_create_dir_and_checkout()
            .returning(|_| Ok(work_dir()));

        let config = Config {
            command: Vec::new(),
            ..Default::default()
        };
        let handler = Handler::new(
            config,
            GithubAppConfig::default(),
            client,
            checkout,
            fetcher,
        );

        let res = handler.handle_event(Default::default()).await;
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "empty COMMAND arg given. See --help."
        );
    }

    #[tokio::test]
    async fn checkout_timedout() {
        let mut fetcher = MockTokenFetcher::new();
        fetcher
            .expect_fetch_token()
            .returning(|| Ok("test_token".to_owned()));

        let mut checkout = MockCheckout::new();
        checkout.expect_create_dir_and_checkout().returning(|_| {
            let d = Duration::from_secs(10);
            Err(CheckoutError::Timeout(d.into()).into())
        });

        let mut seq = Sequence::new();
        let mut client = MockGithubClient::new();
        client
            .expect_create_check_run()
            .once()
            .in_sequence(&mut seq)
            .returning(|_, _, _| Ok(empty_checkrun()));
        client
            .expect_update_check_run()
            .once()
            .in_sequence(&mut seq)
            .withf(|_, _, _, input| {
                input.conclusion == Some(ChecksCreateRequestConclusion::TimedOut)
            })
            .returning(|_, _, _, _| Ok(empty_checkrun()));

        let handler = Handler::new(
            config(),
            GithubAppConfig::default(),
            client,
            checkout,
            fetcher,
        );

        let res = handler.handle_event(Default::default()).await;
        // Checkout timeout is considered as success with reporting failure via Checks API.
        res.unwrap();
    }

    // In normal cases, handler should handle GitHub events from front GitHub App.
    #[tokio::test]
    async fn different_installation_id() {
        let mut fetcher = MockTokenFetcher::new();
        fetcher
            .expect_fetch_token()
            .returning(|| Ok("test_token".to_owned()));
        let mut client = MockGithubClient::new();
        client
            .expect_create_check_run()
            .returning(|_, _, _| Ok(empty_checkrun()));
        client
            .expect_update_check_run()
            .returning(|_, _, _, _| Ok(empty_checkrun()));
        let mut checkout = MockCheckout::new();
        checkout
            .expect_create_dir_and_checkout()
            .returning(|_| Ok(work_dir()));

        let config = Config {
            job_name: "test_job".to_owned(),
            command: vec!["env".to_owned()],
            ..Default::default()
        };
        let handler = Handler::new(
            config,
            GithubAppConfig {
                installation_id: 123,
                ..Default::default()
            },
            client,
            checkout,
            fetcher,
        );

        let req = build_checkrequest();
        let res = handler.handle_event(req).await;
        res.unwrap();
    }

    // check_suite.rerequested event should be handled if the installation_id is the same.
    #[tokio::test]
    async fn different_installation_id_with_check_suite_rerequested() {
        // Handler should not call any GitHub API, just skip the event.
        let fetcher = MockTokenFetcher::new();
        let client = MockGithubClient::new();
        let checkout = MockCheckout::new();

        let config = Config {
            job_name: "test_job".to_owned(),
            command: vec!["env".to_owned()],
            ..Default::default()
        };
        let handler = Handler::new(
            config,
            GithubAppConfig {
                installation_id: 123,
                ..Default::default()
            },
            client,
            checkout,
            fetcher,
        );

        let mut req = build_checkrequest();
        req.installation_id = 456;
        req.event_name = "check_suite".to_owned();
        req.action = "rerequested".to_owned();
        let res = handler.handle_event(req).await;
        res.unwrap();
    }
}
