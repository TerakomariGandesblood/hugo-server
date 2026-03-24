use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;
use std::{fs, thread};

use anyhow::Result;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use gix::{Repository, Url};
use hugo_server::{AlgoliaClient, Args, Config};
use mimalloc::MiMalloc;
use tokio::task;

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

    let config = Config::load_config("config.toml")?;

    if let Err(error) = which::which("hugo") {
        anyhow::bail!("hugo is unavailable: {error}");
    }

    let build_dst = config.hugo.repo_dst.join("public");
    let repo_url = gix::url::parse(config.hugo.repo_url.as_str().into())?;

    let mut repo = clone_and_build(&repo_url, &config.hugo.repo_dst, &build_dst)?;
    upload_algolia_records(&build_dst, &config)?;

    let router = hugo_server::router(&build_dst);
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

    let (tx, rx) = mpsc::channel();
    let handle = task::spawn_blocking(move || {
        'main: loop {
            for _ in 0..60 {
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => break 'main,
                    Err(TryRecvError::Empty) => (),
                }
                thread::sleep(Duration::from_secs(1));
            }

            if !hugo_server::fetch_and_no_change(&repo)? {
                tracing::info!("The website has been updated and will be rebuilt");

                repo = clone_and_build(&repo_url, &config.hugo.repo_dst, &build_dst)?;
                upload_algolia_records(&build_dst, &config)?;

                tracing::info!("Website update completed");
            }
        }

        anyhow::Ok(())
    });

    axum_server::bind_rustls(addr, https_config)
        .handle(server_handle)
        .serve(router.into_make_service())
        .await?;

    tx.send(())?;
    handle.await??;

    Ok(())
}

fn clone_and_build(repo_url: &Url, repo_dst: &Path, build_dst: &Path) -> Result<Repository> {
    if repo_dst.is_dir() {
        tracing::warn!(
            "This repository directory exists and will be deleted: {}",
            repo_dst.display()
        );
        fs::remove_dir_all(repo_dst)?;
    }

    tracing::info!("Repo clone into {}", repo_dst.display());
    let repo = hugo_server::clone(repo_url, repo_dst)?;

    tracing::info!("Hugo build into {}", build_dst.display());
    let repo_dst = repo_dst.to_str().unwrap();
    cmd_lib::run_cmd!(
        cd $repo_dst;
        hugo build --minify --quiet --destination $build_dst;
    )?;

    Ok(repo)
}

fn upload_algolia_records(build_dst: &Path, config: &Config) -> Result<()> {
    let algolia_json = build_dst.join("algolia.json");
    if algolia_json.is_file() {
        tracing::info!("Begin uploading Algolia records");

        let client = AlgoliaClient::build(&config.algolia.application_id, &config.algolia.api_key)?;
        client.delete_all_records(&config.algolia.index_name)?;
        client.add_records(&config.algolia.index_name, algolia_json)?;
    } else {
        tracing::warn!("Cannot find Algolia records: {}", algolia_json.display());
    }

    Ok(())
}
