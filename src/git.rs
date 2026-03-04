use std::fs;
use std::path::Path;

use anyhow::Result;
use gix::progress::Discard;
use gix::remote::Direction;
use gix::remote::fetch::Status;
use gix::{Repository, Url};

pub fn clone<T>(url: &Url, dst: T) -> Result<Repository>
where
    T: AsRef<Path>,
{
    fs::create_dir_all(&dst)?;

    let mut prepare_clone = gix::prepare_clone(url.clone(), &dst)?;
    let (mut prepare_checkout, _) =
        prepare_clone.fetch_then_checkout(Discard, &gix::interrupt::IS_INTERRUPTED)?;
    let (repo, _) = prepare_checkout.main_worktree(Discard, &gix::interrupt::IS_INTERRUPTED)?;

    Ok(repo)
}

pub fn fetch_and_no_change(repo: &Repository) -> Result<bool> {
    let remote = repo.find_default_remote(Direction::Fetch).unwrap()?;

    let connection = remote.connect(Direction::Fetch)?;
    let prepare = connection.prepare_fetch(Discard, Default::default())?;
    let outcome = prepare.receive(Discard, &gix::interrupt::IS_INTERRUPTED)?;

    if matches!(outcome.status, Status::NoPackReceived { .. }) {
        Ok(true)
    } else {
        Ok(false)
    }
}
