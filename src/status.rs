use std::time::{Duration, Instant};

use tokio::time::sleep;

use crate::cli::{StatusCommand, StatusOutput};
use crate::config::{ConfigManager, SecretStore};
use crate::error::{AppError, AppResult};
use crate::output::print_json;
use crate::roblox::{AssetOperation, RobloxAssetsClient};

pub async fn run_status<S: SecretStore>(
    args: StatusCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    let api_key = config_manager.get_api_key()?.ok_or_else(|| {
        AppError::auth("no API key configured. Run `rbxup config set api-key <key>`")
    })?;
    let client = RobloxAssetsClient::new(api_key);
    let operation = client.get_operation(&args.operation_id).await?;

    match args.output {
        StatusOutput::Json => print_json(&operation),
        StatusOutput::Id => print_operation_asset_id(&operation),
        StatusOutput::Pretty => {
            println!("Operation ID: {}", operation.path);
            println!("Done: {}", operation.done);
            println!(
                "Asset ID: {}",
                operation.asset_id().unwrap_or("<unavailable>")
            );

            if let Some(response) = &operation.response {
                if let Some(display_name) = &response.display_name {
                    println!("Display Name: {display_name}");
                }

                if let Some(asset_type) = &response.asset_type {
                    println!("Asset Type: {asset_type}");
                }
            }

            if let Some(message) = operation.error_message() {
                println!("Error: {message}");
            }

            Ok(())
        }
    }
}

pub fn print_operation_asset_id(operation: &AssetOperation) -> AppResult<()> {
    if let Some(message) = operation.error_message() {
        return Err(AppError::upload(format!(
            "operation {} failed: {message}",
            operation.path
        )));
    }

    let asset_id = operation.asset_id().ok_or_else(|| {
        if operation.done {
            AppError::upload(format!(
                "operation {} completed without an asset ID",
                operation.path
            ))
        } else {
            AppError::general(format!(
                "operation {} is still running; asset ID is not available yet",
                operation.path
            ))
        }
    })?;

    println!("{asset_id}");
    Ok(())
}

pub async fn wait_for_operation(
    client: &RobloxAssetsClient,
    operation_id: &str,
    poll_interval: Duration,
    timeout: Duration,
) -> AppResult<AssetOperation> {
    let started_at = Instant::now();
    let mut rate_limit_retries = 0u32;

    loop {
        let operation = match client.get_operation(operation_id).await {
            Ok(operation) => {
                rate_limit_retries = 0;
                operation
            }
            Err(error) if error.is_rate_limited() => {
                rate_limit_retries += 1;
                if started_at.elapsed() >= timeout {
                    return Err(AppError::timeout(format!(
                        "upload still processing after {} while polling operation {}",
                        humantime::format_duration(timeout),
                        operation_id
                    )));
                }

                sleep(rate_limit_backoff(rate_limit_retries)).await;
                continue;
            }
            Err(error) => return Err(error),
        };
        if operation.done {
            return Ok(operation);
        }

        if started_at.elapsed() >= timeout {
            return Err(AppError::timeout(format!(
                "upload still processing after {} for operation {}",
                humantime::format_duration(timeout),
                operation.path
            )));
        }

        sleep(poll_interval).await;
    }
}

fn rate_limit_backoff(retry: u32) -> Duration {
    let seconds = 2u64.saturating_pow(retry.min(4));
    Duration::from_secs(seconds)
}
