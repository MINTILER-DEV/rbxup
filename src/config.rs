use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const APP_NAME: &str = "rbxup";
const API_KEY_ACCOUNT: &str = "api-key";
const OAUTH_SESSION_ACCOUNT: &str = "oauth-session";
const API_KEY_FALLBACK_FILE: &str = "api-key";
const API_KEY_ENV: &str = "RBXUP_API_KEY";
const CREATOR_ENV: &str = "RBXUP_CREATOR";
const OAUTH_CLIENT_ID_ENV: &str = "RBXUP_OAUTH_CLIENT_ID";
const OAUTH_REDIRECT_PORT_ENV: &str = "RBXUP_OAUTH_REDIRECT_PORT";
const AUTH_MODE_ENV: &str = "RBXUP_AUTH_MODE";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub default_creator: Option<String>,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub mode: AuthMode,
    #[serde(default)]
    pub oauth_client_id: Option<String>,
    #[serde(default)]
    pub oauth_redirect_port: Option<u16>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    ApiKey,
    OAuth,
    #[default]
    Auto,
}

pub trait SecretStore {
    fn get_api_key(&self) -> AppResult<Option<String>>;
    fn set_api_key(&self, api_key: &str) -> AppResult<()>;
    fn get_oauth_session(&self) -> AppResult<Option<String>>;
    fn set_oauth_session(&self, session_json: &str) -> AppResult<()>;
    fn delete_oauth_session(&self) -> AppResult<()>;
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
        read_keyring_secret(API_KEY_ACCOUNT)
    }

    fn set_api_key(&self, api_key: &str) -> AppResult<()> {
        write_keyring_secret(API_KEY_ACCOUNT, api_key)
    }

    fn get_oauth_session(&self) -> AppResult<Option<String>> {
        read_keyring_secret(OAUTH_SESSION_ACCOUNT)
    }

    fn set_oauth_session(&self, session_json: &str) -> AppResult<()> {
        write_keyring_secret(OAUTH_SESSION_ACCOUNT, session_json)
    }

    fn delete_oauth_session(&self) -> AppResult<()> {
        let entry = Entry::new(APP_NAME, OAUTH_SESSION_ACCOUNT)
            .map_err(|error| AppError::config(format!("failed to open keyring entry: {error}")))?;

        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(AppError::config(format!(
                "failed to delete OAuth session: {error}"
            ))),
        }
    }
}

fn read_keyring_secret(account: &str) -> AppResult<Option<String>> {
    let entry = Entry::new(APP_NAME, account)
        .map_err(|error| AppError::config(format!("failed to open keyring entry: {error}")))?;

    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(AppError::config(format!(
            "failed to read secret from keyring: {error}"
        ))),
    }
}

fn write_keyring_secret(account: &str, value: &str) -> AppResult<()> {
    let entry = Entry::new(APP_NAME, account)
        .map_err(|error| AppError::config(format!("failed to open keyring entry: {error}")))?;

    entry
        .set_password(value)
        .map_err(|error| AppError::config(format!("failed to store secret: {error}")))
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

    fn api_key_fallback_path(&self) -> PathBuf {
        self.root_dir.join(API_KEY_FALLBACK_FILE)
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
            Ok(_) | Err(env::VarError::NotPresent) => {
                if let Some(api_key) = self.secret_store.get_api_key()? {
                    return Ok(Some(api_key));
                }

                self.read_api_key_fallback()
            }
            Err(error) => Err(AppError::config(format!(
                "failed to read {API_KEY_ENV}: {error}"
            ))),
        }
    }

    pub fn set_api_key(&self, api_key: &str) -> AppResult<()> {
        if api_key.trim().is_empty() {
            return Err(AppError::invalid_args("API key cannot be empty"));
        }

        match self.secret_store.set_api_key(api_key) {
            Ok(()) => match self.secret_store.get_api_key()? {
                Some(stored) if stored == api_key => {
                    self.delete_api_key_fallback()?;
                    Ok(())
                }
                _ => self.write_api_key_fallback(api_key),
            },
            Err(_) => self.write_api_key_fallback(api_key),
        }
    }

    pub fn get_oauth_session(&self) -> AppResult<Option<String>> {
        self.secret_store.get_oauth_session()
    }

    pub fn set_oauth_session(&self, session_json: &str) -> AppResult<()> {
        self.secret_store.set_oauth_session(session_json)
    }

    pub fn clear_oauth_session(&self) -> AppResult<()> {
        self.secret_store.delete_oauth_session()
    }

    fn read_api_key_fallback(&self) -> AppResult<Option<String>> {
        let path = self.api_key_fallback_path();
        if !path.exists() {
            return Ok(None);
        }

        let value = fs::read_to_string(&path).map_err(|error| {
            AppError::config(format!("failed to read {}: {error}", path.display()))
        })?;
        let value = value.trim().to_string();

        if value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    fn write_api_key_fallback(&self, api_key: &str) -> AppResult<()> {
        fs::create_dir_all(&self.root_dir).map_err(|error| {
            AppError::config(format!(
                "failed to create config directory {}: {error}",
                self.root_dir.display()
            ))
        })?;

        let path = self.api_key_fallback_path();
        fs::write(&path, api_key).map_err(|error| {
            AppError::config(format!("failed to write {}: {error}", path.display()))
        })
    }

    fn delete_api_key_fallback(&self) -> AppResult<()> {
        let path = self.api_key_fallback_path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::config(format!(
                "failed to remove {}: {error}",
                path.display()
            ))),
        }
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

    pub fn resolve_auth_mode(&self, config: &AppConfig) -> AppResult<AuthMode> {
        match env::var(AUTH_MODE_ENV) {
            Ok(value) => parse_auth_mode(&value),
            Err(env::VarError::NotPresent) => Ok(config.auth.mode),
            Err(error) => Err(AppError::config(format!(
                "failed to read {AUTH_MODE_ENV}: {error}"
            ))),
        }
    }

    pub fn resolve_oauth_client_id(&self, config: &AppConfig) -> AppResult<Option<String>> {
        match env::var(OAUTH_CLIENT_ID_ENV) {
            Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
            Ok(_) | Err(env::VarError::NotPresent) => Ok(config.auth.oauth_client_id.clone()),
            Err(error) => Err(AppError::config(format!(
                "failed to read {OAUTH_CLIENT_ID_ENV}: {error}"
            ))),
        }
    }

    pub fn resolve_oauth_redirect_port(&self, config: &AppConfig) -> AppResult<Option<u16>> {
        match env::var(OAUTH_REDIRECT_PORT_ENV) {
            Ok(value) if !value.trim().is_empty() => {
                value.parse::<u16>().map(Some).map_err(|error| {
                    AppError::config(format!(
                        "failed to parse {OAUTH_REDIRECT_PORT_ENV}: {error}"
                    ))
                })
            }
            Ok(_) | Err(env::VarError::NotPresent) => Ok(config.auth.oauth_redirect_port),
            Err(error) => Err(AppError::config(format!(
                "failed to read {OAUTH_REDIRECT_PORT_ENV}: {error}"
            ))),
        }
    }
}

fn parse_auth_mode(value: &str) -> AppResult<AuthMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "api_key" | "api-key" => Ok(AuthMode::ApiKey),
        "oauth" => Ok(AuthMode::OAuth),
        "auto" => Ok(AuthMode::Auto),
        other => Err(AppError::config(format!(
            "invalid auth mode `{other}`. Expected auto, api-key, or oauth"
        ))),
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
        oauth_session: RefCell<Option<String>>,
    }

    impl SecretStore for MemorySecretStore {
        fn get_api_key(&self) -> AppResult<Option<String>> {
            Ok(self.api_key.borrow().clone())
        }

        fn set_api_key(&self, api_key: &str) -> AppResult<()> {
            self.api_key.replace(Some(api_key.to_string()));
            Ok(())
        }

        fn get_oauth_session(&self) -> AppResult<Option<String>> {
            Ok(self.oauth_session.borrow().clone())
        }

        fn set_oauth_session(&self, session_json: &str) -> AppResult<()> {
            self.oauth_session.replace(Some(session_json.to_string()));
            Ok(())
        }

        fn delete_oauth_session(&self) -> AppResult<()> {
            self.oauth_session.replace(None);
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
            auth: AuthConfig {
                mode: AuthMode::OAuth,
                oauth_client_id: Some("client".to_string()),
                oauth_redirect_port: Some(9785),
            },
        };

        manager.save(&config).expect("save should succeed");
        let loaded = manager.load().expect("load should succeed");

        assert_eq!(loaded.default_creator.as_deref(), Some("user:123"));
        assert_eq!(loaded.auth.mode, AuthMode::OAuth);
        assert_eq!(loaded.auth.oauth_client_id.as_deref(), Some("client"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn derives_file_stem() {
        let stem = file_stem(Path::new("assets/icon.png")).expect("stem");
        assert_eq!(stem, "icon");
    }

    #[derive(Debug, Default)]
    struct BrokenApiKeyStore;

    impl SecretStore for BrokenApiKeyStore {
        fn get_api_key(&self) -> AppResult<Option<String>> {
            Ok(None)
        }

        fn set_api_key(&self, _api_key: &str) -> AppResult<()> {
            Ok(())
        }

        fn get_oauth_session(&self) -> AppResult<Option<String>> {
            Ok(None)
        }

        fn set_oauth_session(&self, _session_json: &str) -> AppResult<()> {
            Ok(())
        }

        fn delete_oauth_session(&self) -> AppResult<()> {
            Ok(())
        }
    }

    #[test]
    fn falls_back_to_local_file_when_secret_store_does_not_return_api_key() {
        let root = std::env::temp_dir().join(format!(
            "rbxup-test-fallback-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let manager = ConfigManager::with_root(root.clone(), BrokenApiKeyStore);

        manager
            .set_api_key("test-key")
            .expect("fallback write should succeed");

        let stored = manager.get_api_key().expect("fallback read should succeed");
        assert_eq!(stored.as_deref(), Some("test-key"));
        assert!(manager.api_key_fallback_path().exists());

        fs::remove_dir_all(root).expect("cleanup");
    }
}
