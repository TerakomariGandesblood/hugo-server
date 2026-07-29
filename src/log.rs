use std::path::Path;
use std::{env, fs, io};

use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use jiff::Timestamp;
use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
use opentelemetry_semantic_conventions::SCHEMA_URL;
use opentelemetry_semantic_conventions::attribute::{DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_VERSION};
use supports_color::Stream;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use url::Url;

pub fn init_log<T>(
    verbose: &Verbosity,
    opentelemetry_endpoint: &Url,
    log_directory: T,
) -> Result<LogGuard>
where
    T: AsRef<Path>,
{
    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if verbose.is_silent() {
            "none".into()
        } else {
            format!(
                "{1}={0},blog_server={0},file_server={0},tower_http={0},axum::rejection=trace",
                verbose.filter(),
                env!("CARGO_CRATE_NAME"),
            )
            .into()
        }
    });

    if !log_directory.as_ref().try_exists()? {
        fs::create_dir_all(&log_directory)?;
    }

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(service_name()?)
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_directory)?;

    let (file_writer, _file_guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = Layer::new()
        .json()
        .with_writer(file_writer)
        .with_timer(JiffTimer)
        .with_ansi(false);

    let (stderr_writer, _stderr_guard) = tracing_appender::non_blocking(io::stderr());
    let stderr_layer = Layer::new()
        .with_writer(stderr_writer)
        .with_timer(JiffTimer)
        .with_ansi(supports_color::on(Stream::Stderr).is_some());

    let tracer_provider = init_tracer_provider(opentelemetry_endpoint)?;
    let open_telemetry_layer =
        OpenTelemetryLayer::new(tracer_provider.tracer("tracing-otel-subscriber"));

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(file_layer)
        .with(stderr_layer)
        .with(open_telemetry_layer)
        .init();

    Ok(LogGuard {
        _file_guard,
        _stderr_guard,
        tracer_provider,
    })
}

fn init_tracer_provider(endpoint: &Url) -> Result<SdkTracerProvider> {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.to_string())
        .with_protocol(Protocol::Grpc)
        .with_timeout(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT)
        .build()?;

    Ok(SdkTracerProvider::builder()
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
            1.0,
        ))))
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource()?)
        .with_batch_exporter(exporter)
        .build())
}

fn service_name() -> Result<String> {
    let service_name = env::current_exe()?
        .file_stem()
        .unwrap()
        .to_str()
        .expect("the file name is not in valid UTF-8")
        .to_string();

    Ok(service_name)
}

fn resource() -> Result<Resource> {
    Ok(Resource::builder()
        .with_service_name(service_name()?)
        .with_schema_url(
            [
                KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
                KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, "develop"),
            ],
            SCHEMA_URL,
        )
        .build())
}

pub struct LogGuard {
    _file_guard: WorkerGuard,
    _stderr_guard: WorkerGuard,
    tracer_provider: SdkTracerProvider,
}

impl Drop for LogGuard {
    fn drop(&mut self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            eprintln!("{err:?}");
        }
    }
}

struct JiffTimer;

impl FormatTime for JiffTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Timestamp::now())
    }
}
