use anyhow::{bail, Context as _, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::{Method, Response, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::debug;
use url::Url;

use crate::github_config::{reqwest_client, GithubApiConfig, GithubAppConfig};

#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    iat: i64,
    exp: i64,
    aud: String,
}

#[derive(Debug, Deserialize)]
struct InstallationAccessTokenResponse {
    token: String,
}

#[allow(clippy::indexing_slicing)]
#[cfg_attr(test, mockall::automock)]
pub trait TokenFetcher {
    async fn fetch_token(&self) -> Result<String>;
}

// ClientWithMiddleware can be cloned, it's like Arc::clone.
#[derive(Debug, Clone)]
pub struct DefaultTokenFetcher {
    client: ClientWithMiddleware,
    config: GithubAppConfig,
}

const GITHUB_API_URL: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2022-11-28";
const OUR_USER_AGENT: &str = "orgu-github-client";

impl TokenFetcher for DefaultTokenFetcher {
    async fn fetch_token(&self) -> Result<String> {
        self.do_fetch_token().await
    }
}

impl DefaultTokenFetcher {
    pub fn new(config: GithubApiConfig, app: GithubAppConfig) -> Result<Self> {
        Ok(Self {
            client: reqwest_client(config)?,
            config: app,
        })
    }

    /// Fetch installation access token from GitHub App private key.
    /// Use this method before making actual API requests to GitHub.
    pub async fn do_fetch_token(&self) -> Result<String> {
        let id = self.config.installation_id;
        let jwt = self.jwt()?;

        let res = self
            .post(&jwt, &format!("/app/installations/{id}/access_tokens"))
            .await?;
        let status = res.status();
        let body = res.bytes().await?;
        if status != StatusCode::CREATED {
            bail!(
                "failed to fetch installation access token: code={status}, body:\n{}",
                String::from_utf8_lossy(&body)
            );
        }
        let r = serde_json::from_slice::<InstallationAccessTokenResponse>(&body)?;
        Ok(r.token)
    }

    fn jwt(&self) -> Result<String> {
        let now = Utc::now();
        let claims = Claims {
            iss: self.config.app_id.to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::try_minutes(10).with_context(|| "")?).timestamp(),
            aud: format!(
                "{GITHUB_API_URL}/app/installations/{}",
                self.config.installation_id
            ),
        };
        let header = Header::new(Algorithm::RS256);
        let key = EncodingKey::from_rsa_pem(self.config.private_key.as_bytes())
            .with_context(|| "failed to parse GitHub private key")?;
        Ok(encode(&header, &claims, &key)?)
    }

    // `token` can be JWT or Installation Access Token.
    async fn post(&self, token: &str, path: &str) -> Result<Response> {
        self.fetch::<Value>(token, Method::POST, path, &None).await
    }

    // https://docs.rs/backoff/latest/backoff/index.html
    async fn fetch<S: Serialize>(
        &self,
        token: &str,
        method: Method,
        path: &str,
        body: &Option<S>,
    ) -> Result<Response> {
        let url = Url::parse(GITHUB_API_URL)?.join(path)?;
        debug!("TokenFetcher sending HTTP {method} request to {url}");
        let mut req = self
            .client
            .request(method, url)
            .header("accept", "application/vnd.github+json")
            .bearer_auth(token)
            .header("x-github-api-version", GITHUB_API_VERSION)
            .header("user-agent", OUR_USER_AGENT);
        if let Some(b) = body {
            req = req.json(&b);
        }

        Ok(req.send().await?)
    }
}
