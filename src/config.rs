use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::{env, fs};

use anyhow::{Context, Result};
use serde::Deserialize;
use url::Url;

#[must_use]
#[derive(Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub https: HttpsConfig,
    pub hugo: HugoConfig,
    pub algolia: AlgoliaConfig,
}

impl Config {
    pub fn load_config<T>(file_name: T) -> Result<Self>
    where
        T: AsRef<str>,
    {
        let file_name = file_name.as_ref();

        let mut current_dir = env::current_dir()?;
        loop {
            let candidate = current_dir.join(file_name);
            if candidate.try_exists()? {
                tracing::info!("Load config file from `{}`", candidate.display());

                let mut config: Self = toml::from_str(&fs::read_to_string(candidate)?)?;
                config.canonicalize()?;

                return Ok(config);
            }

            if !current_dir.pop() {
                anyhow::bail!("cannot find `{file_name}`");
            }
        }
    }

    fn canonicalize(&mut self) -> Result<()> {
        self.https.cert_path = self
            .https
            .cert_path
            .canonicalize()
            .context(format!("can not find `{}`", self.https.cert_path.display()))?;
        self.https.key_path = self
            .https
            .key_path
            .canonicalize()
            .context(format!("can not find `{}`", self.https.key_path.display()))?;
        self.hugo.repo_dst = env::current_dir()?.join(&self.hugo.repo_dst);

        Ok(())
    }
}

#[must_use]
#[derive(Deserialize)]
pub struct ServerConfig {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[must_use]
#[derive(Deserialize)]
pub struct HttpsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[must_use]
#[derive(Deserialize)]
pub struct HugoConfig {
    pub repo_url: Url,
    pub repo_dst: PathBuf,
}

#[must_use]
#[derive(Deserialize)]
pub struct AlgoliaConfig {
    pub records_file_name: PathBuf,
    pub application_id: String,
    pub api_key: String,
    pub index_name: String,
}

#[cfg(test)]
mod tests {
    use testresult::TestResult;
    use tracing_test::traced_test;

    use super::*;

    #[test]
    #[traced_test]
    fn test_load_config() -> TestResult {
        let _ = Config::load_config(".config.toml")?;

        Ok(())
    }
}
