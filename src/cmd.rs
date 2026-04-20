use std::env;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use tokio::process::Command;

use crate::Config;

pub fn check_cmd() -> Result<()> {
    if let Err(error) = which::which("git") {
        anyhow::bail!("`git` is unavailable: {error}");
    }
    if let Err(error) = which::which("hugo") {
        anyhow::bail!("`hugo` is unavailable: {error}");
    }

    Ok(())
}

pub async fn clone(config: &Config) -> Result<()> {
    let repo_url = config.hugo.repo_url.as_str();
    run_cmd("git", &["clone", repo_url], env::current_dir()?).await?;

    Ok(())
}

pub async fn has_remote_update(config: &Config) -> Result<bool> {
    let repo_dst = config.repo_dst()?;

    run_cmd("git", &["fetch"], &repo_dst).await?;

    let local_hash = run_cmd("git", &["rev-parse", "HEAD"], &repo_dst).await?;
    let remote_hash = run_cmd("git", &["rev-parse", "origin/main"], &repo_dst).await?;

    Ok(local_hash != remote_hash)
}

pub async fn pull(config: &Config) -> Result<()> {
    run_cmd("git", &["pull"], config.repo_dst()?).await?;
    Ok(())
}

pub async fn hugo_build(config: &Config) -> Result<()> {
    let repo_dst = config.repo_dst()?;
    let build_dst = config.build_dst()?;

    run_cmd(
        "hugo",
        &[
            "build",
            "--minify",
            "--quiet",
            "--cleanDestinationDir",
            "--destination",
            build_dst.to_str().expect("Contains invalid UTF-8"),
        ],
        &repo_dst,
    )
    .await?;

    Ok(())
}

async fn run_cmd<T>(program: &str, args: &[&str], current_dir: T) -> Result<String>
where
    T: AsRef<Path>,
{
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        Command::new(program)
            .args(args)
            .current_dir(current_dir)
            .output(),
    )
    .await??;

    if !output.status.success() {
        anyhow::bail!(
            "`{program} {args:?}` failed: `{}`",
            simdutf8::basic::from_utf8(&output.stderr)?
        );
    }

    Ok(simdutf8::basic::from_utf8(&output.stdout)?
        .trim()
        .to_string())
}
