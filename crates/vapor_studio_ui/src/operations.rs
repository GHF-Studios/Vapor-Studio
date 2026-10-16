use super::StudioApp;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::{self, Receiver, TryRecvError}},
    thread,
};
use vapor_core::{
    OperationEvent as CoreOperationEvent,
    OperationRequest as CoreOperationRequest,
    OperationService,
    VaporWorkspace,
};
use vapor_studio_core::{
    ContentIdentity, OperationEvent, OperationId, OperationKind,
    OperationPhase, OperationRequest, OperationStatus,
};

pub(super) struct OperationState {
    pub(super) running: bool,
    pub(super) receiver: Option<Receiver<(OperationId, OperationResult)>>,
    pub(super) latest_operation_event: Arc<Mutex<Option<CoreOperationEvent>>>,
    pub(super) active_operation: Option<OperationEvent>,
    pub(super) operation_history: VecDeque<OperationEvent>,
}

impl Default for OperationState {
    fn default() -> Self {
        Self {
            running: false,
            receiver: None,
            latest_operation_event: Arc::new(Mutex::new(None)),
            active_operation: None,
            operation_history: VecDeque::new(),
        }
    }
}

pub(super) enum OperationResult {
    Workspace(Result<VaporWorkspace, String>),
    WorkspaceSelection(Result<WorkspaceSelectionData, String>),
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
    WorkshopAcquisition(Result<vapor_studio_core::WorkshopAcquisition, String>),
    WorkshopPublish(Result<vapor_studio_core::WorkshopPublication, String>),
    WorkshopSearch(Result<vapor_studio_core::WorkshopPage, String>, String),
    WorkshopSubscriptions(Result<Vec<vapor_studio_core::WorkshopSubscription>, String>),
    WorkshopSubscriptionChange(Result<(), String>, String, bool),
}

pub(super) type WorkspaceSelectionData = (
    PathBuf,
    Vec<vapor_studio_core::SourceContent>,
    Result<Vec<vapor_studio_core::CatalogContent>, String>,
    Vec<PathBuf>,
);

fn operation_failure(result: &OperationResult) -> Option<String> {
    match result {
        OperationResult::Workspace(Err(error))
        | OperationResult::WorkspaceSelection(Err(error))
        | OperationResult::RememberedSources(Err(error))
        | OperationResult::AdoptedContentProject(Err(error))
        | OperationResult::UnregisteredContentProject(Err(error))
        | OperationResult::UnregisteredProjectScan(Err(error))
        | OperationResult::CompositionPreview(Err(error))
        | OperationResult::Message(Err(error))
        | OperationResult::WorkspaceFiles(Err(error))
        | OperationResult::WorkspaceFileCreated(Err(error))
        | OperationResult::FileContents(Err(error))
        | OperationResult::FileSaved(Err(error))
        | OperationResult::ContentWorkspace(Err(error))
        | OperationResult::SourceCompositionLoaded(Err(error))
        | OperationResult::SourceCompositionSaved(Err(error))
        | OperationResult::SourceWorkshopLoaded(Err(error))
        | OperationResult::SourceWorkshopSaved(Err(error))
        | OperationResult::SourceRuntimeOutputsLoaded(Err(error))
        | OperationResult::SourceRuntimeOutputsSaved(Err(error))
        | OperationResult::Install(Err(error))
        | OperationResult::Update(Err(error))
        | OperationResult::RunProfile(Err(error))
        | OperationResult::Verify(Err(error))
        | OperationResult::Repair(Err(error))
        | OperationResult::WorkshopStage(Err(error))
        | OperationResult::WorkshopAcquisition(Err(error))
        | OperationResult::WorkshopPublish(Err(error))
        | OperationResult::WorkshopSearch(Err(error), _)
        | OperationResult::WorkshopSubscriptions(Err(error))
        | OperationResult::WorkshopSubscriptionChange(Err(error), _, _) => Some(error.clone()),
        OperationResult::Discovery(_, Err(error)) => Some(error.clone()),
        OperationResult::Verify(Ok(check)) if !check.ok => Some(check.detail.join("; ")),
        _ => None,
    }
}

fn apply_core_operation_event(
    event: &CoreOperationEvent,
    current: &mut OperationEvent,
) {
    if event.id != current.id {
        return;
    }
    current.phase = event.phase;
    current.progress = event.progress.clone();
    current.status = event.status.clone();
}

impl StudioApp {
    pub(super) fn poll_operation(&mut self) {
        let latest_event = self
            .operation
            .latest_operation_event
            .lock()
            .ok()
            .and_then(|mut event| event.take());
        if let (Some(event), Some(active)) = (latest_event.as_ref(), self.operation.active_operation.as_mut())
        {
            apply_core_operation_event(event, active);
        }
        let received = self.operation.receiver.as_ref().map(|receiver| receiver.try_recv());
        let result = match received {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => {
                self.operation.active_operation.as_ref().map(|operation| {
                    (
                        operation.id,
                        OperationResult::Message(Err(
                            "The background operation stopped before returning a result."
                                .to_owned(),
                        )),
                    )
                })
            }
            Some(Err(TryRecvError::Empty)) | None => None,
        };
        if let Some((operation_id, result)) = result {
            self.operation.receiver = None;
            self.operation.running = false;
            let refresh_after_content_change = matches!(
                &result,
                OperationResult::WorkshopPublish(Ok(_))
                    | OperationResult::WorkshopAcquisition(Ok(_))
                    | OperationResult::ContentWorkspace(Ok(_))
                    | OperationResult::AdoptedContentProject(Ok(_))
                    | OperationResult::UnregisteredContentProject(Ok(_))
                    | OperationResult::SourceCompositionSaved(Ok(_))
                    | OperationResult::SourceWorkshopSaved(Ok(_))
                    | OperationResult::SourceRuntimeOutputsSaved(Ok(_))
            );
            let refresh_after_manifest_save = matches!(
                &result,
                OperationResult::FileSaved(Ok(path))
                    if path.file_name().is_some_and(|name| name.to_string_lossy().ends_with(".vapor.toml"))
            );
            let refresh_after_subscription_change = matches!(
                &result,
                OperationResult::WorkshopSubscriptionChange(Ok(()), _, _)
            );
            let failure = operation_failure(&result);
            self.apply_operation_result(result);
            if let Some(event) = self
                .operation
                .active_operation
                .as_mut()
                .filter(|event| event.id == operation_id)
            {
                event.phase = OperationPhase::Finalizing;
                event.status = match failure {
                    Some(message) => OperationStatus::Failed { message },
                    None => OperationStatus::Succeeded {
                        summary: self.status.clone(),
                    },
                };
                self.operation.operation_history.push_front(event.clone());
                self.operation.operation_history.truncate(20);
            }
            self.operation.active_operation = None;
            if refresh_after_content_change || refresh_after_manifest_save {
                self.refresh_content_catalog();
            }
            if refresh_after_subscription_change {
                self.refresh_workshop_subscriptions();
            }
        }
    }

    fn refresh_content_catalog(&mut self) {
        self.profile_composition.invalidate_preview();
        let root = self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.workspace.as_ref())
            .map(|workspace| workspace.root.clone());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Content action succeeded; catalog refresh failed: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Discover, move |_context| {
            let (source, catalog) = discover_workspace_and_catalog(&executable, root.as_deref());
            OperationResult::Discovery(source, catalog)
        });
    }

    pub(super) fn spawn(
        &mut self,
        kind: OperationKind,
        operation: impl FnOnce(vapor_core::OperationContext) -> OperationResult + Send + 'static,
    ) {
        let request = OperationRequest {
            id: OperationId::fresh(),
            profile_id: self
                .profile
                .items
                .get(self.profile.active_index)
                .map(|profile| profile.id.clone()),
            kind,
        };
        let phase = match &request.kind {
            OperationKind::Resolve
            | OperationKind::LoadComposition
            | OperationKind::LoadWorkshopMetadata
            | OperationKind::LoadRuntimeOutputs
            | OperationKind::Discover
            | OperationKind::SearchWorkshop
            | OperationKind::LoadWorkshopSubscriptions
            | OperationKind::ChangeWorkshopSubscription
            | OperationKind::OpenWorkspace
            | OperationKind::RememberWorkspace
            | OperationKind::AdoptContentProject
            | OperationKind::UnregisterContentProject
            | OperationKind::DiscoverContentProjects
            | OperationKind::DiscoverWorkspace => OperationPhase::Resolving,
            OperationKind::CreateWorkspace => OperationPhase::Building,
            OperationKind::CreateWorkspaceFile
            | OperationKind::SaveComposition
            | OperationKind::SaveWorkshopMetadata
            | OperationKind::SaveRuntimeOutputs => OperationPhase::Building,
            OperationKind::Acquire => OperationPhase::Acquiring,
            OperationKind::Install | OperationKind::Update | OperationKind::Repair => {
                OperationPhase::Installing
            }
            OperationKind::Verify | OperationKind::Validate => OperationPhase::Verifying,
            OperationKind::Build
            | OperationKind::Check
            | OperationKind::Test
            | OperationKind::Fmt
            | OperationKind::Clean => OperationPhase::Building,
            OperationKind::Run | OperationKind::RunInstalled => OperationPhase::Running,
            OperationKind::StageWorkshop | OperationKind::StageRootDistribution => {
                OperationPhase::Staging
            }
            OperationKind::PublishWorkshop => OperationPhase::Publishing,
            _ => OperationPhase::Queued,
        };
        self.operation.active_operation = Some(OperationEvent {
            id: request.id,
            profile_id: request.profile_id.clone(),
            kind: request.kind.clone(),
            phase,
            progress: None,
            status: OperationStatus::Running,
        });
        let operation_id = request.id;
        let initial_phase = phase.clone();
        let core_request = CoreOperationRequest {
            id: request.id,
            domain: "studio".to_owned(),
            action: request.kind.action().to_owned(),
        };
        if let Ok(mut latest) = self.operation.latest_operation_event.lock() {
            *latest = None;
        }
        let latest_event = Arc::clone(&self.operation.latest_operation_event);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let sink: vapor_core::OperationEventSink = Arc::new(move |event| {
                if let Ok(mut latest) = latest_event.lock() {
                    *latest = Some(event);
                }
            });
            let result = OperationService::run_observed(
                core_request,
                sink,
                |context| {
                    context.set_phase(initial_phase);
                    operation(context)
                },
                |result| match operation_failure(result) {
                    Some(message) => Err(message),
                    None => Ok("Completed".to_owned()),
                },
            );
            let _ = sender.send((operation_id, result));
        });
        self.operation.receiver = Some(receiver);
        self.operation.running = true;
    }

}

pub(super) fn discover_workspace_and_catalog(
    executable: &std::path::Path,
    workspace_root: Option<&std::path::Path>,
) -> (
    Result<Vec<vapor_studio_core::SourceContent>, String>,
    Result<Vec<vapor_studio_core::CatalogContent>, String>,
) {
    let source = workspace_root
        .map(|root| vapor_studio_core::discover_source_content(executable, root))
        .unwrap_or_else(|| Err("Open a workspace to browse authored source content.".to_owned()));
    let catalog = vapor_studio_core::discover_content_catalog(executable, workspace_root);
    (source, catalog)
}

