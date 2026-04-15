use std::time::Duration;

use anyhow::Result;
use once_cell::sync::OnceCell;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use reqwest::{Client, redirect};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::Config;

pub async fn upload_algolia_records(config: &Config) -> Result<()> {
    static CLIENT: OnceCell<AlgoliaClient> = OnceCell::new();
    let client = CLIENT.get_or_try_init(|| AlgoliaClient::build(config))?;

    client.delete_all_records(config).await?;
    client.add_records(config).await?;

    Ok(())
}

#[must_use]
struct AlgoliaClient {
    client: Client,
}

impl AlgoliaClient {
    fn build(config: &Config) -> Result<Self> {
        let mut headers = HeaderMap::with_capacity(3);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-algolia-application-id",
            HeaderValue::from_str(&config.algolia.application_id)?,
        );
        headers.insert(
            "x-algolia-api-key",
            HeaderValue::from_str(&config.algolia.api_key)?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;

        Ok(Self { client })
    }

    async fn delete_all_records(&self, config: &Config) -> Result<()> {
        self.client
            .post(format!(
                "https://{}.algolia.net/1/indexes/{}/clear",
                config.algolia.application_id, config.algolia.index_name
            ))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn add_records(&self, config: &Config) -> Result<()> {
        let json = fs::read_to_string(config.algolia_records_file()?).await?;
        let bodys: Vec<BatchRequestBody> = sonic_rs::from_str(&json)?;

        let mut batch_data = BatchData {
            requests: Vec::new(),
        };
        for body in bodys {
            batch_data.requests.push(BatchRequest {
                action: "addObject",
                body,
            });
        }

        self.client
            .post(format!(
                "https://{}.algolia.net/1/indexes/{}/batch",
                config.algolia.application_id, config.algolia.index_name
            ))
            .json(&batch_data)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[must_use]
#[derive(Serialize)]
struct BatchData {
    requests: Vec<BatchRequest>,
}

#[must_use]
#[derive(Serialize)]
struct BatchRequest {
    action: &'static str,
    body: BatchRequestBody,
}

#[must_use]
#[derive(Serialize, Deserialize)]
struct BatchRequestBody {
    #[serde(rename = "objectID")]
    object_id: String,
    permalink: String,
    title: String,
    content: String,
    date: String,
    updated: String,
}
