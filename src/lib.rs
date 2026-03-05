mod algolia;
mod args;
mod env;
mod git;
mod log;
mod router;

pub use algolia::*;
pub use args::*;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
pub use env::*;
pub use git::*;
pub use log::*;
pub use router::*;
use serde::Serialize;
use tokio::signal;

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

pub struct ServerError(StatusCode, anyhow::Error);

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        tracing::error!("Something went wrong: {}({})", self.1, self.0);

        if self.0 == StatusCode::INTERNAL_SERVER_ERROR {
            self.0.into_response()
        } else {
            (
                self.0,
                Json(ErrorResponse {
                    message: self.1.to_string(),
                }),
            )
                .into_response()
        }
    }
}

impl<E> From<E> for ServerError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, err.into())
    }
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
