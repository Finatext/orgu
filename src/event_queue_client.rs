use std::str::from_utf8;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use aws_config::timeout::TimeoutConfigBuilder;
use aws_sdk_cloudwatchevents::{types::PutEventsRequestEntry, Client as CwClient};
use clap::Args;
use reqwest::Client as HttpClient;
use tracing::{info, instrument};
use url::Url;

use crate::events::CheckRequest;

/// Event queue client to send and fan-out events to downstream runners.
/// AWS EventBridge Event Bus Client or relay server client.
#[allow(clippy::indexing_slicing)] // For automock.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait EventQueueClient: Sync + Send {
    async fn send(&self, req: CheckRequest) -> Result<()>;
}

// Prefixed with `event_bus_` to avoid conflict with `GithubApiConfig`.
#[derive(Debug, Clone, Args)]
#[group()]
pub struct AwsEventBusConfig {
    /// The name of the EventBridge event bus to send events.
    #[arg(env, long, default_value = "default")]
    pub event_bus_name: String,
    /// Timeout for connecting to the event bus.
    /// See more detail on: https://docs.rs/aws-config/latest/aws_config/timeout/struct.TimeoutConfigBuilder.html
    /// To customize retry, see: https://docs.aws.amazon.com/sdk-for-rust/latest/dg/retries.html
    #[arg(env, long)]
    pub event_bus_connect_timeout: Option<humantime::Duration>,
    #[arg(env, long)]
    pub event_bus_read_timeout: Option<humantime::Duration>,
    #[arg(env, long)]
    pub event_bus_operation_timeout: Option<humantime::Duration>,
    #[arg(env, long)]
    pub event_bus_operation_attempt_timeout: Option<humantime::Duration>,
}

#[derive(Debug)]
pub struct AwsEventBusClient {
    inner: CwClient,
    event_bus_name: String,
}

impl AwsEventBusClient {
    pub async fn new(config: AwsEventBusConfig) -> Self {
        let mut timeout = TimeoutConfigBuilder::new();
        if let Some(d) = config.event_bus_connect_timeout {
            timeout = timeout.connect_timeout(d.into());
        }
        if let Some(d) = config.event_bus_read_timeout {
            timeout = timeout.read_timeout(d.into());
        }
        if let Some(d) = config.event_bus_operation_timeout {
            timeout = timeout.operation_timeout(d.into());
        }
        if let Some(d) = config.event_bus_operation_attempt_timeout {
            timeout = timeout.operation_attempt_timeout(d.into());
        }

        let sdk_config = aws_config::load_from_env().await;
        let mut builder = sdk_config.into_builder();
        builder.set_timeout_config(Some(timeout.build()));
        Self {
            inner: CwClient::new(&builder.build()),
            event_bus_name: config.event_bus_name,
        }
    }
}

const EVENT_SOURCE: &str = "orgu-front";
const EVENT_TYPE: &str = "orgu.check_request";

#[async_trait]
impl EventQueueClient for AwsEventBusClient {
    // https://docs.rs/aws-sdk-cloudwatchevents/latest/aws_sdk_cloudwatchevents/types/struct.PutEventsRequestEntry.html
    //
    // To propagate trace context, see: https://docs.rs/aws-sdk-cloudwatchevents/latest/aws_sdk_cloudwatchevents/client/customize/index.html
    #[instrument(skip_all, fields(event_bus_name = %self.event_bus_name))]
    async fn send(&self, req: CheckRequest) -> Result<()> {
        info!("sending event to AWS Event Bus");
        let detail =
            serde_json::to_string(&req).with_context(|| "serializing CheckRequest failed")?;
        let input = PutEventsRequestEntry::builder()
            .set_event_bus_name(Some(self.event_bus_name.clone()))
            .set_source(Some(EVENT_SOURCE.to_owned()))
            .set_detail(Some(detail))
            .set_detail_type(Some(EVENT_TYPE.to_owned()))
            .build();
        let out = self
            .inner
            .put_events()
            .entries(input)
            .send()
            .await
            .with_context(|| "sending event to AWS Event Bus failed")?;
        if out.failed_entry_count > 0 {
            bail!(
                "event sent to AWS Event Bus but failed: failed_count={}",
                out.failed_entry_count
            );
        }
        out.entries.into_iter().flatten().for_each(|e| {
            info!(
                "event sent to AWS Event Bus: id={}",
                e.event_id.unwrap_or_default()
            );
        });

        Ok(())
    }
}

#[derive(Debug)]
pub struct EventQueueRelayConfig {
    pub endpoint: Url,
}

#[derive(Debug)]
pub struct EventQueueRelayClient {
    inner: HttpClient,
    url: Url,
}

impl EventQueueRelayClient {
    pub fn new(config: EventQueueRelayConfig) -> Self {
        Self {
            inner: HttpClient::new(),
            url: config.endpoint,
        }
    }
}

#[async_trait]
impl EventQueueClient for EventQueueRelayClient {
    #[instrument(skip_all, fields(url = %self.url))]
    async fn send(&self, req: CheckRequest) -> Result<()> {
        info!("sending event to local server");
        let response = self
            .inner
            .post(self.url.clone())
            .json(&req)
            .send()
            .await
            .with_context(|| format!("sending event failed: uri={}", self.url))?;

        let status = response.status();
        let body = response.bytes().await.with_context(|| {
            format!(
                "reading response body failed: uri={}, status={status}",
                self.url
            )
        })?;

        if status.is_success() {
            Ok(())
        } else {
            bail!(
                "event sent but response failure: uri={}, status={status}, body={}",
                self.url,
                from_utf8(&body)?
            )
        }
    }
}
