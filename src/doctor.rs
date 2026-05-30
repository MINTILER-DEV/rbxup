use serde::Serialize;

use crate::config::{AuthMode, ConfigManager, SecretStore};
use crate::error::AppResult;

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub config_path: String,
    pub config_exists: bool,
    pub api_key_configured: bool,
    pub oauth_session_configured: bool,
    pub auth_mode: String,
    pub default_creator: Option<String>,
    pub upload_ready: bool,
    pub warnings: Vec<String>,
}

impl DoctorReport {
    pub fn build<S: SecretStore>(config_manager: &ConfigManager<S>) -> AppResult<Self> {
        let config = config_manager.load()?;
        let api_key_configured = config_manager.get_api_key()?.is_some();
        let oauth_session_configured = config_manager.get_oauth_session()?.is_some();
        let auth_mode = config_manager.resolve_auth_mode(&config)?;
        let default_creator = config_manager.resolve_creator(&config)?;
        let mut warnings = Vec::new();

        if !api_key_configured && !oauth_session_configured {
            warnings.push(
                "No auth configured. Run `rbxup config set api-key <key>` or `rbxup auth login --client-id <id>`."
                    .to_string(),
            );
        }

        if default_creator.is_none() {
            warnings.push(
                "No default creator configured. Run `rbxup config set creator user:<id>` or pass --creator later."
                    .to_string(),
            );
        }

        Ok(Self {
            config_path: config_manager.config_path().display().to_string(),
            config_exists: config_manager.config_exists(),
            api_key_configured,
            oauth_session_configured,
            auth_mode: auth_mode_label(auth_mode).to_string(),
            default_creator: default_creator.clone(),
            upload_ready: (api_key_configured || oauth_session_configured)
                && default_creator.is_some(),
            warnings,
        })
    }
}

fn auth_mode_label(mode: AuthMode) -> &'static str {
    match mode {
        AuthMode::ApiKey => "api_key",
        AuthMode::OAuth => "oauth",
        AuthMode::Auto => "auto",
    }
}
