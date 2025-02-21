use std::collections::HashMap;

use aws_lambda_events::eventbridge::EventBridgeEvent;
use aws_sdk_cloudwatchevents::Client;
use clap::Args;
use indoc::printdoc;
use serde_json::to_string_pretty;
use tokio::{
    fs,
    io::{self, AsyncReadExt as _},
};

use crate::{
    cli::{CommandResult, FAILURE, GlobalArgs, SUCCESS},
    events::{CheckRequest, GithubRepository, User},
};

use super::{CustomPropsConfig, EventAction, EventType};

#[derive(Debug, Clone, Args)]
pub struct TestArgs {
    #[clap(flatten)]
    cps: CustomPropsConfig,
    /// Input file to read the example event from. Pass `-` to read from stdin.
    #[arg(short, long, default_value = "-")]
    file: String,
    /// Prints the example event to stdout and exits. Does not call test-event-pattern API on AWS.
    #[arg(short, long, default_value = "false")]
    print_only: bool,
    /// GitHub wehook event name for the example event.
    #[arg(short, long, default_value = "pull_request")]
    name: EventType,
    /// GitHub wehook event action for the example event.
    #[arg(short, long, default_value = "synchronize")]
    action: EventAction,
    /// GitHub repository owner for the example event.
    #[arg(short, long, default_value = "Finatext")]
    owner: String,
    /// GitHub repository name for the example event.
    #[arg(short, long, default_value = "orgu")]
    repo: String,
    /// GitHub login name as the sender for the example event.
    #[arg(short, long, default_value = "ferris")]
    sender: String,
}

pub async fn test(global: GlobalArgs, args: TestArgs) -> CommandResult {
    let custom_props = args.cps.custom_props.clone().into_iter().collect();
    let req = example_check_request(args.clone(), custom_props);
    let ev = example_eventbridge_event(req);
    let event_json = to_string_pretty(&ev)?;

    if args.print_only {
        println!("{}", event_json);
        return SUCCESS;
    }

    let input = if args.file == "-" {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).await?;
        buffer
    } else {
        fs::read_to_string(args.file).await?
    };

    let sdk_config = aws_config::load_from_env().await;
    let client = Client::new(&sdk_config);
    let res = client
        .test_event_pattern()
        .event(event_json.clone())
        .event_pattern(input.clone())
        .send()
        .await?;

    // Don't print and exits early.
    if global.verbose.is_silent() {
        if res.result {
            return SUCCESS;
        } else {
            return FAILURE;
        }
    }

    if res.result {
        printdoc! {"
          Event match the pattern.

          Test event:
          {event_json}

          Given pattern:
          {input}
      "};

        SUCCESS
    } else {
        printdoc! {"
          Event does not match the pattern.

          Test event:
          {event_json}

          Given pattern:
          {input}
      "};

        FAILURE
    }
}

fn example_eventbridge_event(check_request: CheckRequest) -> EventBridgeEvent<CheckRequest> {
    EventBridgeEvent {
        version: Some("0".to_owned()),
        id: Some("dc3640c3-4bd0-4a6a-8923-b6f82c859797".to_owned()),
        detail_type: "orgu.check_request".to_owned(),
        source: "orgu-front".to_owned(),
        account: Some("012345678901".to_owned()),
        time: "2024-01-01T12:29:26Z".parse().ok(),
        region: Some("ap-northeast-1".to_owned()),
        resources: Some(Vec::new()),
        detail: check_request,
    }
}

fn example_check_request(args: TestArgs, custom_props: HashMap<String, String>) -> CheckRequest {
    let pr_number = match args.name {
        EventType::PullRequest => Some(5),
        _ => None,
    };
    CheckRequest {
        request_id: "45771944-d356-4540-a0b7-b6dff7637f8d".to_owned(),
        delivery_id: "dc3640c3-4bd0-4a6a-8923-b6f82c859797".to_owned(),
        installation_id: 123456,
        event_name: args.name.to_string(),
        action: args.action.to_string(),
        repository: GithubRepository {
            full_name: format!("{}/{}", args.owner, args.repo),
            name: args.repo,
            private: true,
            owner: User { login: args.owner },
            custom_properties: custom_props,
        },
        head_sha: "a8619f1cf1f6ade02df413b18265f74d3bc9caca".to_owned(),
        base_sha: None,
        base_ref: None,
        before: None,
        after: Some("a8619f1cf1f6ade02df413b18265f74d3bc9caca".to_owned()),
        pull_request_number: pr_number,
        pull_request_head_ref: Some("feature".to_owned()),
        sender: User { login: args.sender },
    }
}
