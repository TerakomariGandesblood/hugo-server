use std::path::Path;
use std::time::Duration;
use std::{env, fs, io};

use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use jiff::Timestamp;
use opentelemetry::trace::TracerProvider;
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::{MetricExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::{MeterProviderBuilder, PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
use opentelemetry_semantic_conventions::SCHEMA_URL;
use opentelemetry_semantic_conventions::attribute::{DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_VERSION};
use supports_color::Stream;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_metrics::RuntimeMonitor;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, filter, fmt};
use url::Url;

pub fn init_log<T>(
    verbose: &Verbosity,
    trace_endpoint: &Url,
    metrics_endpoint: &Url,
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

    let meter_filter_layer = filter::filter_fn(|metadata| {
        if !metadata.is_event() {
            return true;
        }

        metadata.fields().iter().any(|field| {
            let name = field.name();

            if name.starts_with("monotonic_counter.")
                || name.starts_with("counter.")
                || name.starts_with("histogram.")
                || name.starts_with("gauge.")
            {
                return false;
            }

            true
        })
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
    let file_layer = fmt::Layer::new()
        .json()
        .with_writer(file_writer)
        .with_timer(JiffTimer)
        .with_ansi(false)
        .with_filter(meter_filter_layer.clone());

    let (stderr_writer, _stderr_guard) = tracing_appender::non_blocking(io::stderr());
    let stderr_layer = fmt::Layer::new()
        .with_writer(stderr_writer)
        .with_timer(JiffTimer)
        .with_ansi(supports_color::on(Stream::Stderr).is_some())
        .with_filter(meter_filter_layer);

    let tracer_provider = init_tracer_provider(trace_endpoint)?;
    let open_telemetry_layer =
        OpenTelemetryLayer::new(tracer_provider.tracer("tracing-otel-subscriber"));

    let meter_provider = init_meter_provider(metrics_endpoint)?;
    let meter_layer = MetricsLayer::new(meter_provider.clone());

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(file_layer)
        .with(stderr_layer)
        .with(open_telemetry_layer)
        .with(meter_layer)
        .init();

    let meter_task = init_meter_task();

    Ok(LogGuard {
        _file_guard,
        _stderr_guard,
        tracer_provider,
        meter_provider,
        meter_task,
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

fn init_meter_provider(endpoint: &Url) -> Result<SdkMeterProvider> {
    let exporter = MetricExporter::builder()
        .with_http()
        .with_endpoint(endpoint.to_string())
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT)
        .build()?;

    let reader = PeriodicReader::builder(exporter)
        .with_interval(Duration::from_secs(5))
        .build();

    let meter_provider = MeterProviderBuilder::default()
        .with_resource(resource()?)
        .with_reader(reader)
        .build();

    global::set_meter_provider(meter_provider.clone());

    Ok(meter_provider)
}

fn init_meter_task() -> JoinHandle<()> {
    let handle = Handle::current();
    let runtime_monitor = RuntimeMonitor::new(&handle);

    let frequency = Duration::from_secs(1);

    tokio::spawn(async move {
        for metrics in runtime_monitor.intervals() {
            let system = System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                    .with_memory(MemoryRefreshKind::nothing().with_ram()),
            );

            tracing::info!(gauge.cpu = system.global_cpu_usage());
            tracing::info!(
                gauge.memory = system.used_memory() as f32 / system.total_memory() as f32
            );

            tracing::info!(gauge.workers_count = metrics.workers_count);
            tracing::info!(gauge.total_park_count = metrics.total_park_count);
            tracing::info!(gauge.max_park_count = metrics.max_park_count);
            tracing::info!(gauge.min_park_count = metrics.min_park_count);
            tracing::info!(
                gauge.total_busy_duration = metrics.total_busy_duration.as_millis() as u64
            );
            tracing::info!(gauge.max_busy_duration = metrics.max_busy_duration.as_millis() as u64);
            tracing::info!(gauge.min_busy_duration = metrics.min_busy_duration.as_millis() as u64);
            tracing::info!(gauge.global_queue_depth = metrics.global_queue_depth);
            tracing::info!(gauge.live_tasks_count = metrics.live_tasks_count);

            tokio::time::sleep(frequency).await;
        }
    })
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
    meter_provider: SdkMeterProvider,
    meter_task: JoinHandle<()>,
}

impl Drop for LogGuard {
    fn drop(&mut self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            tracing::error!("{err}")
        }
        if let Err(err) = self.meter_provider.shutdown() {
            tracing::error!("{err}")
        }

        self.meter_task.abort();
    }
}

struct JiffTimer;

impl FormatTime for JiffTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Timestamp::now())
    }
}
