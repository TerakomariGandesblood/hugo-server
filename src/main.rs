use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;
use std::{env, fs, thread};

use anyhow::Result;
use clap::Parser;
use gix::{Repository, Url};
use hugo_server::{AlgoliaClient, Args, EnvConfig};
use mimalloc::MiMalloc;
use tokio::net::TcpListener;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    dotenvy::dotenv()?;
    let mut env_config: EnvConfig = envy::from_env()?;
    env_config.repo_dst = env::current_dir()?.join(&env_config.repo_dst);

    let _guard = hugo_server::init_log(&args.verbose, "log", env!("CARGO_CRATE_NAME"))?;

    if let Some(shell) = args.completion {
        hugo_server::generate_completion(shell)?;
        return Ok(());
    }

    if let Err(error) = which::which("hugo") {
        anyhow::bail!("hugo is unavailable: {error}");
    }

    let build_dst = env_config.repo_dst.join("public");
    let repo_url = gix::url::parse(env_config.repo_url.as_str().into())?;

    let router = hugo_server::router(&build_dst);
    let listener = TcpListener::bind(SocketAddr::new(
        IpAddr::V4(env_config.host),
        env_config.port,
    ))
    .await?;

    let (tx, rx) = mpsc::channel();
    let mut repo = clone_and_build(&repo_url, &env_config.repo_dst, &build_dst)?;
    upload_algolia_records(
        &build_dst,
        &env_config.algolia_application_id,
        &env_config.algolia_api_key,
        &env_config.algolia_index_name,
    )?;

    let handle = thread::spawn(move || {
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

                repo = clone_and_build(&repo_url, &env_config.repo_dst, &build_dst)?;
                upload_algolia_records(
                    &build_dst,
                    &env_config.algolia_application_id,
                    &env_config.algolia_api_key,
                    &env_config.algolia_index_name,
                )?;
            }
        }

        anyhow::Ok(())
    });

    tracing::info!(
        "Web Server is available at http://localhost:{}/ (bind address {})",
        env_config.port,
        env_config.host
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(hugo_server::shutdown_signal())
        .await?;

    tx.send(())?;
    handle.join().unwrap()?;

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

fn upload_algolia_records(
    build_dst: &Path,
    application_id: &str,
    api_key: &str,
    index_name: &str,
) -> Result<()> {
    let algolia_json = build_dst.join("algolia.json");
    if algolia_json.is_file() {
        tracing::info!("Begin uploading Algolia records");

        let client = AlgoliaClient::build(application_id, api_key)?;
        client.delete_all_records(index_name)?;
        client.add_records(index_name, algolia_json)?;
    }

    Ok(())
}
