use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use octorust::auth::{Credentials, InstallationTokenGenerator, JWTCredentials};
use octorust::checks::Checks;
use octorust::repos::Repos;
use octorust::types::{CheckRun, ChecksUpdateRequestOutput, JobStatus};
use octorust::types::{ChecksCreateRequest, ChecksUpdateRequest, Output};
use reqwest::Method;
use reqwest_middleware::ClientWithMiddleware;
use tracing::info;
use url::Url;

use crate::events::GithubRepository;
use crate::github_config::{reqwest_client, GithubApiConfig, GithubAppConfig};

#[allow(clippy::indexing_slicing)] // For automock.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait GithubClient: Send + Sync {
    async fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        input: &ChecksCreateRequest,
    ) -> Result<CheckRun>;

    async fn update_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: i64,
        input: &ChecksUpdateRequest,
    ) -> Result<CheckRun>;
}

pub struct OctorustClient {
    checks: Checks,
    repos: Repos,
    http: ClientWithMiddleware,
}

impl OctorustClient {
    pub fn new(config: GithubApiConfig, app: GithubAppConfig) -> Result<Self> {
        let p =
            pem::parse(app.private_key).with_context(|| "failed to parse GitHub private key")?;
        let jwt_c = JWTCredentials::new(app.app_id, p.contents().to_owned())
            .with_context(|| "failed to create JWT credentials")?;
        let token_generator = InstallationTokenGenerator::new(app.installation_id, jwt_c);
        Self::build(config, Credentials::InstallationToken(token_generator))
    }

    pub fn new_with_token(config: GithubApiConfig, token: String) -> Result<Self> {
        Self::build(config, Credentials::Token(token))
    }

    pub async fn fetch_head_sha(&self, owner: &str, repo: &str) -> Result<String> {
        let res = self
            .repos
            .list_commits(owner, repo, "", "", "", None, None, 1, 0)
            .await?;
        let commit = res
            .body
            .first()
            .with_context(|| format!("no commits found: owner={owner}, repo={repo}"))?;
        Ok(commit.sha.to_owned())
    }

    const GITHUB_API_URL: &'static str = "https://api.github.com";
    const GITHUB_API_VERSION: &'static str = "2022-11-28";
    const OUR_USER_AGENT: &'static str = "orgu-github-client";

    // XXX: Use raw reqwest Client instead of octorust until it supports Custom Properties.
    pub async fn get_repo(&self, token: &str, owner: &str, repo: &str) -> Result<GithubRepository> {
        let path = format!("/repos/{owner}/{repo}");
        let url = Url::parse(Self::GITHUB_API_URL)?.join(&path)?;
        let req = self
            .http
            .request(Method::GET, url)
            .header("accept", "application/vnd.github+json")
            .bearer_auth(token)
            .header("x-github-api-version", Self::GITHUB_API_VERSION)
            .header("user-agent", Self::OUR_USER_AGENT);
        Ok(req.send().await?.json().await?)
    }

    fn build(config: GithubApiConfig, credential: Credentials) -> Result<Self> {
        let http = reqwest_client(config)?;
        let inner = octorust::Client::custom(
            concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")),
            credential,
            http.clone(),
        );
        // checks() clones the inner client so initializing it here to avoid cloning it multiple times.
        Ok(Self {
            checks: inner.checks(),
            repos: inner.repos(),
            http,
        })
    }
}

#[async_trait]
impl GithubClient for OctorustClient {
    async fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        input: &ChecksCreateRequest,
    ) -> Result<CheckRun> {
        info!(owner, repo, "creating check run");
        if let Some(output) = &input.output {
            validate_text_length(&output.summary)?;
            validate_text_length(&output.text)?;
        }

        self.checks
            .create(owner, repo, input)
            .await
            .with_context(|| {
                format!(
                    "failed to create check_run: owner={}, repo={}, head_sha={}",
                    owner, repo, input.head_sha
                )
            })
            .map(|r| r.body)
    }

    async fn update_check_run(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: i64,
        input: &ChecksUpdateRequest,
    ) -> Result<CheckRun> {
        info!(owner, repo, check_run_id, "updating check run");
        if let Some(output) = &input.output {
            validate_text_length(&output.summary)?;
            validate_text_length(&output.text)?;
        }

        self.checks
            .update(owner, repo, check_run_id, input)
            .await
            .with_context(|| {
                format!(
                    "failed to update check_run: owner={}, repo={}, id={}, ",
                    owner, repo, check_run_id
                )
            })
            .map(|r| r.body)
    }
}

/// A null implementation of the GithubClient trait.
/// This is for oneshot command which can't interact with check_run API.
/// To interact with check_run API, we need check_suite but for oneshot
/// command, we don't have it.
pub struct NullClient;

#[async_trait]
impl GithubClient for NullClient {
    async fn create_check_run(
        &self,
        _owner: &str,
        _repo: &str,
        _input: &ChecksCreateRequest,
    ) -> Result<CheckRun> {
        Ok(empty_checkrun())
    }

    async fn update_check_run(
        &self,
        _owner: &str,
        _repo: &str,
        _check_run_id: i64,
        _input: &ChecksUpdateRequest,
    ) -> Result<CheckRun> {
        Ok(empty_checkrun())
    }
}

pub fn into_update_request(r: ChecksCreateRequest) -> ChecksUpdateRequest {
    ChecksUpdateRequest {
        name: r.name,
        status: r.status,
        conclusion: r.conclusion,
        output: r.output.map(|o| ChecksUpdateRequestOutput {
            title: o.title,
            summary: o.summary,
            text: o.text,
            annotations: o.annotations,
            images: o.images,
        }),
        actions: r.actions,
        completed_at: r.completed_at,
        started_at: r.started_at,
        details_url: r.details_url,
        external_id: r.external_id,
    }
}

pub fn empty_checkrun() -> CheckRun {
    CheckRun {
        id: 0,
        head_sha: String::new(),
        html_url: String::new(),
        external_id: String::new(),
        details_url: String::new(),
        status: JobStatus::InProgress,
        conclusion: None,
        started_at: None,
        completed_at: None,
        output: Output {
            title: String::new(),
            summary: String::new(),
            text: String::new(),
            annotations_count: 0,
            annotations_url: String::new(),
        },
        name: String::new(),
        check_suite: None,
        app: None,
        pull_requests: Vec::new(),
        deployment: None,
        node_id: String::new(),
        url: String::new(),
    }
}

fn validate_text_length(text: &str) -> Result<()> {
    if text.len() > 65535 {
        bail!("text length must be less than 65536 characters");
    }
    Ok(())
}
