use crate::cli::{
    AuthCommand, Cli, Commands, ConfigCommand, ConfigSetCommand, DoctorArgs, DoctorOutput,
};
use crate::config::{ConfigManager, SecretStore, SystemSecretStore};
use crate::creator::CreatorTarget;
use crate::doctor::DoctorReport;
use crate::error::{AppError, AppResult};
use crate::output::print_json;
use crate::upload::run_upload;

pub async fn run(cli: Cli) -> AppResult<()> {
    let config_manager = ConfigManager::new(SystemSecretStore::new())?;

    match cli.command {
        Commands::Config { command } => run_config(command, &config_manager),
        Commands::Doctor(args) => run_doctor(args, &config_manager),
        Commands::Auth { command } => run_auth(command),
        Commands::Status { operation_id } => Err(AppError::general(format!(
            "status is planned for phase 3. Operation requested: {operation_id}"
        ))),
        Commands::Upload(args) => run_upload(args, &config_manager).await,
    }
}

fn run_config<S: SecretStore>(
    command: ConfigCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    match command {
        ConfigCommand::Get => {
            let config = config_manager.load()?;
            let api_key_configured = config_manager.get_api_key()?.is_some();
            let payload = serde_json::json!({
                "configPath": config_manager.config_path().display().to_string(),
                "defaultCreator": config.default_creator,
                "apiKeyConfigured": api_key_configured,
            });

            print_json(&payload)
        }
        ConfigCommand::Set {
            command: set_command,
        } => {
            let mut config = config_manager.load()?;

            match set_command {
                ConfigSetCommand::ApiKey { key } => {
                    config_manager.set_api_key(&key)?;
                    let payload = serde_json::json!({
                        "stored": true,
                        "field": "apiKey",
                    });
                    print_json(&payload)
                }
                ConfigSetCommand::Creator { creator } => {
                    let creator = CreatorTarget::parse(&creator)?.to_string();
                    config.default_creator = Some(creator);
                    config_manager.save(&config)?;
                    let payload = serde_json::json!({
                        "stored": true,
                        "field": "defaultCreator",
                        "value": config.default_creator,
                    });
                    print_json(&payload)
                }
            }
        }
    }
}

fn run_doctor<S: SecretStore>(
    args: DoctorArgs,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    let report = DoctorReport::build(config_manager)?;

    match args.output {
        DoctorOutput::Json => print_json(&report),
        DoctorOutput::Pretty => {
            println!("Config Path: {}", report.config_path);
            println!("Config Exists: {}", report.config_exists);
            println!("API Key Configured: {}", report.api_key_configured);
            println!(
                "Default Creator: {}",
                report
                    .default_creator
                    .unwrap_or_else(|| "<none>".to_string())
            );
            println!("Upload Ready: {}", report.upload_ready);

            if !report.warnings.is_empty() {
                println!("Warnings:");
                for warning in report.warnings {
                    println!("- {warning}");
                }
            }

            Ok(())
        }
    }
}

fn run_auth(command: AuthCommand) -> AppResult<()> {
    let action = match command {
        AuthCommand::Login => "login",
        AuthCommand::Logout => "logout",
        AuthCommand::Whoami => "whoami",
    };

    Err(AppError::general(format!(
        "auth {action} is planned for phase 7 when OAuth support is added"
    )))
}
