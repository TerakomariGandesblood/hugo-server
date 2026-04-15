use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use anyhow::Result;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use hugo_server::{Args, Config};
use mimalloc::MiMalloc;
use tokio::fs;
use tokio_util::sync::CancellationToken;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(shell) = args.completion {
        hugo_server::generate_completion(shell)?;
        return Ok(());
    }

    let _guard = hugo_server::init_log(&args.verbose, "log")?;

    let config = Config::load_config(".config.toml")?;

    hugo_server::check_cmd()?;

    init_website(&config).await?;

    let router = hugo_server::router(config.build_dst()?);
    let addr = SocketAddr::new(IpAddr::V4(config.server.host), config.server.port);
    let https_config =
        RustlsConfig::from_pem_file(&config.https.cert_path, &config.https.key_path).await?;

    let server_handle = Handle::new();
    tokio::spawn(hugo_server::shutdown_signal(server_handle.clone()));

    tracing::info!(
        "Web Server is available at https://localhost:{}/ (bind address {})",
        config.server.port,
        config.server.host
    );

    let token = CancellationToken::new();
    let cloned_token = token.clone();
    let handle = tokio::spawn(async move {
        cloned_token
            .run_until_cancelled(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(60)).await;

                    if let Err(error) = try_update_website(&config).await {
                        tracing::error!("try update website failed: {error}");
                    }
                }
            })
            .await
    });

    axum_server::bind_rustls(addr, https_config)
        .handle(server_handle)
        .serve(router.into_make_service())
        .await?;

    token.cancel();
    handle.await?;

    Ok(())
}

async fn init_website(config: &Config) -> Result<()> {
    if config.repo_dst()?.is_dir() {
        tracing::warn!(
            "This repository directory exists and will be deleted: `{}`",
            config.repo_dst()?.display()
        );
        fs::remove_dir_all(config.repo_dst()?).await?;
    }

    tracing::info!("Repo clone into `{}`", config.repo_dst()?.display());
    hugo_server::clone(config).await?;

    tracing::info!("Hugo build into `{}`", config.build_dst()?.display());
    hugo_server::hugo_build(config).await?;

    if config.algolia_records_file()?.is_file() {
        tracing::info!(
            "Begin uploading Algolia records: `{}`",
            config.algolia_records_file()?.display()
        );
        hugo_server::upload_algolia_records(config).await?;
    } else {
        tracing::warn!(
            "Cannot find Algolia records file: `{}`",
            config.algolia_records_file()?.display()
        );
    }

    Ok(())
}

async fn try_update_website(config: &Config) -> Result<()> {
    if hugo_server::has_remote_update(config).await? {
        tracing::info!("The website has been updated and will be rebuilt");

        hugo_server::pull(config).await?;
        hugo_server::hugo_build(config).await?;
        hugo_server::upload_algolia_records(config).await?;

        tracing::info!("Website update completed");
    }

    Ok(())
}
