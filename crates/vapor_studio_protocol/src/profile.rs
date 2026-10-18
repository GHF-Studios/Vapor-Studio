use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContentIdentity {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedDependencyIdentity {
    pub binding: String,
    pub content: ContentIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedNodeIdentity {
    pub content: ContentIdentity,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub dependencies: Vec<ResolvedDependencyIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedCompositionGraph {
    pub root: ContentIdentity,
    pub effective_engine: ContentIdentity,
    #[serde(default)]
    pub effective_game: Option<ContentIdentity>,
    pub nodes: Vec<ResolvedNodeIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceAssociation {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchSettings {
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledArtifactRecord {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    pub root: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum InstallationState {
    #[default]
    Unknown,
    NotInstalled,
    Installed {
        artifacts: Vec<InstalledArtifactRecord>,
        graph_fingerprint: Option<String>,
    },
    OutOfDate {
        detail: String,
        actual_graph: ResolvedCompositionGraph,
        artifacts: Vec<InstalledArtifactRecord>,
        #[serde(default)]
        verified: bool,
    },
    Partial {
        detail: String,
    },
    NeedsRepair {
        detail: String,
        artifacts: Vec<InstalledArtifactRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub root_package: Option<ContentIdentity>,
    #[serde(default)]
    pub resolved_graph: Option<ResolvedCompositionGraph>,
    #[serde(default)]
    pub workspace: Option<WorkspaceAssociation>,
    #[serde(default)]
    pub launch: LaunchSettings,
    #[serde(default)]
    pub installation: InstallationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocument {
    pub schema_version: u32,
    pub profiles: Vec<CompositionProfile>,
}

impl Default for ProfileDocument {
    fn default() -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            profiles: vec![CompositionProfile::default_profile()],
        }
    }
}

impl CompositionProfile {
    pub fn default_profile() -> Self {
        Self {
            id: "default".to_owned(),
            name: "Default".to_owned(),
            root_package: None,
            resolved_graph: None,
            workspace: None,
            launch: LaunchSettings::default(),
            installation: InstallationState::Unknown,
        }
    }
}

/// Values the GUI's former eframe store may offer during the first service startup.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyStorageSeed {
    pub profiles: Option<String>,
    pub active_profile_id: Option<String>,
    pub active_profile_index: Option<String>,
    pub workspace_root: Option<String>,
    pub editor_drafts: Option<String>,
    pub operation_history: Option<String>,
    pub recovery_backup: Option<String>,
}
