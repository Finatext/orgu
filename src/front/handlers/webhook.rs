use std::sync::Arc;

use anyhow::{Context as _, Result};
use axum::{extract::State, response::IntoResponse};
use http::{HeaderMap, StatusCode};
use octorust::types::{
    ChecksCreateRequest, ChecksCreateRequestConclusion, ChecksUpdateRequestOutput, JobStatus,
};
use serde_json::from_str;
use tracing::{Span, field::Empty, info, instrument, warn};

use crate::{
    app_error::AppError,
    event_queue_client::EventQueueClient,
    events::GithubRepository,
    front::{
        github_events::{
            CheckRunEvent, CheckSuiteEvent, GithubEvent, PullRequestEvent, WebhookCommonFields,
        },
        handlers::AppState,
    },
    github_client::{GithubClient, into_update_request},
    github_verifier::GithubRequestVerifier,
};

const CHECK_RUN_NAME: &str = "orgu-trigger";
const SUPPORTED_EVENTS: &[(&str, &[&str])] = &[
    ("ping", &[]),
    ("check_suite", &["requested", "rerequested"]),
    ("check_run", &["rerequested"]),
    (
        "pull_request",
        &["opened", "synchronize", "reopened", "ready_for_review"],
    ),
];

#[instrument(
    skip_all,
    fields(
        delivery_id = Empty,
        event_name = Empty,
        installation_id = Empty,
        action = Empty,
        owner = Empty,
        repo = Empty,
    )
)]
pub async fn webhook<EB, GH, V>(
    headers: HeaderMap,
    State(state): State<Arc<AppState<EB, GH>>>,
    body: String,
) -> Result<impl IntoResponse, AppError>
where
    EB: EventQueueClient,
    GH: GithubClient,
    V: GithubRequestVerifier,
{
    if let Err(e) = V::verify_request(&headers, &body, &state.config.webhook_secret) {
        warn!("Request verification failed: {e}");
        return Err(AppError::AuthorizationError);
    }

    let delivery_id = get_header_str(&headers, "x-github-delivery")?;
    Span::current().record("delivery_id", delivery_id);
    let event_name = get_header_str(&headers, "x-github-event")?;
    Span::current().record("event_name", event_name);
    let supported_actions = match SUPPORTED_EVENTS
        .iter()
        .find(|(name, _)| name == &event_name)
    {
        None => {
            info!("unsupported event type");
            return Ok((
                StatusCode::OK,
                format!("Unsupported event type, skipping: {event_name}"),
            ));
        }
        Some(ev) => ev.1,
    };
    if event_name == "ping" {
        return Ok((StatusCode::OK, "pong".to_owned()));
    }

    let event = from_str::<WebhookCommonFields>(&body).with_context(|| {
        format!("failed to parse payload to common event type: event={event_name}, body:\n{body}")
    })?;
    Span::current().record("action", &event.action);
    Span::current().record("installation_id", event.installation.id);
    Span::current().record("owner", &event.repository.owner.login);
    Span::current().record("repo", &event.repository.name);

    if !supported_actions.contains(&event.action.as_ref()) {
        info!("action not supported");
        return Ok((
            StatusCode::OK,
            format!("Unsupported event action, skipping: {}", event.action),
        ));
    }

    if !event.repository.private {
        info!("skipping public repository");
        return Ok((StatusCode::OK, "Public repository, skipping".to_owned()));
    }

    // See `docs/re-run.md` for more details.
    match (event_name, event.action.as_str(), event.installation.id) {
        // `rerequested` events are always accepted regardless of the installation ID.
        ("check_suite", "rerequested", _) | ("check_run", "rerequested", _) => (),
        // Other events are accepted only if the installation ID matches.
        (_, _, installation_id) if installation_id == state.github_config.installation_id => (),
        (_, _, installation_id) => {
            info!(
                ours = state.github_config.installation_id,
                theirs = installation_id,
                "skipping different installation"
            );
            return Ok((
                StatusCode::OK,
                "Different installation, skipping".to_owned(),
            ));
        }
    }

    let repository = event.repository;
    // We must allocate objects for Future pinning problem.
    let event: Box<dyn GithubEvent + Send + Sync> = match event_name {
        "check_run" => Box::new(parse_as::<CheckRunEvent>("check_run", event_name, &body)?),
        "check_suite" => Box::new(parse_as::<CheckSuiteEvent>(
            "check_suite",
            event_name,
            &body,
        )?),
        "pull_request" => Box::new(parse_as::<PullRequestEvent>(
            "pull_request",
            event_name,
            &body,
        )?),
        _ => return Err(anyhow::anyhow!("unsupported event type: {event_name}").into()),
    };

    let request_id = get_header_str(&headers, "x-request-id")?;
    let req = event.build_check_request(request_id.to_owned(), delivery_id.to_owned());
    info!("publishing event");
    state.event_bus_client.send(req).await?;

    // Creating checkrun can fail so ignore the error because it's not must-have.
    if let Err(e) = report_via_check_run(&state, event, &repository, delivery_id, request_id).await
    {
        warn!(error = ?e, "failed to report via check_run API and safely ignored");
        return Ok((
            StatusCode::OK,
            "failed to report via check_run API and safely ignored".to_owned(),
        ));
    }

    Ok((StatusCode::OK, "ok".to_owned()))
}

fn parse_as<T: for<'de> serde::Deserialize<'de>>(
    event_as: &str,
    event_name: &str,
    body: &str,
) -> Result<T> {
    from_str::<T>(body).with_context(|| {
        format!("failed to parse payload as {event_as} event: event={event_name}, body:\n{body}")
    })
}

fn get_header_str<'hdr>(headers: &'hdr HeaderMap, key: &str) -> Result<&'hdr str> {
    headers
        .get(key)
        .with_context(|| format!("missing {key} header field"))?
        .to_str()
        .map_err(Into::into)
}

async fn report_via_check_run<EB: EventQueueClient, GH: GithubClient>(
    state: &AppState<EB, GH>,
    event: Box<dyn GithubEvent + Send + Sync>,
    repository: &GithubRepository,
    delivery_id: &str,
    requiest_id: &str,
) -> Result<()> {
    let input = ChecksCreateRequest {
        name: CHECK_RUN_NAME.to_owned(),
        head_sha: event.head_sha().to_owned(),
        status: Some(JobStatus::InProgress),
        conclusion: None,
        output: None,
        actions: Default::default(),
        completed_at: None,
        started_at: None,
        details_url: Default::default(),
        external_id: Default::default(),
    };
    let owner = &repository.owner.login;
    let repo = &repository.name;
    let res = state
        .github_client
        .create_check_run(owner, repo, &input)
        .await?;

    let mut input = into_update_request(input);
    input.status = Some(JobStatus::Completed);
    input.conclusion = Some(ChecksCreateRequestConclusion::Success);
    input.output = Some(ChecksUpdateRequestOutput {
        title: "orgu-front queued".to_owned(),
        summary: format!(
            "Delivery ID (not unique for re-delivery): {delivery_id}\nRequest ID (unique for re-delivery): {requiest_id}"
        ),
        text: Default::default(),
        annotations: Default::default(),
        images: Default::default(),
    });

    state
        .github_client
        .update_check_run(owner, repo, res.id, &input)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::bail;
    use axum::{Router, routing::post};
    use axum_test::{TestResponse, TestServer};
    use serde::Serialize;

    use crate::{
        event_queue_client::{EventQueueClient, MockEventQueueClient},
        events::CheckRequest,
        front::{
            config::FrontConfig,
            github_events::{CheckSuiteEvent, Installation, PullRequestEvent},
        },
        github_client::{MockGithubClient, empty_checkrun},
        github_verifier::test::NullVerifier,
    };

    use super::*;

    fn init_state<EB, GH>(eb: EB, gh: GH) -> Arc<AppState<EB, GH>>
    where
        EB: EventQueueClient,
        GH: GithubClient,
    {
        Arc::new(AppState {
            config: FrontConfig {
                webhook_secret: "test_secret".to_owned(),
                server_timeout: Duration::from_secs(0).into(),
            },
            event_bus_client: eb,
            github_client: gh,
            github_config: Default::default(),
        })
    }

    fn init_state_never() -> Arc<AppState<MockEventQueueClient, MockGithubClient>> {
        let mut mock_event_bus_client = MockEventQueueClient::new();
        mock_event_bus_client.expect_send().never();
        let mut mock_github_client = MockGithubClient::new();
        mock_github_client.expect_create_check_run().never();
        init_state(mock_event_bus_client, mock_github_client)
    }

    async fn call<J: ?Sized + Serialize>(
        state: Arc<AppState<MockEventQueueClient, MockGithubClient>>,
        mut headers: HeaderMap,
        body: &J,
    ) -> Result<TestResponse> {
        let path = "/github/events";
        let app = Router::new()
            .route(path, post(webhook::<_, _, NullVerifier>))
            .with_state(state);
        let mut server = TestServer::new(app)?;
        headers.insert("x-github-delivery", "test".parse().unwrap());
        headers.insert("x-request-id", "test".parse().unwrap());
        headers.into_iter().for_each(|(k, v)| {
            server.add_header(k.unwrap(), v);
        });
        let req = server.post(path);
        Ok(req.json(body).await)
    }

    // vefify_ng case is in routes.rs

    #[tokio::test]
    async fn invalid_request_body() -> Result<()> {
        let res = call(init_state_never(), Default::default(), "invalid json").await?;
        res.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn unsupported_event_type() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "test".parse().unwrap());
        let res = call(init_state_never(), headers, "").await?;
        res.assert_status_ok();
        res.assert_text("Unsupported event type, skipping: test");
        Ok(())
    }

    #[tokio::test]
    async fn ping() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "ping".parse().unwrap());
        let res = call(init_state_never(), headers, "").await?;
        res.assert_status_ok();
        res.assert_text("pong");
        Ok(())
    }

    #[tokio::test]
    async fn unsupported_action() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "pull_request".parse().unwrap());
        let payload = WebhookCommonFields {
            action: "test".to_owned(),
            ..Default::default()
        };
        let res = call(init_state_never(), headers, &payload).await?;
        res.assert_status_ok();
        res.assert_text("Unsupported event action, skipping: test");
        Ok(())
    }

    #[tokio::test]
    async fn different_installation_id() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "pull_request".parse().unwrap());
        let payload = WebhookCommonFields {
            action: "synchronize".to_owned(),
            repository: GithubRepository {
                private: true,
                ..Default::default()
            },
            installation: Installation { id: 999 },
            ..Default::default()
        };
        let res = call(init_state_never(), headers, &payload).await?;
        res.assert_status_ok();
        res.assert_text("Different installation, skipping");
        Ok(())
    }

    #[tokio::test]
    async fn different_installation_id_with_check_suite_rerequested() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "check_suite".parse().unwrap());
        let payload = CheckSuiteEvent {
            common: WebhookCommonFields {
                action: "rerequested".to_owned(),
                repository: GithubRepository {
                    private: true,
                    ..Default::default()
                },
                installation: Installation { id: 999 },
                ..Default::default()
            },
            ..Default::default()
        };

        let mut mock_event_bus_client = MockEventQueueClient::new();
        mock_event_bus_client
            .expect_send()
            .withf(|req: &CheckRequest| req.action == "rerequested" && req.installation_id == 999)
            .once()
            .returning(|_| Ok(()));
        let mut mock_github_client = MockGithubClient::new();
        mock_github_client
            .expect_create_check_run()
            .once()
            .returning(|_, _, _| Ok(empty_checkrun()));
        mock_github_client
            .expect_update_check_run()
            .once()
            .returning(|_, _, _, _| Ok(empty_checkrun()));
        let state = init_state(mock_event_bus_client, mock_github_client);

        // Keep state alive to not drop mock objects. The mock expectation will be checked on dropping.
        let res = call(Arc::clone(&state), headers, &payload).await?;
        res.assert_status_ok();
        res.assert_text("ok");
        Ok(())
    }

    #[tokio::test]
    async fn public_repository() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "pull_request".parse().unwrap());
        let payload = WebhookCommonFields {
            action: "synchronize".to_owned(),
            repository: GithubRepository {
                private: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let res = call(init_state_never(), headers, &payload).await?;
        res.assert_status_ok();
        res.assert_text("Public repository, skipping");
        Ok(())
    }

    #[tokio::test]
    async fn pull_request() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "pull_request".parse().unwrap());
        let payload = PullRequestEvent {
            common: WebhookCommonFields {
                action: "synchronize".to_owned(),
                repository: GithubRepository {
                    private: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let mut mock_event_bus_client = MockEventQueueClient::new();
        mock_event_bus_client
            .expect_send()
            .once()
            .returning(|_| Ok(()));
        let mut mock_github_client = MockGithubClient::new();
        mock_github_client
            .expect_create_check_run()
            .once()
            .returning(|_, _, _| Ok(empty_checkrun()));
        mock_github_client
            .expect_update_check_run()
            .once()
            .returning(|_, _, _, _| Ok(empty_checkrun()));
        let state = init_state(mock_event_bus_client, mock_github_client);

        let res = call(Arc::clone(&state), headers, &payload).await?;
        res.assert_status_ok();
        res.assert_text("ok");
        Ok(())
    }

    #[tokio::test]
    async fn success_if_github_api_fails() -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "pull_request".parse().unwrap());
        let payload = PullRequestEvent {
            common: WebhookCommonFields {
                action: "synchronize".to_owned(),
                repository: GithubRepository {
                    private: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let mut mock_event_bus_client = MockEventQueueClient::new();
        mock_event_bus_client
            .expect_send()
            .once()
            .returning(|_| Ok(()));
        let mut mock_github_client = MockGithubClient::new();
        mock_github_client
            .expect_create_check_run()
            .once()
            .returning(|_, _, _| Ok(empty_checkrun()));
        mock_github_client
            .expect_update_check_run()
            .once()
            .returning(|_, _, _, _| bail!("fail"));
        let state = init_state(mock_event_bus_client, mock_github_client);

        let res = call(Arc::clone(&state), headers, &payload).await?;
        res.assert_status_ok();
        res.assert_text("failed to report via check_run API and safely ignored");
        Ok(())
    }
}
