use crate::auth::{login as auth_login, logout as auth_logout, whoami as auth_whoami};
use crate::cli::{
    AuthCommand, AuthOutput, Cli, Commands, ConfigCommand, ConfigSetCommand, DoctorArgs,
    DoctorOutput, InitCommand,
};
use crate::config::{ConfigManager, SecretStore, SystemSecretStore};
use crate::creator::CreatorTarget;
use crate::doctor::DoctorReport;
use crate::error::AppResult;
use crate::output::print_json;
use crate::project::init_project_config;
use crate::status::run_status;
use crate::update::run_update;
use crate::upload::run_upload;

fn auth_mode_label(mode: crate::config::AuthMode) -> &'static str {
    match mode {
        crate::config::AuthMode::ApiKey => "api_key",
        crate::config::AuthMode::OAuth => "oauth",
        crate::config::AuthMode::Auto => "auto",
    }
}

pub async fn run(cli: Cli) -> AppResult<()> {
    let config_manager = ConfigManager::new(SystemSecretStore::new())?;

    match cli.command {
        Commands::Init(args) => run_init(args),
        Commands::Config { command } => run_config(command, &config_manager),
        Commands::Doctor(args) => run_doctor(args, &config_manager),
        Commands::Auth { command } => run_auth(command, &config_manager).await,
        Commands::Status(args) => run_status(args, &config_manager).await,
        Commands::Update(args) => run_update(args, &config_manager).await,
        Commands::Upload(args) => run_upload(args, &config_manager).await,
    }
}

fn run_init(args: InitCommand) -> AppResult<()> {
    let path = init_project_config(args.force)?;
    let payload = serde_json::json!({
        "created": true,
        "path": path.display().to_string(),
    });
    print_json(&payload)
}

fn run_config<S: SecretStore>(
    command: ConfigCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    match command {
        ConfigCommand::Get => {
            let config = config_manager.load()?;
            let api_key_configured = config_manager.get_api_key()?.is_some();
            let oauth_session_configured = config_manager.get_oauth_session()?.is_some();
            let auth_mode = auth_mode_label(config_manager.resolve_auth_mode(&config)?);
            let payload = serde_json::json!({
                "configPath": config_manager.config_path().display().to_string(),
                "defaultCreator": config.default_creator,
                "apiKeyConfigured": api_key_configured,
                "oauthSessionConfigured": oauth_session_configured,
                "authMode": auth_mode,
                "oauthClientId": config.auth.oauth_client_id,
                "oauthRedirectPort": config.auth.oauth_redirect_port,
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
                    config.auth.mode = crate::config::AuthMode::ApiKey;
                    config_manager.save(&config)?;
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
            println!("Auth Mode: {}", report.auth_mode);
            println!("API Key Configured: {}", report.api_key_configured);
            println!(
                "OAuth Session Configured: {}",
                report.oauth_session_configured
            );
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

async fn run_auth<S: SecretStore>(
    command: AuthCommand,
    config_manager: &ConfigManager<S>,
) -> AppResult<()> {
    match command {
        AuthCommand::Login(args) => {
            let status = auth_login(config_manager, args).await?;
            print_json(&status)
        }
        AuthCommand::Logout => {
            let status = auth_logout(config_manager).await?;
            print_json(&status)
        }
        AuthCommand::Whoami(args) => {
            let status = auth_whoami(config_manager).await?;
            match args.output {
                AuthOutput::Json => print_json(&status),
                AuthOutput::Pretty => {
                    println!("Auth Mode: {}", status.auth_mode);
                    if let Some(user_id) = status.user_id {
                        println!("User ID: {user_id}");
                    }
                    if let Some(username) = status.username {
                        println!("Username: {username}");
                    }
                    if let Some(display_name) = status.display_name {
                        println!("Display Name: {display_name}");
                    }
                    if let Some(client_id) = status.client_id {
                        println!("Client ID: {client_id}");
                    }
                    if let Some(expires_at_unix) = status.expires_at_unix {
                        println!("Expires At Unix: {expires_at_unix}");
                    }
                    if let Some(scopes) = status.scopes {
                        println!("Scopes: {}", scopes.join(", "));
                    }
                    Ok(())
                }
            }
        }
    }
}
