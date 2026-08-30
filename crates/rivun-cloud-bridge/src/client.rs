//! HTTP client for communicating with Rivun Cloud API.

use reqwest::{Client, StatusCode};
use thiserror::Error;
use uuid::Uuid;

use crate::models::{
    BridgeConfig, IncidentIngestPayload, IngestResponse, PolicyBundle, ReceiptIngestBatch,
    TelemetryIngestPayload,
};

#[derive(Debug, Error)]
pub enum BridgeClientError {
    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error ({status}): {message}")]
    ApiError {
        status: StatusCode,
        message: String,
    },
    #[error("Failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct CloudBridgeClient {
    client: Client,
    config: BridgeConfig,
}

impl CloudBridgeClient {
    pub fn new(config: BridgeConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client, config }
    }

    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    /// Pushes edge telemetry report to Rivun Cloud.
    pub async fn push_telemetry(
        &self,
        payload: &TelemetryIngestPayload,
    ) -> Result<IngestResponse, BridgeClientError> {
        let url = format!(
            "{}/v1/orgs/{}/ingest/telemetry",
            self.config.cloud_url.trim_end_matches('/'),
            self.config.org_slug
        );

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.api_token)
            .json(payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(BridgeClientError::ApiError { status, message });
        }

        let body = resp.json::<IngestResponse>().await?;
        Ok(body)
    }

    /// Streams a batch of compact receipt metadata to Rivun Cloud.
    pub async fn push_receipts(
        &self,
        batch: &ReceiptIngestBatch,
    ) -> Result<IngestResponse, BridgeClientError> {
        let url = format!(
            "{}/v1/orgs/{}/ingest/receipts",
            self.config.cloud_url.trim_end_matches('/'),
            self.config.org_slug
        );

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.api_token)
            .json(batch)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(BridgeClientError::ApiError { status, message });
        }

        let body = resp.json::<IngestResponse>().await?;
        Ok(body)
    }

    /// Polls Rivun Cloud for pending signed policy bundles waiting for deployment.
    pub async fn pull_pending_policies(&self) -> Result<Vec<PolicyBundle>, BridgeClientError> {
        let url = format!(
            "{}/v1/orgs/{}/policies/pending?node_id={}",
            self.config.cloud_url.trim_end_matches('/'),
            self.config.org_slug,
            self.config.node_id
        );

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.config.api_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(BridgeClientError::ApiError { status, message });
        }

        let bundles = resp.json::<Vec<PolicyBundle>>().await?;
        Ok(bundles)
    }

    /// Acknowledges deployment of a signed policy bundle.
    pub async fn acknowledge_policy(
        &self,
        policy_id: Uuid,
        applied_version: u32,
    ) -> Result<IngestResponse, BridgeClientError> {
        let url = format!(
            "{}/v1/orgs/{}/policies/{}/ack",
            self.config.cloud_url.trim_end_matches('/'),
            self.config.org_slug,
            policy_id
        );

        let body = serde_json::json!({
            "node_id": self.config.node_id,
            "applied_version": applied_version,
            "acknowledged_at_micros": rivun_core::now_micros().unwrap_or(0),
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.api_token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(BridgeClientError::ApiError { status, message });
        }

        let res = resp.json::<IngestResponse>().await?;
        Ok(res)
    }

    /// Uploads an evidence-redacted incident snapshot to Rivun Cloud.
    pub async fn push_incident(
        &self,
        incident: &IncidentIngestPayload,
    ) -> Result<IngestResponse, BridgeClientError> {
        let url = format!(
            "{}/v1/orgs/{}/incidents",
            self.config.cloud_url.trim_end_matches('/'),
            self.config.org_slug
        );

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.api_token)
            .json(incident)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(BridgeClientError::ApiError { status, message });
        }

        let res = resp.json::<IngestResponse>().await?;
        Ok(res)
    }
}
