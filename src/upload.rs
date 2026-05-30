use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::cli::{UploadAssetType, UploadCommand, UploadOutput};
use crate::config::{ConfigManager, SecretStore, file_stem};
use crate::creator::CreatorTarget;
use crate::error::{AppError, AppResult};
use crate::output::print_json;
use crate::roblox::{CreateAssetParams, RobloxAssetsClient};
use crate::status::wait_for_operation;

const MAX_UPLOAD_SIZE_BYTES: u64 = 20 * 1024 * 1024;
const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 2;
const DEFAULT_YIELD_TIMEOUT_SECONDS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetType {
    Image,
    Audio,
    Model,
    Animation,
}

impl AssetType {
    fn from_cli(value: Option<UploadAssetType>, path: &Path) -> AppResult<Self> {
        match value {
            Some(UploadAssetType::Image) => Self::validate_extension(path, Self::Image),
            Some(UploadAssetType::Audio) => Self::validate_extension(path, Self::Audio),
            Some(UploadAssetType::Model) => Self::validate_extension(path, Self::Model),
            Some(UploadAssetType::Animation) => Self::validate_extension(path, Self::Animation),
            None => Self::infer(path),
        }
    }

    fn infer(path: &Path) -> AppResult<Self> {
        match extension(path)?.as_str() {
            "png" | "jpg" | "jpeg" | "bmp" | "tga" => Ok(Self::Image),
            "mp3" | "ogg" | "wav" | "flac" => Ok(Self::Audio),
            "fbx" => Ok(Self::Model),
            "rbxm" | "rbxmx" => Ok(Self::Animation),
            value => Err(AppError::invalid_args(format!(
                "could not infer an asset type from .{value}. Pass --type explicitly"
            ))),
        }
    }

    fn validate_extension(path: &Path, asset_type: Self) -> AppResult<Self> {
        let ext = extension(path)?;
        let valid = matches!(
            (asset_type, ext.as_str()),
            (Self::Image, "png" | "jpg" | "jpeg" | "bmp" | "tga")
                | (Self::Audio, "mp3" | "ogg" | "wav" | "flac")
                | (Self::Model, "fbx")
                | (Self::Animation, "rbxm" | "rbxmx")
        );

        if valid {
            Ok(asset_type)
        } else {
            Err(AppError::invalid_args(format!(
                "file extension .{ext} is not supported for {} uploads",
                asset_type.cli_name()
            )))
        }
    }

    fn api_name(&self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Audio => "Audio",
            Self::Model => "Model",
            Self::Animation => "Animation",
        }
    }

    fn cli_name(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Model => "model",
            Self::Animation => "animation",
        }
    }

    fn content_type(&self, path: &Path) -> AppResult<&'static str> {
        let ext = extension(path)?;

        match ext.as_str() {
            "png" => Ok("image/png"),
            "jpg" | "jpeg" => Ok("image/jpeg"),
            "bmp" => Ok("image/bmp"),
            "tga" => Ok("image/tga"),
            "mp3" => Ok("audio/mpeg"),
            "ogg" => Ok("audio/ogg"),
            "wav" => Ok("audio/wav"),
            "flac" => Ok("audio/flac"),
            "fbx" => Ok("model/fbx"),
            "rbxm" => Ok("model/x-rbxm"),
            "rbxmx" => Ok("model/x-rbxmx"),
            _ => Err(AppError::invalid_args(format!(
                "no content type mapping exists for {}",
                path.display()
            ))),
        }
    }
}

#[derive(Debug, Serialize)]
struct UploadJsonOutput {
    file: String,
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "assetId", skip_serializing_if = "Option::is_none")]
    asset_id: Option<String>,
    #[serde(rename = "assetType")]
    asset_type: String,
    #[serde(rename = "displayName")]
    display_name: String,
}

pub async fn run_upload<S: SecretStore>(
    args: UploadCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    if matches!(args.output, Some(UploadOutput::Id)) && !args.yield_until_done {
        return Err(AppError::invalid_args(
            "--output id requires --yield for uploads",
        ));
    }

    if !args.yield_until_done && (args.timeout.is_some() || args.poll_interval.is_some()) {
        return Err(AppError::invalid_args(
            "--timeout and --poll-interval require --yield",
        ));
    }

    if !args.path.exists() {
        return Err(AppError::invalid_args(format!(
            "file does not exist: {}",
            args.path.display()
        )));
    }

    if !args.path.is_file() {
        return Err(AppError::invalid_args(format!(
            "phase 2 only supports single-file uploads. Received: {}",
            args.path.display()
        )));
    }

    let metadata = fs::metadata(&args.path).map_err(|error| {
        AppError::invalid_args(format!(
            "failed to inspect {}: {error}",
            args.path.display()
        ))
    })?;

    if metadata.len() > MAX_UPLOAD_SIZE_BYTES {
        return Err(AppError::invalid_args(format!(
            "{} is {} bytes, which exceeds the 20 MB Roblox upload limit",
            args.path.display(),
            metadata.len()
        )));
    }

    let asset_type = AssetType::from_cli(args.asset_type, &args.path)?;
    let display_name = args
        .display_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(file_stem(&args.path)?);
    let description = args
        .description
        .clone()
        .filter(|value| !value.trim().is_empty());
    let content_type = asset_type.content_type(&args.path)?;

    let config = config_manager.load()?;
    let api_key = config_manager.get_api_key()?.ok_or_else(|| {
        AppError::auth("no API key configured. Run `rbxup config set api-key <key>`")
    })?;
    let creator = resolve_creator(&args, config_manager, &config)?;
    let file_name = args
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            AppError::invalid_args(format!(
                "could not determine the file name for {}",
                args.path.display()
            ))
        })?
        .to_string();
    let file_bytes = fs::read(&args.path).map_err(|error| {
        AppError::upload(format!("failed to read {}: {error}", args.path.display()))
    })?;
    let output_mode = args.output.unwrap_or(if args.yield_until_done {
        UploadOutput::Id
    } else {
        UploadOutput::Job
    });

    let client = RobloxAssetsClient::new(api_key);
    let create_response = client
        .create_asset(CreateAssetParams {
            asset_type: asset_type.api_name().to_string(),
            display_name: display_name.clone(),
            description,
            creator,
            file_name,
            file_bytes,
            content_type,
        })
        .await?;
    let mut output = UploadJsonOutput {
        file: args
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_else(|| args.path.as_os_str().to_str().unwrap_or_default())
            .to_string(),
        operation_id: create_response.path.clone(),
        asset_id: None,
        asset_type: asset_type.api_name().to_string(),
        display_name,
    };

    if args.yield_until_done {
        let poll_interval = args.poll_interval.unwrap_or(std::time::Duration::from_secs(
            DEFAULT_POLL_INTERVAL_SECONDS,
        ));
        let timeout = args.timeout.unwrap_or(std::time::Duration::from_secs(
            DEFAULT_YIELD_TIMEOUT_SECONDS,
        ));

        if poll_interval.is_zero() {
            return Err(AppError::invalid_args(
                "--poll-interval must be greater than 0s",
            ));
        }

        if timeout.is_zero() {
            return Err(AppError::invalid_args("--timeout must be greater than 0s"));
        }

        let operation =
            wait_for_operation(&client, &create_response.path, poll_interval, timeout).await?;
        let asset_id = operation.asset_id().ok_or_else(|| {
            if let Some(message) = operation.error_message() {
                AppError::upload(format!("operation {} failed: {message}", operation.path))
            } else {
                AppError::upload(format!(
                    "operation {} completed without an asset ID",
                    operation.path
                ))
            }
        })?;

        output.asset_id = Some(asset_id.to_string());
    }

    match output_mode {
        UploadOutput::Job => {
            println!("{}", output.operation_id);
            Ok(())
        }
        UploadOutput::Id => {
            let asset_id = output.asset_id.as_deref().ok_or_else(|| {
                AppError::invalid_args("--output id requires --yield for uploads")
            })?;
            println!("{asset_id}");
            Ok(())
        }
        UploadOutput::Json => print_json(&output),
        UploadOutput::Pretty => {
            println!("Operation ID: {}", output.operation_id);
            if let Some(asset_id) = &output.asset_id {
                println!("Asset ID: {asset_id}");
            }
            println!("File: {}", output.file);
            println!("Asset Type: {}", output.asset_type);
            println!("Display Name: {}", output.display_name);
            Ok(())
        }
    }
}

fn resolve_creator<S: SecretStore>(
    args: &UploadCommand,
    config_manager: &ConfigManager<S>,
    config: &crate::config::AppConfig,
) -> AppResult<CreatorTarget> {
    let raw_creator = match &args.creator {
        Some(value) => value.clone(),
        None => config_manager.resolve_creator(config)?.ok_or_else(|| {
            AppError::config(
                "no creator configured. Pass --creator user:<id> or run `rbxup config set creator user:<id>`",
            )
        })?,
    };

    CreatorTarget::parse(&raw_creator)
}

fn extension(path: &Path) -> AppResult<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            AppError::invalid_args(format!(
                "{} has no supported file extension",
                path.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::AssetType;

    #[test]
    fn infers_image_asset_type() {
        let asset_type = AssetType::infer(Path::new("icon.png")).expect("asset type should infer");
        assert_eq!(asset_type.api_name(), "Image");
    }

    #[test]
    fn rejects_invalid_model_extension() {
        let error = AssetType::validate_extension(Path::new("sound.mp3"), AssetType::Model)
            .expect_err("validation should fail");
        assert_eq!(
            error.to_string(),
            "file extension .mp3 is not supported for model uploads"
        );
    }
}
