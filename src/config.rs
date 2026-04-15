use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::{env, fs};

use anyhow::Result;
use once_cell::sync::OnceCell;
use serde::Deserialize;
use url::Url;

#[must_use]
#[derive(Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub https: HttpsConfig,
    pub hugo: HugoConfig,
    pub algolia: AlgoliaConfig,

    #[serde(skip_deserializing)]
    config_path: PathBuf,
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

                let mut config: Self = toml::from_str(&fs::read_to_string(&candidate)?)?;
                config.config_path = candidate;
                config.canonicalize()?;

                return Ok(config);
            }

            if !current_dir.pop() {
                anyhow::bail!("cannot find `{file_name}`");
            }
        }
    }

    pub fn repo_dst(&self) -> Result<&'static PathBuf> {
        static REPO_DST: OnceCell<PathBuf> = OnceCell::new();

        REPO_DST.get_or_try_init(|| {
            let repo_dst = self
                .hugo
                .repo_url
                .path_segments()
                .ok_or(anyhow::anyhow!(
                    "`{}` cannot be a base URL",
                    self.hugo.repo_url
                ))?
                .next_back()
                .ok_or(anyhow::anyhow!(
                    "unable to get the repository name from the URL: {}",
                    self.hugo.repo_url
                ))?
                .trim_end_matches(".git");

            Ok(env::current_dir()?.join(repo_dst))
        })
    }

    pub fn build_dst(&self) -> Result<&'static PathBuf> {
        static BUILD_DST: OnceCell<PathBuf> = OnceCell::new();
        BUILD_DST.get_or_try_init(|| Ok(self.repo_dst()?.join("public")))
    }

    pub fn algolia_records_file(&self) -> Result<&'static PathBuf> {
        static ALGOLIA_RECORDS_FILE: OnceCell<PathBuf> = OnceCell::new();
        ALGOLIA_RECORDS_FILE.get_or_try_init(|| Ok(self.build_dst()?.join("algolia.json")))
    }

    fn canonicalize(&mut self) -> Result<()> {
        let parent_dir = self.config_path.parent().unwrap();

        if !self.https.cert_path.is_absolute() {
            self.https.cert_path = parent_dir.join(&self.https.cert_path);
        }
        if !self.https.cert_path.try_exists()? {
            anyhow::bail!("can not find `{}`", self.https.cert_path.display());
        }

        if !self.https.key_path.is_absolute() {
            self.https.key_path = parent_dir.join(&self.https.key_path);
        }
        if !self.https.key_path.try_exists()? {
            anyhow::bail!("can not find `{}`", self.https.key_path.display());
        }

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
