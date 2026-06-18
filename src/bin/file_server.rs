mod file;

use std::net::{IpAddr, SocketAddr};

use anyhow::Result;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use file::Config;
use mimalloc::MiMalloc;
use servers::Args;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(shell) = args.completion {
        servers::generate_completion(shell)?;
        return Ok(());
    }

    let _guard = servers::init_log(&args.verbose, "log")?;

    let config = Config::load_config(".file_server_config.toml")?;

    let router = file::router(&config)?;
    let addr = SocketAddr::new(IpAddr::V4(config.server.host), config.server.port);
    let https_config =
        RustlsConfig::from_pem_file(&config.https.cert_path, &config.https.key_path).await?;

    let server_handle = Handle::new();
    tokio::spawn(servers::shutdown_signal(server_handle.clone()));

    tracing::info!(
        "Web Server is available at https://localhost:{}/ (bind address {})",
        config.server.port,
        config.server.host
    );

    axum_server::bind_rustls(addr, https_config)
        .handle(server_handle)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}
