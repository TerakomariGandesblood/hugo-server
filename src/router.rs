use std::path::Path;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{BoxError, Router, middleware};
use http_cache::MokaManager;
use http_cache_tower_server::ServerCacheLayer;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::decompression::RequestDecompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::ServerError;

pub fn router<T>(path: T) -> Router
where
    T: AsRef<Path>,
{
    Router::new()
        .fallback_service(
            ServiceBuilder::new()
                .layer(middleware::from_fn(set_cache_control))
                .service(
                    ServeDir::new(&path)
                        .not_found_service(ServeFile::new(path.as_ref().join("404.html"))),
                ),
        )
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                .concurrency_limit(1024)
                .timeout(Duration::from_secs(10))
                .layer(RequestDecompressionLayer::new())
                .layer(CompressionLayer::new())
                .layer(ServerCacheLayer::new(MokaManager::default())),
        )
}

async fn set_cache_control(uri: Uri, request: Request, next: Next) -> Response {
    let path = uri.path();
    let value = if path.ends_with(".html") || path.ends_with("/") || !path.contains('.') {
        "no-cache"
    } else {
        "public, max-age=86400"
    };

    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    response
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return ServerError(
            StatusCode::REQUEST_TIMEOUT,
            anyhow::anyhow!("request timed out"),
        );
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return ServerError(
            StatusCode::SERVICE_UNAVAILABLE,
            anyhow::anyhow!("service is overloaded, try again later"),
        );
    }

    ServerError(
        StatusCode::INTERNAL_SERVER_ERROR,
        anyhow::anyhow!("unhandled internal error: {error}"),
    )
}
