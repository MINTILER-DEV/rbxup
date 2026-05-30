use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::auth::{resolve_auth_provider, whoami as auth_whoami};
use crate::cli::{
    InfoCommand, ListCommand, QuotasCommand, ReadOutput, RollbackCommand, UploadOutput,
    VersionsCommand,
};
use crate::config::{ConfigManager, SecretStore};
use crate::creator::CreatorTarget;
use crate::error::{AppError, AppResult};
use crate::output::print_json;
use crate::roblox::{AssetOperation, RobloxAssetsClient};
use crate::status::wait_for_operation;

const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 2;
const DEFAULT_YIELD_TIMEOUT_SECONDS: u64 = 300;

pub async fn run_info<S: SecretStore>(
    args: InfoCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    let client = RobloxAssetsClient::new(resolve_auth_provider(config_manager).await?);
    let value = client
        .get_asset(&args.asset_id, args.read_mask.as_deref())
        .await?;

    print_read_value("Asset", &value, args.output)
}

pub async fn run_list<S: SecretStore>(
    args: ListCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    let user_id = resolve_user_id(args.user_id, config_manager).await?;
    let client = RobloxAssetsClient::new(resolve_auth_provider(config_manager).await?);
    let value = client
        .list_inventory_items(
            &user_id,
            args.filter.as_deref(),
            args.page_size,
            args.page_token.as_deref(),
        )
        .await?;

    print_read_value("Inventory", &value, args.output)
}

pub async fn run_quotas<S: SecretStore>(
    args: QuotasCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    let user_id = resolve_user_id(args.user_id, config_manager).await?;
    let client = RobloxAssetsClient::new(resolve_auth_provider(config_manager).await?);
    let value = client.get_asset_quotas(&user_id).await?;

    print_read_value("Quotas", &value, args.output)
}

pub async fn run_versions<S: SecretStore>(
    args: VersionsCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    let client = RobloxAssetsClient::new(resolve_auth_provider(config_manager).await?);
    let value = match args.version_number {
        Some(version_number) => {
            client
                .get_asset_version(&args.asset_id, version_number)
                .await?
        }
        None => {
            client
                .list_asset_versions(&args.asset_id, args.page_size, args.page_token.as_deref())
                .await?
        }
    };

    print_read_value("Versions", &value, args.output)
}

pub async fn run_rollback<S: SecretStore>(
    args: RollbackCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    if matches!(args.output, Some(UploadOutput::Jsonl | UploadOutput::Map)) {
        return Err(AppError::invalid_args(
            "rollback supports --output job, id, json, or pretty",
        ));
    }

    if matches!(args.output, Some(UploadOutput::Id)) && !args.yield_until_done {
        return Err(AppError::invalid_args(
            "--output id requires --yield for rollbacks",
        ));
    }

    if !args.yield_until_done && (args.timeout.is_some() || args.poll_interval.is_some()) {
        return Err(AppError::invalid_args(
            "--timeout and --poll-interval require --yield",
        ));
    }

    let client = RobloxAssetsClient::new(resolve_auth_provider(config_manager).await?);
    let operation = client
        .rollback_asset_version(&args.asset_id, args.version_number)
        .await?;
    let output = args.output.unwrap_or(if args.yield_until_done {
        UploadOutput::Id
    } else {
        UploadOutput::Job
    });

    let completed_operation = if args.yield_until_done {
        let poll_interval = args
            .poll_interval
            .unwrap_or(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECONDS));
        let timeout = args
            .timeout
            .unwrap_or(Duration::from_secs(DEFAULT_YIELD_TIMEOUT_SECONDS));
        Some(wait_for_operation(&client, &operation.path, poll_interval, timeout).await?)
    } else {
        None
    };

    let result = RollbackResult {
        asset_id: args.asset_id,
        version_number: args.version_number,
        operation_id: operation.path,
        rolled_back_asset_id: completed_operation
            .as_ref()
            .and_then(AssetOperation::asset_id)
            .map(ToOwned::to_owned),
    };
    print_rollback_result(result, output)
}

fn print_read_value(label: &str, value: &Value, output: ReadOutput) -> AppResult<()> {
    match output {
        ReadOutput::Json => print_json(value),
        ReadOutput::Pretty => {
            println!("{label}:");
            print_pretty_value(value, 0);
            Ok(())
        }
    }
}

fn print_pretty_value(value: &Value, indent: usize) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map {
                match entry {
                    Value::Object(_) | Value::Array(_) => {
                        println!("{:indent$}{key}:", "", indent = indent);
                        print_pretty_value(entry, indent + 2);
                    }
                    _ => println!(
                        "{:indent$}{key}: {}",
                        "",
                        scalar_to_string(entry),
                        indent = indent
                    ),
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                if value.is_object() || value.is_array() {
                    println!("{:indent$}-", "", indent = indent);
                    print_pretty_value(value, indent + 2);
                } else {
                    println!(
                        "{:indent$}- {}",
                        "",
                        scalar_to_string(value),
                        indent = indent
                    );
                }
            }
        }
        _ => println!("{:indent$}{}", "", scalar_to_string(value), indent = indent),
    }
}

fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::Null => "<null>".to_string(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}

async fn resolve_user_id<S: SecretStore>(
    explicit_user_id: Option<String>,
    config_manager: &ConfigManager<S>,
) -> AppResult<String> {
    if let Some(user_id) = explicit_user_id {
        return Ok(user_id);
    }

    let config = config_manager.load()?;
    if let Some(creator) = config_manager.resolve_creator(&config)? {
        let creator = CreatorTarget::parse(&creator)?;
        if let Some(user_id) = creator.user_id() {
            return Ok(user_id.to_string());
        }
    }

    let auth_status = auth_whoami(config_manager).await?;
    if let Some(user_id) = auth_status.user_id {
        return Ok(user_id);
    }

    Err(AppError::config(
        "no user ID available. Pass --user-id, use OAuth, or set the default creator to user:<id>",
    ))
}

#[derive(Debug, Serialize)]
struct RollbackResult {
    #[serde(rename = "assetId")]
    asset_id: String,
    #[serde(rename = "versionNumber")]
    version_number: u64,
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "rolledBackAssetId", skip_serializing_if = "Option::is_none")]
    rolled_back_asset_id: Option<String>,
}

fn print_rollback_result(result: RollbackResult, output: UploadOutput) -> AppResult<()> {
    match output {
        UploadOutput::Job => {
            println!("{}", result.operation_id);
            Ok(())
        }
        UploadOutput::Id => {
            let asset_id = result.rolled_back_asset_id.unwrap_or(result.asset_id);
            println!("{asset_id}");
            Ok(())
        }
        UploadOutput::Json => print_json(&result),
        UploadOutput::Pretty => {
            println!("Asset ID: {}", result.asset_id);
            println!("Version Number: {}", result.version_number);
            println!("Operation ID: {}", result.operation_id);
            if let Some(rolled_back_asset_id) = result.rolled_back_asset_id {
                println!("Rolled Back Asset ID: {rolled_back_asset_id}");
            }
            Ok(())
        }
        UploadOutput::Jsonl | UploadOutput::Map => Err(AppError::invalid_args(
            "rollback supports --output job, id, json, or pretty",
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::scalar_to_string;

    #[test]
    fn scalar_pretty_output_is_human_readable() {
        assert_eq!(scalar_to_string(&json!(true)), "true");
        assert_eq!(scalar_to_string(&json!(42)), "42");
        assert_eq!(scalar_to_string(&json!(null)), "<null>");
    }
}
