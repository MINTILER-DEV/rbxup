use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use tokio::task::JoinSet;
use tokio::time::sleep;
use walkdir::WalkDir;

use crate::auth::resolve_auth_provider;
use crate::cli::{UploadAssetType, UploadCommand, UploadOutput};
use crate::config::{ConfigManager, SecretStore, file_stem};
use crate::creator::CreatorTarget;
use crate::error::{AppError, AppResult};
use crate::output::{print_json, print_json_compact};
use crate::roblox::{CreateAssetParams, RobloxAssetsClient};
use crate::status::wait_for_operation;

const MAX_UPLOAD_SIZE_BYTES: u64 = 20 * 1024 * 1024;
const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 2;
const DEFAULT_YIELD_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_BULK_CONCURRENCY: usize = 3;
const MAX_RATE_LIMIT_RETRIES: u32 = 4;

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

#[derive(Debug, Clone)]
struct UploadItem {
    absolute_path: PathBuf,
    output_path: String,
}

#[derive(Debug)]
enum UploadPlan {
    Single(UploadItem),
    Bulk(Vec<UploadItem>),
}

#[derive(Debug, Clone)]
struct PreparedUpload {
    file_path: PathBuf,
    output_path: String,
    asset_type: AssetType,
    content_type: &'static str,
    display_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct UploadResult {
    file: String,
    #[serde(rename = "operationId", skip_serializing_if = "Option::is_none")]
    operation_id: Option<String>,
    #[serde(rename = "assetId", skip_serializing_if = "Option::is_none")]
    asset_id: Option<String>,
    #[serde(rename = "assetType", skip_serializing_if = "Option::is_none")]
    asset_type: Option<String>,
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Default)]
struct MatcherConfig {
    include: Vec<String>,
    exclude: Vec<String>,
    extensions: BTreeSet<String>,
}

#[derive(Debug)]
struct BulkMatcher {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
    extensions: BTreeSet<String>,
}

impl BulkMatcher {
    fn build(config: MatcherConfig) -> AppResult<Self> {
        Ok(Self {
            include: build_glob_set(&config.include)?,
            exclude: build_glob_set(&config.exclude)?,
            extensions: config.extensions,
        })
    }

    fn matches(&self, relative_path: &Path) -> bool {
        let relative = relative_path.to_string_lossy().replace('\\', "/");

        if let Some(include) = &self.include {
            if !include.is_match(&relative) {
                return false;
            }
        }

        if let Some(exclude) = &self.exclude {
            if exclude.is_match(&relative) {
                return false;
            }
        }

        if self.extensions.is_empty() {
            return true;
        }

        relative_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .is_some_and(|value| self.extensions.contains(&value))
    }
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
            "file or folder does not exist: {}",
            args.path.display()
        )));
    }

    let plan = build_upload_plan(&args)?;

    if args.dry_run {
        return print_dry_run(&plan);
    }

    let config = config_manager.load()?;
    let auth_provider = resolve_auth_provider(config_manager).await?;
    let creator = resolve_creator(&args, config_manager, &config)?;
    let client = RobloxAssetsClient::new(auth_provider);

    match plan {
        UploadPlan::Single(item) => {
            let prepared = prepare_upload(item, args.asset_type)?;
            let output_mode = args.output.unwrap_or(if args.yield_until_done {
                UploadOutput::Id
            } else {
                UploadOutput::Job
            });
            let result =
                execute_upload(&client, prepared, creator, args.yield_until_done, &args).await?;
            print_single_result(result, output_mode)
        }
        UploadPlan::Bulk(items) => {
            let output_mode = args.output.unwrap_or(UploadOutput::Jsonl);
            validate_bulk_output(output_mode)?;
            let results = execute_bulk_uploads(client, items, creator, args.clone()).await?;
            print_bulk_results(&results, output_mode, args.yield_until_done)
                .and_then(|_| summarize_bulk_results(&results))
        }
    }
}

fn validate_bulk_output(output: UploadOutput) -> AppResult<()> {
    match output {
        UploadOutput::Json | UploadOutput::Jsonl | UploadOutput::Map | UploadOutput::Pretty => {
            Ok(())
        }
        UploadOutput::Job | UploadOutput::Id => Err(AppError::invalid_args(
            "folder uploads support --output json, jsonl, map, or pretty",
        )),
    }
}

fn build_upload_plan(args: &UploadCommand) -> AppResult<UploadPlan> {
    if args.path.is_file() {
        validate_single_file_flags(args)?;

        return Ok(UploadPlan::Single(UploadItem {
            absolute_path: args.path.clone(),
            output_path: args
                .path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_else(|| args.path.as_os_str().to_str().unwrap_or_default())
                .to_string(),
        }));
    }

    if !args.path.is_dir() {
        return Err(AppError::invalid_args(format!(
            "upload path must be a file or folder: {}",
            args.path.display()
        )));
    }

    let matcher = BulkMatcher::build(MatcherConfig {
        include: args.include.clone(),
        exclude: args.exclude.clone(),
        extensions: args
            .ext
            .iter()
            .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect(),
    })?;
    let max_depth = if args.recursive {
        args.max_depth.map(|value| value + 1)
    } else {
        Some(1)
    };
    let mut walker = WalkDir::new(&args.path).min_depth(1);
    if let Some(max_depth) = max_depth {
        walker = walker.max_depth(max_depth);
    }

    let mut items = Vec::new();
    for entry in walker {
        let entry = entry.map_err(|error| {
            AppError::invalid_args(format!("failed to scan {}: {error}", args.path.display()))
        })?;

        if !entry.file_type().is_file() {
            continue;
        }

        let absolute_path = entry.into_path();
        let relative_path = absolute_path
            .strip_prefix(&args.path)
            .map_err(|error| {
                AppError::general(format!(
                    "failed to compute a relative file path for {}: {error}",
                    absolute_path.display()
                ))
            })?
            .to_path_buf();

        if !matcher.matches(&relative_path) {
            continue;
        }

        items.push(UploadItem {
            absolute_path,
            output_path: relative_path.to_string_lossy().replace('\\', "/"),
        });

        if let Some(limit) = args.limit {
            if items.len() >= limit {
                break;
            }
        }
    }

    if items.is_empty() {
        return Err(AppError::invalid_args(format!(
            "no files matched {}",
            args.path.display()
        )));
    }

    Ok(UploadPlan::Bulk(items))
}

fn validate_single_file_flags(args: &UploadCommand) -> AppResult<()> {
    if !args.include.is_empty()
        || !args.exclude.is_empty()
        || !args.ext.is_empty()
        || args.recursive
        || args.max_depth.is_some()
        || args.limit.is_some()
        || args.concurrency.is_some()
    {
        return Err(AppError::invalid_args(
            "include/exclude/ext/recursive/max-depth/limit/concurrency are only supported for folder uploads",
        ));
    }

    Ok(())
}

fn prepare_upload(
    item: UploadItem,
    asset_type_override: Option<UploadAssetType>,
) -> AppResult<PreparedUpload> {
    let metadata = fs::metadata(&item.absolute_path).map_err(|error| {
        AppError::invalid_args(format!(
            "failed to inspect {}: {error}",
            item.absolute_path.display()
        ))
    })?;

    if metadata.len() > MAX_UPLOAD_SIZE_BYTES {
        return Err(AppError::invalid_args(format!(
            "{} is {} bytes, which exceeds the 20 MB Roblox upload limit",
            item.absolute_path.display(),
            metadata.len()
        )));
    }

    let asset_type = AssetType::from_cli(asset_type_override, &item.absolute_path)?;

    Ok(PreparedUpload {
        content_type: asset_type.content_type(&item.absolute_path)?,
        display_name: file_stem(&item.absolute_path)?,
        asset_type,
        file_path: item.absolute_path,
        output_path: item.output_path,
    })
}

async fn execute_upload(
    client: &RobloxAssetsClient,
    prepared: PreparedUpload,
    creator: CreatorTarget,
    yield_until_done: bool,
    args: &UploadCommand,
) -> AppResult<UploadResult> {
    let file_name = prepared
        .file_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            AppError::invalid_args(format!(
                "could not determine the file name for {}",
                prepared.file_path.display()
            ))
        })?
        .to_string();
    let description = args
        .description
        .clone()
        .filter(|value| !value.trim().is_empty());
    let file_bytes = fs::read(&prepared.file_path).map_err(|error| {
        AppError::upload(format!(
            "failed to read {}: {error}",
            prepared.file_path.display()
        ))
    })?;
    let create_response = client
        .create_asset(CreateAssetParams {
            asset_type: prepared.asset_type.api_name().to_string(),
            display_name: prepared.display_name.clone(),
            description,
            creator,
            file_name,
            file_bytes,
            content_type: prepared.content_type,
        })
        .await?;

    let mut result = UploadResult {
        file: prepared.output_path,
        operation_id: Some(create_response.path.clone()),
        asset_id: None,
        asset_type: Some(prepared.asset_type.api_name().to_string()),
        display_name: Some(prepared.display_name),
        error: None,
        message: None,
    };

    if yield_until_done {
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
            wait_for_operation(client, &create_response.path, poll_interval, timeout).await?;
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
        result.asset_id = Some(asset_id.to_string());
    }

    Ok(result)
}

async fn execute_bulk_uploads(
    client: RobloxAssetsClient,
    items: Vec<UploadItem>,
    creator: CreatorTarget,
    args: UploadCommand,
) -> AppResult<Vec<UploadResult>> {
    let concurrency = args.concurrency.unwrap_or(DEFAULT_BULK_CONCURRENCY);
    if concurrency == 0 {
        return Err(AppError::invalid_args("--concurrency must be at least 1"));
    }

    let total = items.len();
    let mut results = Vec::with_capacity(total);
    let mut join_set = JoinSet::new();
    let mut next_index = 0usize;
    let mut items_iter = items.into_iter().enumerate();

    while next_index < concurrency {
        if let Some((index, item)) = items_iter.next() {
            spawn_bulk_task(
                &mut join_set,
                client.clone(),
                creator.clone(),
                args.clone(),
                index,
                item,
            );
            next_index += 1;
        } else {
            break;
        }
    }

    while let Some(joined) = join_set.join_next().await {
        let (index, result) = joined.map_err(|error| {
            AppError::general(format!("bulk upload task failed unexpectedly: {error}"))
        })?;
        results.push((index, result));

        if let Some((index, item)) = items_iter.next() {
            spawn_bulk_task(
                &mut join_set,
                client.clone(),
                creator.clone(),
                args.clone(),
                index,
                item,
            );
        }
    }

    results.sort_by_key(|(index, _)| *index);
    Ok(results.into_iter().map(|(_, result)| result).collect())
}

fn spawn_bulk_task(
    join_set: &mut JoinSet<(usize, UploadResult)>,
    client: RobloxAssetsClient,
    creator: CreatorTarget,
    args: UploadCommand,
    index: usize,
    item: UploadItem,
) {
    join_set.spawn(async move {
        let output_path = item.output_path.clone();
        let result = match prepare_upload(item, args.asset_type) {
            Ok(prepared) => execute_upload_with_retries(
                &client,
                prepared,
                creator,
                args.yield_until_done,
                &args,
            )
            .await
            .unwrap_or_else(|error| failure_result(output_path, error)),
            Err(error) => failure_result(output_path, error),
        };

        (index, result)
    });
}

async fn execute_upload_with_retries(
    client: &RobloxAssetsClient,
    prepared: PreparedUpload,
    creator: CreatorTarget,
    yield_until_done: bool,
    args: &UploadCommand,
) -> AppResult<UploadResult> {
    let mut retry = 0u32;

    loop {
        match execute_upload(
            client,
            prepared.clone(),
            creator.clone(),
            yield_until_done,
            args,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(error) if error.is_rate_limited() && retry < MAX_RATE_LIMIT_RETRIES => {
                retry += 1;
                sleep(rate_limit_backoff(retry)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn rate_limit_backoff(retry: u32) -> std::time::Duration {
    let seconds = 2u64.saturating_pow(retry.saturating_sub(1).min(4));
    std::time::Duration::from_secs(seconds.max(1))
}

fn failure_result(file: String, error: AppError) -> UploadResult {
    UploadResult {
        file,
        operation_id: None,
        asset_id: None,
        asset_type: None,
        display_name: None,
        error: Some(error_label(error.code()).to_string()),
        message: Some(error.to_string()),
    }
}

fn error_label(code: crate::error::ExitCode) -> &'static str {
    match code {
        crate::error::ExitCode::General => "GeneralError",
        crate::error::ExitCode::Auth => "AuthError",
        crate::error::ExitCode::Config => "ConfigError",
        crate::error::ExitCode::UploadFailed => "UploadFailed",
        crate::error::ExitCode::PartialFailure => "PartialFailure",
        crate::error::ExitCode::RateLimited => "RateLimited",
        crate::error::ExitCode::Timeout => "Timeout",
        crate::error::ExitCode::InvalidArguments => "InvalidArguments",
    }
}

fn summarize_bulk_results(results: &[UploadResult]) -> AppResult<()> {
    let failures = results
        .iter()
        .filter(|result| result.error.is_some())
        .count();
    if failures == 0 {
        return Ok(());
    }

    if failures == results.len() {
        Err(AppError::upload(format!(
            "all {} uploads failed",
            results.len()
        )))
    } else {
        Err(AppError::partial_failure(format!(
            "{} of {} uploads failed",
            failures,
            results.len()
        )))
    }
}

fn print_single_result(result: UploadResult, output_mode: UploadOutput) -> AppResult<()> {
    match output_mode {
        UploadOutput::Job => {
            println!(
                "{}",
                result
                    .operation_id
                    .as_deref()
                    .ok_or_else(|| AppError::general("missing operation id"))?
            );
            Ok(())
        }
        UploadOutput::Id => {
            println!(
                "{}",
                result
                    .asset_id
                    .as_deref()
                    .ok_or_else(|| AppError::invalid_args(
                        "--output id requires --yield for uploads"
                    ))?
            );
            Ok(())
        }
        UploadOutput::Json => print_json(&result),
        UploadOutput::Jsonl => print_json_compact(&result),
        UploadOutput::Map => {
            let value = if let Some(asset_id) = result.asset_id {
                serde_json::json!({ result.file: asset_id })
            } else {
                serde_json::json!({
                    result.file: result.operation_id.ok_or_else(|| AppError::general("missing operation id"))?
                })
            };
            print_json(&value)
        }
        UploadOutput::Pretty => {
            if let Some(operation_id) = &result.operation_id {
                println!("Operation ID: {operation_id}");
            }
            if let Some(asset_id) = &result.asset_id {
                println!("Asset ID: {asset_id}");
            }
            println!("File: {}", result.file);
            if let Some(asset_type) = &result.asset_type {
                println!("Asset Type: {asset_type}");
            }
            if let Some(display_name) = &result.display_name {
                println!("Display Name: {display_name}");
            }
            Ok(())
        }
    }
}

fn print_bulk_results(
    results: &[UploadResult],
    output_mode: UploadOutput,
    yield_until_done: bool,
) -> AppResult<()> {
    match output_mode {
        UploadOutput::Json => print_json(results),
        UploadOutput::Jsonl => {
            for result in results {
                print_json_compact(result)?;
            }
            Ok(())
        }
        UploadOutput::Map => {
            let mut value = BTreeMap::new();
            for result in results {
                let mapped_value = if yield_until_done {
                    result.asset_id.clone()
                } else {
                    result.operation_id.clone()
                };

                if let Some(mapped_value) = mapped_value {
                    value.insert(result.file.clone(), mapped_value);
                }
            }
            print_json(&value)
        }
        UploadOutput::Pretty => {
            println!("Uploaded {} file(s):", results.len());
            for result in results {
                if let Some(error) = &result.error {
                    println!(
                        "{} -> {} ({})",
                        result.file,
                        error,
                        result.message.as_deref().unwrap_or("no additional details")
                    );
                } else if let Some(asset_id) = &result.asset_id {
                    println!("{} -> {}", result.file, asset_id);
                } else if let Some(operation_id) = &result.operation_id {
                    println!("{} -> {}", result.file, operation_id);
                }
            }
            Ok(())
        }
        UploadOutput::Job | UploadOutput::Id => Err(AppError::invalid_args(
            "folder uploads support --output json, jsonl, map, or pretty",
        )),
    }
}

fn print_dry_run(plan: &UploadPlan) -> AppResult<()> {
    match plan {
        UploadPlan::Single(item) => {
            println!("Would upload 1 file:");
            println!("{}", item.output_path);
        }
        UploadPlan::Bulk(items) => {
            println!("Would upload {} files:", items.len());
            for item in items {
                println!("{}", item.output_path);
            }
        }
    }

    Ok(())
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

fn build_glob_set(patterns: &[String]) -> AppResult<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|error| {
            AppError::invalid_args(format!("invalid glob pattern `{pattern}`: {error}"))
        })?);
    }

    builder
        .build()
        .map(Some)
        .map_err(|error| AppError::invalid_args(format!("invalid glob configuration: {error}")))
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

    use super::{AssetType, BulkMatcher, MatcherConfig, UploadResult, summarize_bulk_results};

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

    #[test]
    fn matcher_respects_include_and_exclude_rules() {
        let matcher = BulkMatcher::build(MatcherConfig {
            include: vec!["**/*.png".to_string()],
            exclude: vec!["**/drafts/**".to_string()],
            extensions: Default::default(),
        })
        .expect("matcher");

        assert!(matcher.matches(Path::new("ui/button.png")));
        assert!(!matcher.matches(Path::new("drafts/button.png")));
    }

    #[test]
    fn summarize_bulk_results_uses_partial_failure_exit_code() {
        let results = vec![
            UploadResult {
                file: "good.png".to_string(),
                operation_id: Some("operations/a".to_string()),
                asset_id: None,
                asset_type: None,
                display_name: None,
                error: None,
                message: None,
            },
            UploadResult {
                file: "bad.txt".to_string(),
                operation_id: None,
                asset_id: None,
                asset_type: None,
                display_name: None,
                error: Some("InvalidArguments".to_string()),
                message: Some("bad".to_string()),
            },
        ];

        let error = summarize_bulk_results(&results).expect_err("summary should fail");
        assert_eq!(error.exit_code(), 5);
    }
}
