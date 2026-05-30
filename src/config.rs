use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const APP_NAME: &str = "rbxup";
const API_KEY_ACCOUNT: &str = "default";
const API_KEY_ENV: &str = "RBXUP_API_KEY";
const CREATOR_ENV: &str = "RBXUP_CREATOR";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub default_creator: Option<String>,
}

pub trait SecretStore {
    fn get_api_key(&self) -> AppResult<Option<String>>;
    fn set_api_key(&self, api_key: &str) -> AppResult<()>;
}

#[derive(Debug, Clone, Default)]
pub struct SystemSecretStore;

impl SystemSecretStore {
    pub fn new() -> Self {
        Self
    }
}

impl SecretStore for SystemSecretStore {
    fn get_api_key(&self) -> AppResult<Option<String>> {
        let entry = Entry::new(APP_NAME, API_KEY_ACCOUNT)
            .map_err(|error| AppError::config(format!("failed to open keyring entry: {error}")))?;

        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(AppError::config(format!(
                "failed to read API key from keyring: {error}"
            ))),
        }
    }

    fn set_api_key(&self, api_key: &str) -> AppResult<()> {
        let entry = Entry::new(APP_NAME, API_KEY_ACCOUNT)
            .map_err(|error| AppError::config(format!("failed to open keyring entry: {error}")))?;

        entry
            .set_password(api_key)
            .map_err(|error| AppError::config(format!("failed to store API key: {error}")))
    }
}

#[derive(Debug, Clone)]
pub struct ConfigManager<S> {
    root_dir: PathBuf,
    secret_store: S,
}

impl<S: SecretStore> ConfigManager<S> {
    pub fn new(secret_store: S) -> AppResult<Self> {
        let base_dir = dirs::config_dir().ok_or_else(|| {
            AppError::config("could not determine the platform config directory".to_string())
        })?;

        Ok(Self::with_root(base_dir.join(APP_NAME), secret_store))
    }

    pub fn with_root(root_dir: PathBuf, secret_store: S) -> Self {
        Self {
            root_dir,
            secret_store,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.root_dir.join("config.toml")
    }

    pub fn config_exists(&self) -> bool {
        self.config_path().exists()
    }

    pub fn load(&self) -> AppResult<AppConfig> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(AppConfig::default());
        }

        let contents = fs::read_to_string(&path).map_err(|error| {
            AppError::config(format!("failed to read {}: {error}", path.display()))
        })?;

        toml::from_str(&contents).map_err(|error| {
            AppError::config(format!("failed to parse {}: {error}", path.display()))
        })
    }

    pub fn save(&self, config: &AppConfig) -> AppResult<()> {
        fs::create_dir_all(&self.root_dir).map_err(|error| {
            AppError::config(format!(
                "failed to create config directory {}: {error}",
                self.root_dir.display()
            ))
        })?;

        let contents = toml::to_string_pretty(config)
            .map_err(|error| AppError::config(format!("failed to serialize config: {error}")))?;
        let path = self.config_path();

        fs::write(&path, contents).map_err(|error| {
            AppError::config(format!("failed to write {}: {error}", path.display()))
        })
    }

    pub fn get_api_key(&self) -> AppResult<Option<String>> {
        match env::var(API_KEY_ENV) {
            Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
            Ok(_) | Err(env::VarError::NotPresent) => self.secret_store.get_api_key(),
            Err(error) => Err(AppError::config(format!(
                "failed to read {API_KEY_ENV}: {error}"
            ))),
        }
    }

    pub fn set_api_key(&self, api_key: &str) -> AppResult<()> {
        if api_key.trim().is_empty() {
            return Err(AppError::invalid_args("API key cannot be empty"));
        }

        self.secret_store.set_api_key(api_key)
    }

    pub fn resolve_creator(&self, config: &AppConfig) -> AppResult<Option<String>> {
        match env::var(CREATOR_ENV) {
            Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
            Ok(_) | Err(env::VarError::NotPresent) => Ok(config.default_creator.clone()),
            Err(error) => Err(AppError::config(format!(
                "failed to read {CREATOR_ENV}: {error}"
            ))),
        }
    }
}

pub fn file_stem(path: &Path) -> AppResult<String> {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            AppError::invalid_args(format!(
                "could not derive a display name from {}",
                path.display()
            ))
        })?;

    Ok(stem.to_string())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Debug, Default)]
    struct MemorySecretStore {
        api_key: RefCell<Option<String>>,
    }

    impl SecretStore for MemorySecretStore {
        fn get_api_key(&self) -> AppResult<Option<String>> {
            Ok(self.api_key.borrow().clone())
        }

        fn set_api_key(&self, api_key: &str) -> AppResult<()> {
            self.api_key.replace(Some(api_key.to_string()));
            Ok(())
        }
    }

    #[test]
    fn saves_and_loads_config_file() {
        let root = std::env::temp_dir().join(format!(
            "rbxup-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let manager = ConfigManager::with_root(root.clone(), MemorySecretStore::default());
        let config = AppConfig {
            default_creator: Some("user:123".to_string()),
        };

        manager.save(&config).expect("save should succeed");
        let loaded = manager.load().expect("load should succeed");

        assert_eq!(loaded.default_creator.as_deref(), Some("user:123"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn derives_file_stem() {
        let stem = file_stem(Path::new("assets/icon.png")).expect("stem");
        assert_eq!(stem, "icon");
    }
}
