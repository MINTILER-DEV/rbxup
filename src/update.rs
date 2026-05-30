use std::fs;
use std::time::Duration;

use serde::Serialize;

use crate::auth::resolve_auth_provider;
use crate::cli::{UpdateCommand, UploadAssetType, UploadOutput};
use crate::config::{ConfigManager, SecretStore};
use crate::creator::CreatorTarget;
use crate::error::{AppError, AppResult};
use crate::output::print_json;
use crate::project::load_project_context;
use crate::roblox::{RobloxAssetsClient, UpdateAssetParams};
use crate::status::wait_for_operation;

const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 2;
const DEFAULT_YIELD_TIMEOUT_SECONDS: u64 = 300;
const MAX_UPLOAD_SIZE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone)]
struct ResolvedUpdateCommand {
    asset_id: String,
    path: std::path::PathBuf,
    creator: Option<String>,
    asset_type: Option<UploadAssetType>,
    yield_until_done: bool,
    timeout: Option<Duration>,
    poll_interval: Option<Duration>,
    output: Option<UploadOutput>,
}

#[derive(Debug, Serialize)]
struct UpdateResult {
    file: String,
    #[serde(rename = "assetId")]
    asset_id: String,
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "assetType")]
    asset_type: String,
}

pub async fn run_update<S: SecretStore>(
    args: UpdateCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    let args = resolve_update_command(args)?;

    if matches!(args.output, Some(UploadOutput::Jsonl | UploadOutput::Map)) {
        return Err(AppError::invalid_args(
            "update supports --output job, id, json, or pretty",
        ));
    }

    if matches!(args.output, Some(UploadOutput::Id)) && !args.yield_until_done {
        return Err(AppError::invalid_args(
            "--output id requires --yield for updates",
        ));
    }

    if !args.yield_until_done && (args.timeout.is_some() || args.poll_interval.is_some()) {
        return Err(AppError::invalid_args(
            "--timeout and --poll-interval require --yield",
        ));
    }

    if !args.path.exists() || !args.path.is_file() {
        return Err(AppError::invalid_args(format!(
            "update path must be a file: {}",
            args.path.display()
        )));
    }

    let creator = resolve_creator(&args, config_manager)?;
    let auth_provider = resolve_auth_provider(config_manager).await?;
    let client = RobloxAssetsClient::new(auth_provider);
    let prepared = prepare_update(&args)?;
    let operation = client
        .update_asset(UpdateAssetParams {
            asset_id: args.asset_id.clone(),
            asset_type: "Model".to_string(),
            creator,
            file_name: args
                .path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    AppError::invalid_args(format!(
                        "could not determine the file name for {}",
                        args.path.display()
                    ))
                })?
                .to_string(),
            file_bytes: prepared,
            content_type: "model/fbx",
        })
        .await?;

    if args.yield_until_done {
        let poll_interval = args
            .poll_interval
            .unwrap_or(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECONDS));
        let timeout = args
            .timeout
            .unwrap_or(Duration::from_secs(DEFAULT_YIELD_TIMEOUT_SECONDS));
        let operation_result =
            wait_for_operation(&client, &operation.path, poll_interval, timeout).await?;
        if let Some(message) = operation_result.error_message() {
            return Err(AppError::upload(format!(
                "operation {} failed: {message}",
                operation_result.path
            )));
        }
    }

    let result = UpdateResult {
        file: args.path.display().to_string(),
        asset_id: args.asset_id,
        operation_id: operation.path,
        asset_type: "Model".to_string(),
    };
    print_update_result(
        result,
        args.output.unwrap_or(if args.yield_until_done {
            UploadOutput::Id
        } else {
            UploadOutput::Job
        }),
    )
}

fn resolve_update_command(args: UpdateCommand) -> AppResult<ResolvedUpdateCommand> {
    let project_settings = load_project_context(args.profile.as_deref())?
        .as_ref()
        .map(|context| context.resolve_update_settings(&args));

    Ok(ResolvedUpdateCommand {
        asset_id: args.asset_id,
        path: args.path,
        creator: args.creator.or_else(|| {
            project_settings
                .as_ref()
                .and_then(|value| value.creator.clone())
        }),
        asset_type: args.asset_type.or_else(|| {
            project_settings
                .as_ref()
                .and_then(|value| value.asset_type)
                .filter(|value| *value == UploadAssetType::Model)
        }),
        yield_until_done: args.yield_until_done,
        timeout: args.timeout,
        poll_interval: args.poll_interval,
        output: args.output,
    })
}

fn resolve_creator<S: SecretStore>(
    args: &ResolvedUpdateCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<CreatorTarget> {
    let config = config_manager.load()?;
    let raw_creator = match &args.creator {
        Some(value) => value.clone(),
        None => config_manager.resolve_creator(&config)?.ok_or_else(|| {
            AppError::config(
                "no creator configured. Pass --creator user:<id>, set it in rbxup.toml, or run `rbxup config set creator user:<id>`",
            )
        })?,
    };

    CreatorTarget::parse(&raw_creator)
}

fn prepare_update(args: &ResolvedUpdateCommand) -> AppResult<Vec<u8>> {
    let asset_type = args.asset_type.unwrap_or(UploadAssetType::Model);
    if asset_type != UploadAssetType::Model {
        return Err(AppError::invalid_args(
            "Roblox Open Cloud content updates currently only support .fbx model assets",
        ));
    }

    let extension = args
        .path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            AppError::invalid_args(format!("{} has no file extension", args.path.display()))
        })?;
    if extension != "fbx" {
        return Err(AppError::invalid_args(
            "Roblox Open Cloud content updates currently only support .fbx model assets",
        ));
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

    fs::read(&args.path).map_err(|error| {
        AppError::upload(format!("failed to read {}: {error}", args.path.display()))
    })
}

fn print_update_result(result: UpdateResult, output: UploadOutput) -> AppResult<()> {
    match output {
        UploadOutput::Job => {
            println!("{}", result.operation_id);
            Ok(())
        }
        UploadOutput::Id => {
            println!("{}", result.asset_id);
            Ok(())
        }
        UploadOutput::Json => print_json(&result),
        UploadOutput::Pretty => {
            println!("Asset ID: {}", result.asset_id);
            println!("Operation ID: {}", result.operation_id);
            println!("File: {}", result.file);
            println!("Asset Type: {}", result.asset_type);
            Ok(())
        }
        UploadOutput::Jsonl | UploadOutput::Map => Err(AppError::invalid_args(
            "update supports --output job, id, json, or pretty",
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ResolvedUpdateCommand, prepare_update};
    use crate::cli::UploadAssetType;

    #[test]
    fn rejects_non_fbx_updates() {
        let args = ResolvedUpdateCommand {
            asset_id: "123".to_string(),
            path: Path::new("icon.png").to_path_buf(),
            creator: None,
            asset_type: Some(UploadAssetType::Image),
            yield_until_done: false,
            timeout: None,
            poll_interval: None,
            output: None,
        };

        let error = prepare_update(&args).expect_err("update should fail");
        assert_eq!(
            error.to_string(),
            "Roblox Open Cloud content updates currently only support .fbx model assets"
        );
    }
}
