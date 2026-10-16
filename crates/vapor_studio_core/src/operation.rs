use serde::{Deserialize, Serialize};
pub use vapor_core::{OperationId, OperationPhase, OperationStatus, Progress};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKind {
    Discover,
    Resolve,
    Acquire,
    Install,
    Verify,
    Update,
    Repair,
    CreateWorkspace,
    CreateContent,
    LoadComposition,
    SaveComposition,
    LoadWorkshopMetadata,
    SaveWorkshopMetadata,
    LoadRuntimeOutputs,
    SaveRuntimeOutputs,
    CreateWorkspaceFile,
    OpenWorkspace,
    RememberWorkspace,
    AdoptContentProject,
    UnregisterContentProject,
    DiscoverContentProjects,
    DiscoverWorkspace,
    OpenWorkspaceFile,
    SaveWorkspaceFile,
    Fmt,
    Check,
    Test,
    Build,
    Clean,
    Validate,
    Run,
    RunInstalled,
    StageWorkshop,
    PublishWorkshop,
    StageRootDistribution,
}

impl OperationKind {
    /// Stable action identifier used by shared host operation services.
    pub fn action(&self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::Resolve => "resolve",
            Self::Acquire => "acquire",
            Self::Install => "install",
            Self::Verify => "verify",
            Self::Update => "update",
            Self::Repair => "repair",
            Self::CreateWorkspace => "create-workspace",
            Self::CreateContent => "create-content",
            Self::LoadComposition => "load-composition",
            Self::SaveComposition => "save-composition",
            Self::LoadWorkshopMetadata => "load-workshop-metadata",
            Self::SaveWorkshopMetadata => "save-workshop-metadata",
            Self::LoadRuntimeOutputs => "load-runtime-outputs",
            Self::SaveRuntimeOutputs => "save-runtime-outputs",
            Self::CreateWorkspaceFile => "create-workspace-file",
            Self::OpenWorkspace => "open-workspace",
            Self::RememberWorkspace => "remember-workspace",
            Self::AdoptContentProject => "adopt-content-project",
            Self::UnregisterContentProject => "unregister-content-project",
            Self::DiscoverContentProjects => "discover-content-projects",
            Self::DiscoverWorkspace => "discover-workspace",
            Self::OpenWorkspaceFile => "open-workspace-file",
            Self::SaveWorkspaceFile => "save-workspace-file",
            Self::Fmt => "fmt",
            Self::Check => "check",
            Self::Test => "test",
            Self::Build => "build",
            Self::Clean => "clean",
            Self::Validate => "validate",
            Self::Run => "run",
            Self::RunInstalled => "run-installed",
            Self::StageWorkshop => "stage-workshop",
            Self::PublishWorkshop => "publish-workshop",
            Self::StageRootDistribution => "stage-root-distribution",
        }
    }
}

/// Studio-specific request metadata layered over the shared operation identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRequest {
    pub id: OperationId,
    pub profile_id: Option<String>,
    pub kind: OperationKind,
}

/// Studio's typed operation projection for its operation center and history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationEvent {
    pub id: OperationId,
    pub profile_id: Option<String>,
    pub kind: OperationKind,
    pub phase: OperationPhase,
    pub progress: Option<Progress>,
    pub status: OperationStatus,
}
