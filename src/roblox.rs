use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};

use crate::creator::CreatorTarget;
use crate::error::{AppError, AppResult};

const ASSET_CREATE_URL: &str = "https://apis.roblox.com/assets/v1/assets";

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

#[derive(Debug, Clone)]
pub struct RobloxAssetsClient {
    client: reqwest::Client,
    api_key: String,
}

impl RobloxAssetsClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
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
            .client
            .post(ASSET_CREATE_URL)
            .header("x-api-key", &self.api_key)
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
