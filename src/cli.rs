use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "rbxup",
    version,
    about = "Upload Roblox assets from the command line"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Upload a single asset file
    Upload(UploadCommand),
    /// Initialize rbxup.toml in the current directory
    Init(InitCommand),
    /// Inspect or change local configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Check local setup health
    Doctor(DoctorArgs),
    /// Manage OAuth authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Check the status of an upload operation
    Status(StatusCommand),
    /// Update an existing asset
    Update(UpdateCommand),
}

#[derive(Debug, Clone, clap::Args)]
pub struct UploadCommand {
    /// File path to upload
    pub path: Option<PathBuf>,
    /// Explicit Roblox asset type
    #[arg(long = "type", value_enum)]
    pub asset_type: Option<UploadAssetType>,
    /// Override the asset display name
    #[arg(long)]
    pub display_name: Option<String>,
    /// Optional asset description
    #[arg(long)]
    pub description: Option<String>,
    /// Upload owner, for example user:123 or group:456
    #[arg(long)]
    pub creator: Option<String>,
    /// Project config profile from rbxup.toml
    #[arg(long)]
    pub profile: Option<String>,
    /// Display name template like {stem} or {parent}_{stem}
    #[arg(long)]
    pub name_template: Option<String>,
    /// Only include files that match this glob. Can be repeated.
    #[arg(long)]
    pub include: Vec<String>,
    /// Exclude files that match this glob. Can be repeated.
    #[arg(long)]
    pub exclude: Vec<String>,
    /// Restrict files by extension, for example png,jpg
    #[arg(long, value_delimiter = ',')]
    pub ext: Vec<String>,
    /// Recurse into subdirectories for folder uploads
    #[arg(long)]
    pub recursive: bool,
    /// Maximum directory depth to scan when uploading folders
    #[arg(long)]
    pub max_depth: Option<usize>,
    /// Maximum number of files to upload
    #[arg(long)]
    pub limit: Option<usize>,
    /// Print the files that would upload without sending requests
    #[arg(long)]
    pub dry_run: bool,
    /// Number of folder uploads to process in parallel
    #[arg(long)]
    pub concurrency: Option<usize>,
    /// Wait for the operation to finish and return the final asset
    #[arg(long = "yield")]
    pub yield_until_done: bool,
    /// Maximum amount of time to wait when --yield is enabled
    #[arg(long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
    /// Delay between status polls when --yield is enabled
    #[arg(long, value_parser = parse_duration)]
    pub poll_interval: Option<Duration>,
    /// Stdout output mode
    #[arg(long, value_enum)]
    pub output: Option<UploadOutput>,
}

#[derive(Debug, Clone, clap::Args)]
pub struct InitCommand {
    /// Overwrite an existing rbxup.toml
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, clap::Args)]
pub struct StatusCommand {
    /// Operation ID like operations/abc123
    pub operation_id: String,
    /// Stdout output mode
    #[arg(long, value_enum, default_value_t = StatusOutput::Json)]
    pub output: StatusOutput,
}

#[derive(Debug, Clone, clap::Args)]
pub struct UpdateCommand {
    /// Roblox asset ID
    pub asset_id: String,
    /// File path to upload as the next version
    pub path: PathBuf,
    /// Explicit Roblox asset type
    #[arg(long = "type", value_enum)]
    pub asset_type: Option<UploadAssetType>,
    /// Upload owner, for example user:123 or group:456
    #[arg(long)]
    pub creator: Option<String>,
    /// Project config profile from rbxup.toml
    #[arg(long)]
    pub profile: Option<String>,
    /// Wait for the operation to finish and return the final asset
    #[arg(long = "yield")]
    pub yield_until_done: bool,
    /// Maximum amount of time to wait when --yield is enabled
    #[arg(long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
    /// Delay between status polls when --yield is enabled
    #[arg(long, value_parser = parse_duration)]
    pub poll_interval: Option<Duration>,
    /// Stdout output mode
    #[arg(long, value_enum)]
    pub output: Option<UploadOutput>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective local configuration
    Get,
    /// Persist a configuration value
    Set {
        #[command(subcommand)]
        command: ConfigSetCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigSetCommand {
    /// Store an API key in the OS secret store
    ApiKey { key: String },
    /// Set the default upload creator, for example user:123 or group:456
    Creator { creator: String },
}

#[derive(Debug, clap::Args)]
pub struct DoctorArgs {
    /// Reserved for future human-friendly output
    #[arg(long, value_enum, default_value_t = DoctorOutput::Json)]
    pub output: DoctorOutput,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DoctorOutput {
    Json,
    Pretty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum UploadAssetType {
    Image,
    Audio,
    Model,
    Animation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum UploadOutput {
    Job,
    Id,
    Json,
    Jsonl,
    Map,
    Pretty,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusOutput {
    Json,
    Id,
    Pretty,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AuthOutput {
    Json,
    Pretty,
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    humantime::parse_duration(value).map_err(|error| format!("invalid duration `{value}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::parse_duration;

    #[test]
    fn parses_human_duration() {
        let duration = parse_duration("5m").expect("duration should parse");
        assert_eq!(duration.as_secs(), 300);
    }
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    Login(AuthLoginCommand),
    Logout,
    Whoami(AuthWhoamiCommand),
}

#[derive(Debug, Clone, clap::Args)]
pub struct AuthLoginCommand {
    /// Roblox OAuth client ID. Stored in config after a successful login.
    #[arg(long)]
    pub client_id: Option<String>,
    /// Local callback port, which must match a registered redirect URL.
    #[arg(long)]
    pub redirect_port: Option<u16>,
    /// OAuth scopes to request. Defaults to openid,profile,asset:write
    #[arg(long = "scope", value_delimiter = ',')]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, clap::Args)]
pub struct AuthWhoamiCommand {
    #[arg(long, value_enum, default_value_t = AuthOutput::Json)]
    pub output: AuthOutput,
}
