use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::{env, fs};

use anyhow::Result;
use serde::Deserialize;
use url::Url;

#[must_use]
#[derive(Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub https: HttpsConfig,

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
    pub url: Url,
    pub host: Ipv4Addr,
    pub port: u16,
}

#[must_use]
#[derive(Deserialize)]
pub struct HttpsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use testresult::TestResult;
    use tracing_test::traced_test;

    use super::*;

    #[test]
    #[traced_test]
    fn test_load_config() -> TestResult {
        let _ = Config::load_config(".file_server_config.toml")?;

        Ok(())
    }
}
