use serde::Serialize;

use crate::config::{ConfigManager, SecretStore};
use crate::error::AppResult;

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub config_path: String,
    pub config_exists: bool,
    pub api_key_configured: bool,
    pub default_creator: Option<String>,
    pub upload_ready: bool,
    pub warnings: Vec<String>,
}

impl DoctorReport {
    pub fn build<S: SecretStore>(config_manager: &ConfigManager<S>) -> AppResult<Self> {
        let config = config_manager.load()?;
        let api_key_configured = config_manager.get_api_key()?.is_some();
        let default_creator = config_manager.resolve_creator(&config)?;
        let mut warnings = Vec::new();

        if !api_key_configured {
            warnings
                .push("No API key configured. Run `rbxup config set api-key <key>`.".to_string());
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
            default_creator: default_creator.clone(),
            upload_ready: api_key_configured && default_creator.is_some(),
            warnings,
        })
    }
}
