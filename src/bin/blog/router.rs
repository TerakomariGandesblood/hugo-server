use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::body::Body;
use axum::error_handling::HandleErrorLayer;
use axum::extract::MatchedPath;
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{BoxError, Router};
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::csrf::CsrfLayer;
use tower_http::on_early_drop::{EarlyDropsAsFailures, OnEarlyDropLayer};
use tower_http::request_id::{MakeRequestId, RequestId};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultOnFailure, TraceLayer};
use tracing::{Span, field};
use url::Url;
use uuid::Uuid;

pub fn router<T>(path: T, url: &Url) -> Result<Router>
where
    T: AsRef<Path>,
{
    let sensitive_headers: Arc<[_]> =
        vec![header::AUTHORIZATION, header::COOKIE, header::SET_COOKIE].into();

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &Request<Body>| {
            let name = if let Some(target) = request.extensions().get::<MatchedPath>() {
                format!("{} {}", request.method(), target.as_str())
            } else {
                request.method().to_string()
            };

            let span = tracing::debug_span!(
                "request",
                version = field::debug(request.version()),
                otel.name = name,
                http.request.method = field::display(request.method()),
                url.path = request.uri().path(),
                http.request.header = field::debug(request.headers()),
                http.route = field::Empty,
                http.response.status_code = field::Empty,
                http.response.header = field::Empty,
            );

            if let Some(route) = request.extensions().get::<MatchedPath>() {
                span.record("http.route", route.as_str().to_string());
            }

            span
        })
        .on_response(|response: &Response, _latency: Duration, span: &Span| {
            span.record(
                "http.response.status_code",
                field::display(response.status()),
            );
            span.record("http.response.header", field::debug(response.headers()));

            tracing::debug!(parent: span, "finished processing request");
        });

    let middleware = ServiceBuilder::new()
        .sensitive_request_headers(sensitive_headers.clone())
        .set_x_request_id(UuidRequestId)
        .layer(trace_layer)
        .propagate_x_request_id()
        .layer(HandleErrorLayer::new(handle_error))
        .layer(CatchPanicLayer::new())
        .layer(OnEarlyDropLayer::new(EarlyDropsAsFailures::new(
            DefaultOnFailure::default(),
        )))
        .layer(CsrfLayer::new().add_trusted_origin(url.to_string().trim_end_matches("/"))?)
        .sensitive_response_headers(sensitive_headers)
        .load_shed()
        .concurrency_limit(1024)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .decompression()
        .compression();

    let router = Router::new()
        .fallback_service(
            ServeDir::new(&path).not_found_service(ServeFile::new(path.as_ref().join("404.html"))),
        )
        .layer(middleware);

    Ok(router)
}

#[derive(Clone)]
struct UuidRequestId;

impl MakeRequestId for UuidRequestId {
    fn make_request_id<B>(&mut self, _: &Request<B>) -> Option<RequestId> {
        let request_id = Uuid::new_v4().to_string().parse().unwrap();
        Some(RequestId::new(request_id))
    }
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::load_shed::error::Overloaded>() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    StatusCode::INTERNAL_SERVER_ERROR
}
