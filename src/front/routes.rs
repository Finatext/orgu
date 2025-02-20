use std::sync::Arc;

use axum::{
    http::{header, HeaderValue, Request},
    routing::{get, post},
    Router,
};
use http::HeaderName;
use lambda_http::Context;
use tower::{Layer, ServiceBuilder};
use tower_http::{
    normalize_path::{NormalizePath, NormalizePathLayer},
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    set_header::SetRequestHeaderLayer,
    timeout::TimeoutLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::{info_span, Level};
use uuid::Uuid;

use crate::{
    event_queue_client::EventQueueClient,
    front::{
        config::FrontConfig,
        handlers::{health_check, webhook, AppState},
    },
    github_client::GithubClient,
    github_config::GithubAppConfig,
    github_verifier::DefaultVerifier,
};

pub fn build_app<EB, GH>(
    config: FrontConfig,
    event_bus_client: EB,
    github_client: GH,
    github_config: GithubAppConfig,
) -> NormalizePath<Router>
where
    EB: EventQueueClient + 'static,
    GH: GithubClient + 'static,
{
    let shared_state = Arc::new(AppState {
        config: config.clone(),
        event_bus_client,
        github_client,
        github_config,
    });

    let router = Router::new()
        .route("/hc", get(health_check))
        .route("/github/events", post(webhook::<_, _, DefaultVerifier>))
        .with_state(shared_state);

    let router = apply_middleware(router, &config);
    NormalizePathLayer::trim_trailing_slash().layer(router)
}

fn apply_middleware(router: Router, config: &FrontConfig) -> Router {
    let headers = ["x-hub-signature", "x-hub-signature-256"]
        .into_iter()
        .flat_map(str::parse)
        .chain([header::AUTHORIZATION, header::COOKIE])
        .collect::<Vec<_>>();
    let middleware = ServiceBuilder::new()
        .layer(SetSensitiveRequestHeadersLayer::new(headers))
        .layer(SetRequestIdLayer::new(
            HeaderName::from_static("x-request-id"),
            OrguReqeustIdMaker {},
        ))
        .layer(PropagateRequestIdLayer::new(HeaderName::from_static(
            "x-request-id",
        )))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request<_>| {
                    // This expects the request id is already set by the previous RequestId middleware.
                    let id = get_request_id_or_default(req);
                    info_span!(
                        "request",
                        method = %req.method(),
                        uri = %req.uri(),
                        version = ?req.version(),
                        request_id = id,
                    )
                })
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(SetRequestHeaderLayer::if_not_present(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        ))
        .layer(TimeoutLayer::new(config.server_timeout.into()));

    router.layer(middleware)
}

fn get_request_id_or_default<T>(req: &Request<T>) -> String {
    req.extensions()
        .get::<RequestId>()
        .and_then(|id| id.header_value().to_str().ok())
        .map_or_else(|| Uuid::new_v4().to_string(), ToOwned::to_owned)
}

#[derive(Debug, Clone)]
struct OrguReqeustIdMaker;

impl MakeRequestId for OrguReqeustIdMaker {
    // If we have a lambda context, use the request id from there. Otherwise, generate a new one.
    // Use Extensions::get which is more general than lambda_http::RequestExt.
    fn make_request_id<B>(&mut self, req: &Request<B>) -> Option<RequestId> {
        let id = req
            .extensions()
            .get::<Context>()
            .map_or_else(|| Uuid::new_v4().to_string(), |ctx| ctx.request_id.clone());
        id.parse().map(RequestId::new).ok()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use http::{Method, StatusCode};
    use tower::ServiceExt as _;

    use super::*;

    use crate::{event_queue_client::MockEventQueueClient, github_client::NullClient};

    fn build_default_app() -> NormalizePath<Router> {
        build_app(
            FrontConfig::default(),
            MockEventQueueClient::new(),
            NullClient,
            GithubAppConfig::default(),
        )
    }

    async fn call_app(method: Method, path: &'static str, body: Body) -> http::Response<Body> {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .body(body)
            .unwrap();
        build_default_app().oneshot(req).await.unwrap()
    }

    #[tokio::test]
    async fn routes_top() {
        let response = call_app(Method::GET, "/", Body::empty()).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn routes_hc() {
        let response = call_app(Method::GET, "/hc", Body::empty()).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn routes_github_events_get() {
        let response = call_app(Method::GET, "/github/events", Body::empty()).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn routes_github_events_post() {
        let response = call_app(Method::POST, "/github/events", Body::empty()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn normalize_path() {
        let response = call_app(Method::GET, "//hc/", Body::empty()).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn request_id() {
        let test_id = "test-request-id";
        let req = Request::builder()
            .method(Method::GET)
            .uri("/hc")
            .header("x-request-id", test_id)
            .body(Body::empty())
            .unwrap();
        let response = build_default_app().oneshot(req).await.unwrap();
        let actual = response
            .headers()
            .get("x-request-id")
            .expect("x-request-id header not found")
            .to_str()
            .unwrap();
        assert_eq!(actual, test_id);
    }
}
