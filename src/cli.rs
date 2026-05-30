use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

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
    /// Inspect or change local configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Check local setup health
    Doctor(DoctorArgs),
    /// Future OAuth login flow
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Future operation lookup flow
    Status {
        /// Operation ID like operations/abc123
        operation_id: String,
    },
}

#[derive(Debug, clap::Args)]
pub struct UploadCommand {
    /// File path to upload
    pub path: PathBuf,
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

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    Login,
    Logout,
    Whoami,
}
