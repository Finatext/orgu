use std::time::Duration;

use clap::Args;

#[derive(Debug, Args, Clone)]
pub struct FrontConfig {
    /// GitHub webhook secret to verify incoming webhook requests.
    #[arg(env = "GITHUB_WEBHOOK_SECRET", hide_env_values = true, long)]
    pub webhook_secret: String,
    /// Timeout for server to process each request.
    #[arg(env, long, default_value = "15m")]
    pub server_timeout: humantime::Duration,
}

impl Default for FrontConfig {
    fn default() -> Self {
        Self {
            webhook_secret: Default::default(),
            server_timeout: Duration::from_secs(60 * 15).into(),
        }
    }
}
