use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use reqwest::{Client, redirect};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::task;

#[must_use]
pub struct AlgoliaClient {
    client: Client,
    application_id: String,
}

impl AlgoliaClient {
    pub fn build<T, E>(application_id: T, api_key: E) -> Result<Self>
    where
        T: AsRef<str>,
        E: AsRef<str>,
    {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-algolia-application-id",
            HeaderValue::from_str(application_id.as_ref())?,
        );
        headers.insert(
            "x-algolia-api-key",
            HeaderValue::from_str(api_key.as_ref())?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;

        Ok(Self {
            client,
            application_id: application_id.as_ref().to_string(),
        })
    }

    pub fn delete_all_records<T>(&self, index_name: T) -> Result<()>
    where
        T: AsRef<str>,
    {
        task::block_in_place(move || {
            Handle::current().block_on(async move {
                self.client
                    .post(format!(
                        "https://{}.algolia.net/1/indexes/{}/clear",
                        self.application_id,
                        index_name.as_ref()
                    ))
                    .send()
                    .await?
                    .error_for_status()
            })
        })?;

        Ok(())
    }

    pub fn add_records<T, E>(&self, index_name: T, path: E) -> Result<()>
    where
        T: AsRef<str>,
        E: AsRef<Path>,
    {
        let json = fs::read_to_string(path)?;
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

        task::block_in_place(move || {
            Handle::current().block_on(async move {
                self.client
                    .post(format!(
                        "https://{}.algolia.net/1/indexes/{}/batch",
                        self.application_id,
                        index_name.as_ref()
                    ))
                    .json(&batch_data)
                    .send()
                    .await?
                    .error_for_status()
            })
        })?;

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
