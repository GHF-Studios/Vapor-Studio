use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StudioRevision(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSummary {
    pub app_root: Option<String>,
    pub workspace: Option<String>,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableAction {
    pub id: String,
    pub enabled: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudioSnapshot {
    pub revision: StudioRevision,
    pub host: HostSummary,
    pub active_profile_id: Option<String>,
    pub active_operation: Option<OperationSummary>,
    pub catalog: Vec<CatalogEntry>,
    pub selected_content_id: Option<String>,
    pub available_actions: Vec<AvailableAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub published_file_id: Option<String>,
    pub dependencies: Vec<String>,
    pub conflicts: Vec<CatalogConflict>,
    pub locations: Vec<CatalogLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogConflict {
    pub field: String,
    pub first_value: String,
    pub first_location: String,
    pub conflicting_value: String,
    pub conflicting_location: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogLocation {
    pub kind: CatalogLocationKind,
    pub root: String,
    pub enabled: bool,
    pub fingerprint: Option<String>,
    pub install_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogLocationKind {
    Workspace,
    Cache,
    Installed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationSummary {
    pub id: String,
    pub action: String,
    pub phase: OperationPhase,
    pub status: OperationStatus,
    pub progress: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationEvent {
    pub revision: StudioRevision,
    pub id: String,
    pub action: String,
    pub phase: OperationPhase,
    pub status: OperationStatus,
    pub progress: Option<f32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationPhase {
    Queued,
    Resolving,
    Acquiring,
    Installing,
    Verifying,
    Building,
    Staging,
    Publishing,
    Running,
    Finalizing,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail", rename_all = "kebab-case")]
pub enum OperationStatus {
    Running,
    Succeeded,
    Failed { message: String },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload", rename_all = "kebab-case")]
pub enum StudioEvent {
    ServiceReady { protocol_version: u16 },
    Snapshot(StudioSnapshot),
    OperationChanged(OperationEvent),
    ServiceStopping,
    ProtocolError(crate::StudioError),
}
