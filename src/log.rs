use std::path::Path;
use std::{env, fs, io};

use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use jiff::Timestamp;
use supports_color::Stream;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init_log<T>(verbose: &Verbosity, log_directory: T) -> Result<WorkerGuard>
where
    T: AsRef<Path>,
{
    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if verbose.is_silent() {
            "none".into()
        } else {
            format!(
                "{}={1},tower_http={1},axum::rejection=trace",
                env!("CARGO_CRATE_NAME"),
                verbose.filter(),
            )
            .into()
        }
    });

    let log_file_name = env::current_exe()?
        .with_extension("log")
        .file_name()
        .unwrap()
        .to_str()
        .expect("the file name is not in valid UTF-8")
        .to_string();

    if !log_directory.as_ref().try_exists()? {
        fs::create_dir_all(&log_directory)?;
    }

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(log_file_name)
        .max_log_files(7)
        .build(log_directory)?;

    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = Layer::new()
        .with_writer(file_writer)
        .with_timer(JiffTimer)
        .with_ansi(false);

    let stdout_layer = Layer::new()
        .with_writer(io::stdout)
        .with_timer(JiffTimer)
        .with_ansi(supports_color::on(Stream::Stdout).is_some());

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    Ok(guard)
}

struct JiffTimer;

impl FormatTime for JiffTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Timestamp::now())
    }
}

#[cfg(test)]
mod tests {
    use testresult::TestResult;

    use super::*;

    #[test]
    fn test_init_log() -> TestResult {
        let _ = init_log(&clap_verbosity_flag::Verbosity::new(4, 0), "log")?;
        tracing::trace!("test trace");

        Ok(())
    }
}
