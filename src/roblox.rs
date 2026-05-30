use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::auth::SharedAuthProvider;
use crate::creator::CreatorTarget;
use crate::error::{AppError, AppResult};

const ASSET_CREATE_URL: &str = "https://apis.roblox.com/assets/v1/assets";
const ASSET_OPERATION_URL: &str = "https://apis.roblox.com/assets/v1/operations";

#[derive(Debug, Clone)]
pub struct CreateAssetParams {
    pub asset_type: String,
    pub display_name: String,
    pub description: Option<String>,
    pub creator: CreatorTarget,
    pub file_name: String,
    pub file_bytes: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Clone)]
pub struct RobloxAssetsClient {
    client: reqwest::Client,
    auth_provider: SharedAuthProvider,
}

impl RobloxAssetsClient {
    pub fn new(auth_provider: SharedAuthProvider) -> Self {
        Self {
            client: reqwest::Client::new(),
            auth_provider,
        }
    }

    pub async fn create_asset(&self, params: CreateAssetParams) -> AppResult<CreateAssetResponse> {
        let request_body = CreateAssetRequest {
            asset_type: params.asset_type,
            display_name: params.display_name,
            description: params.description,
            creation_context: CreationContext {
                creator: RequestCreator {
                    user_id: params.creator.user_id().map(ToOwned::to_owned),
                    group_id: params.creator.group_id().map(ToOwned::to_owned),
                },
            },
        };

        let request_json = serde_json::to_string(&request_body).map_err(|error| {
            AppError::general(format!("failed to serialize upload request: {error}"))
        })?;
        let file_part = Part::bytes(params.file_bytes)
            .file_name(params.file_name)
            .mime_str(params.content_type)
            .map_err(|error| {
                AppError::invalid_args(format!(
                    "failed to prepare file content type {}: {error}",
                    params.content_type
                ))
            })?;
        let form = Form::new()
            .text("request", request_json)
            .part("fileContent", file_part);

        let response = self
            .auth_provider
            .apply(self.client.post(ASSET_CREATE_URL))
            .multipart(form)
            .send()
            .await
            .map_err(|error| AppError::upload(format!("failed to send upload request: {error}")))?;
        let status = response.status();

        if status.is_success() {
            return response
                .json::<CreateAssetResponse>()
                .await
                .map_err(|error| {
                    AppError::upload(format!(
                        "upload succeeded but the response was invalid: {error}"
                    ))
                });
        }

        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<response body unavailable>".to_string());
        let body = body.trim();
        let message = format!(
            "Roblox upload failed with HTTP {}: {}",
            status.as_u16(),
            if body.is_empty() {
                "<empty response body>"
            } else {
                body
            }
        );

        if status.as_u16() == 401 || status.as_u16() == 403 {
            Err(AppError::auth(message))
        } else if status.as_u16() == 429 {
            Err(AppError::rate_limited(message))
        } else {
            Err(AppError::upload(message))
        }
    }

    pub async fn get_operation(&self, operation_id: &str) -> AppResult<AssetOperation> {
        let normalized_id = normalize_operation_id(operation_id);
        let response = self
            .auth_provider
            .apply(
                self.client
                    .get(format!("{ASSET_OPERATION_URL}/{normalized_id}")),
            )
            .send()
            .await
            .map_err(|error| {
                AppError::general(format!("failed to fetch operation status: {error}"))
            })?;
        let status = response.status();

        if status.is_success() {
            return response.json::<AssetOperation>().await.map_err(|error| {
                AppError::general(format!(
                    "operation status succeeded but the response was invalid: {error}"
                ))
            });
        }

        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<response body unavailable>".to_string());
        let body = body.trim();
        let message = format!(
            "Roblox status request failed with HTTP {}: {}",
            status.as_u16(),
            if body.is_empty() {
                "<empty response body>"
            } else {
                body
            }
        );

        if status.as_u16() == 401 || status.as_u16() == 403 {
            Err(AppError::auth(message))
        } else if status.as_u16() == 429 {
            Err(AppError::rate_limited(message))
        } else {
            Err(AppError::general(message))
        }
    }
}

#[derive(Debug, Serialize)]
struct CreateAssetRequest {
    #[serde(rename = "assetType")]
    asset_type: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "creationContext")]
    creation_context: CreationContext,
}

#[derive(Debug, Serialize)]
struct CreationContext {
    creator: RequestCreator,
}

#[derive(Debug, Serialize)]
struct RequestCreator {
    #[serde(rename = "userId", skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(rename = "groupId", skip_serializing_if = "Option::is_none")]
    group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAssetResponse {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOperation {
    pub path: String,
    pub done: bool,
    #[serde(default)]
    pub response: Option<AssetOperationResponse>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

impl AssetOperation {
    pub fn asset_id(&self) -> Option<&str> {
        self.response.as_ref()?.asset_id.as_deref()
    }

    pub fn error_message(&self) -> Option<String> {
        let error = self.error.as_ref()?;

        if let Some(message) = error.get("message").and_then(|value| value.as_str()) {
            return Some(message.to_string());
        }

        if let Some(message) = error.get("error").and_then(|value| value.as_str()) {
            return Some(message.to_string());
        }

        Some(error.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOperationResponse {
    #[serde(rename = "@type", default)]
    pub type_url: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(rename = "revisionId", default)]
    pub revision_id: Option<String>,
    #[serde(rename = "revisionCreateTime", default)]
    pub revision_create_time: Option<String>,
    #[serde(rename = "assetId", default)]
    pub asset_id: Option<String>,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "assetType", default)]
    pub asset_type: Option<String>,
    #[serde(rename = "creationContext", default)]
    pub creation_context: Option<serde_json::Value>,
    #[serde(rename = "moderationResult", default)]
    pub moderation_result: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn normalize_operation_id(operation_id: &str) -> String {
    let trimmed = operation_id.trim();

    if let Some(value) = trimmed.strip_prefix("operations/") {
        value.to_string()
    } else if let Some((_, value)) = trimmed.rsplit_once("/operations/") {
        value.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_operation_id;

    #[test]
    fn strips_operations_prefix() {
        assert_eq!(normalize_operation_id("operations/abc123"), "abc123");
    }

    #[test]
    fn strips_full_url_prefix() {
        assert_eq!(
            normalize_operation_id("https://apis.roblox.com/assets/v1/operations/abc123"),
            "abc123"
        );
    }
}
