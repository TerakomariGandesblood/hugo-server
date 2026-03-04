use std::path::Path;
use std::{env, fs};

use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use supports_color::Stream;
use tracing::subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_log::LogTracer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::layer::SubscriberExt;

pub fn init_log<T>(verbose: &Verbosity, log_file_path: T, app_name: &str) -> Result<WorkerGuard>
where
    T: AsRef<Path>,
{
    if env::var("RUST_LOG").is_err() {
        if verbose.is_silent() {
            unsafe {
                env::set_var("RUST_LOG", "none");
            }
        } else {
            LogTracer::init()?;
            unsafe {
                env::set_var(
                    "RUST_LOG",
                    format!(
                        "none,tower_http={0},axum={0},{1}={0}",
                        verbose.filter(),
                        env!("CARGO_CRATE_NAME")
                    ),
                );
            }
        }
    }

    if !log_file_path.as_ref().try_exists()? {
        fs::create_dir_all(&log_file_path)?;
    }

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(format!("{}.log", app_name))
        .max_log_files(7)
        .build(log_file_path)?;
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let layer = Layer::default()
        .with_writer(file_writer)
        .with_timer(ChronoLocal::rfc_3339())
        .with_ansi(false);

    let subscriber = tracing_subscriber::fmt()
        .with_timer(ChronoLocal::rfc_3339())
        .with_env_filter(EnvFilter::from_default_env())
        .with_ansi(supports_color::on(Stream::Stdout).is_some())
        .finish()
        .with(layer);

    subscriber::set_global_default(subscriber)?;

    Ok(guard)
}
