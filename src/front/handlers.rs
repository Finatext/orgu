mod health_check;
mod webhook;

pub use health_check::health_check;
pub use webhook::webhook;

use crate::{
    event_queue_client::EventQueueClient, front::config::FrontConfig, github_client::GithubClient,
    github_config::GithubAppConfig,
};

#[derive(Debug)]
pub struct AppState<EB: EventQueueClient, GH: GithubClient> {
    pub config: FrontConfig,
    pub event_bus_client: EB,
    pub github_client: GH,
    pub github_config: GithubAppConfig,
}
