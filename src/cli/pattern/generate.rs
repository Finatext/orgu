use std::collections::HashMap;

use clap::Args;
use serde::Serialize;
use serde_json::to_string_pretty;

use crate::cli::{CommandResult, GlobalArgs, SUCCESS};

use super::{CustomPropsConfig, EventType};

#[derive(Debug, Clone, Args)]
pub struct GenerateArgs {
    /// GitHub wehook event name to subscribe to.
    event_type: EventType,
    #[command(flatten)]
    cps: CustomPropsConfig,
}

#[derive(Debug, Serialize)]
// Note: this JSON object is structed by AWS EventBridge, so this uses kebab-case.
#[serde(rename_all = "kebab-case")]
struct Pattern {
    source: Vec<String>,
    detail_type: Vec<String>,
    detail: Detail,
}

#[derive(Debug, Serialize)]
// Note:
// this and its child JSON objects are structed by orgu, so this uses snake_case.
// This is a subset of orgu::events::CheckRequest struct.
struct Detail {
    event_name: Vec<String>,
    action: Vec<String>,
    #[serde(skip_serializing_if = "DetailRepository::is_empty")]
    repository: DetailRepository,
}

#[derive(Debug, Serialize)]
struct DetailRepository {
    custom_properties: HashMap<String, Vec<String>>,
}

impl DetailRepository {
    fn is_empty(&self) -> bool {
        self.custom_properties.is_empty()
    }
}

pub fn generate(_global: GlobalArgs, args: GenerateArgs) -> CommandResult {
    let custom_properties = args
        .cps
        .custom_props
        .into_iter()
        .map(|(k, v)| (k, vec![v]))
        .collect();

    let source = vec!["orgu-front".to_owned()];
    let detail_type = vec!["orgu.check_request".to_owned()];
    let pattern = match args.event_type {
        EventType::PullRequest => Pattern {
            source,
            detail_type,
            detail: Detail {
                // To response to "Re-run all checks", subscribe check_suite/rerequested event.
                event_name: vec!["pull_request".to_owned(), "check_suite".to_owned()],
                action: vec![
                    // For pull_request event.
                    "opened".to_owned(),
                    "synchronize".to_owned(),
                    "reopened".to_owned(),
                    "ready_for_review".to_owned(),
                    // For check_suite event.
                    "rerequested".to_owned(),
                ],
                repository: DetailRepository { custom_properties },
            },
        },
        EventType::CheckSuite => Pattern {
            source,
            detail_type,
            detail: Detail {
                event_name: vec!["check_suite".to_owned()],
                action: vec!["requested".to_owned(), "rerequested".to_owned()],
                repository: DetailRepository { custom_properties },
            },
        },
    };

    println!("{}", to_string_pretty(&pattern)?);
    SUCCESS
}
