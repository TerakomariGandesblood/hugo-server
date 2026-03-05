use std::net::Ipv4Addr;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct EnvConfig {
    pub host: Ipv4Addr,
    pub port: u16,
    pub repo_url: String,
    pub repo_dst: PathBuf,
    pub algolia_index_name: String,
    pub algolia_application_id: String,
    pub algolia_api_key: String,
}
