use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StudioRevision(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSummary {
    pub app_root: Option<String>,
    pub workspace: Option<String>,
    pub mode: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub steam_client_available: bool,
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
    #[serde(default)]
    pub durable_state: DurableStateSnapshot,
    pub active_operation: Option<OperationSummary>,
    pub catalog: Vec<CatalogEntry>,
    #[serde(default)]
    pub workspace_content: WorkspaceContentSnapshot,
    pub selected_content_id: Option<String>,
    pub available_actions: Vec<AvailableAction>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableStateSnapshot {
    #[serde(default)]
    pub profile_revision: u64,
    pub profiles: crate::ProfileDocument,
    pub active_profile_id: String,
    pub workspace_root: Option<String>,
    pub editor_drafts: Option<String>,
    pub operation_history: Option<String>,
    pub recovery_backup: Option<String>,
}

/// Authoritative discovery state for source projects in the active workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceContentSnapshot {
    pub source_projects: Vec<WorkspaceSourceProject>,
    pub source_projects_loaded: bool,
    pub source_projects_error: Option<String>,
    pub unregistered_projects: Vec<WorkspaceProjectCandidate>,
    pub unregistered_diagnostics: Vec<String>,
    pub unregistered_projects_loaded: bool,
    pub unregistered_projects_error: Option<String>,
    pub workspace_validation_error: Option<String>,
}

/// A content project declared by the active workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSourceProject {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub root: String,
    pub manifest: String,
    pub binaries: Vec<String>,
    pub libraries: Vec<String>,
    pub dependencies: Vec<String>,
    pub workshop: WorkspaceWorkshopPolicy,
}

/// Workshop publication intent attached to a workspace source project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceWorkshopPolicy {
    pub app_id: Option<u32>,
    pub published_file_id: Option<String>,
    pub visibility: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub change_note: Option<String>,
}

/// A valid content project that can be explicitly adopted into the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceProjectCandidate {
    pub path: String,
    pub id: String,
    pub kind: String,
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
    Snapshot(Box<StudioSnapshot>),
    OperationChanged(OperationEvent),
    ServiceStopping,
    ProtocolError(crate::StudioError),
}
