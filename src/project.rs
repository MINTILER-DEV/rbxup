use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::{UploadAssetType, UploadCommand, UploadOutput};
use crate::error::{AppError, AppResult};

pub const PROJECT_CONFIG_FILE_NAME: &str = "rbxup.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(flatten)]
    defaults: ProjectSection,
    #[serde(default)]
    profiles: BTreeMap<String, ProjectSection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSection {
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default)]
    creator: Option<String>,
    #[serde(rename = "type", default)]
    asset_type: Option<UploadAssetType>,
    #[serde(default)]
    recursive: Option<bool>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    ext: Vec<String>,
    #[serde(default)]
    max_depth: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    output: Option<UploadOutput>,
    #[serde(default)]
    concurrency: Option<usize>,
    #[serde(default)]
    upload: UploadProjectConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UploadProjectConfig {
    #[serde(default)]
    name_template: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectContext {
    root_dir: PathBuf,
    config: ProjectConfig,
    profile_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedUploadSettings {
    pub path: Option<PathBuf>,
    pub creator: Option<String>,
    pub asset_type: Option<UploadAssetType>,
    pub recursive: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub ext: Vec<String>,
    pub max_depth: Option<usize>,
    pub limit: Option<usize>,
    pub output: Option<UploadOutput>,
    pub concurrency: Option<usize>,
    pub name_template: Option<String>,
}

pub fn load_project_context(profile_name: Option<&str>) -> AppResult<Option<ProjectContext>> {
    let cwd = env::current_dir().map_err(|error| {
        AppError::config(format!("failed to read the current directory: {error}"))
    })?;
    let Some(config_path) = find_project_config(&cwd) else {
        if profile_name.is_some() {
            return Err(AppError::config(format!(
                "no {PROJECT_CONFIG_FILE_NAME} found in {} or its parents",
                cwd.display()
            )));
        }

        return Ok(None);
    };

    let contents = fs::read_to_string(&config_path).map_err(|error| {
        AppError::config(format!("failed to read {}: {error}", config_path.display()))
    })?;
    let config = toml::from_str::<ProjectConfig>(&contents).map_err(|error| {
        AppError::config(format!(
            "failed to parse {}: {error}",
            config_path.display()
        ))
    })?;

    if let Some(profile_name) = profile_name {
        if !config.profiles.contains_key(profile_name) {
            return Err(AppError::config(format!(
                "profile `{profile_name}` was not found in {}",
                config_path.display()
            )));
        }
    }

    Ok(Some(ProjectContext {
        root_dir: config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        config,
        profile_name: profile_name.map(ToOwned::to_owned),
    }))
}

pub fn init_project_config(force: bool) -> AppResult<PathBuf> {
    let cwd = env::current_dir().map_err(|error| {
        AppError::config(format!("failed to read the current directory: {error}"))
    })?;
    let path = cwd.join(PROJECT_CONFIG_FILE_NAME);
    if path.exists() && !force {
        return Err(AppError::config(format!(
            "{} already exists. Re-run with --force to overwrite it",
            path.display()
        )));
    }

    fs::write(&path, default_project_config()).map_err(|error| {
        AppError::config(format!("failed to write {}: {error}", path.display()))
    })?;

    Ok(path)
}

impl ProjectContext {
    pub fn resolve_upload_settings(&self, args: &UploadCommand) -> ResolvedUploadSettings {
        let profile = self
            .profile_name
            .as_ref()
            .and_then(|name| self.config.profiles.get(name));

        let mut include = self.config.defaults.include.clone();
        if let Some(profile) = profile {
            include.extend(profile.include.clone());
        }
        include.extend(args.include.clone());

        let mut exclude = self.config.defaults.exclude.clone();
        if let Some(profile) = profile {
            exclude.extend(profile.exclude.clone());
        }
        exclude.extend(args.exclude.clone());

        let mut ext = self.config.defaults.ext.clone();
        if let Some(profile) = profile {
            ext.extend(profile.ext.clone());
        }
        ext.extend(args.ext.clone());

        ResolvedUploadSettings {
            path: args
                .path
                .clone()
                .or_else(|| profile.and_then(|value| value.path.clone()))
                .or_else(|| self.config.defaults.path.clone())
                .map(|path| self.resolve_path(path)),
            creator: args
                .creator
                .clone()
                .or_else(|| profile.and_then(|value| value.creator.clone()))
                .or_else(|| self.config.defaults.creator.clone()),
            asset_type: args
                .asset_type
                .or_else(|| profile.and_then(|value| value.asset_type))
                .or(self.config.defaults.asset_type),
            recursive: args.recursive
                || profile
                    .and_then(|value| value.recursive)
                    .or(self.config.defaults.recursive)
                    .unwrap_or(false),
            include,
            exclude,
            ext,
            max_depth: args
                .max_depth
                .or_else(|| profile.and_then(|value| value.max_depth))
                .or(self.config.defaults.max_depth),
            limit: args
                .limit
                .or_else(|| profile.and_then(|value| value.limit))
                .or(self.config.defaults.limit),
            output: args
                .output
                .or_else(|| profile.and_then(|value| value.output))
                .or(self.config.defaults.output),
            concurrency: args
                .concurrency
                .or_else(|| profile.and_then(|value| value.concurrency))
                .or(self.config.defaults.concurrency),
            name_template: args
                .name_template
                .clone()
                .or_else(|| profile.and_then(|value| value.upload.name_template.clone()))
                .or_else(|| self.config.defaults.upload.name_template.clone()),
        }
    }

    fn resolve_path(&self, path: PathBuf) -> PathBuf {
        if path.is_absolute() {
            path
        } else {
            self.root_dir.join(path)
        }
    }
}

fn find_project_config(start_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(start_dir);

    while let Some(dir) = current {
        let candidate = dir.join(PROJECT_CONFIG_FILE_NAME);
        if candidate.exists() {
            return Some(candidate);
        }

        current = dir.parent();
    }

    None
}

fn default_project_config() -> &'static str {
    r#"creator = "user:123456789"
type = "image"
path = "assets"
recursive = true
include = ["**/*.png"]
exclude = ["**/drafts/**"]
output = "jsonl"
concurrency = 3

[upload]
name_template = "{stem}"

[profiles.dev]
path = "assets/dev"

[profiles.dev.upload]
name_template = "dev_{stem}"
"#
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rbxup-project-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn resolves_upload_settings_from_profile_and_cli() {
        let _guard = cwd_lock().lock().expect("cwd lock");
        let root = temp_dir("resolve");
        fs::create_dir_all(&root).expect("create temp root");
        fs::write(
            root.join(PROJECT_CONFIG_FILE_NAME),
            r#"
creator = "group:10"
type = "image"
path = "assets"
recursive = true
include = ["**/*.png"]

[upload]
name_template = "{parent}_{stem}"

[profiles.dev]
path = "assets/dev"
exclude = ["**/drafts/**"]
concurrency = 4
"#,
        )
        .expect("write config");

        let original_dir = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(&root).expect("chdir to temp root");

        let context = load_project_context(Some("dev"))
            .expect("load should succeed")
            .expect("project config should exist");
        let args = UploadCommand {
            path: None,
            asset_type: None,
            display_name: None,
            description: None,
            creator: None,
            profile: Some("dev".to_string()),
            name_template: None,
            include: vec!["**/*.jpg".to_string()],
            exclude: Vec::new(),
            ext: Vec::new(),
            recursive: false,
            max_depth: None,
            limit: None,
            dry_run: false,
            concurrency: None,
            yield_until_done: false,
            timeout: None,
            poll_interval: None,
            output: None,
        };

        let resolved = context.resolve_upload_settings(&args);
        assert_eq!(
            resolved.path.as_deref(),
            Some(root.join("assets/dev").as_path())
        );
        assert_eq!(resolved.creator.as_deref(), Some("group:10"));
        assert_eq!(resolved.asset_type, Some(UploadAssetType::Image));
        assert!(resolved.recursive);
        assert_eq!(
            resolved.include,
            vec!["**/*.png".to_string(), "**/*.jpg".to_string()]
        );
        assert_eq!(resolved.exclude, vec!["**/drafts/**".to_string()]);
        assert_eq!(resolved.concurrency, Some(4));
        assert_eq!(resolved.name_template.as_deref(), Some("{parent}_{stem}"));

        std::env::set_current_dir(original_dir).expect("restore cwd");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn init_writes_project_file() {
        let _guard = cwd_lock().lock().expect("cwd lock");
        let root = temp_dir("init");
        fs::create_dir_all(&root).expect("create temp root");
        let original_dir = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(&root).expect("chdir to temp root");

        let path = init_project_config(false).expect("init should succeed");
        let contents = fs::read_to_string(&path).expect("read init file");
        assert!(contents.contains("name_template = \"{stem}\""));

        std::env::set_current_dir(original_dir).expect("restore cwd");
        fs::remove_dir_all(root).expect("cleanup");
    }
}
