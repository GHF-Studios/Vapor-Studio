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
    #[serde(default)]
    pub composition_preview: Option<CompositionPreviewSnapshot>,
    pub catalog: Vec<CatalogEntry>,
    #[serde(default)]
    pub workspace_content: WorkspaceContentSnapshot,
    #[serde(default)]
    pub workshop: WorkshopSnapshot,
    pub selected_content_id: Option<String>,
    pub available_actions: Vec<AvailableAction>,
}

/// Read-only view of the latest Workshop query and the account's subscriptions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopSnapshot {
    #[serde(default)]
    pub search: Option<WorkshopSearchSnapshot>,
    #[serde(default)]
    pub subscriptions: Vec<WorkshopSubscriptionSnapshot>,
    #[serde(default)]
    pub subscriptions_loaded: bool,
    #[serde(default)]
    pub subscriptions_revision: u64,
    #[serde(default)]
    pub acquisition: Option<WorkshopAcquisitionBatchSnapshot>,
    #[serde(default)]
    pub review: Option<WorkshopReviewSnapshot>,
}

/// Service-issued review token paired with the exact staged artifact facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopReviewSnapshot {
    pub review_id: String,
    pub selector: String,
    pub preview: WorkshopPackagePreviewSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopPackagePreviewSnapshot {
    pub id: String,
    pub version: Option<String>,
    pub app_id: Option<u32>,
    pub published_file_id: Option<String>,
    pub title: Option<String>,
    pub visibility: Option<String>,
    pub dependencies: Vec<String>,
    pub root: String,
    pub target: String,
    pub fingerprint: String,
    pub files: u64,
    pub bytes: u64,
    pub file_paths: Vec<String>,
    pub omitted_file_paths: usize,
}

/// Result of the latest successful import batch from Steam Workshop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopAcquisitionBatchSnapshot {
    pub revision: u64,
    pub requested: usize,
    pub items: Vec<WorkshopAcquisitionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopAcquisitionSnapshot {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub cache_root: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopSearchSnapshot {
    #[serde(default)]
    pub revision: u64,
    pub query: String,
    pub page: u32,
    pub total_results: u32,
    pub items: Vec<WorkshopItemSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopItemSnapshot {
    pub published_file_id: String,
    pub title: String,
    pub description: String,
    pub preview_url: Option<String>,
    pub updated_at_unix: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopSubscriptionSnapshot {
    pub published_file_id: String,
    pub title: String,
    pub description: String,
    pub preview_url: Option<String>,
    pub updated_at_unix: u32,
    pub installed: bool,
    pub needs_update: bool,
    pub downloading: bool,
    pub download_progress: Option<(u64, u64)>,
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
    #[serde(default)]
    pub profile_id: Option<String>,
    pub action: String,
    pub phase: OperationPhase,
    pub status: OperationStatus,
    pub progress: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationEvent {
    pub revision: StudioRevision,
    pub id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    pub action: String,
    pub phase: OperationPhase,
    pub status: OperationStatus,
    pub progress: Option<f32>,
    pub message: Option<String>,
}

/// Result of a service-owned workspace editor operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", content = "payload", rename_all = "kebab-case")]
pub enum WorkspaceFileResult {
    Listed {
        operation_id: String,
        files: Vec<String>,
    },
    Opened {
        operation_id: String,
        path: String,
        contents: String,
        fingerprint: String,
    },
    Saved {
        operation_id: String,
        path: String,
        fingerprint: String,
    },
    Created {
        operation_id: String,
        path: String,
    },
    Completed {
        operation_id: String,
    },
    RememberedSources {
        operation_id: String,
        roots: Vec<String>,
    },
    UnregisteredContent {
        operation_id: String,
        content_id: String,
        version: Option<String>,
    },
}

/// Result payload for an authoring operation executed by the Studio service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringResult {
    pub operation_id: String,
    pub result: AuthoringResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", content = "payload", rename_all = "kebab-case")]
pub enum AuthoringResultKind {
    RegisteredSources {
        roots: Vec<String>,
        active: Option<String>,
    },
    Composition {
        id: String,
        version: Option<String>,
        kind: String,
        manifest: String,
        source: String,
        links: Vec<AuthoringCompositionLink>,
    },
    WorkshopMetadata {
        id: String,
        version: Option<String>,
        kind: String,
        manifest: String,
        source: String,
        policy: WorkshopMetadataDraft,
    },
    RuntimeOutputs {
        id: String,
        version: Option<String>,
        kind: String,
        manifest: String,
        source: String,
        binaries: Vec<String>,
        libraries: Vec<String>,
    },
    ContentValidated {
        id: String,
    },
    ContentCreated {
        workspace_root: String,
        manifest: String,
        manifest_contents: String,
        files: Vec<String>,
    },
    WorkspaceCreated {
        root: String,
        files: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringCompositionLink {
    pub id: String,
    pub relationship: String,
    pub optional: bool,
    pub version: Option<String>,
    pub workshop_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopMetadataDraft {
    pub app_id: Option<u32>,
    pub published_file_id: Option<String>,
    pub visibility: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub change_note: Option<String>,
}

/// Resolved composition plan produced by the service for a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionPreviewSnapshot {
    pub profile_id: String,
    pub selector: String,
    pub nodes: Vec<CompositionPreviewNode>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionPreviewNode {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub published_file_id: Option<String>,
    pub source: CompositionPreviewSource,
    pub already_installed: bool,
    pub dependencies: Vec<CompositionPreviewDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionPreviewDependency {
    pub id: String,
    pub relationship: String,
    pub version: Option<String>,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompositionPreviewSource {
    Workspace,
    Cache,
    Installed,
    DownloadRequired,
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
    WorkspaceFileResult(Box<WorkspaceFileResult>),
    AuthoringResult(Box<AuthoringResult>),
    ServiceStopping,
    ProtocolError(crate::StudioError),
}
