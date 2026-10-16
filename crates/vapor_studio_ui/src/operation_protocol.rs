use std::path::PathBuf;
use vapor_core::VaporWorkspace;
use vapor_studio_core::ContentIdentity;

pub(super) struct WorkspaceOpenSnapshot {
    pub(super) root: PathBuf,
    pub(super) source_content: Vec<vapor_studio_core::SourceContent>,
    pub(super) local_catalog: Result<Vec<vapor_studio_core::CatalogContent>, String>,
    pub(super) workspace_files: Vec<PathBuf>,
}

pub(super) enum OperationResult {
    Workspace(Result<VaporWorkspace, String>),
    WorkspaceSelection(Result<WorkspaceOpenSnapshot, String>),
    RememberedSources(Result<Vec<PathBuf>, String>),
    AdoptedContentProject(Result<(String, Vec<vapor_studio_core::SourceContent>), String>),
    UnregisteredContentProject(
        Result<(String, Vec<vapor_studio_core::SourceContent>, ContentIdentity), String>,
    ),
    UnregisteredProjectScan(Result<vapor_studio_core::UnregisteredProjectScan, String>),
    Discovery(
        Result<Vec<vapor_studio_core::SourceContent>, String>,
        Result<Vec<vapor_studio_core::CatalogContent>, String>,
    ),
    Message(Result<String, String>),
    CompositionPreview(Result<vapor_studio_core::CompositionPreview, String>),
    WorkspaceFiles(Result<Vec<PathBuf>, String>),
    WorkspaceFileCreated(Result<PathBuf, String>),
    FileContents(Result<(PathBuf, String), String>),
    FileSaved(Result<PathBuf, String>),
    WorkspaceFilesSaved(Vec<Result<PathBuf, String>>),
    ContentWorkspace(Result<(PathBuf, String, Vec<PathBuf>), String>),
    SourceCompositionLoaded(Result<vapor_studio_core::SourceComposition, String>),
    SourceCompositionSaved(Result<vapor_studio_core::SourceComposition, String>),
    SourceWorkshopLoaded(Result<vapor_studio_core::SourceWorkshopMetadata, String>),
    SourceWorkshopSaved(Result<vapor_studio_core::SourceWorkshopMetadata, String>),
    SourceRuntimeOutputsLoaded(Result<vapor_studio_core::SourceRuntimeOutputs, String>),
    SourceRuntimeOutputsSaved(Result<vapor_studio_core::SourceRuntimeOutputs, String>),
    Install(Result<vapor_studio_core::InstalledComposition, String>),
    Update(Result<vapor_studio_core::InstalledComposition, String>),
    RunProfile(Result<vapor_studio_core::InstalledComposition, String>),
    Verify(Result<vapor_studio_core::InstallationCheck, String>),
    Repair(Result<vapor_studio_core::InstalledComposition, String>),
    WorkshopStage(Result<vapor_studio_core::WorkshopPackagePreview, String>),
    WorkshopAcquisition(Result<Vec<vapor_studio_core::WorkshopAcquisition>, String>),
    WorkshopPublish(Result<vapor_studio_core::WorkshopPublication, String>),
    WorkshopSearch(Result<vapor_studio_core::WorkshopPage, String>, String),
    WorkshopSubscriptions(Result<Vec<vapor_studio_core::WorkshopSubscription>, String>),
    WorkshopSubscriptionBatchChange(Result<(), String>, Vec<String>, bool),
}

impl OperationResult {
    pub(super) fn failure(&self) -> Option<String> {
        match self {
            Self::Workspace(Err(error))
            | Self::WorkspaceSelection(Err(error))
            | Self::RememberedSources(Err(error))
            | Self::AdoptedContentProject(Err(error))
            | Self::UnregisteredContentProject(Err(error))
            | Self::UnregisteredProjectScan(Err(error))
            | Self::CompositionPreview(Err(error))
            | Self::Message(Err(error))
            | Self::WorkspaceFiles(Err(error))
            | Self::WorkspaceFileCreated(Err(error))
            | Self::FileContents(Err(error))
            | Self::FileSaved(Err(error))
            | Self::ContentWorkspace(Err(error))
            | Self::SourceCompositionLoaded(Err(error))
            | Self::SourceCompositionSaved(Err(error))
            | Self::SourceWorkshopLoaded(Err(error))
            | Self::SourceWorkshopSaved(Err(error))
            | Self::SourceRuntimeOutputsLoaded(Err(error))
            | Self::SourceRuntimeOutputsSaved(Err(error))
            | Self::Install(Err(error))
            | Self::Update(Err(error))
            | Self::RunProfile(Err(error))
            | Self::Verify(Err(error))
            | Self::Repair(Err(error))
            | Self::WorkshopStage(Err(error))
            | Self::WorkshopAcquisition(Err(error))
            | Self::WorkshopPublish(Err(error))
            | Self::WorkshopSearch(Err(error), _)
            | Self::WorkshopSubscriptions(Err(error))
            | Self::WorkshopSubscriptionBatchChange(Err(error), _, _) => Some(error.clone()),
            Self::WorkspaceFilesSaved(results) => {
                let errors = results.iter().filter_map(|result| result.as_ref().err());
                let messages = errors.cloned().collect::<Vec<_>>();
                (!messages.is_empty()).then(|| messages.join("; "))
            }
            Self::Discovery(_, Err(error)) => Some(error.clone()),
            Self::Verify(Ok(check)) if !check.ok => Some(check.detail.join("; ")),
            _ => None,
        }
    }

    pub(super) fn refreshes_content_catalog(&self) -> bool {
        match self {
            Self::WorkshopAcquisition(_) => true,
            Self::FileSaved(Ok(path)) => path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".vapor.toml")),
            Self::WorkspaceFilesSaved(results) => results.iter().any(|result| {
                result.as_ref().is_ok_and(|path| {
                    path.file_name()
                        .is_some_and(|name| name.to_string_lossy().ends_with(".vapor.toml"))
                })
            }),
            _ => matches!(
                self,
                Self::WorkshopPublish(Ok(_))
                    | Self::ContentWorkspace(Ok(_))
                    | Self::AdoptedContentProject(Ok(_))
                    | Self::UnregisteredContentProject(Ok(_))
                    | Self::SourceCompositionSaved(Ok(_))
                    | Self::SourceWorkshopSaved(Ok(_))
                    | Self::SourceRuntimeOutputsSaved(Ok(_))
            ),
        }
    }

    pub(super) fn refreshes_workshop_subscriptions(&self) -> bool {
        matches!(
            self,
            Self::WorkshopSubscriptionBatchChange(_, _, _)
        )
    }
}
