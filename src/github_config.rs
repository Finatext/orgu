use anyhow::Result;
use clap::{Args, ValueEnum};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};

#[derive(Debug, Args, Clone)]
pub struct GithubAppConfig {
    /// GitHub App ID.
    #[arg(env = "GITHUB_APP_ID", long)]
    pub app_id: i64,
    /// GitHub App installation ID.
    #[arg(env = "GITHUB_INSTALLATION_ID", long)]
    pub installation_id: i64,
    /// GitHub App private key.
    #[arg(env = "GITHUB_PRIVATE_KEY", hide_env_values = true, long)]
    pub private_key: String,
}

// Default retry config is from retry-policies crate except for retry.
#[derive(Debug, Args, Clone)]
pub struct GithubApiConfig {
    /// Connect timeout for GitHub API requests.
    #[arg(env, long, default_value = "1s")]
    pub github_connect_timeout: humantime::Duration,
    /// Read timeout for GitHub API requests. Currently applied from connect to read operation.
    #[arg(env, long, default_value = "10s")]
    pub github_read_timeout: humantime::Duration,
    /// Number of retries for GitHub API requests.
    #[arg(env, long, default_value = "3")]
    pub github_max_retry: u32,
    /// Minimum interval between retries.
    #[arg(env, long, default_value = "1s")]
    pub github_min_retry_interval: humantime::Duration,
    /// Maximum interval between retries.
    #[arg(env, long, default_value = "5m")]
    pub github_max_retry_interval: humantime::Duration,
    /// Jitter configuration for retry interval.
    #[arg(env, long, default_value = "full")]
    pub github_retry_jitter: JitterConfig,
    /// Base for exponential backoff.
    #[arg(env, long, default_value = "2")]
    pub github_retry_base: u32,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum JitterConfig {
    /// Don't apply any jitter.
    None,
    /// Jitter between 0 and the calculated backoff duration.
    Full,
    /// Jitter between `min_retry_interval` and the calculated backoff duration.
    Bounded,
}

impl From<JitterConfig> for Jitter {
    fn from(jitter: JitterConfig) -> Self {
        match jitter {
            JitterConfig::None => Self::None,
            JitterConfig::Full => Self::Full,
            JitterConfig::Bounded => Self::Bounded,
        }
    }
}

pub fn reqwest_client(config: GithubApiConfig) -> Result<ClientWithMiddleware> {
    let http = reqwest::Client::builder()
        .connect_timeout(config.github_connect_timeout.into())
        .read_timeout(config.github_read_timeout.into())
        .build()?;
    let retry_policy = ExponentialBackoff::builder()
        .jitter(config.github_retry_jitter.into())
        .base(config.github_retry_base)
        .retry_bounds(
            config.github_min_retry_interval.into(),
            config.github_max_retry_interval.into(),
        )
        .build_with_max_retries(config.github_max_retry);

    Ok(ClientBuilder::new(http)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build())
}

#[cfg(test)]
mod test {
    use super::*;

    impl Default for GithubAppConfig {
        fn default() -> Self {
            Self {
                app_id: 1,
                installation_id: 0,
                private_key: "test-private-key".to_owned(),
            }
        }
    }
}
