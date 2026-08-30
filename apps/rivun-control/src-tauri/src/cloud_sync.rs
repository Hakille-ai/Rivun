//! Cloud Synchronization Client for Operator Station.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API rejected request ({status}): {message}")]
    Api { status: u16, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedPolicyInfo {
    pub id: Uuid,
    pub name: String,
    pub version: u32,
    pub status: String,
    pub body_toml: String,
}

pub struct CloudSyncClient {
    client: Client,
    cloud_url: String,
    org_slug: String,
    token: String,
}

impl CloudSyncClient {
    pub fn new(cloud_url: &str, org_slug: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            cloud_url: cloud_url.trim_end_matches('/').to_string(),
            org_slug: org_slug.to_string(),
            token: token.to_string(),
        }
    }

    /// Fetches all staged policies awaiting operator signature.
    pub async fn fetch_staged_policies(&self) -> Result<Vec<StagedPolicyInfo>, SyncError> {
        let url = format!("{}/v1/orgs/{}/policies", self.cloud_url, self.org_slug);
        let resp = self.client.get(&url).bearer_auth(&self.token).send().await?;
        if !resp.status().is_success() {
            return Err(SyncError::Api {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let all = resp.json::<Vec<StagedPolicyInfo>>().await?;
        Ok(all.into_iter().filter(|p| p.status == "staged").collect())
    }

    /// Submits a locally signed policy back to the Cloud API.
    pub async fn submit_signature(
        &self,
        policy_id: Uuid,
        public_key: &str,
        signature: &str,
    ) -> Result<(), SyncError> {
        let url = format!(
            "{}/v1/orgs/{}/policies/{}/sign",
            self.cloud_url, self.org_slug, policy_id
        );
        let body = serde_json::json!({
            "public_key": public_key,
            "signature": signature,
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(SyncError::Api {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }
}
