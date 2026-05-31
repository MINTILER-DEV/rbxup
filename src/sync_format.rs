use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub format: String,
    pub version: u32,
    pub services: BTreeMap<String, String>,
    #[serde(default)]
    pub bundles: BTreeMap<String, String>,
    pub sync: ProjectSyncConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSyncConfig {
    #[serde(rename = "bundleThreshold")]
    pub bundle_threshold: usize,
    #[serde(rename = "bundleGrouping")]
    pub bundle_grouping: BundleGrouping,
    #[serde(rename = "smallBundleBehavior")]
    pub small_bundle_behavior: SmallBundleBehavior,
    #[serde(rename = "bundleMode")]
    pub bundle_mode: BundleMode,
    #[serde(rename = "bundleDir")]
    pub bundle_dir: String,
    #[serde(rename = "deleteMissingOnPush")]
    pub delete_missing_on_push: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundleGrouping {
    ClassName,
    Category,
    Single,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmallBundleBehavior {
    Explode,
    Misc,
    AlwaysBundle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundleMode {
    BundleableDirectChildren,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XupInstance {
    pub format: String,
    pub version: u32,
    pub id: String,
    #[serde(rename = "className")]
    pub class_name: String,
    pub name: String,
    #[serde(default)]
    pub properties: BTreeMap<String, EncodedValue>,
    #[serde(default)]
    pub attributes: BTreeMap<String, EncodedValue>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XzupBundledInstance {
    pub format: String,
    pub version: u32,
    pub kind: String,
    pub id: String,
    #[serde(rename = "className")]
    pub class_name: String,
    pub name: String,
    #[serde(default)]
    pub properties: BTreeMap<String, EncodedValue>,
    #[serde(default)]
    pub attributes: BTreeMap<String, EncodedValue>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub bundle: BundlePointer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XzupChildGroup {
    pub format: String,
    pub version: u32,
    pub kind: String,
    #[serde(rename = "groupBy")]
    pub group_by: String,
    #[serde(rename = "className")]
    pub class_name: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: String,
    pub bundle: BundlePointer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum XzupFile {
    Instance(XzupBundledInstance),
    ChildGroup(XzupChildGroup),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePointer {
    pub path: String,
    pub hash: String,
    #[serde(rename = "childCount")]
    pub child_count: usize,
    #[serde(rename = "contains", skip_serializing_if = "Option::is_none")]
    pub contains: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub format: String,
    pub version: u32,
    #[serde(rename = "groupBy")]
    pub group_by: String,
    #[serde(rename = "className", skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: String,
    #[serde(rename = "instanceCount")]
    pub instance_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInstanceRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "className", skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default)]
    pub properties: BTreeMap<String, EncodedValue>,
    #[serde(default)]
    pub attributes: BTreeMap<String, EncodedValue>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "children", skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<BundleInstanceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedValue {
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#enum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMessage {
    pub kind: String,
    pub version: u32,
    pub command: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub ok: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl ProjectManifest {
    pub fn default_manifest() -> Self {
        Self {
            format: "rbxup-project".to_string(),
            version: 1,
            services: BTreeMap::from([
                (
                    "Workspace".to_string(),
                    "services/Workspace/init.xup".to_string(),
                ),
                (
                    "ReplicatedStorage".to_string(),
                    "services/ReplicatedStorage/init.xup".to_string(),
                ),
                (
                    "ServerScriptService".to_string(),
                    "services/ServerScriptService/init.xup".to_string(),
                ),
                (
                    "StarterGui".to_string(),
                    "services/StarterGui/init.xup".to_string(),
                ),
            ]),
            bundles: BTreeMap::new(),
            sync: ProjectSyncConfig {
                bundle_threshold: 20,
                bundle_grouping: BundleGrouping::ClassName,
                small_bundle_behavior: SmallBundleBehavior::Explode,
                bundle_mode: BundleMode::BundleableDirectChildren,
                bundle_dir: "bundles".to_string(),
                delete_missing_on_push: false,
            },
        }
    }
}
