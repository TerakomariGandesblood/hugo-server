use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{io, pin};

use anyhow::Result;
use axum::body::Bytes;
use axum::error_handling::HandleErrorLayer;
use axum::extract::{self, DefaultBodyLimit, Multipart};
use axum::http::{Request, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{BoxError, Json, Router, routing};
use axum_extra::response::file_stream::FileStream;
use futures_util::{Stream, TryStreamExt};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use my_servers::ServerError;
use serde::Serialize;
use tokio::fs::{self, File};
use tokio::io::BufWriter;
use tokio_util::io::{ReaderStream, StreamReader};
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::csrf::CsrfLayer;
use tower_http::on_early_drop::{EarlyDropsAsFailures, OnEarlyDropGuard, OnEarlyDropLayer};
use tower_http::request_id::{MakeRequestId, RequestId};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tower_http::{LatencyUnit, ServiceBuilderExt};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::Config;

const UPLOADS_DIRECTORY: &str = "uploads";

pub fn router(config: &Config) -> Result<Router> {
    let sensitive_headers: Arc<[_]> =
        vec![header::AUTHORIZATION, header::COOKIE, header::SET_COOKIE].into();

    let middleware = ServiceBuilder::new()
        .sensitive_request_headers(sensitive_headers.clone())
        .set_x_request_id(UuidRequestId)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(
                    DefaultOnResponse::new()
                        .include_headers(true)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .propagate_x_request_id()
        .layer(HandleErrorLayer::new(handle_error))
        .layer(CatchPanicLayer::new())
        .layer(OnEarlyDropLayer::new(EarlyDropsAsFailures::new(
            DefaultOnFailure::default(),
        )))
        .layer(
            CsrfLayer::new()
                .add_trusted_origin(config.server.url.to_string().trim_end_matches("/"))?,
        )
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024 * 1024))
        .sensitive_response_headers(sensitive_headers)
        .load_shed()
        .concurrency_limit(1024)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(60 * 60),
        ))
        .decompression()
        .compression();

    let router = Router::new()
        .route("/", routing::get(index))
        .route("/api/upload", routing::post(upload))
        .route("/api/list", routing::get(list))
        .route(
            &format!("/{UPLOADS_DIRECTORY}/{{file_name}}"),
            routing::get(file_stream),
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

async fn index() -> Html<&'static str> {
    Html(include_str!("./index.html"))
}

async fn upload(mut multipart: Multipart) -> Result<impl IntoResponse, ServerError> {
    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };

        stream_to_file(&file_name, field).await?;
    }

    Ok(StatusCode::CREATED)
}

async fn stream_to_file<S, E>(file_name: &str, stream: S) -> Result<(), ServerError>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    if !path_is_valid(file_name) {
        return Err(ServerError(
            StatusCode::BAD_REQUEST,
            anyhow::anyhow!("Invalid path"),
        ));
    }

    async {
        let body_with_io_error = stream.map_err(io::Error::other);
        let mut body_reader = pin::pin!(StreamReader::new(body_with_io_error));

        if !Path::new(UPLOADS_DIRECTORY).try_exists()? {
            fs::create_dir(UPLOADS_DIRECTORY).await?;
        }

        let path = Path::new(UPLOADS_DIRECTORY).join(sanitize_filename::sanitize(file_name));

        let path_clone = path.clone();
        let mut guard = OnEarlyDropGuard::new(|| {
            if path_clone.exists() {
                tracing::info!("Remove incomplete file: `{}`", path_clone.display());
                if let Err(error) = std::fs::remove_file(path_clone) {
                    tracing::error!("An error occurred while removing the file: `{error}`")
                }
            }
        });
        let mut file = BufWriter::new(File::create(path).await?);

        tokio::io::copy(&mut body_reader, &mut file).await?;
        guard.completed();

        anyhow::Ok(())
    }
    .await
    .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    Ok(())
}

fn path_is_valid(path: &str) -> bool {
    let path = std::path::Path::new(path);
    let mut components = path.components().peekable();

    if let Some(first) = components.peek()
        && !matches!(first, std::path::Component::Normal(_))
    {
        return false;
    }

    components.count() == 1
}

async fn file_stream(
    extract::Path(file_name): extract::Path<String>,
) -> Result<Response, ServerError> {
    if !path_is_valid(&file_name) {
        return Err(ServerError(
            StatusCode::BAD_REQUEST,
            anyhow::anyhow!("Invalid path"),
        ));
    }

    let file = File::open(Path::new(UPLOADS_DIRECTORY).join(&file_name))
        .await
        .map_err(|e| {
            ServerError(
                StatusCode::NOT_FOUND,
                anyhow::anyhow!("File not found: `{e}`"),
            )
        })?;

    let stream = ReaderStream::new(file);
    let file_stream_resp = FileStream::new(stream).file_name(file_name);

    Ok(file_stream_resp.into_response())
}

#[derive(Serialize)]
struct FileInfo {
    path: PathBuf,
    file_name: String,
    create_time: Zoned,
}

async fn list() -> Result<Json<Vec<FileInfo>>, ServerError> {
    let mut files = Vec::new();

    for entry in WalkDir::new(UPLOADS_DIRECTORY)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let path = entry.path().to_path_buf();
        let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
        let create_time =
            Timestamp::try_from(entry.metadata()?.created()?)?.to_zoned(TimeZone::system());

        files.push(FileInfo {
            path,
            file_name,
            create_time,
        });
    }

    Ok(Json(files))
}
