use std::path::Path;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{BoxError, Router};
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
            ServeDir::new(&path).not_found_service(ServeFile::new(path.as_ref().join("404.html"))),
        )
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .timeout(Duration::from_secs(10))
                .load_shed()
                .concurrency_limit(1024)
                .layer(RequestDecompressionLayer::new())
                .layer(CompressionLayer::new())
                .layer(TraceLayer::new_for_http()),
        )
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
        anyhow::anyhow!("Unhandled internal error: {error}"),
    )
}
