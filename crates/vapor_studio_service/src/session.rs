use crate::{
    host::StudioHostContext,
    persistence::{StudioPersistentState, StudioStateStore},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use vapor_studio_core as vapor_core;
use vapor_studio_core::{
    CatalogContent, SourceContent, add_profile, discover_content_catalog, discover_source_content,
    duplicate_profile, remove_profile, rename_profile, scan_unregistered_content_projects,
};
use vapor_studio_protocol::{
    AuthoringAction, AuthoringCompositionLink, AuthoringResult, AuthoringResultKind,
    AvailableAction, CatalogAction, CatalogConflict, CatalogEntry, CatalogLocation,
    CatalogLocationKind, CompositionProfile, ContentIdentity, DurableStateSnapshot, HostSummary,
    InstallationState, LegacyStorageSeed, OperationEvent, OperationPhase, OperationStatus,
    OperationSummary, ProfileAction, SessionAction, StudioAction, StudioError, StudioErrorCode,
    StudioEvent, StudioRequest, StudioResponse, StudioResponseKind, StudioRevision, StudioSnapshot,
    WorkshopAcquisitionBatchSnapshot, WorkshopAcquisitionSnapshot, WorkshopAction,
    WorkshopItemSnapshot, WorkshopMetadataDraft, WorkshopPackagePreviewSnapshot,
    WorkshopReviewSnapshot, WorkshopSearchSnapshot, WorkshopSnapshot, WorkshopSubscriptionSnapshot,
    WorkspaceAction, WorkspaceAssociation, WorkspaceContentSnapshot, WorkspaceFileResult,
    WorkspaceProjectCandidate, WorkspaceSourceProject, WorkspaceWorkshopPolicy,
};

const MAX_CATALOG_ITEMS: usize = 10_000;
const MAX_WORKSPACE_PROJECTS: usize = 10_000;
const MAX_WORKSPACE_DIAGNOSTICS: usize = 1_000;
const MAX_WORKSHOP_REVIEW_FILE_PATHS: usize = 50;

#[derive(Debug, Clone)]
pub struct ServiceBootstrap {
    pub application_executable: PathBuf,
    pub workshop_app_id: Option<u32>,
    pub workspace_root: Option<PathBuf>,
    pub active_profile_id: Option<String>,
    pub fixture_mode: bool,
}

pub struct StudioSession {
    bootstrap: ServiceBootstrap,
    revision: StudioRevision,
    active_profile_id: Option<String>,
    selected_content_id: Option<String>,
    catalog: Vec<CatalogEntry>,
    catalog_query: String,
    catalog_error: Option<String>,
    workspace_content: WorkspaceContentSnapshot,
    durable_state: StudioPersistentState,
    state_store: Option<StudioStateStore>,
    host_context: Option<StudioHostContext>,
    active_operation: Option<OperationSummary>,
    composition_preview: Option<vapor_studio_protocol::CompositionPreviewSnapshot>,
    workshop: WorkshopSnapshot,
    workshop_review: Option<WorkshopReviewState>,
    fixture_workshop_subscriptions: BTreeSet<String>,
    fixture_acquisitions: BTreeMap<String, WorkshopAcquisitionSnapshot>,
    operation_events: VecDeque<StudioEvent>,
    operation_receiver: Receiver<BackgroundOperationMessage>,
    operation_sender: Sender<BackgroundOperationMessage>,
}

enum BackgroundOperationMessage {
    Lifecycle(vapor_core::CoreOperationEvent),
    Complete(CompositionOperationCompletion),
    CompleteWorkshop(Box<WorkshopOperationCompletion>),
    CompleteWorkspace(Box<WorkspaceOperationCompletion>),
    CompleteAuthoring(Box<AuthoringOperationCompletion>),
}

struct CompositionOperationCompletion {
    id: String,
    profile_id: String,
    action: CompositionOperationKind,
    result: CompositionOperationResult,
}

#[derive(Clone, Copy)]
enum CompositionOperationKind {
    Preview,
    Install,
    Verify,
    Repair,
    Update,
    Launch,
    RunSource,
}

enum CompositionOperationResult {
    Preview(Result<vapor_studio_core::CompositionPreview, String>),
    Install(Result<vapor_studio_core::InstalledComposition, String>),
    Verify(Result<vapor_studio_core::InstallationCheck, String>),
    Repair(Result<vapor_studio_core::InstalledComposition, String>),
    Update(Result<vapor_studio_core::InstalledComposition, String>),
    Launch(Result<vapor_studio_core::ProfileRunResult, String>),
    RunSource(Result<vapor_studio_core::ProfileRunResult, String>),
}

struct WorkshopOperationCompletion {
    id: String,
    action: WorkshopOperationKind,
    result: Result<WorkshopOperationResult, String>,
}

struct WorkspaceOperationCompletion {
    id: String,
    action: WorkspaceOperationKind,
    root: PathBuf,
    result: Result<WorkspaceOperationResult, String>,
}

struct AuthoringOperationCompletion {
    id: String,
    action: String,
    workspace: Option<PathBuf>,
    workspace_bound: bool,
    result: Result<AuthoringResultKind, String>,
}

#[derive(Clone)]
enum WorkspaceOperationKind {
    ListFiles,
    OpenFile(PathBuf),
    SaveFile(Box<WorkspaceFileSave>),
    CreateFile(PathBuf),
    Validate,
    Remember,
    AdoptProject(PathBuf),
    UnregisterProject(String),
}

#[derive(Clone)]
struct WorkspaceFileSave {
    path: PathBuf,
    expected_fingerprint: String,
    contents: String,
}

enum WorkspaceOperationResult {
    Listed(Vec<PathBuf>),
    Opened(Box<WorkspaceFileOpen>),
    Saved { path: PathBuf, fingerprint: String },
    Created(PathBuf),
    Completed,
    Remembered(Vec<PathBuf>),
    Unregistered(vapor_studio_core::ContentIdentity),
}

struct WorkspaceFileOpen {
    path: PathBuf,
    contents: String,
    fingerprint: String,
}

#[derive(Debug, Clone)]
struct WorkshopReviewState {
    review_id: String,
    selector: String,
    workspace_root: PathBuf,
    preview: vapor_studio_core::WorkshopPackagePreview,
}

#[derive(Clone)]
enum WorkshopOperationKind {
    Search {
        query: String,
        page: u32,
    },
    LoadSubscriptions,
    ChangeSubscriptions {
        ids: Vec<u64>,
        subscribe: bool,
    },
    Acquire {
        ids: Vec<String>,
    },
    Stage {
        selector: String,
        workspace_root: PathBuf,
    },
    Publish {
        review: Box<WorkshopReviewState>,
        account: String,
        change_note: Option<String>,
    },
}

enum WorkshopOperationResult {
    Search(vapor_studio_core::WorkshopPage),
    Subscriptions(Vec<vapor_studio_core::WorkshopSubscription>),
    Acquisitions {
        items: Vec<vapor_studio_core::WorkshopAcquisition>,
        requested: usize,
    },
    Review(vapor_studio_core::WorkshopPackagePreview),
    Publication(vapor_studio_core::WorkshopPublication),
}

impl StudioSession {
    pub fn new(bootstrap: ServiceBootstrap) -> Self {
        let mut durable_state = StudioPersistentState::default();
        if let Some(active_profile_id) = bootstrap.active_profile_id.as_ref()
            && durable_state
                .profiles
                .profiles
                .iter()
                .any(|profile| profile.id == *active_profile_id)
        {
            durable_state.active_profile_id = active_profile_id.clone();
        }
        durable_state.workspace_root = bootstrap
            .workspace_root
            .as_ref()
            .map(|root| root.display().to_string());
        Self::with_state(bootstrap, durable_state, None, None)
    }

    pub fn with_persistent_state(
        mut bootstrap: ServiceBootstrap,
        host_context: StudioHostContext,
        store: StudioStateStore,
        legacy_storage: Option<&LegacyStorageSeed>,
    ) -> Result<Self, String> {
        let mut loaded = store.load_or_import(legacy_storage)?;
        if let Some(workspace_root) = bootstrap.workspace_root.as_ref() {
            let workspace = Some(workspace_root.display().to_string());
            let mut profile_changed = loaded.state.workspace_root != workspace;
            loaded.state.workspace_root = workspace;
            if let Some(profile) = loaded
                .state
                .profiles
                .profiles
                .iter_mut()
                .find(|profile| profile.id == loaded.state.active_profile_id)
            {
                let association = Some(WorkspaceAssociation {
                    root: workspace_root.clone(),
                });
                profile_changed |= profile.workspace != association;
                profile.workspace = association;
            }
            if profile_changed {
                loaded.state.profile_revision = loaded.state.profile_revision.saturating_add(1);
            }
            store.save(&loaded.state)?;
        }
        if bootstrap.workspace_root.is_none() {
            bootstrap.workspace_root = loaded.state.workspace_root.as_deref().map(PathBuf::from);
        }
        bootstrap.active_profile_id = Some(loaded.state.active_profile_id.clone());
        let mut session =
            Self::with_state(bootstrap, loaded.state, Some(store), Some(host_context));
        if let Some(message) = loaded.recovery_message {
            session.catalog_error = Some(message);
        }
        Ok(session)
    }

    fn with_state(
        bootstrap: ServiceBootstrap,
        durable_state: StudioPersistentState,
        state_store: Option<StudioStateStore>,
        host_context: Option<StudioHostContext>,
    ) -> Self {
        let active_profile_id = Some(durable_state.active_profile_id.clone());
        let (operation_sender, operation_receiver) = mpsc::channel();
        Self {
            bootstrap,
            revision: StudioRevision(0),
            active_profile_id,
            selected_content_id: None,
            catalog: Vec::new(),
            catalog_query: String::new(),
            catalog_error: None,
            workspace_content: WorkspaceContentSnapshot::default(),
            durable_state,
            state_store,
            host_context,
            active_operation: None,
            composition_preview: None,
            workshop: WorkshopSnapshot::default(),
            workshop_review: None,
            fixture_workshop_subscriptions: BTreeSet::from(["1001".to_owned()]),
            fixture_acquisitions: BTreeMap::new(),
            operation_events: VecDeque::new(),
            operation_receiver,
            operation_sender,
        }
    }

    pub fn snapshot(&self) -> StudioSnapshot {
        let mut available_actions = vec![AvailableAction {
            id: "catalog.refresh".to_owned(),
            enabled: true,
            reason: None,
        }];
        available_actions.extend([
            AvailableAction {
                id: "catalog.discover-content-projects".to_owned(),
                enabled: self.bootstrap.workspace_root.is_some(),
                reason: self
                    .bootstrap
                    .workspace_root
                    .is_none()
                    .then(|| "Open a workspace first.".to_owned()),
            },
            AvailableAction {
                id: "profile.select".to_owned(),
                enabled: true,
                reason: None,
            },
            AvailableAction {
                id: "catalog.search".to_owned(),
                enabled: true,
                reason: None,
            },
            AvailableAction {
                id: "catalog.inspect".to_owned(),
                enabled: !self.catalog.is_empty(),
                reason: self
                    .catalog
                    .is_empty()
                    .then(|| "Refresh the catalog first.".to_owned()),
            },
            AvailableAction {
                id: "workspace.open".to_owned(),
                enabled: true,
                reason: None,
            },
        ]);
        let mut workshop = self.workshop.clone();
        workshop.review = self.workshop_review.as_ref().map(workshop_review_snapshot);
        StudioSnapshot {
            revision: self.revision,
            host: HostSummary {
                app_root: self
                    .host_context
                    .as_ref()
                    .and_then(|context| context.app_root.as_ref())
                    .map(|root| root.display().to_string())
                    .or_else(|| {
                        self.bootstrap
                            .application_executable
                            .parent()
                            .and_then(|path| path.parent())
                            .and_then(|path| path.parent())
                            .map(|root| root.display().to_string())
                    }),
                workspace: self
                    .bootstrap
                    .workspace_root
                    .as_ref()
                    .map(|path| path.display().to_string()),
                mode: if self.bootstrap.fixture_mode {
                    "fixture".to_owned()
                } else {
                    "production".to_owned()
                },
                platform: self.host_context.as_ref().map_or_else(
                    || std::env::consts::OS.to_owned(),
                    |context| format!("{:?}", context.platform).to_lowercase(),
                ),
                steam_client_available: self
                    .host_context
                    .as_ref()
                    .is_some_and(|context| context.steam_client_available),
            },
            active_profile_id: self.active_profile_id.clone(),
            durable_state: DurableStateSnapshot {
                profile_revision: self.durable_state.profile_revision,
                profiles: self.durable_state.profiles.clone(),
                active_profile_id: self.durable_state.active_profile_id.clone(),
                workspace_root: self.durable_state.workspace_root.clone(),
                editor_drafts: self.durable_state.editor_drafts.clone(),
                operation_history: self.durable_state.operation_history.clone(),
                recovery_backup: self.durable_state.recovery_backup.clone(),
            },
            active_operation: self.active_operation.clone(),
            composition_preview: self.composition_preview.clone(),
            catalog: self
                .catalog
                .iter()
                .filter(|entry| {
                    self.catalog_query.is_empty()
                        || entry.id.to_lowercase().contains(&self.catalog_query)
                        || entry.kind.to_lowercase().contains(&self.catalog_query)
                        || entry.version.as_deref().is_some_and(|version| {
                            version.to_lowercase().contains(&self.catalog_query)
                        })
                })
                .cloned()
                .collect(),
            workspace_content: self.workspace_content.clone(),
            workshop,
            selected_content_id: self.selected_content_id.clone(),
            available_actions,
        }
    }

    pub fn dispatch(&mut self, request: StudioRequest) -> StudioResponse {
        self.poll_background_operation_messages();
        let mut response_kind = StudioResponseKind::Applied;
        let mut error = None;
        if request
            .expected_revision
            .is_some_and(|expected| expected != self.revision)
        {
            response_kind = StudioResponseKind::Rejected;
            error = Some(StudioError {
                code: StudioErrorCode::StaleRevision,
                message: format!(
                    "Studio state changed from revision {} to {}; refresh before retrying.",
                    request.expected_revision.map_or(0, |revision| revision.0),
                    self.revision.0
                ),
                retryable: true,
            });
        } else {
            match request.action {
                StudioAction::Catalog(CatalogAction::Refresh) => {
                    self.refresh_catalog();
                    self.revision.0 = self.revision.0.saturating_add(1);
                    if let Some(message) = self.catalog_error.clone() {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(StudioError {
                            code: StudioErrorCode::ProviderFailure,
                            message,
                            retryable: true,
                        });
                    }
                }
                StudioAction::Catalog(CatalogAction::DiscoverContentProjects)
                    if self.bootstrap.workspace_root.is_some() =>
                {
                    self.refresh_workspace_content();
                    self.revision.0 = self.revision.0.saturating_add(1);
                }
                StudioAction::Catalog(CatalogAction::DiscoverContentProjects) => {
                    response_kind = StudioResponseKind::Rejected;
                    error = Some(StudioError {
                        code: StudioErrorCode::ActionUnavailable,
                        message: "Open a workspace before discovering its content projects."
                            .to_owned(),
                        retryable: false,
                    });
                }
                StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open { root }) => {
                    let requested = PathBuf::from(root);
                    match std::fs::canonicalize(&requested) {
                        Ok(root) if root.is_dir() => {
                            let previous_state = self.durable_state.clone();
                            let previous_workspace = self.bootstrap.workspace_root.clone();
                            let root_changed =
                                self.bootstrap.workspace_root.as_deref() != Some(&root);
                            self.bootstrap.workspace_root = Some(root);
                            self.durable_state.workspace_root = self
                                .bootstrap
                                .workspace_root
                                .as_ref()
                                .map(|root| root.display().to_string());
                            let workspace = self
                                .bootstrap
                                .workspace_root
                                .clone()
                                .map(|root| WorkspaceAssociation { root });
                            let workspace_profile_changed = self
                                .durable_state
                                .profiles
                                .profiles
                                .iter()
                                .find(|profile| profile.id == self.durable_state.active_profile_id)
                                .is_some_and(|profile| profile.workspace != workspace);
                            if workspace_profile_changed {
                                self.durable_state.profile_revision =
                                    self.durable_state.profile_revision.saturating_add(1);
                            }
                            if workspace_profile_changed
                                && let Some(profile) = self.active_profile_mut()
                            {
                                profile.workspace = workspace;
                                profile.root_package = None;
                                profile.resolved_graph = None;
                                profile.installation = InstallationState::Unknown;
                            }
                            if let Some(store) = &self.state_store
                                && let Err(message) = store.save(&self.durable_state)
                            {
                                self.durable_state = previous_state;
                                self.bootstrap.workspace_root = previous_workspace;
                                response_kind = StudioResponseKind::Rejected;
                                error = Some(StudioError {
                                    code: StudioErrorCode::PersistenceFailure,
                                    message,
                                    retryable: true,
                                });
                                let snapshot = self.snapshot();
                                return StudioResponse {
                                    protocol_version: vapor_studio_protocol::PROTOCOL_VERSION,
                                    request_id: request.request_id,
                                    revision: snapshot.revision,
                                    kind: response_kind,
                                    snapshot,
                                    error,
                                };
                            }
                            self.selected_content_id = None;
                            if root_changed {
                                self.workspace_content = WorkspaceContentSnapshot::default();
                            }
                            self.refresh_catalog();
                            self.revision.0 = self.revision.0.saturating_add(1);
                            if let Some(message) = self.catalog_error.clone() {
                                response_kind = StudioResponseKind::Rejected;
                                error = Some(StudioError {
                                    code: StudioErrorCode::ProviderFailure,
                                    message,
                                    retryable: true,
                                });
                            }
                        }
                        Ok(root) => {
                            response_kind = StudioResponseKind::Rejected;
                            error = Some(StudioError {
                                code: StudioErrorCode::InvalidRequest,
                                message: format!("{} is not a workspace folder", root.display()),
                                retryable: true,
                            });
                        }
                        Err(failure) => {
                            response_kind = StudioResponseKind::Rejected;
                            error = Some(StudioError {
                                code: StudioErrorCode::InvalidRequest,
                                message: format!("{}: {failure}", requested.display()),
                                retryable: true,
                            });
                        }
                    }
                }
                StudioAction::Workspace(
                    action @ (WorkspaceAction::ListFiles { .. }
                    | WorkspaceAction::OpenFile { .. }
                    | WorkspaceAction::SaveFile { .. }
                    | WorkspaceAction::CreateFile { .. }
                    | WorkspaceAction::Validate
                    | WorkspaceAction::Remember
                    | WorkspaceAction::AdoptProject { .. }
                    | WorkspaceAction::UnregisterProject { .. }),
                ) => match self.start_workspace_operation(action) {
                    Ok(()) => {
                        response_kind = StudioResponseKind::OperationAccepted;
                        self.revision.0 = self.revision.0.saturating_add(1);
                    }
                    Err(operation_error) => {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(operation_error);
                    }
                },
                StudioAction::Authoring(action) => match self.start_authoring_operation(action) {
                    Ok(()) => {
                        response_kind = StudioResponseKind::OperationAccepted;
                        self.revision.0 = self.revision.0.saturating_add(1);
                    }
                    Err(operation_error) => {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(operation_error);
                    }
                },
                StudioAction::Catalog(CatalogAction::Search { query }) => {
                    self.catalog_query = query.trim().to_lowercase();
                    self.revision.0 = self.revision.0.saturating_add(1);
                }
                StudioAction::Catalog(CatalogAction::Inspect {
                    content_id,
                    version,
                }) => {
                    if let Some(entry) = self
                        .catalog
                        .iter()
                        .find(|entry| entry.id == content_id && entry.version == version)
                    {
                        self.selected_content_id = Some(match entry.version.as_deref() {
                            Some(version) => format!("{}@{version}", entry.id),
                            None => entry.id.clone(),
                        });
                        self.revision.0 = self.revision.0.saturating_add(1);
                    } else {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(StudioError {
                            code: StudioErrorCode::ActionUnavailable,
                            message:
                                "That content identity is not in the current catalog snapshot."
                                    .to_owned(),
                            retryable: true,
                        });
                    }
                }
                StudioAction::Session(SessionAction::CheckpointLocalState {
                    editor_drafts,
                    operation_history,
                }) => {
                    let previous_state = self.durable_state.clone();
                    self.durable_state.editor_drafts = editor_drafts;
                    self.durable_state.operation_history = operation_history;
                    if let Some(store) = &self.state_store
                        && let Err(message) = store.save(&self.durable_state)
                    {
                        self.durable_state = previous_state;
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(StudioError {
                            code: StudioErrorCode::PersistenceFailure,
                            message,
                            retryable: true,
                        });
                    } else {
                        self.revision.0 = self.revision.0.saturating_add(1);
                    }
                }
                StudioAction::Profile(action) => match self.apply_profile_action(action) {
                    Ok(()) => self.revision.0 = self.revision.0.saturating_add(1),
                    Err(profile_error) => {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(profile_error);
                    }
                },
                StudioAction::Composition(action) => {
                    match self.start_composition_operation(action) {
                        Ok(()) => {
                            response_kind = StudioResponseKind::OperationAccepted;
                            self.revision.0 = self.revision.0.saturating_add(1);
                        }
                        Err(operation_error) => {
                            response_kind = StudioResponseKind::Rejected;
                            error = Some(operation_error);
                        }
                    }
                }
                StudioAction::Workshop(WorkshopAction::DiscardReview) => {
                    self.workshop_review = None;
                    self.revision.0 = self.revision.0.saturating_add(1);
                }
                StudioAction::Workshop(action) => match self.start_workshop_operation(action) {
                    Ok(()) => {
                        response_kind = StudioResponseKind::OperationAccepted;
                        self.revision.0 = self.revision.0.saturating_add(1);
                    }
                    Err(operation_error) => {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(operation_error);
                    }
                },
                action => {
                    response_kind = StudioResponseKind::Rejected;
                    error = Some(StudioError {
                        code: StudioErrorCode::ActionUnavailable,
                        message: format!(
                            "Action '{}' is not connected to the service yet.",
                            action.id()
                        ),
                        retryable: false,
                    });
                }
            }
        }
        let snapshot = self.snapshot();
        StudioResponse {
            protocol_version: vapor_studio_protocol::PROTOCOL_VERSION,
            request_id: request.request_id,
            revision: snapshot.revision,
            kind: response_kind,
            snapshot,
            error,
        }
    }

    /// Drain background lifecycle updates and turn completed composition work into durable profile state.
    pub fn poll_background_operations(&mut self) -> Vec<StudioEvent> {
        self.poll_background_operation_messages();
        self.operation_events.drain(..).collect()
    }

    fn validate_workshop_stage(&mut self, selector: &str) -> Result<PathBuf, StudioError> {
        let Some(workspace_root) = self.bootstrap.workspace_root.clone() else {
            return Err(StudioError {
                code: StudioErrorCode::ActionUnavailable,
                message: "Open the authored content workspace before staging a Workshop item."
                    .to_owned(),
                retryable: false,
            });
        };
        self.refresh_catalog();
        if let Some(error) = self.catalog_error.as_ref() {
            return Err(StudioError {
                code: StudioErrorCode::ProviderFailure,
                message: format!(
                    "Studio could not refresh on-device content before staging: {error}"
                ),
                retryable: true,
            });
        }
        if !self.workspace_content.source_projects_loaded
            || self.workspace_content.source_projects_error.is_some()
        {
            return Err(StudioError {
                code: StudioErrorCode::ProviderFailure,
                message: self
                    .workspace_content
                    .source_projects_error
                    .clone()
                    .unwrap_or_else(|| {
                        "Studio could not refresh the authored project list.".into()
                    }),
                retryable: true,
            });
        }
        let (id, version) = selector
            .split_once('@')
            .map(|(id, version)| (id, Some(version)))
            .unwrap_or((selector, None));
        if id.is_empty()
            || version.is_some_and(str::is_empty)
            || !self
                .workspace_content
                .source_projects
                .iter()
                .any(|project| project.id == id && project.version.as_deref() == version)
        {
            return Err(invalid_workshop_request(
                "Select an exact content identity from the active workspace before staging it.",
            ));
        }
        if self.catalog.iter().any(|entry| {
            entry.id == id && entry.version.as_deref() == version && !entry.conflicts.is_empty()
        }) {
            return Err(invalid_workshop_request(
                "On-device copies of this content version disagree. Resolve the conflict before staging it.",
            ));
        }
        Ok(workspace_root)
    }

    fn start_composition_operation(
        &mut self,
        action: vapor_studio_protocol::CompositionAction,
    ) -> Result<(), StudioError> {
        if self.active_operation.is_some() {
            return Err(StudioError {
                code: StudioErrorCode::OperationInProgress,
                message: "Wait for the current Studio operation to finish.".to_owned(),
                retryable: true,
            });
        }
        use vapor_studio_protocol::CompositionAction as Action;
        let (profile_id, kind, selector_override) = match action {
            Action::Preview {
                profile_id,
                selector,
            } => (profile_id, CompositionOperationKind::Preview, selector),
            Action::Install { profile_id } => (profile_id, CompositionOperationKind::Install, None),
            Action::Verify { profile_id } => (profile_id, CompositionOperationKind::Verify, None),
            Action::Repair { profile_id } => (profile_id, CompositionOperationKind::Repair, None),
            Action::Update { profile_id } => (profile_id, CompositionOperationKind::Update, None),
            Action::Launch { profile_id } => (profile_id, CompositionOperationKind::Launch, None),
            Action::RunSource { profile_id } => {
                (profile_id, CompositionOperationKind::RunSource, None)
            }
        };
        let profile = self
            .durable_state
            .profiles
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| StudioError {
                code: StudioErrorCode::InvalidRequest,
                message: "The selected profile no longer exists.".to_owned(),
                retryable: true,
            })?;
        if profile.root_package.is_none() && selector_override.is_none() {
            return Err(StudioError {
                code: StudioErrorCode::ActionUnavailable,
                message: "Choose a packagepack for this profile first.".to_owned(),
                retryable: false,
            });
        }
        if selector_override.is_some() && !matches!(kind, CompositionOperationKind::Preview) {
            return Err(StudioError {
                code: StudioErrorCode::InvalidRequest,
                message: "Only preview accepts an explicit content selector.".to_owned(),
                retryable: false,
            });
        }
        if matches!(kind, CompositionOperationKind::RunSource) && profile.workspace.is_none() {
            return Err(StudioError {
                code: StudioErrorCode::ActionUnavailable,
                message: "Open the profile's workspace before running its source composition."
                    .to_owned(),
                retryable: false,
            });
        }
        self.composition_preview = None;
        let operation_id = vapor_core::OperationId::fresh();
        let id = operation_id.0.to_string();
        let action_id = match kind {
            CompositionOperationKind::Preview => "composition.preview",
            CompositionOperationKind::Install => "composition.install",
            CompositionOperationKind::Verify => "composition.verify",
            CompositionOperationKind::Repair => "composition.repair",
            CompositionOperationKind::Update => "composition.update",
            CompositionOperationKind::Launch => "composition.launch",
            CompositionOperationKind::RunSource => "composition.run-source",
        };
        self.active_operation = Some(OperationSummary {
            id: id.clone(),
            profile_id: Some(profile_id.clone()),
            action: action_id.to_owned(),
            phase: initial_protocol_phase(kind),
            status: OperationStatus::Running,
            progress: None,
        });

        let bootstrap = self.bootstrap.clone();
        let operation_selector = selector_override.clone();
        let lifecycle_sender = self.operation_sender.clone();
        let completion_sender = self.operation_sender.clone();
        thread::Builder::new()
            .name(format!("studio-composition-{id}"))
            .spawn(move || {
                let request = vapor_core::CoreOperationRequest {
                    id: operation_id,
                    domain: "studio-composition".to_owned(),
                    action: action_id.to_owned(),
                };
                let sink: vapor_core::OperationEventSink = std::sync::Arc::new(move |event| {
                    // The operation runner's terminal event describes the core
                    // call only. The Studio service still has to apply and
                    // persist the resulting profile state before it can report
                    // that the user-visible operation succeeded.
                    if matches!(event.status, vapor_core::OperationStatus::Running) {
                        let _ = lifecycle_sender.send(BackgroundOperationMessage::Lifecycle(event));
                    }
                });
                let result = vapor_core::OperationService::run_observed(
                    request,
                    sink,
                    |context| {
                        context.set_phase(core_operation_phase(kind));
                        execute_composition_operation(
                            &bootstrap,
                            &profile,
                            kind,
                            operation_selector.as_deref(),
                        )
                    },
                    composition_result_outcome,
                );
                let _ = completion_sender.send(BackgroundOperationMessage::Complete(
                    CompositionOperationCompletion {
                        id,
                        profile_id,
                        action: kind,
                        result,
                    },
                ));
            })
            .map_err(|error| {
                self.active_operation = None;
                StudioError {
                    code: StudioErrorCode::InternalFailure,
                    message: format!("Could not start composition operation: {error}"),
                    retryable: true,
                }
            })?;
        Ok(())
    }

    fn poll_background_operation_messages(&mut self) {
        while let Ok(message) = self.operation_receiver.try_recv() {
            match message {
                BackgroundOperationMessage::Lifecycle(event) => {
                    let id = event.id.0.to_string();
                    let Some(active) = self.active_operation.as_mut().filter(|op| op.id == id)
                    else {
                        continue;
                    };
                    active.phase = protocol_phase(event.phase);
                    active.progress = event.progress.as_ref().and_then(|progress| {
                        progress
                            .total
                            .filter(|total| *total > 0)
                            .map(|total| (progress.completed as f32 / total as f32).clamp(0.0, 1.0))
                    });
                    let (status, message) = protocol_status(event.status);
                    active.status = status.clone();
                    self.revision.0 = self.revision.0.saturating_add(1);
                    self.operation_events
                        .push_back(StudioEvent::OperationChanged(OperationEvent {
                            revision: self.revision,
                            id,
                            profile_id: active.profile_id.clone(),
                            action: active.action.clone(),
                            phase: active.phase,
                            status,
                            progress: active.progress,
                            message,
                        }));
                }
                BackgroundOperationMessage::Complete(completion) => {
                    self.apply_composition_completion(completion);
                }
                BackgroundOperationMessage::CompleteWorkshop(completion) => {
                    self.apply_workshop_completion(*completion);
                }
                BackgroundOperationMessage::CompleteWorkspace(completion) => {
                    self.apply_workspace_completion(*completion);
                }
                BackgroundOperationMessage::CompleteAuthoring(completion) => {
                    self.apply_authoring_completion(*completion);
                }
            }
        }
    }

    fn start_workspace_operation(&mut self, action: WorkspaceAction) -> Result<(), StudioError> {
        if self.active_operation.is_some() {
            return Err(StudioError {
                code: StudioErrorCode::OperationInProgress,
                message: "Wait for the current Studio operation to finish.".to_owned(),
                retryable: true,
            });
        }
        let root = self
            .bootstrap
            .workspace_root
            .clone()
            .ok_or_else(|| StudioError {
                code: StudioErrorCode::ActionUnavailable,
                message: "Open a workspace before using its file editor.".to_owned(),
                retryable: false,
            })?;
        let kind = match action {
            WorkspaceAction::ListFiles { path } => {
                if !path.is_empty() && path != "." {
                    return Err(invalid_workspace_request(
                        "Workspace file discovery currently starts at the workspace root.",
                    ));
                }
                WorkspaceOperationKind::ListFiles
            }
            WorkspaceAction::OpenFile { path } => {
                WorkspaceOperationKind::OpenFile(validate_workspace_relative_path(&path)?)
            }
            WorkspaceAction::SaveFile {
                path,
                expected_fingerprint,
                contents,
            } => {
                if contents.len() > 4 * 1024 * 1024 {
                    return Err(invalid_workspace_request(
                        "Workspace editor saves are limited to 4 MiB per file.",
                    ));
                }
                WorkspaceOperationKind::SaveFile(Box::new(WorkspaceFileSave {
                    path: validate_workspace_relative_path(&path)?,
                    expected_fingerprint,
                    contents,
                }))
            }
            WorkspaceAction::CreateFile { path, contents } => {
                if !contents.is_empty() {
                    return Err(invalid_workspace_request(
                        "New workspace files are created empty; edit and save them after creation.",
                    ));
                }
                WorkspaceOperationKind::CreateFile(validate_workspace_relative_path(&path)?)
            }
            WorkspaceAction::Validate => WorkspaceOperationKind::Validate,
            WorkspaceAction::Remember => WorkspaceOperationKind::Remember,
            WorkspaceAction::AdoptProject { path } => {
                let candidate = PathBuf::from(path);
                let candidate = if candidate.is_absolute() {
                    candidate
                } else {
                    root.join(candidate)
                };
                let canonical = candidate.canonicalize().map_err(|error| {
                    invalid_workspace_request(&format!("{}: {error}", candidate.display()))
                })?;
                let canonical_root = root.canonicalize().map_err(|error| {
                    invalid_workspace_request(&format!("{}: {error}", root.display()))
                })?;
                if !canonical.starts_with(&canonical_root) || !canonical.is_dir() {
                    return Err(invalid_workspace_request(
                        "Choose a content project folder inside the active workspace.",
                    ));
                }
                WorkspaceOperationKind::AdoptProject(canonical)
            }
            WorkspaceAction::UnregisterProject { project_id } => {
                if project_id.trim().is_empty() || project_id.len() > 256 {
                    return Err(invalid_workspace_request(
                        "Choose a valid content project ID.",
                    ));
                }
                WorkspaceOperationKind::UnregisterProject(project_id)
            }
            _ => {
                return Err(StudioError {
                    code: StudioErrorCode::ActionUnavailable,
                    message: "This workspace action is not a file editor operation.".to_owned(),
                    retryable: false,
                });
            }
        };
        let operation_id = vapor_core::OperationId::fresh();
        let id = operation_id.0.to_string();
        let action_id = workspace_operation_action_id(&kind);
        self.active_operation = Some(OperationSummary {
            id: id.clone(),
            profile_id: None,
            action: action_id.to_owned(),
            phase: workspace_operation_phase(&kind),
            status: OperationStatus::Running,
            progress: None,
        });

        let action_for_worker = kind.clone();
        let root_for_worker = root.clone();
        let executable_for_worker = self.bootstrap.application_executable.clone();
        let lifecycle_sender = self.operation_sender.clone();
        let completion_sender = self.operation_sender.clone();
        thread::Builder::new()
            .name(format!("studio-workspace-{id}"))
            .spawn(move || {
                let request = vapor_core::CoreOperationRequest {
                    id: operation_id,
                    domain: "studio-workspace".to_owned(),
                    action: action_id.to_owned(),
                };
                let sink: vapor_core::OperationEventSink = std::sync::Arc::new(move |event| {
                    if matches!(event.status, vapor_core::OperationStatus::Running) {
                        let _ = lifecycle_sender.send(BackgroundOperationMessage::Lifecycle(event));
                    }
                });
                let result = vapor_core::OperationService::run_observed(
                    request,
                    sink,
                    |_| {
                        execute_workspace_operation(
                            &executable_for_worker,
                            &root_for_worker,
                            action_for_worker.clone(),
                        )
                    },
                    workspace_result_outcome,
                );
                let _ = completion_sender.send(BackgroundOperationMessage::CompleteWorkspace(
                    Box::new(WorkspaceOperationCompletion {
                        id,
                        action: kind,
                        root,
                        result,
                    }),
                ));
            })
            .map_err(|error| {
                self.active_operation = None;
                StudioError {
                    code: StudioErrorCode::InternalFailure,
                    message: format!("Could not start workspace file operation: {error}"),
                    retryable: true,
                }
            })?;
        Ok(())
    }

    fn apply_workspace_completion(&mut self, completion: WorkspaceOperationCompletion) {
        let workspace_is_current =
            self.bootstrap.workspace_root.as_deref() == Some(&completion.root);
        let mut result = completion.result;
        if !workspace_is_current {
            result =
                Err("The active workspace changed while the file operation was running.".into());
        }
        let mut workspace_file_result = None;
        let mut operation_succeeded = false;
        let message = match result {
            Ok(WorkspaceOperationResult::Listed(files)) => {
                operation_succeeded = true;
                let files = files
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                let message = format!("Found {} workspace files.", files.len());
                workspace_file_result = Some(WorkspaceFileResult::Listed {
                    operation_id: completion.id.clone(),
                    files,
                });
                message
            }
            Ok(WorkspaceOperationResult::Opened(opened)) => {
                operation_succeeded = true;
                let path = opened.path.to_string_lossy().into_owned();
                workspace_file_result = Some(WorkspaceFileResult::Opened {
                    operation_id: completion.id.clone(),
                    path: path.clone(),
                    contents: opened.contents,
                    fingerprint: opened.fingerprint,
                });
                format!("Opened {path}.")
            }
            Ok(WorkspaceOperationResult::Saved { path, fingerprint }) => {
                operation_succeeded = true;
                let path = path.to_string_lossy().into_owned();
                self.workshop_review = None;
                if is_content_manifest_path(Path::new(&path)) {
                    self.refresh_catalog();
                }
                workspace_file_result = Some(WorkspaceFileResult::Saved {
                    operation_id: completion.id.clone(),
                    path: path.clone(),
                    fingerprint,
                });
                format!("Saved {path}.")
            }
            Ok(WorkspaceOperationResult::Created(path)) => {
                operation_succeeded = true;
                let path = path.to_string_lossy().into_owned();
                self.workshop_review = None;
                workspace_file_result = Some(WorkspaceFileResult::Created {
                    operation_id: completion.id.clone(),
                    path: path.clone(),
                });
                format!("Created {path}.")
            }
            Ok(WorkspaceOperationResult::Completed) => {
                operation_succeeded = true;
                if matches!(
                    &completion.action,
                    WorkspaceOperationKind::AdoptProject(_)
                        | WorkspaceOperationKind::UnregisterProject(_)
                ) {
                    self.workshop_review = None;
                    self.refresh_catalog();
                }
                workspace_file_result = Some(WorkspaceFileResult::Completed {
                    operation_id: completion.id.clone(),
                });
                match &completion.action {
                    WorkspaceOperationKind::Validate => {
                        "Workspace validation completed.".to_owned()
                    }
                    WorkspaceOperationKind::AdoptProject(_) => {
                        "Content project added to the workspace.".to_owned()
                    }
                    WorkspaceOperationKind::UnregisterProject(_) => {
                        "Content project removed from discovery.".to_owned()
                    }
                    _ => "Workspace operation completed.".to_owned(),
                }
            }
            Ok(WorkspaceOperationResult::Unregistered(identity)) => {
                operation_succeeded = true;
                self.workshop_review = None;
                self.refresh_catalog();
                workspace_file_result = Some(WorkspaceFileResult::UnregisteredContent {
                    operation_id: completion.id.clone(),
                    content_id: identity.id,
                    version: identity.version,
                });
                "Content project removed from discovery.".to_owned()
            }
            Ok(WorkspaceOperationResult::Remembered(roots)) => {
                operation_succeeded = true;
                let roots = roots
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                workspace_file_result = Some(WorkspaceFileResult::RememberedSources {
                    operation_id: completion.id.clone(),
                    roots: roots.clone(),
                });
                format!(
                    "Registered {} workspace{}.",
                    roots.len(),
                    if roots.len() == 1 { "" } else { "s" }
                )
            }
            Err(error) => error,
        };
        self.active_operation = None;
        self.revision.0 = self.revision.0.saturating_add(1);
        let succeeded = operation_succeeded;
        self.operation_events
            .push_back(StudioEvent::OperationChanged(OperationEvent {
                revision: self.revision,
                id: completion.id.clone(),
                profile_id: None,
                action: workspace_operation_action_id(&completion.action).to_owned(),
                phase: OperationPhase::Complete,
                status: if succeeded {
                    OperationStatus::Succeeded
                } else {
                    OperationStatus::Failed {
                        message: message.clone(),
                    }
                },
                progress: Some(1.0),
                message: Some(message),
            }));
        if let Some(result) = workspace_file_result {
            self.operation_events
                .push_back(StudioEvent::WorkspaceFileResult(Box::new(result)));
        }
    }

    fn start_authoring_operation(&mut self, action: AuthoringAction) -> Result<(), StudioError> {
        if self.active_operation.is_some() {
            return Err(StudioError {
                code: StudioErrorCode::OperationInProgress,
                message: "Wait for the current Studio operation to finish.".to_owned(),
                retryable: true,
            });
        }
        let workspace = self.bootstrap.workspace_root.clone();
        let workspace_bound = !matches!(
            &action,
            AuthoringAction::CreateWorkspace { .. } | AuthoringAction::ListRegisteredSources
        );
        let executable = self.bootstrap.application_executable.clone();
        let operation_id = vapor_core::OperationId::fresh();
        let id = operation_id.0.to_string();
        let action_id = authoring_action_id(&action);
        self.active_operation = Some(OperationSummary {
            id: id.clone(),
            profile_id: None,
            action: action_id.to_owned(),
            phase: OperationPhase::Resolving,
            status: OperationStatus::Running,
            progress: None,
        });
        let lifecycle_sender = self.operation_sender.clone();
        let completion_sender = self.operation_sender.clone();
        thread::Builder::new()
            .name(format!("studio-authoring-{id}"))
            .spawn(move || {
                let request = vapor_core::CoreOperationRequest {
                    id: operation_id,
                    domain: "studio-authoring".to_owned(),
                    action: action_id.to_owned(),
                };
                let sink: vapor_core::OperationEventSink = std::sync::Arc::new(move |event| {
                    if matches!(event.status, vapor_core::OperationStatus::Running) {
                        let _ = lifecycle_sender.send(BackgroundOperationMessage::Lifecycle(event));
                    }
                });
                let result = vapor_core::OperationService::run_observed(
                    request,
                    sink,
                    |_| execute_authoring_operation(&executable, workspace.as_deref(), action),
                    authoring_result_outcome,
                );
                let _ = completion_sender.send(BackgroundOperationMessage::CompleteAuthoring(
                    Box::new(AuthoringOperationCompletion {
                        id,
                        action: action_id.to_owned(),
                        workspace,
                        workspace_bound,
                        result,
                    }),
                ));
            })
            .map_err(|error| {
                self.active_operation = None;
                StudioError {
                    code: StudioErrorCode::InternalFailure,
                    message: format!("Could not start authoring operation: {error}"),
                    retryable: true,
                }
            })?;
        Ok(())
    }

    fn apply_authoring_completion(&mut self, completion: AuthoringOperationCompletion) {
        let mut result = completion.result;
        if completion.workspace_bound
            && self.bootstrap.workspace_root.as_deref() != completion.workspace.as_deref()
        {
            result = Err("The active workspace changed while authoring was in progress.".into());
        }
        let message = result
            .as_ref()
            .map(authoring_result_message)
            .unwrap_or_else(|error| error.clone());
        let succeeded = result.is_ok();
        self.active_operation = None;
        self.revision.0 = self.revision.0.saturating_add(1);
        self.operation_events
            .push_back(StudioEvent::OperationChanged(OperationEvent {
                revision: self.revision,
                id: completion.id.clone(),
                profile_id: None,
                action: completion.action,
                phase: OperationPhase::Complete,
                status: if succeeded {
                    OperationStatus::Succeeded
                } else {
                    OperationStatus::Failed {
                        message: message.clone(),
                    }
                },
                progress: Some(1.0),
                message: Some(message),
            }));
        if let Ok(result) = result {
            match &result {
                AuthoringResultKind::RegisteredSources { .. }
                | AuthoringResultKind::ContentValidated { .. } => {}
                AuthoringResultKind::Composition { .. }
                | AuthoringResultKind::WorkshopMetadata { .. }
                | AuthoringResultKind::RuntimeOutputs { .. } => self.refresh_catalog(),
                AuthoringResultKind::ContentCreated { .. }
                | AuthoringResultKind::WorkspaceCreated { .. } => {}
            }
            self.operation_events
                .push_back(StudioEvent::AuthoringResult(Box::new(AuthoringResult {
                    operation_id: completion.id,
                    result,
                })));
        }
    }

    fn start_workshop_operation(&mut self, action: WorkshopAction) -> Result<(), StudioError> {
        if self.active_operation.is_some() {
            return Err(StudioError {
                code: StudioErrorCode::OperationInProgress,
                message: "Wait for the current Studio operation to finish.".to_owned(),
                retryable: true,
            });
        }
        let kind = match action {
            WorkshopAction::Search { query, page } => {
                if query.len() > 256 {
                    return Err(invalid_workshop_request(
                        "Workshop search terms must be 256 bytes or fewer.",
                    ));
                }
                if !(1..=1_000).contains(&page) {
                    return Err(invalid_workshop_request(
                        "Workshop search page must be between 1 and 1000.",
                    ));
                }
                WorkshopOperationKind::Search {
                    query: query.trim().to_owned(),
                    page,
                }
            }
            WorkshopAction::LoadSubscriptions => WorkshopOperationKind::LoadSubscriptions,
            WorkshopAction::Subscribe { item_ids } => WorkshopOperationKind::ChangeSubscriptions {
                ids: parse_workshop_ids(item_ids)?,
                subscribe: true,
            },
            WorkshopAction::Unsubscribe { item_ids } => {
                WorkshopOperationKind::ChangeSubscriptions {
                    ids: parse_workshop_ids(item_ids)?,
                    subscribe: false,
                }
            }
            WorkshopAction::Acquire { item_ids } => WorkshopOperationKind::Acquire {
                ids: parse_workshop_ids(item_ids)?
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
            },
            WorkshopAction::Stage { selector } => {
                let workspace_root = self.validate_workshop_stage(&selector)?;
                self.workshop_review = None;
                WorkshopOperationKind::Stage {
                    selector,
                    workspace_root,
                }
            }
            WorkshopAction::Publish {
                review_id,
                account,
                confirmation,
                change_note,
            } => {
                let review = self
                    .workshop_review
                    .as_ref()
                    .filter(|review| review.review_id == review_id)
                    .cloned()
                    .ok_or_else(|| {
                        invalid_workshop_request(
                            "This Workshop review is no longer current. Stage the content again.",
                        )
                    })?;
                let workspace_root = self.validate_workshop_stage(&review.selector)?;
                if workspace_root != review.workspace_root {
                    return Err(invalid_workshop_request(
                        "The active workspace changed after this review was staged.",
                    ));
                }
                let account = account.trim().to_owned();
                if account.is_empty() || account.len() > 128 {
                    return Err(invalid_workshop_request(
                        "Enter a publisher account name of 1 to 128 bytes.",
                    ));
                }
                let expected_confirmation = format!("PUBLISH {} TO {account}", review.preview.id);
                if confirmation.trim() != expected_confirmation {
                    return Err(invalid_workshop_request(&format!(
                        "Type {expected_confirmation} exactly to authorize this Workshop upload."
                    )));
                }
                let change_note = change_note.trim();
                if change_note.len() > 500 {
                    return Err(invalid_workshop_request(
                        "Workshop change notes must be 500 bytes or fewer.",
                    ));
                }
                WorkshopOperationKind::Publish {
                    review: Box::new(review),
                    account,
                    change_note: (!change_note.is_empty()).then(|| change_note.to_owned()),
                }
            }
            WorkshopAction::DiscardReview => {
                return Err(invalid_workshop_request(
                    "Workshop review discard must use the synchronous service path.",
                ));
            }
        };
        let operation_id = vapor_core::OperationId::fresh();
        let id = operation_id.0.to_string();
        let action_id = workshop_operation_action_id(&kind);
        self.active_operation = Some(OperationSummary {
            id: id.clone(),
            profile_id: None,
            action: action_id.to_owned(),
            phase: OperationPhase::Resolving,
            status: OperationStatus::Running,
            progress: None,
        });

        let fixture_mode = self.bootstrap.fixture_mode;
        let application_executable = self.bootstrap.application_executable.clone();
        let workshop_app_id = self.bootstrap.workshop_app_id;
        let fixture_subscriptions = self.fixture_workshop_subscriptions.clone();
        let lifecycle_sender = self.operation_sender.clone();
        let completion_sender = self.operation_sender.clone();
        thread::Builder::new()
            .name(format!("studio-workshop-{id}"))
            .spawn(move || {
                let request = vapor_core::CoreOperationRequest {
                    id: operation_id,
                    domain: "studio-workshop".to_owned(),
                    action: action_id.to_owned(),
                };
                let sink: vapor_core::OperationEventSink = std::sync::Arc::new(move |event| {
                    if matches!(event.status, vapor_core::OperationStatus::Running) {
                        let _ = lifecycle_sender.send(BackgroundOperationMessage::Lifecycle(event));
                    }
                });
                let action_for_worker = kind.clone();
                let result = vapor_core::OperationService::run_observed(
                    request,
                    sink,
                    |_| {
                        execute_workshop_operation(
                            &application_executable,
                            fixture_mode,
                            workshop_app_id,
                            fixture_subscriptions,
                            action_for_worker,
                        )
                    },
                    workshop_result_outcome,
                );
                let _ = completion_sender.send(BackgroundOperationMessage::CompleteWorkshop(
                    Box::new(WorkshopOperationCompletion {
                        id,
                        action: kind,
                        result,
                    }),
                ));
            })
            .map_err(|error| {
                self.active_operation = None;
                StudioError {
                    code: StudioErrorCode::InternalFailure,
                    message: format!("Could not start Workshop operation: {error}"),
                    retryable: true,
                }
            })?;
        Ok(())
    }

    fn apply_workshop_completion(&mut self, completion: WorkshopOperationCompletion) {
        let mut succeeded = completion.result.is_ok();
        let mut failure = completion.result.as_ref().err().cloned();
        let mut summary = None;
        match completion.result {
            Ok(WorkshopOperationResult::Search(page)) => {
                summary = Some(format!(
                    "Steam Workshop returned {} items on page {} of {}.",
                    page.items.len(),
                    page.page,
                    page.total_results
                ));
                let revision = self
                    .workshop
                    .search
                    .as_ref()
                    .map_or(1, |search| search.revision.saturating_add(1));
                self.workshop.search = Some(workshop_search_snapshot(
                    match &completion.action {
                        WorkshopOperationKind::Search { query, .. } => query.clone(),
                        _ => String::new(),
                    },
                    page,
                    revision,
                ));
            }
            Ok(WorkshopOperationResult::Subscriptions(subscriptions)) => {
                self.fixture_workshop_subscriptions = subscriptions
                    .iter()
                    .map(|item| item.published_file_id.clone())
                    .collect();
                self.workshop.subscriptions = subscriptions
                    .iter()
                    .map(workshop_subscription_snapshot)
                    .collect();
                self.workshop.subscriptions_loaded = true;
                self.workshop.subscriptions_revision =
                    self.workshop.subscriptions_revision.saturating_add(1);
                summary = Some(format!(
                    "Loaded {} Steam Workshop subscriptions.",
                    subscriptions.len()
                ));
            }
            Ok(WorkshopOperationResult::Acquisitions { items, requested }) => {
                let revision = self
                    .workshop
                    .acquisition
                    .as_ref()
                    .map_or(1, |batch| batch.revision.saturating_add(1));
                let items = items
                    .iter()
                    .map(workshop_acquisition_snapshot)
                    .collect::<Vec<_>>();
                let imported = items.len();
                if self.bootstrap.fixture_mode {
                    self.fixture_acquisitions
                        .extend(items.iter().cloned().map(|item| (item.id.clone(), item)));
                }
                self.workshop.acquisition = Some(WorkshopAcquisitionBatchSnapshot {
                    revision,
                    requested,
                    items,
                });
                summary = Some(format!(
                    "Added {} Workshop items to content available on this device.",
                    imported
                ));
                self.refresh_catalog();
            }
            Ok(WorkshopOperationResult::Review(preview)) => {
                let (selector, workspace_root) = match &completion.action {
                    WorkshopOperationKind::Stage {
                        selector,
                        workspace_root,
                    } => (selector.clone(), workspace_root.clone()),
                    _ => (String::new(), PathBuf::new()),
                };
                if self.bootstrap.workspace_root.as_deref() != Some(workspace_root.as_path()) {
                    succeeded = false;
                    failure = Some(
                        "The active workspace changed while this Workshop item was being staged. Stage it again from the current workspace.".to_owned(),
                    );
                } else {
                    self.workshop_review = Some(WorkshopReviewState {
                        review_id: completion.id.clone(),
                        selector,
                        workspace_root,
                        preview: preview.clone(),
                    });
                    summary = Some(format!(
                        "Staged {} · {} files · {} bytes for review.",
                        preview.id, preview.files, preview.bytes
                    ));
                }
            }
            Ok(WorkshopOperationResult::Publication(publication)) => {
                self.workshop_review = None;
                summary = Some(format!(
                    "{} Workshop item {}{}; receipt saved at {}.",
                    if publication.created {
                        "Created"
                    } else {
                        "Updated"
                    },
                    publication.id,
                    publication
                        .published_file_id
                        .as_ref()
                        .map(|id| format!(" · PublishedFileId {id}"))
                        .unwrap_or_default(),
                    publication.receipt.display()
                ));
                self.refresh_catalog();
            }
            Err(_) => {}
        }
        self.active_operation = None;
        self.revision.0 = self.revision.0.saturating_add(1);
        let action = workshop_operation_action_id(&completion.action).to_owned();
        let message = failure.or(summary);
        self.operation_events
            .push_back(StudioEvent::OperationChanged(OperationEvent {
                revision: self.revision,
                id: completion.id,
                profile_id: None,
                action,
                phase: OperationPhase::Complete,
                status: if succeeded {
                    OperationStatus::Succeeded
                } else {
                    OperationStatus::Failed {
                        message: message
                            .clone()
                            .unwrap_or_else(|| "Workshop operation failed.".to_owned()),
                    }
                },
                progress: Some(1.0),
                message,
            }));
    }

    fn apply_composition_completion(&mut self, completion: CompositionOperationCompletion) {
        let profile_id = completion.profile_id.clone();
        let previous_profiles = self.durable_state.profiles.profiles.clone();
        let previous_profile_revision = self.durable_state.profile_revision;
        let outcome = composition_result_outcome(&completion.result);
        let mut failure = apply_operation_result_to_profile(
            &mut self.composition_preview,
            &mut self.durable_state.profiles.profiles,
            &profile_id,
            completion.action,
            completion.result,
        );
        if failure.is_none() {
            failure = outcome.as_ref().err().cloned();
        }
        if self.durable_state.profiles.profiles != previous_profiles {
            self.durable_state.profile_revision =
                self.durable_state.profile_revision.saturating_add(1);
            if let Some(store) = &self.state_store
                && let Err(error) = store.save(&self.durable_state)
            {
                self.durable_state.profiles.profiles = previous_profiles;
                self.durable_state.profile_revision = previous_profile_revision;
                failure = Some(format!("Could not save the composition result: {error}"));
            }
        }
        if let Some(message) = failure {
            self.active_operation = None;
            self.revision.0 = self.revision.0.saturating_add(1);
            self.operation_events
                .push_back(StudioEvent::OperationChanged(OperationEvent {
                    revision: self.revision,
                    id: completion.id,
                    profile_id: Some(profile_id),
                    action: operation_action_id(completion.action).to_owned(),
                    phase: OperationPhase::Complete,
                    status: OperationStatus::Failed {
                        message: message.clone(),
                    },
                    progress: None,
                    message: Some(message),
                }));
            return;
        }
        let summary = outcome.unwrap_or_else(|_| "Completed".to_owned());
        self.active_operation = None;
        if matches!(
            completion.action,
            CompositionOperationKind::Install
                | CompositionOperationKind::Repair
                | CompositionOperationKind::Update
                | CompositionOperationKind::Launch
                | CompositionOperationKind::RunSource
        ) {
            self.refresh_catalog();
        }
        self.revision.0 = self.revision.0.saturating_add(1);
        self.operation_events
            .push_back(StudioEvent::OperationChanged(OperationEvent {
                revision: self.revision,
                id: completion.id,
                profile_id: Some(profile_id),
                action: operation_action_id(completion.action).to_owned(),
                phase: OperationPhase::Complete,
                status: OperationStatus::Succeeded,
                progress: Some(1.0),
                message: Some(summary),
            }));
    }

    fn refresh_catalog(&mut self) {
        self.refresh_workspace_content();
        if self.bootstrap.fixture_mode {
            self.catalog_query.clear();
            self.catalog = vec![CatalogEntry {
                id: "fixture.engine".to_owned(),
                version: Some("1.0.0".to_owned()),
                kind: "engine".to_owned(),
                published_file_id: None,
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                locations: vec![CatalogLocation {
                    kind: CatalogLocationKind::Cache,
                    root: "fixture/cache/fixture.engine".to_owned(),
                    enabled: true,
                    fingerprint: Some("fixture:v1".to_owned()),
                    install_source: None,
                }],
            }];
            self.catalog
                .extend(self.fixture_acquisitions.values().map(|item| CatalogEntry {
                    id: item.id.clone(),
                    version: item.version.clone(),
                    kind: item.kind.clone(),
                    published_file_id: None,
                    dependencies: Vec::new(),
                    conflicts: Vec::new(),
                    locations: vec![CatalogLocation {
                        kind: CatalogLocationKind::Cache,
                        root: item.cache_root.clone(),
                        enabled: true,
                        fingerprint: Some(item.fingerprint.clone()),
                        install_source: None,
                    }],
                }));
            self.catalog_error = None;
            return;
        }
        match discover_content_catalog(
            &self.bootstrap.application_executable,
            self.bootstrap.workspace_root.as_deref(),
        ) {
            Ok(catalog) => {
                self.catalog_query.clear();
                self.catalog = catalog
                    .into_iter()
                    .take(MAX_CATALOG_ITEMS)
                    .map(catalog_entry)
                    .collect();
                self.catalog_error = None;
            }
            Err(error) => self.catalog_error = Some(error),
        }
    }

    fn refresh_workspace_content(&mut self) {
        let Some(root) = self.bootstrap.workspace_root.clone() else {
            self.workspace_content = WorkspaceContentSnapshot {
                source_projects_loaded: true,
                unregistered_projects_loaded: true,
                ..WorkspaceContentSnapshot::default()
            };
            return;
        };
        if self.bootstrap.fixture_mode {
            self.workspace_content = fixture_workspace_content(&root);
            return;
        }

        self.workspace_content.workspace_validation_error =
            vapor_studio_core::validate_workspace(&root).err();

        match discover_source_content(&self.bootstrap.application_executable, &root) {
            Ok(projects) => {
                self.workspace_content.source_projects = projects
                    .into_iter()
                    .take(MAX_WORKSPACE_PROJECTS)
                    .map(source_project)
                    .collect();
                self.workspace_content.source_projects_loaded = true;
                self.workspace_content.source_projects_error = None;
            }
            Err(error) => self.workspace_content.source_projects_error = Some(error),
        }

        match scan_unregistered_content_projects(&self.bootstrap.application_executable, &root) {
            Ok(scan) => {
                self.workspace_content.unregistered_projects = scan
                    .candidates
                    .into_iter()
                    .take(MAX_WORKSPACE_PROJECTS)
                    .map(|candidate| WorkspaceProjectCandidate {
                        path: candidate.path.display().to_string(),
                        id: candidate.id,
                        kind: candidate.kind.to_string(),
                    })
                    .collect();
                self.workspace_content.unregistered_diagnostics = scan
                    .diagnostics
                    .into_iter()
                    .take(MAX_WORKSPACE_DIAGNOSTICS)
                    .collect();
                self.workspace_content.unregistered_projects_loaded = true;
                self.workspace_content.unregistered_projects_error = None;
            }
            Err(error) => self.workspace_content.unregistered_projects_error = Some(error),
        }
    }

    fn active_profile_mut(&mut self) -> Option<&mut CompositionProfile> {
        let active = self.durable_state.active_profile_id.clone();
        self.durable_state
            .profiles
            .profiles
            .iter_mut()
            .find(|profile| profile.id == active)
    }

    fn apply_profile_action(&mut self, action: ProfileAction) -> Result<(), StudioError> {
        let previous_state = self.durable_state.clone();
        let previous_workspace = self.bootstrap.workspace_root.clone();
        let transition = match action {
            ProfileAction::Create { name } => add_profile(
                &mut self.durable_state.profiles.profiles,
                &name,
                self.bootstrap.workspace_root.clone(),
            )
            .map(|index| {
                self.durable_state.active_profile_id =
                    self.durable_state.profiles.profiles[index].id.clone();
            }),
            ProfileAction::Duplicate { profile_id, name } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    duplicate_profile(&mut self.durable_state.profiles.profiles, index)
                })
                .and_then(|index| {
                    if name.trim().is_empty() {
                        Ok(index)
                    } else {
                        rename_profile(&mut self.durable_state.profiles.profiles, index, &name)?;
                        Ok(index)
                    }
                })
                .map(|index| {
                    self.durable_state.active_profile_id =
                        self.durable_state.profiles.profiles[index].id.clone();
                }),
            ProfileAction::Rename { profile_id, name } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    rename_profile(&mut self.durable_state.profiles.profiles, index, &name)
                }),
            ProfileAction::Remove { profile_id } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    remove_profile(&mut self.durable_state.profiles.profiles, index).map(|_| ())
                })
                .map(|()| {
                    if self.durable_state.active_profile_id == profile_id {
                        self.durable_state.active_profile_id =
                            self.durable_state.profiles.profiles[0].id.clone();
                    }
                }),
            ProfileAction::Select { profile_id } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .map(|index| {
                    self.durable_state.active_profile_id =
                        self.durable_state.profiles.profiles[index].id.clone();
                }),
            ProfileAction::SetRoot {
                profile_id,
                content_id,
                content_version,
                workspace_root,
            } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .map(|index| {
                    self.durable_state.profiles.profiles[index].root_package =
                        content_id.map(|id| ContentIdentity {
                            id,
                            version: content_version,
                        });
                    self.durable_state.profiles.profiles[index].resolved_graph = None;
                    self.durable_state.profiles.profiles[index].installation =
                        InstallationState::Unknown;
                    self.durable_state.profiles.profiles[index].workspace = workspace_root
                        .map(PathBuf::from)
                        .map(|root| WorkspaceAssociation { root });
                }),
            ProfileAction::ResolveDefaultRoot { profile_id } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    if self.durable_state.profiles.profiles[index]
                        .root_package
                        .is_some()
                    {
                        return Err("This profile already has a selected composition root.".into());
                    }
                    let content_id = if self.bootstrap.fixture_mode {
                        "fixture.pack".to_owned()
                    } else {
                        vapor_studio_core::default_packagepack_selector(
                            &self.bootstrap.application_executable,
                            "default",
                        )
                        .map_err(|error| {
                            format!("This app has no default playable composition: {error}")
                        })?
                    };
                    let profile = &mut self.durable_state.profiles.profiles[index];
                    profile.root_package = Some(ContentIdentity {
                        id: content_id,
                        version: None,
                    });
                    profile.resolved_graph = None;
                    profile.installation = InstallationState::Unknown;
                    Ok(())
                }),
            ProfileAction::SetLaunchArguments {
                profile_id,
                arguments,
            } => {
                if arguments.len() > 64 || arguments.iter().any(|argument| argument.len() > 4096) {
                    Err(
                        "A profile can have at most 64 launch arguments, each 4096 bytes or fewer."
                            .to_owned(),
                    )
                } else {
                    self.profile_index(&profile_id)
                        .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                        .map(|index| {
                            self.durable_state.profiles.profiles[index].launch.arguments =
                                arguments;
                        })
                }
            }
            ProfileAction::AdoptVerifiedGraph { profile_id } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    let profile = &mut self.durable_state.profiles.profiles[index];
                    let InstallationState::OutOfDate {
                        actual_graph,
                        artifacts,
                        verified: true,
                        ..
                    } = profile.installation.clone()
                    else {
                        return Err("Only a verified installed graph can be adopted.".to_owned());
                    };
                    profile.resolved_graph = Some(actual_graph.clone());
                    profile.installation = InstallationState::Installed {
                        graph_fingerprint: vapor_studio_core::fingerprint_resolved_graph(
                            &actual_graph,
                        ),
                        artifacts,
                    };
                    Ok(())
                }),
            ProfileAction::InvalidateContent {
                content_id,
                content_version,
            } => {
                let matches_content = |identity: &ContentIdentity| {
                    identity.id == content_id
                        && content_version
                            .as_ref()
                            .is_none_or(|version| identity.version.as_ref() == Some(version))
                };
                for profile in &mut self.durable_state.profiles.profiles {
                    let affected = profile.root_package.as_ref().is_some_and(&matches_content)
                        || profile.resolved_graph.as_ref().is_some_and(|graph| {
                            graph
                                .nodes
                                .iter()
                                .any(|node| matches_content(&node.content))
                        });
                    if affected {
                        profile.resolved_graph = None;
                        profile.installation = InstallationState::Unknown;
                    }
                }
                Ok(())
            }
        };
        if let Err(message) = transition {
            self.durable_state = previous_state;
            self.active_profile_id = Some(self.durable_state.active_profile_id.clone());
            self.bootstrap.workspace_root = previous_workspace;
            return Err(StudioError {
                code: StudioErrorCode::InvalidRequest,
                message,
                retryable: true,
            });
        }
        self.durable_state.profile_revision = previous_state.profile_revision.saturating_add(1);
        self.active_profile_id = Some(self.durable_state.active_profile_id.clone());
        self.bootstrap.workspace_root = self.active_profile_mut().and_then(|profile| {
            profile
                .workspace
                .as_ref()
                .map(|workspace| workspace.root.clone())
        });
        self.durable_state.workspace_root = self
            .bootstrap
            .workspace_root
            .as_ref()
            .map(|root| root.display().to_string());
        let workspace_changed = previous_workspace != self.bootstrap.workspace_root;
        if workspace_changed {
            self.selected_content_id = None;
            self.workspace_content = WorkspaceContentSnapshot::default();
            self.refresh_catalog();
        }
        if let Some(store) = &self.state_store
            && let Err(message) = store.save(&self.durable_state)
        {
            self.durable_state = previous_state;
            self.active_profile_id = Some(self.durable_state.active_profile_id.clone());
            self.bootstrap.workspace_root = previous_workspace;
            return Err(StudioError {
                code: StudioErrorCode::PersistenceFailure,
                message,
                retryable: true,
            });
        }
        Ok(())
    }

    fn profile_index(&self, profile_id: &str) -> Option<usize> {
        self.durable_state
            .profiles
            .profiles
            .iter()
            .position(|profile| profile.id == profile_id)
    }
}

fn operation_action_id(kind: CompositionOperationKind) -> &'static str {
    match kind {
        CompositionOperationKind::Preview => "composition.preview",
        CompositionOperationKind::Install => "composition.install",
        CompositionOperationKind::Verify => "composition.verify",
        CompositionOperationKind::Repair => "composition.repair",
        CompositionOperationKind::Update => "composition.update",
        CompositionOperationKind::Launch => "composition.launch",
        CompositionOperationKind::RunSource => "composition.run-source",
    }
}

fn invalid_workshop_request(message: &str) -> StudioError {
    StudioError {
        code: StudioErrorCode::InvalidRequest,
        message: message.to_owned(),
        retryable: false,
    }
}

fn parse_workshop_ids(ids: Vec<String>) -> Result<Vec<u64>, StudioError> {
    if ids.is_empty() {
        return Err(invalid_workshop_request(
            "Select at least one Workshop item.",
        ));
    }
    if ids.len() > vapor_studio_core::MAX_WORKSHOP_SUBSCRIPTION_BATCH {
        return Err(invalid_workshop_request(&format!(
            "At most {} Workshop items can be changed at once.",
            vapor_studio_core::MAX_WORKSHOP_SUBSCRIPTION_BATCH
        )));
    }
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::new();
    for id in ids {
        let value = id.trim().parse::<u64>().ok().filter(|value| *value != 0);
        let Some(value) = value else {
            return Err(invalid_workshop_request(&format!(
                "Invalid Steam PublishedFileId: {}",
                id.trim()
            )));
        };
        if seen.insert(value) {
            parsed.push(value);
        }
    }
    Ok(parsed)
}

fn workshop_operation_action_id(kind: &WorkshopOperationKind) -> &'static str {
    match kind {
        WorkshopOperationKind::Search { .. } => "workshop.search",
        WorkshopOperationKind::LoadSubscriptions => "workshop.load-subscriptions",
        WorkshopOperationKind::ChangeSubscriptions {
            subscribe: true, ..
        } => "workshop.subscribe",
        WorkshopOperationKind::ChangeSubscriptions {
            subscribe: false, ..
        } => "workshop.unsubscribe",
        WorkshopOperationKind::Acquire { .. } => "workshop.acquire",
        WorkshopOperationKind::Stage { .. } => "workshop.stage",
        WorkshopOperationKind::Publish { .. } => "workshop.publish",
    }
}

fn validate_workspace_relative_path(path: &str) -> Result<PathBuf, StudioError> {
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(invalid_workspace_request(
            "Choose a file path relative to the open workspace.",
        ));
    }
    Ok(path)
}

fn authoring_action_id(action: &AuthoringAction) -> &'static str {
    match action {
        AuthoringAction::LoadComposition { .. } => "authoring.load-composition",
        AuthoringAction::ListRegisteredSources => "authoring.list-registered-sources",
        AuthoringAction::SaveComposition { .. } => "authoring.save-composition",
        AuthoringAction::LoadWorkshopMetadata { .. } => "authoring.load-workshop-metadata",
        AuthoringAction::SaveWorkshopMetadata { .. } => "authoring.save-workshop-metadata",
        AuthoringAction::LoadRuntimeOutputs { .. } => "authoring.load-runtime-outputs",
        AuthoringAction::ValidateContent { .. } => "authoring.validate-content",
        AuthoringAction::SaveRuntimeOutputs { .. } => "authoring.save-runtime-outputs",
        AuthoringAction::CreateContent { .. } => "authoring.create-content",
        AuthoringAction::CreateWorkspace { .. } => "authoring.create-workspace",
    }
}

fn execute_authoring_operation(
    executable: &Path,
    active_workspace: Option<&Path>,
    action: AuthoringAction,
) -> Result<AuthoringResultKind, String> {
    use AuthoringAction as Action;
    match action {
        Action::ListRegisteredSources => {
            let installation =
                vapor_core::VaporInstallation::discover().map_err(|error| error.to_string())?;
            let state =
                vapor_core::source_state(&installation).map_err(|error| error.to_string())?;
            Ok(AuthoringResultKind::RegisteredSources {
                roots: state
                    .known
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
                active: state.active.map(|path| path.to_string_lossy().into_owned()),
            })
        }
        Action::LoadComposition { project_id } => {
            let workspace = active_workspace.ok_or("Open a workspace before editing content.")?;
            vapor_studio_core::load_source_composition(executable, workspace, &project_id).map(
                |value| AuthoringResultKind::Composition {
                    id: value.id,
                    version: value.version,
                    kind: value.kind,
                    manifest: value.manifest.to_string_lossy().into_owned(),
                    source: value.source,
                    links: value
                        .links
                        .into_iter()
                        .map(|link| AuthoringCompositionLink {
                            id: link.id,
                            relationship: link.relationship,
                            optional: link.optional,
                            version: link.version,
                            workshop_id: link.workshop_id,
                            reason: link.reason,
                        })
                        .collect(),
                },
            )
        }
        Action::SaveComposition {
            project_id,
            expected_fingerprint,
            dependencies,
        } => {
            let workspace = active_workspace.ok_or("Open a workspace before editing content.")?;
            let links = dependencies
                .into_iter()
                .map(|link| vapor_studio_core::SourceCompositionLink {
                    id: link.project_id,
                    relationship: link.relationship,
                    optional: link.optional,
                    version: link.version_requirement,
                    workshop_id: link.workshop_id,
                    reason: link.reason,
                })
                .collect::<Vec<_>>();
            vapor_studio_core::save_source_composition(
                executable,
                workspace,
                &project_id,
                &expected_fingerprint,
                &links,
            )
            .map(|value| AuthoringResultKind::Composition {
                id: value.id,
                version: value.version,
                kind: value.kind,
                manifest: value.manifest.to_string_lossy().into_owned(),
                source: value.source,
                links: value
                    .links
                    .into_iter()
                    .map(|link| AuthoringCompositionLink {
                        id: link.id,
                        relationship: link.relationship,
                        optional: link.optional,
                        version: link.version,
                        workshop_id: link.workshop_id,
                        reason: link.reason,
                    })
                    .collect(),
            })
        }
        Action::LoadWorkshopMetadata { project_id } => {
            let workspace = active_workspace.ok_or("Open a workspace before editing content.")?;
            vapor_studio_core::load_source_workshop_metadata(executable, workspace, &project_id)
                .map(|value| AuthoringResultKind::WorkshopMetadata {
                    id: value.id,
                    version: value.version,
                    kind: value.kind,
                    manifest: value.manifest.to_string_lossy().into_owned(),
                    source: value.source,
                    policy: workshop_policy_to_protocol(value.policy),
                })
        }
        Action::SaveWorkshopMetadata {
            project_id,
            expected_fingerprint,
            app_id,
            published_file_id,
            visibility,
            title,
            description,
            tags,
            change_note,
        } => {
            let workspace = active_workspace.ok_or("Open a workspace before editing content.")?;
            vapor_studio_core::save_source_workshop_metadata(
                executable,
                workspace,
                &project_id,
                &expected_fingerprint,
                &vapor_studio_core::WorkshopPolicyDraft {
                    app_id,
                    published_file_id,
                    visibility,
                    title,
                    description,
                    tags,
                    change_note,
                },
            )
            .map(|value| AuthoringResultKind::WorkshopMetadata {
                id: value.id,
                version: value.version,
                kind: value.kind,
                manifest: value.manifest.to_string_lossy().into_owned(),
                source: value.source,
                policy: workshop_policy_to_protocol(value.policy),
            })
        }
        Action::LoadRuntimeOutputs { project_id } => {
            let workspace = active_workspace.ok_or("Open a workspace before editing content.")?;
            vapor_studio_core::load_source_runtime_outputs(executable, workspace, &project_id).map(
                |value| AuthoringResultKind::RuntimeOutputs {
                    id: value.id,
                    version: value.version,
                    kind: value.kind,
                    manifest: value.manifest.to_string_lossy().into_owned(),
                    source: value.source,
                    binaries: value.binaries,
                    libraries: value.libraries,
                },
            )
        }
        Action::ValidateContent { project_id } => {
            let workspace =
                active_workspace.ok_or("Open a workspace before validating content.")?;
            vapor_studio_core::validate_profile_source(executable, workspace, &project_id)
                .map(|()| AuthoringResultKind::ContentValidated { id: project_id })
        }
        Action::SaveRuntimeOutputs {
            project_id,
            expected_fingerprint,
            binaries,
            libraries,
        } => {
            let workspace = active_workspace.ok_or("Open a workspace before editing content.")?;
            vapor_studio_core::save_source_runtime_outputs(
                executable,
                workspace,
                &project_id,
                &expected_fingerprint,
                &binaries,
                &libraries,
            )
            .map(|value| AuthoringResultKind::RuntimeOutputs {
                id: value.id,
                version: value.version,
                kind: value.kind,
                manifest: value.manifest.to_string_lossy().into_owned(),
                source: value.source,
                binaries: value.binaries,
                libraries: value.libraries,
            })
        }
        Action::CreateContent {
            workspace_root,
            kind,
            id,
            version,
            dependencies,
            workshop,
        } => {
            let active = active_workspace.ok_or("Open a workspace before creating content.")?;
            let requested = PathBuf::from(workspace_root)
                .canonicalize()
                .map_err(|error| error.to_string())?;
            let active = active.canonicalize().map_err(|error| error.to_string())?;
            if requested != active {
                return Err("Content creation must target the currently open workspace.".to_owned());
            }
            let kind = parse_authoring_content_kind(&kind)?;
            let policy = vapor_studio_core::WorkshopPolicyDraft {
                app_id: workshop.app_id,
                published_file_id: workshop.published_file_id,
                visibility: workshop.visibility,
                title: workshop.title,
                description: workshop.description,
                tags: workshop.tags,
                change_note: workshop.change_note,
            };
            let manifest = vapor_studio_core::create_content_workspace_with_workshop(
                executable,
                &active,
                &id,
                &version,
                kind,
                &dependencies,
                &policy,
            )?;
            let contents = std::fs::read_to_string(&manifest)
                .map_err(|error| format!("{}: {error}", manifest.display()))?;
            let files = vapor_studio_core::discover_workspace_files(&active)?
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            Ok(AuthoringResultKind::ContentCreated {
                workspace_root: active.to_string_lossy().into_owned(),
                manifest: manifest
                    .strip_prefix(&active)
                    .unwrap_or(&manifest)
                    .to_string_lossy()
                    .into_owned(),
                manifest_contents: contents,
                files,
            })
        }
        Action::CreateWorkspace {
            parent,
            organization,
            name,
        } => {
            if !valid_workspace_slug(&organization) || !valid_workspace_slug(&name) {
                return Err(
                    "Workspace organization and name must use lowercase letters, digits, and internal hyphens."
                        .to_owned(),
                );
            }
            let parent = PathBuf::from(parent);
            let parent = parent
                .canonicalize()
                .map_err(|error| format!("{}: {error}", parent.display()))?;
            if !parent.is_dir() {
                return Err(format!("{} is not a directory", parent.display()));
            }
            let destination = parent.join(&name);
            let root = vapor_studio_core::create_basic_content_workspace(
                executable,
                &destination,
                &organization,
                &name,
            )?;
            let files = vapor_studio_core::discover_workspace_files(&root)?
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            Ok(AuthoringResultKind::WorkspaceCreated {
                root: root.to_string_lossy().into_owned(),
                files,
            })
        }
    }
}

fn valid_workspace_slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn workshop_policy_to_protocol(
    value: vapor_studio_core::WorkshopPolicyDraft,
) -> WorkshopMetadataDraft {
    WorkshopMetadataDraft {
        app_id: value.app_id,
        published_file_id: value.published_file_id,
        visibility: value.visibility,
        title: value.title,
        description: value.description,
        tags: value.tags,
        change_note: value.change_note,
    }
}

fn parse_authoring_content_kind(kind: &str) -> Result<vapor_core::ContentKind, String> {
    Ok(match kind {
        "engine" => vapor_core::ContentKind::Engine,
        "game" => vapor_core::ContentKind::Game,
        "packagepack" => vapor_core::ContentKind::Packagepack,
        "enginepack" => vapor_core::ContentKind::Enginepack,
        "gamepack" => vapor_core::ContentKind::Gamepack,
        "modpack" => vapor_core::ContentKind::Modpack,
        "engine-mod" => vapor_core::ContentKind::EngineMod,
        "game-mod" => vapor_core::ContentKind::GameMod,
        "extension-mod" => vapor_core::ContentKind::ExtensionMod,
        _ => return Err(format!("Unsupported source content role: {kind}")),
    })
}

fn authoring_result_outcome(
    result: &Result<AuthoringResultKind, String>,
) -> Result<String, String> {
    result
        .as_ref()
        .map(authoring_result_message)
        .map_err(Clone::clone)
}

fn authoring_result_message(result: &AuthoringResultKind) -> String {
    match result {
        AuthoringResultKind::Composition { id, .. } => format!("Loaded composition for {id}."),
        AuthoringResultKind::RegisteredSources { roots, .. } => {
            format!(
                "Loaded {} registered workspace{}.",
                roots.len(),
                if roots.len() == 1 { "" } else { "s" }
            )
        }
        AuthoringResultKind::WorkshopMetadata { id, .. } => {
            format!("Loaded Workshop metadata for {id}.")
        }
        AuthoringResultKind::RuntimeOutputs { id, .. } => {
            format!("Loaded runtime outputs for {id}.")
        }
        AuthoringResultKind::ContentValidated { id } => format!("Validated {id}."),
        AuthoringResultKind::ContentCreated { manifest, .. } => {
            format!("Created content at {manifest}.")
        }
        AuthoringResultKind::WorkspaceCreated { root, .. } => {
            format!("Created workspace at {root}.")
        }
    }
}

fn invalid_workspace_request(message: &str) -> StudioError {
    StudioError {
        code: StudioErrorCode::InvalidRequest,
        message: message.to_owned(),
        retryable: false,
    }
}

fn workspace_operation_action_id(kind: &WorkspaceOperationKind) -> &'static str {
    match kind {
        WorkspaceOperationKind::ListFiles => "workspace.list-files",
        WorkspaceOperationKind::OpenFile(_) => "workspace.open-file",
        WorkspaceOperationKind::SaveFile(_) => "workspace.save-file",
        WorkspaceOperationKind::CreateFile(_) => "workspace.create-file",
        WorkspaceOperationKind::Validate => "workspace.validate",
        WorkspaceOperationKind::Remember => "workspace.remember",
        WorkspaceOperationKind::AdoptProject(_) => "workspace.adopt-project",
        WorkspaceOperationKind::UnregisterProject(_) => "workspace.unregister-project",
    }
}

fn workspace_operation_phase(kind: &WorkspaceOperationKind) -> OperationPhase {
    match kind {
        WorkspaceOperationKind::ListFiles | WorkspaceOperationKind::OpenFile(_) => {
            OperationPhase::Resolving
        }
        WorkspaceOperationKind::SaveFile(_) | WorkspaceOperationKind::CreateFile(_) => {
            OperationPhase::Building
        }
        WorkspaceOperationKind::Validate
        | WorkspaceOperationKind::Remember
        | WorkspaceOperationKind::AdoptProject(_)
        | WorkspaceOperationKind::UnregisterProject(_) => OperationPhase::Resolving,
    }
}

fn execute_workspace_operation(
    executable: &Path,
    root: &Path,
    kind: WorkspaceOperationKind,
) -> Result<WorkspaceOperationResult, String> {
    match kind {
        WorkspaceOperationKind::ListFiles => {
            vapor_studio_core::discover_workspace_files(root).map(WorkspaceOperationResult::Listed)
        }
        WorkspaceOperationKind::OpenFile(path) => {
            vapor_studio_core::read_workspace_file_with_fingerprint(root, &path).map(
                |(contents, fingerprint)| {
                    WorkspaceOperationResult::Opened(Box::new(WorkspaceFileOpen {
                        path,
                        contents,
                        fingerprint,
                    }))
                },
            )
        }
        WorkspaceOperationKind::SaveFile(save) => {
            vapor_studio_core::write_workspace_file_if_fingerprint(
                root,
                &save.path,
                &save.expected_fingerprint,
                &save.contents,
            )
            .map(|fingerprint| WorkspaceOperationResult::Saved {
                path: save.path,
                fingerprint,
            })
        }
        WorkspaceOperationKind::CreateFile(path) => {
            vapor_studio_core::create_new_workspace_file(root, &path)
                .map(WorkspaceOperationResult::Created)
        }
        WorkspaceOperationKind::Validate => {
            vapor_studio_core::validate_workspace_source(executable, root)
                .map(|()| WorkspaceOperationResult::Completed)
        }
        WorkspaceOperationKind::Remember => {
            vapor_studio_core::remember_workspace(root).map(WorkspaceOperationResult::Remembered)
        }
        WorkspaceOperationKind::AdoptProject(path) => {
            vapor_studio_core::adopt_workspace_content_project(executable, root, &path)
                .map(|_| WorkspaceOperationResult::Completed)
        }
        WorkspaceOperationKind::UnregisterProject(id) => {
            vapor_studio_core::unregister_workspace_content_project(executable, root, &id)
                .map(|(_, _, identity)| WorkspaceOperationResult::Unregistered(identity))
        }
    }
}

fn workspace_result_outcome(
    result: &Result<WorkspaceOperationResult, String>,
) -> Result<String, String> {
    match result {
        Ok(WorkspaceOperationResult::Listed(files)) => {
            Ok(format!("Found {} workspace files.", files.len()))
        }
        Ok(WorkspaceOperationResult::Opened(opened)) => {
            Ok(format!("Opened {}.", opened.path.display()))
        }
        Ok(WorkspaceOperationResult::Saved { path, .. }) => {
            Ok(format!("Saved {}.", path.display()))
        }
        Ok(WorkspaceOperationResult::Created(path)) => Ok(format!("Created {}.", path.display())),
        Ok(WorkspaceOperationResult::Completed) => Ok("Workspace operation completed.".to_owned()),
        Ok(WorkspaceOperationResult::Remembered(paths)) => Ok(format!(
            "Registered {} workspace{}.",
            paths.len(),
            if paths.len() == 1 { "" } else { "s" }
        )),
        Ok(WorkspaceOperationResult::Unregistered(identity)) => {
            Ok(format!("Unregistered {}.", identity.id))
        }
        Err(error) => Err(error.clone()),
    }
}

fn is_content_manifest_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == "Vapor.toml" || name.to_string_lossy().ends_with(".vapor.toml"))
}

fn workshop_result_outcome(
    result: &Result<WorkshopOperationResult, String>,
) -> Result<String, String> {
    match result {
        Ok(WorkshopOperationResult::Search(page)) => Ok(format!(
            "Steam Workshop returned {} items on page {} of {}.",
            page.items.len(),
            page.page,
            page.total_results
        )),
        Ok(WorkshopOperationResult::Subscriptions(items)) => Ok(format!(
            "Loaded {} Steam Workshop subscriptions.",
            items.len()
        )),
        Ok(WorkshopOperationResult::Acquisitions { items, requested }) => {
            if items.len() == *requested {
                Ok(format!(
                    "Added {} Workshop items to the local cache.",
                    items.len()
                ))
            } else {
                Err(format!(
                    "Downloaded {} of {requested} requested Workshop items.",
                    items.len()
                ))
            }
        }
        Ok(WorkshopOperationResult::Review(preview)) => Ok(format!(
            "Staged {} · {} files · {} bytes for review.",
            preview.id, preview.files, preview.bytes
        )),
        Ok(WorkshopOperationResult::Publication(publication)) => Ok(format!(
            "{} Workshop item {}{}; receipt saved at {}.",
            if publication.created {
                "Created"
            } else {
                "Updated"
            },
            publication.id,
            publication
                .published_file_id
                .as_ref()
                .map(|id| format!(" · PublishedFileId {id}"))
                .unwrap_or_default(),
            publication.receipt.display()
        )),
        Err(error) => Err(error.clone()),
    }
}

fn execute_workshop_operation(
    application_executable: &Path,
    fixture_mode: bool,
    workshop_app_id: Option<u32>,
    fixture_subscriptions: BTreeSet<String>,
    kind: WorkshopOperationKind,
) -> Result<WorkshopOperationResult, String> {
    if fixture_mode {
        return Ok(fixture_workshop_operation(fixture_subscriptions, kind));
    }
    match kind {
        WorkshopOperationKind::Search { query, page } => {
            let app_id = required_workshop_app_id(workshop_app_id)?;
            vapor_studio_core::search_workshop(app_id, &query, page)
                .map(WorkshopOperationResult::Search)
        }
        WorkshopOperationKind::LoadSubscriptions => {
            let app_id = required_workshop_app_id(workshop_app_id)?;
            vapor_studio_core::workshop_subscriptions(app_id)
                .map(WorkshopOperationResult::Subscriptions)
        }
        WorkshopOperationKind::ChangeSubscriptions { ids, subscribe } => {
            let app_id = required_workshop_app_id(workshop_app_id)?;
            let change = if subscribe {
                vapor_studio_core::subscribe_workshop_items(app_id, &ids)
            } else {
                vapor_studio_core::unsubscribe_workshop_items(app_id, &ids)
            };
            change?;
            vapor_studio_core::workshop_subscriptions(app_id)
                .map(WorkshopOperationResult::Subscriptions)
                .map_err(|error| {
                    format!(
                        "Subscription change completed, but the updated list could not be loaded: {error}"
                    )
                })
        }
        WorkshopOperationKind::Acquire { ids } => {
            let requested = ids.len();
            vapor_studio_core::download_workshop_items(application_executable, &ids)
                .map(|items| WorkshopOperationResult::Acquisitions { items, requested })
        }
        WorkshopOperationKind::Stage {
            selector,
            workspace_root,
        } => vapor_studio_core::stage_workshop_preview(
            application_executable,
            &workspace_root,
            &selector,
        )
        .map(WorkshopOperationResult::Review),
        WorkshopOperationKind::Publish {
            review,
            account,
            change_note,
        } => vapor_studio_core::publish_reviewed_workshop_content(
            application_executable,
            &review.workspace_root,
            &review.selector,
            &review.preview,
            &account,
            change_note.as_deref(),
        )
        .map(WorkshopOperationResult::Publication),
    }
}

fn required_workshop_app_id(app_id: Option<u32>) -> Result<u32, String> {
    app_id.ok_or_else(|| {
        "This Studio installation has no configured Workshop application.".to_owned()
    })
}

fn fixture_workshop_operation(
    mut subscriptions: BTreeSet<String>,
    kind: WorkshopOperationKind,
) -> WorkshopOperationResult {
    let items = vec![
        fixture_workshop_item("1001", "Fixture Starter Pack"),
        fixture_workshop_item("1002", "Fixture Moon Pack"),
    ];
    match kind {
        WorkshopOperationKind::Search { query, page } => {
            let query = query.to_lowercase();
            let items = items
                .into_iter()
                .filter(|item| item.title.to_lowercase().contains(&query))
                .collect::<Vec<_>>();
            WorkshopOperationResult::Search(vapor_studio_core::WorkshopPage {
                total_results: items.len() as u32,
                items,
                page,
            })
        }
        WorkshopOperationKind::LoadSubscriptions => WorkshopOperationResult::Subscriptions(
            subscriptions
                .iter()
                .map(|id| fixture_workshop_subscription(id))
                .collect(),
        ),
        WorkshopOperationKind::ChangeSubscriptions { ids, subscribe } => {
            for id in ids {
                if subscribe {
                    subscriptions.insert(id.to_string());
                } else {
                    subscriptions.remove(&id.to_string());
                }
            }
            WorkshopOperationResult::Subscriptions(
                subscriptions
                    .iter()
                    .map(|id| fixture_workshop_subscription(id))
                    .collect(),
            )
        }
        WorkshopOperationKind::Acquire { ids } => {
            let requested = ids.len();
            let items = ids
                .into_iter()
                .map(|id| vapor_studio_core::WorkshopAcquisition {
                    id: format!("fixture.content.{id}"),
                    version: Some("1.0.0".to_owned()),
                    kind: "mod".to_owned(),
                    cache_root: PathBuf::from(format!("fixture/cache/fixture.content.{id}")),
                    fingerprint: format!("fixture:{id}"),
                })
                .collect();
            WorkshopOperationResult::Acquisitions { items, requested }
        }
        WorkshopOperationKind::Stage {
            selector,
            workspace_root: _,
        } => {
            let (id, version) = selector
                .split_once('@')
                .map(|(id, version)| (id.to_owned(), Some(version.to_owned())))
                .unwrap_or((selector, Some("1.0.0".to_owned())));
            WorkshopOperationResult::Review(vapor_studio_core::WorkshopPackagePreview {
                id,
                version,
                app_id: Some(2122620),
                published_file_id: None,
                title: Some("Fixture Workshop artifact".to_owned()),
                visibility: Some("public".to_owned()),
                dependencies: Vec::new(),
                root: PathBuf::from("fixture/staged/artifact"),
                target: "fixture".to_owned(),
                fingerprint: "fixture:review".to_owned(),
                files: 1,
                bytes: 128,
                file_paths: vec!["Vapor.toml".to_owned()],
            })
        }
        WorkshopOperationKind::Publish { review, .. } => {
            WorkshopOperationResult::Publication(vapor_studio_core::WorkshopPublication {
                id: review.preview.id.clone(),
                published_file_id: Some("1002".to_owned()),
                receipt: PathBuf::from("fixture/receipts/workshop.json"),
                created: review.preview.published_file_id.is_none(),
            })
        }
    }
}

fn fixture_workshop_item(id: &str, title: &str) -> vapor_studio_core::WorkshopItem {
    vapor_studio_core::WorkshopItem {
        published_file_id: id.to_owned(),
        title: title.to_owned(),
        description: "Deterministic Studio fixture; no Steam connection is used.".to_owned(),
        preview_url: None,
        updated_at_unix: 1_797_000_000,
    }
}

fn fixture_workshop_subscription(id: &str) -> vapor_studio_core::WorkshopSubscription {
    let item = match id {
        "1001" => fixture_workshop_item(id, "Fixture Starter Pack"),
        "1002" => fixture_workshop_item(id, "Fixture Moon Pack"),
        _ => fixture_workshop_item(id, &format!("Fixture Workshop item {id}")),
    };
    vapor_studio_core::WorkshopSubscription {
        published_file_id: item.published_file_id,
        title: item.title,
        description: item.description,
        preview_url: item.preview_url,
        updated_at_unix: item.updated_at_unix,
        installed: false,
        needs_update: false,
        downloading: false,
        download_progress: None,
    }
}

fn workshop_search_snapshot(
    query: String,
    page: vapor_studio_core::WorkshopPage,
    revision: u64,
) -> WorkshopSearchSnapshot {
    WorkshopSearchSnapshot {
        revision,
        query,
        page: page.page,
        total_results: page.total_results,
        items: page.items.into_iter().map(workshop_item_snapshot).collect(),
    }
}

fn workshop_item_snapshot(item: vapor_studio_core::WorkshopItem) -> WorkshopItemSnapshot {
    WorkshopItemSnapshot {
        published_file_id: item.published_file_id,
        title: item.title,
        description: item.description,
        preview_url: item.preview_url,
        updated_at_unix: item.updated_at_unix,
    }
}

fn workshop_subscription_snapshot(
    item: &vapor_studio_core::WorkshopSubscription,
) -> WorkshopSubscriptionSnapshot {
    WorkshopSubscriptionSnapshot {
        published_file_id: item.published_file_id.clone(),
        title: item.title.clone(),
        description: item.description.clone(),
        preview_url: item.preview_url.clone(),
        updated_at_unix: item.updated_at_unix,
        installed: item.installed,
        needs_update: item.needs_update,
        downloading: item.downloading,
        download_progress: item.download_progress,
    }
}

fn workshop_acquisition_snapshot(
    item: &vapor_studio_core::WorkshopAcquisition,
) -> WorkshopAcquisitionSnapshot {
    WorkshopAcquisitionSnapshot {
        id: item.id.clone(),
        version: item.version.clone(),
        kind: item.kind.clone(),
        cache_root: item.cache_root.display().to_string(),
        fingerprint: item.fingerprint.clone(),
    }
}

fn workshop_review_snapshot(review: &WorkshopReviewState) -> WorkshopReviewSnapshot {
    let preview = &review.preview;
    let visible_paths = preview
        .file_paths
        .iter()
        .take(MAX_WORKSHOP_REVIEW_FILE_PATHS)
        .cloned()
        .collect::<Vec<_>>();
    WorkshopReviewSnapshot {
        review_id: review.review_id.clone(),
        selector: review.selector.clone(),
        preview: WorkshopPackagePreviewSnapshot {
            id: preview.id.clone(),
            version: preview.version.clone(),
            app_id: preview.app_id,
            published_file_id: preview.published_file_id.clone(),
            title: preview.title.clone(),
            visibility: preview.visibility.clone(),
            dependencies: preview.dependencies.clone(),
            root: preview.root.display().to_string(),
            target: preview.target.clone(),
            fingerprint: preview.fingerprint.clone(),
            files: preview.files,
            bytes: preview.bytes,
            omitted_file_paths: preview.file_paths.len().saturating_sub(visible_paths.len()),
            file_paths: visible_paths,
        },
    }
}

fn initial_protocol_phase(kind: CompositionOperationKind) -> OperationPhase {
    match kind {
        CompositionOperationKind::Preview => OperationPhase::Resolving,
        CompositionOperationKind::Install
        | CompositionOperationKind::Repair
        | CompositionOperationKind::Update => OperationPhase::Acquiring,
        CompositionOperationKind::Verify => OperationPhase::Verifying,
        CompositionOperationKind::Launch | CompositionOperationKind::RunSource => {
            OperationPhase::Running
        }
    }
}

fn core_operation_phase(kind: CompositionOperationKind) -> vapor_core::OperationPhase {
    match kind {
        CompositionOperationKind::Preview => vapor_core::OperationPhase::Resolving,
        CompositionOperationKind::Install
        | CompositionOperationKind::Repair
        | CompositionOperationKind::Update => vapor_core::OperationPhase::Acquiring,
        CompositionOperationKind::Verify => vapor_core::OperationPhase::Verifying,
        CompositionOperationKind::Launch | CompositionOperationKind::RunSource => {
            vapor_core::OperationPhase::Running
        }
    }
}

fn protocol_phase(phase: vapor_core::OperationPhase) -> OperationPhase {
    match phase {
        vapor_core::OperationPhase::Queued => OperationPhase::Queued,
        vapor_core::OperationPhase::Resolving => OperationPhase::Resolving,
        vapor_core::OperationPhase::Acquiring => OperationPhase::Acquiring,
        vapor_core::OperationPhase::Installing => OperationPhase::Installing,
        vapor_core::OperationPhase::Verifying => OperationPhase::Verifying,
        vapor_core::OperationPhase::Building => OperationPhase::Building,
        vapor_core::OperationPhase::Running => OperationPhase::Running,
        vapor_core::OperationPhase::Staging => OperationPhase::Staging,
        vapor_core::OperationPhase::Publishing => OperationPhase::Publishing,
        vapor_core::OperationPhase::Finalizing => OperationPhase::Finalizing,
    }
}

fn protocol_status(status: vapor_core::OperationStatus) -> (OperationStatus, Option<String>) {
    match status {
        vapor_core::OperationStatus::Running => (OperationStatus::Running, None),
        vapor_core::OperationStatus::Succeeded { summary } => {
            (OperationStatus::Succeeded, Some(summary))
        }
        vapor_core::OperationStatus::Failed { message } => (
            OperationStatus::Failed {
                message: message.clone(),
            },
            Some(message),
        ),
        vapor_core::OperationStatus::Cancelled => (OperationStatus::Cancelled, None),
    }
}

fn composition_result_outcome(result: &CompositionOperationResult) -> Result<String, String> {
    match result {
        CompositionOperationResult::Preview(Ok(preview)) => Ok(format!(
            "Resolved {} composition items{}",
            preview.nodes.len(),
            if preview.blockers.is_empty() {
                String::new()
            } else {
                format!(" with {} blockers", preview.blockers.len())
            }
        )),
        CompositionOperationResult::Install(Ok(composition))
        | CompositionOperationResult::Repair(Ok(composition))
        | CompositionOperationResult::Update(Ok(composition)) => Ok(format!(
            "Composition installed with {} content items",
            composition.graph.nodes.len()
        )),
        CompositionOperationResult::Verify(Ok(check)) if check.ok => {
            Ok(format!("Verified {} content items", check.detail.len()))
        }
        CompositionOperationResult::Verify(Ok(check)) => Err(check.detail.join("; ")),
        CompositionOperationResult::Launch(Ok(result))
        | CompositionOperationResult::RunSource(Ok(result)) => {
            if result.profile_graph_matches {
                result
                    .launch_error
                    .clone()
                    .map_or_else(|| Ok("Composition launched".to_owned()), Err)
            } else {
                Err(result.launch_error.clone().unwrap_or_else(|| {
                    "The installed composition differs from the profile.".to_owned()
                }))
            }
        }
        CompositionOperationResult::Preview(Err(error))
        | CompositionOperationResult::Install(Err(error))
        | CompositionOperationResult::Verify(Err(error))
        | CompositionOperationResult::Repair(Err(error))
        | CompositionOperationResult::Update(Err(error))
        | CompositionOperationResult::Launch(Err(error))
        | CompositionOperationResult::RunSource(Err(error)) => Err(error.clone()),
    }
}

fn execute_composition_operation(
    bootstrap: &ServiceBootstrap,
    profile: &CompositionProfile,
    kind: CompositionOperationKind,
    selector_override: Option<&str>,
) -> CompositionOperationResult {
    let selector = selector_override
        .map(ToOwned::to_owned)
        .or_else(|| {
            profile.root_package.as_ref().map(|root| {
                root.version.as_deref().map_or_else(
                    || root.id.clone(),
                    |version| format!("{}@{version}", root.id),
                )
            })
        })
        .expect("validated before spawn");
    let workspace = profile
        .workspace
        .as_ref()
        .map(|workspace| workspace.root.as_path());
    if bootstrap.fixture_mode {
        return fixture_composition_result(profile, selector, kind);
    }
    let executable = &bootstrap.application_executable;
    let graph_selectors = profile
        .resolved_graph
        .as_ref()
        .map(|graph| {
            graph
                .nodes
                .iter()
                .map(|node| {
                    node.content.version.as_deref().map_or_else(
                        || node.content.id.clone(),
                        |version| format!("{}@{version}", node.content.id),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![selector.clone()]);
    match kind {
        CompositionOperationKind::Preview => CompositionOperationResult::Preview(
            vapor_studio_core::preview_profile_composition(executable, workspace, &selector),
        ),
        CompositionOperationKind::Install => CompositionOperationResult::Install(
            vapor_studio_core::install_profile_composition(executable, workspace, &selector),
        ),
        CompositionOperationKind::Verify => CompositionOperationResult::Verify(
            vapor_studio_core::verify_installation(executable, &selector, &graph_selectors),
        ),
        CompositionOperationKind::Repair => {
            CompositionOperationResult::Repair(vapor_studio_core::repair_profile_composition(
                executable,
                workspace,
                &selector,
                &graph_selectors,
            ))
        }
        CompositionOperationKind::Update => CompositionOperationResult::Update(
            vapor_studio_core::update_profile_composition(executable, workspace, &selector),
        ),
        CompositionOperationKind::Launch => {
            CompositionOperationResult::Launch(vapor_studio_core::run_installed_profile(
                executable,
                workspace,
                &selector,
                profile.resolved_graph.as_ref(),
                &profile.launch.arguments,
            ))
        }
        CompositionOperationKind::RunSource => {
            let Some(workspace) = workspace else {
                return CompositionOperationResult::RunSource(Err(
                    "Open the profile's workspace before running its source composition."
                        .to_owned(),
                ));
            };
            CompositionOperationResult::RunSource(vapor_studio_core::run_source_profile(
                executable,
                workspace,
                &selector,
                &profile.launch.arguments,
            ))
        }
    }
}

fn fixture_composition_result(
    profile: &CompositionProfile,
    selector: String,
    kind: CompositionOperationKind,
) -> CompositionOperationResult {
    use vapor_studio_core::{
        CompositionPreview, CompositionPreviewNode, CompositionPreviewSource, InstallationCheck,
        InstalledArtifact, InstalledComposition,
    };
    let root = profile.root_package.clone().unwrap_or_else(|| {
        let (id, version) = selector.split_once('@').map_or_else(
            || (selector.as_str(), None),
            |(id, version)| (id, Some(version)),
        );
        ContentIdentity {
            id: id.to_owned(),
            version: version.map(ToOwned::to_owned),
        }
    });
    let preview = CompositionPreview {
        selector,
        nodes: vec![CompositionPreviewNode {
            id: root.id.clone(),
            version: root.version.clone(),
            kind: "packagepack".to_owned(),
            published_file_id: None,
            source: if matches!(&profile.installation, InstallationState::Installed { .. }) {
                CompositionPreviewSource::Installed
            } else {
                CompositionPreviewSource::Workspace
            },
            already_installed: matches!(&profile.installation, InstallationState::Installed { .. }),
            dependencies: Vec::new(),
        }],
        blockers: Vec::new(),
        warnings: vec![
            "Fixture mode: no content provider, Steam, or game process was used.".into(),
        ],
    };
    let graph = profile.resolved_graph.clone().unwrap_or_else(|| {
        let engine = ContentIdentity {
            id: "fixture.engine".to_owned(),
            version: Some("1.0.0".to_owned()),
        };
        vapor_studio_protocol::ResolvedCompositionGraph {
            root: root.clone(),
            effective_engine: engine.clone(),
            effective_game: None,
            nodes: vec![
                vapor_studio_protocol::ResolvedNodeIdentity {
                    content: root.clone(),
                    kind: "packagepack".to_owned(),
                    fingerprint: Some("fixture-root".to_owned()),
                    source: Some("fixture".to_owned()),
                    dependencies: vec![vapor_studio_protocol::ResolvedDependencyIdentity {
                        binding: "engine".to_owned(),
                        content: engine.clone(),
                    }],
                },
                vapor_studio_protocol::ResolvedNodeIdentity {
                    content: engine,
                    kind: "engine".to_owned(),
                    fingerprint: Some("fixture-engine".to_owned()),
                    source: Some("fixture".to_owned()),
                    dependencies: Vec::new(),
                },
            ],
        }
    });
    let composition = InstalledComposition {
        artifacts: graph
            .nodes
            .iter()
            .map(|node| InstalledArtifact {
                id: node.content.id.clone(),
                version: node.content.version.clone(),
                root: PathBuf::from(format!("/fixture/installed/{}", node.content.id)),
                fingerprint: node.fingerprint.clone().unwrap_or_default(),
            })
            .collect(),
        graph: graph.clone(),
    };
    match kind {
        CompositionOperationKind::Preview => CompositionOperationResult::Preview(Ok(preview)),
        CompositionOperationKind::Install => CompositionOperationResult::Install(Ok(composition)),
        CompositionOperationKind::Verify => {
            CompositionOperationResult::Verify(Ok(InstallationCheck {
                ok: true,
                detail: vec!["Fixture installation verified".into()],
                artifacts: composition.artifacts.clone(),
                graph: Some(graph),
            }))
        }
        CompositionOperationKind::Repair => CompositionOperationResult::Repair(Ok(composition)),
        CompositionOperationKind::Update => CompositionOperationResult::Update(Ok(composition)),
        CompositionOperationKind::Launch => {
            CompositionOperationResult::Launch(Ok(vapor_studio_core::ProfileRunResult {
                composition,
                profile_graph_matches: true,
                launch_error: None,
            }))
        }
        CompositionOperationKind::RunSource => {
            CompositionOperationResult::RunSource(Ok(vapor_studio_core::ProfileRunResult {
                composition,
                profile_graph_matches: true,
                launch_error: None,
            }))
        }
    }
}

fn apply_operation_result_to_profile(
    preview: &mut Option<vapor_studio_protocol::CompositionPreviewSnapshot>,
    profiles: &mut [CompositionProfile],
    profile_id: &str,
    kind: CompositionOperationKind,
    result: CompositionOperationResult,
) -> Option<String> {
    use vapor_studio_protocol::InstallationState;
    let Some(profile) = profiles.iter_mut().find(|profile| profile.id == profile_id) else {
        return Some("The profile was removed while its operation was running.".to_owned());
    };
    match result {
        CompositionOperationResult::Preview(Ok(value)) => {
            *preview = Some(preview_snapshot(profile_id, value));
        }
        CompositionOperationResult::Preview(Err(error)) => return Some(error),
        CompositionOperationResult::Install(Ok(composition))
        | CompositionOperationResult::Repair(Ok(composition))
        | CompositionOperationResult::Update(Ok(composition)) => {
            let graph = composition.graph;
            if matches!(kind, CompositionOperationKind::Repair)
                && profile
                    .resolved_graph
                    .as_ref()
                    .is_some_and(|saved| !vapor_studio_core::resolved_graph_matches(saved, &graph))
            {
                profile.installation = InstallationState::OutOfDate {
                    detail: "Repair restored valid content, but its graph differs from this profile's saved composition.".into(),
                    actual_graph: graph,
                    artifacts: installed_records(composition.artifacts),
                    verified: false,
                };
            } else {
                profile.resolved_graph = Some(graph.clone());
                profile.installation = installed_state(graph, composition.artifacts);
            }
        }
        CompositionOperationResult::Verify(Ok(check)) => {
            let artifacts = protocol_artifacts(check.artifacts);
            if !check.ok {
                profile.installation = InstallationState::NeedsRepair {
                    detail: check.detail.join("; "),
                    artifacts,
                };
            } else if let Some(graph) = check.graph {
                if profile
                    .resolved_graph
                    .as_ref()
                    .is_some_and(|saved| !vapor_studio_core::resolved_graph_matches(saved, &graph))
                {
                    profile.installation = InstallationState::OutOfDate {
                        detail:
                            "Verified installed content differs from this profile's saved setup."
                                .into(),
                        actual_graph: graph,
                        artifacts,
                        verified: true,
                    };
                } else {
                    profile.resolved_graph = Some(graph.clone());
                    profile.installation = InstallationState::Installed {
                        artifacts,
                        graph_fingerprint: vapor_studio_core::fingerprint_resolved_graph(&graph),
                    };
                }
            } else {
                profile.installation = InstallationState::NeedsRepair {
                    detail: "Verification completed without a resolved installed graph.".into(),
                    artifacts,
                };
            }
        }
        CompositionOperationResult::Verify(Err(error)) => {
            let artifacts = installation_artifacts(&profile.installation);
            profile.installation = InstallationState::NeedsRepair {
                detail: error.clone(),
                artifacts,
            };
            return Some(error);
        }
        CompositionOperationResult::Launch(Ok(result))
        | CompositionOperationResult::RunSource(Ok(result)) => {
            let graph = result.composition.graph;
            let artifacts = protocol_artifacts(result.composition.artifacts);
            if result.profile_graph_matches {
                profile.resolved_graph = Some(graph.clone());
                profile.installation = InstallationState::Installed {
                    artifacts,
                    graph_fingerprint: vapor_studio_core::fingerprint_resolved_graph(&graph),
                };
            } else {
                profile.installation = InstallationState::OutOfDate {
                    detail: result.launch_error.clone().unwrap_or_else(|| {
                        "The installed composition differs from this profile.".into()
                    }),
                    actual_graph: graph,
                    artifacts,
                    verified: false,
                };
            }
            if result.launch_error.is_some() || !result.profile_graph_matches {
                return result.launch_error.or_else(|| {
                    Some("The installed composition differs from this profile.".into())
                });
            }
        }
        CompositionOperationResult::Install(Err(error))
        | CompositionOperationResult::Repair(Err(error))
        | CompositionOperationResult::Update(Err(error)) => {
            profile.installation = InstallationState::Partial {
                detail: error.clone(),
            };
            return Some(error);
        }
        CompositionOperationResult::Launch(Err(error))
        | CompositionOperationResult::RunSource(Err(error)) => return Some(error),
    }
    None
}

fn preview_snapshot(
    profile_id: &str,
    preview: vapor_studio_core::CompositionPreview,
) -> vapor_studio_protocol::CompositionPreviewSnapshot {
    vapor_studio_protocol::CompositionPreviewSnapshot {
        profile_id: profile_id.to_owned(),
        selector: preview.selector,
        nodes: preview
            .nodes
            .into_iter()
            .map(|node| vapor_studio_protocol::CompositionPreviewNode {
                id: node.id,
                version: node.version,
                kind: node.kind,
                published_file_id: node.published_file_id,
                source: match node.source {
                    vapor_studio_core::CompositionPreviewSource::Workspace => {
                        vapor_studio_protocol::CompositionPreviewSource::Workspace
                    }
                    vapor_studio_core::CompositionPreviewSource::Cache => {
                        vapor_studio_protocol::CompositionPreviewSource::Cache
                    }
                    vapor_studio_core::CompositionPreviewSource::Installed => {
                        vapor_studio_protocol::CompositionPreviewSource::Installed
                    }
                    vapor_studio_core::CompositionPreviewSource::DownloadRequired => {
                        vapor_studio_protocol::CompositionPreviewSource::DownloadRequired
                    }
                },
                already_installed: node.already_installed,
                dependencies: node
                    .dependencies
                    .into_iter()
                    .map(
                        |dependency| vapor_studio_protocol::CompositionPreviewDependency {
                            id: dependency.id,
                            relationship: dependency.relationship,
                            version: dependency.version,
                            optional: dependency.optional,
                        },
                    )
                    .collect(),
            })
            .collect(),
        blockers: preview.blockers,
        warnings: preview.warnings,
    }
}

fn installed_state(
    graph: vapor_studio_protocol::ResolvedCompositionGraph,
    artifacts: Vec<vapor_studio_core::InstalledArtifact>,
) -> vapor_studio_protocol::InstallationState {
    vapor_studio_protocol::InstallationState::Installed {
        graph_fingerprint: vapor_studio_core::fingerprint_resolved_graph(&graph),
        artifacts: installed_records(artifacts),
    }
}

fn installed_records(
    artifacts: Vec<vapor_studio_core::InstalledArtifact>,
) -> Vec<vapor_studio_protocol::InstalledArtifactRecord> {
    artifacts
        .into_iter()
        .map(|artifact| vapor_studio_protocol::InstalledArtifactRecord {
            id: artifact.id,
            version: artifact.version,
            root: artifact.root,
            fingerprint: artifact.fingerprint,
        })
        .collect()
}

fn protocol_artifacts(
    artifacts: Vec<vapor_studio_core::InstalledArtifact>,
) -> Vec<vapor_studio_protocol::InstalledArtifactRecord> {
    installed_records(artifacts)
}

fn installation_artifacts(
    state: &InstallationState,
) -> Vec<vapor_studio_protocol::InstalledArtifactRecord> {
    match state {
        InstallationState::Installed { artifacts, .. }
        | InstallationState::NeedsRepair { artifacts, .. }
        | InstallationState::OutOfDate { artifacts, .. } => artifacts.clone(),
        _ => Vec::new(),
    }
}

fn source_project(project: SourceContent) -> WorkspaceSourceProject {
    WorkspaceSourceProject {
        id: project.id,
        version: project.version,
        kind: project.kind,
        root: project.root.display().to_string(),
        manifest: project.manifest.display().to_string(),
        binaries: project.binaries,
        libraries: project.libraries,
        dependencies: project.dependencies,
        workshop: WorkspaceWorkshopPolicy {
            app_id: project.workshop.app_id,
            published_file_id: project.workshop.published_file_id,
            visibility: project.workshop.visibility,
            title: project.workshop.title,
            description: project.workshop.description,
            tags: project.workshop.tags,
            change_note: project.workshop.change_note,
        },
    }
}

fn fixture_workspace_content(root: &Path) -> WorkspaceContentSnapshot {
    WorkspaceContentSnapshot {
        source_projects: vec![WorkspaceSourceProject {
            id: "fixture.game".to_owned(),
            version: Some("1.0.0".to_owned()),
            kind: "game".to_owned(),
            root: root.join("game").display().to_string(),
            manifest: root.join("game/Vapor.toml").display().to_string(),
            binaries: vec!["fixture-game".to_owned()],
            libraries: Vec::new(),
            dependencies: vec!["required · fixture.engine".to_owned()],
            workshop: WorkspaceWorkshopPolicy::default(),
        }],
        source_projects_loaded: true,
        source_projects_error: None,
        unregistered_projects: vec![WorkspaceProjectCandidate {
            path: root.join("unregistered-addon").display().to_string(),
            id: "fixture.addon".to_owned(),
            kind: "game-mod".to_owned(),
        }],
        unregistered_diagnostics: vec!["fixture diagnostic".to_owned()],
        unregistered_projects_loaded: true,
        unregistered_projects_error: None,
        workspace_validation_error: None,
    }
}

fn catalog_entry(entry: CatalogContent) -> CatalogEntry {
    CatalogEntry {
        id: entry.id,
        version: entry.version,
        kind: entry.kind,
        published_file_id: entry.published_file_id,
        dependencies: entry.dependencies,
        conflicts: entry
            .conflicts
            .into_iter()
            .map(|conflict| CatalogConflict {
                field: conflict.field,
                first_value: conflict.first_value,
                first_location: conflict.first_location.display().to_string(),
                conflicting_value: conflict.conflicting_value,
                conflicting_location: conflict.conflicting_location.display().to_string(),
            })
            .collect(),
        locations: entry
            .locations
            .into_iter()
            .map(|location| CatalogLocation {
                kind: match location.kind {
                    vapor_studio_core::CatalogLocationKind::Workspace => {
                        CatalogLocationKind::Workspace
                    }
                    vapor_studio_core::CatalogLocationKind::Cache => CatalogLocationKind::Cache,
                    vapor_studio_core::CatalogLocationKind::Installed => {
                        CatalogLocationKind::Installed
                    }
                },
                root: location.root.display().to_string(),
                enabled: location.enabled,
                fingerprint: location.fingerprint,
                install_source: location.install_source,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use vapor_studio_protocol::{RequestId, StudioAction, StudioRequest};

    fn session() -> StudioSession {
        StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: None,
            workspace_root: None,
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
        })
    }

    fn fixture_workspace_session() -> StudioSession {
        StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: Some(2122620),
            workspace_root: Some(PathBuf::from("/fixture/workspace")),
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
        })
    }

    fn request(revision: Option<u64>, action: StudioAction) -> StudioRequest {
        StudioRequest {
            protocol_version: vapor_studio_protocol::PROTOCOL_VERSION,
            request_id: RequestId("test-request".to_owned()),
            expected_revision: revision.map(StudioRevision),
            action,
        }
    }

    fn wait_for_operation(session: &mut StudioSession) -> Vec<StudioEvent> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            events.extend(session.poll_background_operations());
            if session.active_operation.is_none() {
                return events;
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("fixture composition operation did not complete");
    }

    #[test]
    fn fixture_catalog_refresh_is_deterministic_and_advances_revision() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Refresh),
        ));
        assert_eq!(response.kind, StudioResponseKind::Applied);
        assert_eq!(response.revision, StudioRevision(1));
        assert_eq!(response.snapshot.catalog[0].id, "fixture.engine");
        assert_eq!(response.snapshot.host.mode, "fixture");
        assert!(response.snapshot.workspace_content.source_projects_loaded);
        assert!(
            response
                .snapshot
                .workspace_content
                .unregistered_projects_loaded
        );
    }

    #[test]
    fn fixture_workshop_search_is_owned_by_the_service_and_returns_results() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Workshop(vapor_studio_protocol::WorkshopAction::Search {
                query: "moon".into(),
                page: 1,
            }),
        ));

        assert_eq!(response.kind, StudioResponseKind::OperationAccepted);
        let events = wait_for_operation(&mut session);
        assert!(events.iter().any(|event| matches!(
            event,
            StudioEvent::OperationChanged(event)
                if event.action == "workshop.search"
                    && event.status == OperationStatus::Succeeded
        )));
        let snapshot = serde_json::to_value(session.snapshot()).expect("encode snapshot");
        assert_eq!(snapshot["workshop"]["search"]["query"], "moon");
        assert_eq!(snapshot["workshop"]["search"]["page"], 1);
        assert_eq!(
            snapshot["workshop"]["search"]["items"][0]["published_file_id"],
            "1002"
        );
    }

    #[test]
    fn fixture_workshop_acquisition_is_owned_by_the_service_and_updates_its_catalog() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Workshop(WorkshopAction::Acquire {
                item_ids: vec!["1002".into(), "1002".into()],
            }),
        ));

        assert_eq!(response.kind, StudioResponseKind::OperationAccepted);
        let events = wait_for_operation(&mut session);
        assert!(events.iter().any(|event| matches!(
            event,
            StudioEvent::OperationChanged(event)
                if event.action == "workshop.acquire"
                    && event.status == OperationStatus::Succeeded
        )));
        let snapshot = session.snapshot();
        let acquisition = snapshot
            .workshop
            .acquisition
            .expect("service publishes the acquired items");
        assert_eq!(acquisition.requested, 1);
        assert_eq!(acquisition.items.len(), 1);
        assert_eq!(acquisition.items[0].id, "fixture.content.1002");
        assert!(snapshot.catalog.iter().any(|item| {
            item.id == "fixture.content.1002"
                && item.locations.iter().any(|location| {
                    location.kind == CatalogLocationKind::Cache
                        && location.root == "fixture/cache/fixture.content.1002"
                })
        }));
    }

    #[test]
    fn fixture_workshop_review_and_publish_are_service_owned_and_confirmation_bound() {
        let mut session = fixture_workspace_session();
        let staged = session.dispatch(request(
            Some(0),
            StudioAction::Workshop(WorkshopAction::Stage {
                selector: "fixture.game@1.0.0".into(),
            }),
        ));
        assert_eq!(staged.kind, StudioResponseKind::OperationAccepted);
        let stage_events = wait_for_operation(&mut session);
        assert!(stage_events.iter().any(|event| matches!(
            event,
            StudioEvent::OperationChanged(event)
                if event.action == "workshop.stage"
                    && event.status == OperationStatus::Succeeded
        )));
        let review = session
            .snapshot()
            .workshop
            .review
            .expect("stage result is retained by the service");
        assert_eq!(review.selector, "fixture.game@1.0.0");
        assert_eq!(review.preview.file_paths, ["Vapor.toml"]);

        let rejected = session.dispatch(request(
            None,
            StudioAction::Workshop(WorkshopAction::Publish {
                review_id: review.review_id.clone(),
                account: "publisher".into(),
                confirmation: "PUBLISH fixture.game".into(),
                change_note: String::new(),
            }),
        ));
        assert_eq!(rejected.kind, StudioResponseKind::Rejected);
        assert!(session.snapshot().workshop.review.is_some());

        let published = session.dispatch(request(
            None,
            StudioAction::Workshop(WorkshopAction::Publish {
                review_id: review.review_id,
                account: "publisher".into(),
                confirmation: "PUBLISH fixture.game TO publisher".into(),
                change_note: "Fixture only".into(),
            }),
        ));
        assert_eq!(published.kind, StudioResponseKind::OperationAccepted);
        let publish_events = wait_for_operation(&mut session);
        assert!(publish_events.iter().any(|event| matches!(
            event,
            StudioEvent::OperationChanged(event)
                if event.action == "workshop.publish"
                    && event.status == OperationStatus::Succeeded
        )));
        assert!(session.snapshot().workshop.review.is_none());
    }

    #[test]
    fn workshop_stage_requires_a_current_workspace_identity() {
        let mut session = session();
        let response = session.dispatch(request(
            None,
            StudioAction::Workshop(WorkshopAction::Stage {
                selector: "fixture.game@1.0.0".into(),
            }),
        ));
        assert_eq!(response.kind, StudioResponseKind::Rejected);
        assert_eq!(
            response.error.expect("workspace rejection").code,
            StudioErrorCode::ActionUnavailable
        );
    }

    #[test]
    fn fixture_workshop_subscription_changes_return_the_service_owned_list() {
        let mut session = session();
        let loaded = session.dispatch(request(
            None,
            StudioAction::Workshop(WorkshopAction::LoadSubscriptions),
        ));
        assert_eq!(loaded.kind, StudioResponseKind::OperationAccepted);
        wait_for_operation(&mut session);
        assert!(session.snapshot().workshop.subscriptions_loaded);
        assert_eq!(
            session.snapshot().workshop.subscriptions[0].published_file_id,
            "1001"
        );

        let subscribed = session.dispatch(request(
            None,
            StudioAction::Workshop(WorkshopAction::Subscribe {
                item_ids: vec!["1002".into(), "1002".into()],
            }),
        ));
        assert_eq!(subscribed.kind, StudioResponseKind::OperationAccepted);
        wait_for_operation(&mut session);
        let ids = session
            .snapshot()
            .workshop
            .subscriptions
            .iter()
            .map(|item| item.published_file_id.clone())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["1001", "1002"]);

        let invalid = session.dispatch(request(
            None,
            StudioAction::Workshop(WorkshopAction::Subscribe {
                item_ids: vec!["not-an-id".into()],
            }),
        ));
        assert_eq!(invalid.kind, StudioResponseKind::Rejected);
        assert_eq!(
            invalid.error.expect("invalid ID rejection").code,
            StudioErrorCode::InvalidRequest
        );
    }

    #[test]
    fn fixture_preview_is_profile_scoped_and_returns_a_typed_plan() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Preview {
                profile_id: "default".into(),
                selector: Some("fixture.pack@1.0.0".into()),
            }),
        ));
        assert_eq!(response.kind, StudioResponseKind::OperationAccepted);
        assert_eq!(
            response
                .snapshot
                .active_operation
                .as_ref()
                .unwrap()
                .profile_id
                .as_deref(),
            Some("default")
        );

        let events = wait_for_operation(&mut session);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    StudioEvent::OperationChanged(OperationEvent {
                        status: OperationStatus::Succeeded,
                        ..
                    })
                ))
                .count(),
            1,
            "profile persistence must contribute exactly one terminal success event"
        );
        let preview = session.snapshot().composition_preview.unwrap();
        assert_eq!(preview.profile_id, "default");
        assert_eq!(preview.selector, "fixture.pack@1.0.0");
        assert_eq!(preview.nodes[0].kind, "packagepack");
        assert!(preview.warnings[0].contains("Fixture mode"));
    }

    #[test]
    fn fixture_install_updates_service_profile_without_launching_a_process() {
        let mut session = session();
        let root = ContentIdentity {
            id: "fixture.pack".into(),
            version: Some("1.0.0".into()),
        };
        let assigned = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::SetRoot {
                profile_id: "default".into(),
                content_id: Some(root.id.clone()),
                content_version: root.version.clone(),
                workspace_root: Some("/fixture/workspace".into()),
            }),
        ));
        assert_eq!(assigned.kind, StudioResponseKind::Applied);

        let accepted = session.dispatch(request(
            Some(1),
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Install {
                profile_id: "default".into(),
            }),
        ));
        assert_eq!(accepted.kind, StudioResponseKind::OperationAccepted);
        let events = wait_for_operation(&mut session);
        assert!(events.iter().any(|event| matches!(
            event,
            StudioEvent::OperationChanged(OperationEvent {
                status: OperationStatus::Succeeded,
                ..
            })
        )));
        let profile = &session.snapshot().durable_state.profiles.profiles[0];
        assert_eq!(profile.resolved_graph.as_ref().unwrap().root, root);
        assert!(matches!(
            &profile.installation,
            InstallationState::Installed { .. }
        ));
    }

    #[test]
    fn verification_failure_has_one_terminal_event_after_profile_reconciliation() {
        let mut session = session();
        session.active_operation = Some(OperationSummary {
            id: "operation-verify".to_owned(),
            profile_id: Some("default".to_owned()),
            action: "composition.verify".to_owned(),
            phase: OperationPhase::Verifying,
            status: OperationStatus::Running,
            progress: None,
        });
        session.apply_composition_completion(CompositionOperationCompletion {
            id: "operation-verify".to_owned(),
            profile_id: "default".to_owned(),
            action: CompositionOperationKind::Verify,
            result: CompositionOperationResult::Verify(Ok(vapor_studio_core::InstallationCheck {
                ok: false,
                detail: vec!["packagepack payload is missing".to_owned()],
                artifacts: Vec::new(),
                graph: None,
            })),
        });

        assert!(session.active_operation.is_none());
        assert!(matches!(
            &session.durable_state.profiles.profiles[0].installation,
            InstallationState::NeedsRepair { .. }
        ));
        let terminal_events = session
            .operation_events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    StudioEvent::OperationChanged(OperationEvent {
                        id,
                        status: OperationStatus::Failed { .. },
                        ..
                    }) if id == "operation-verify"
                )
            })
            .count();
        assert_eq!(terminal_events, 1);
    }

    #[test]
    fn workspace_project_discovery_is_a_service_action() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: None,
            workspace_root: Some(PathBuf::from("/fixture/workspace")),
            active_profile_id: None,
            fixture_mode: true,
        });
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::DiscoverContentProjects),
        ));

        assert_eq!(response.kind, StudioResponseKind::Applied);
        assert_eq!(response.revision, StudioRevision(1));
        assert_eq!(
            response.snapshot.workspace_content.source_projects[0].id,
            "fixture.game"
        );
        assert_eq!(
            response.snapshot.workspace_content.unregistered_projects[0].id,
            "fixture.addon"
        );
    }

    #[test]
    fn workspace_discovery_is_unavailable_until_a_workspace_is_open() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::DiscoverContentProjects),
        ));

        assert_eq!(response.kind, StudioResponseKind::Rejected);
        assert_eq!(response.revision, StudioRevision(0));
        assert_eq!(
            response.error.unwrap().code,
            StudioErrorCode::ActionUnavailable
        );
        let action = response
            .snapshot
            .available_actions
            .into_iter()
            .find(|action| action.id == "catalog.discover-content-projects")
            .expect("availability projection contains workspace discovery");
        assert!(!action.enabled);
        assert_eq!(action.reason.as_deref(), Some("Open a workspace first."));
    }

    #[test]
    fn workspace_open_returns_source_and_unregistered_projects_in_its_snapshot() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: None,
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: true,
        });
        let root = std::env::current_dir().expect("current directory");
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open {
                root: root.display().to_string(),
            }),
        ));

        assert_eq!(response.kind, StudioResponseKind::Applied);
        let content = response.snapshot.workspace_content;
        assert!(content.source_projects_loaded);
        assert_eq!(content.source_projects[0].id, "fixture.game");
        assert!(content.unregistered_projects_loaded);
        assert_eq!(content.unregistered_projects[0].id, "fixture.addon");
        assert_eq!(content.unregistered_diagnostics, ["fixture diagnostic"]);
    }

    #[test]
    fn workspace_file_lifecycle_runs_through_the_service_and_guards_external_edits() {
        let root = std::env::temp_dir().join(format!(
            "vapor-studio-service-workspace-files-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock follows epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create fixture workspace");
        std::fs::write(root.join("Vapor.toml"), "before").expect("write fixture manifest");
        let root = std::fs::canonicalize(root).expect("canonicalize fixture workspace");
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: None,
            workspace_root: Some(root.clone()),
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
        });

        let response = session.dispatch(request(
            None,
            StudioAction::Workspace(WorkspaceAction::ListFiles { path: ".".into() }),
        ));
        assert_eq!(response.kind, StudioResponseKind::OperationAccepted);
        let events = wait_for_operation(&mut session);
        assert!(events.iter().any(|event| matches!(
            event,
            StudioEvent::WorkspaceFileResult(result)
                if matches!(result.as_ref(), WorkspaceFileResult::Listed { files, .. }
                    if files == &["Vapor.toml"])
        )));

        let response = session.dispatch(request(
            None,
            StudioAction::Workspace(WorkspaceAction::OpenFile {
                path: "Vapor.toml".into(),
            }),
        ));
        assert_eq!(response.kind, StudioResponseKind::OperationAccepted);
        let events = wait_for_operation(&mut session);
        let fingerprint = events
            .iter()
            .find_map(|event| match event {
                StudioEvent::WorkspaceFileResult(result) => match result.as_ref() {
                    WorkspaceFileResult::Opened {
                        contents,
                        fingerprint,
                        ..
                    } if contents == "before" => Some(fingerprint.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("open returns text and its save fingerprint");

        std::fs::write(root.join("Vapor.toml"), "external").expect("simulate external edit");
        let response = session.dispatch(request(
            None,
            StudioAction::Workspace(WorkspaceAction::SaveFile {
                path: "Vapor.toml".into(),
                expected_fingerprint: fingerprint,
                contents: "mine".into(),
            }),
        ));
        assert_eq!(response.kind, StudioResponseKind::OperationAccepted);
        let events = wait_for_operation(&mut session);
        assert!(events.iter().any(|event| matches!(
            event,
            StudioEvent::OperationChanged(event)
                if event.action == "workspace.save-file"
                    && matches!(&event.status, OperationStatus::Failed { message }
                        if message.contains("changed on disk"))
        )));
        assert_eq!(
            std::fs::read_to_string(root.join("Vapor.toml")).expect("read after stale save"),
            "external"
        );

        let response = session.dispatch(request(
            None,
            StudioAction::Workspace(WorkspaceAction::CreateFile {
                path: "nested/new.toml".into(),
                contents: String::new(),
            }),
        ));
        assert_eq!(response.kind, StudioResponseKind::OperationAccepted);
        let events = wait_for_operation(&mut session);
        assert!(events.iter().any(|event| matches!(
            event,
            StudioEvent::WorkspaceFileResult(result)
                if matches!(result.as_ref(), WorkspaceFileResult::Created { path, .. }
                    if path == "nested/new.toml")
        )));
        assert!(root.join("nested/new.toml").is_file());
        std::fs::remove_dir_all(root).expect("remove fixture workspace");
    }

    #[test]
    fn switching_workspace_clears_the_old_composition_in_service_state() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: None,
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: true,
        });
        let assigned = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::SetRoot {
                profile_id: "default".to_owned(),
                content_id: Some("old.game".to_owned()),
                content_version: Some("1.0".to_owned()),
                workspace_root: None,
            }),
        ));
        assert_eq!(assigned.kind, StudioResponseKind::Applied);

        let root = std::env::current_dir().expect("current directory");
        let opened = session.dispatch(request(
            Some(1),
            StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open {
                root: root.display().to_string(),
            }),
        ));
        assert_eq!(opened.kind, StudioResponseKind::Applied);
        let profile = &opened.snapshot.durable_state.profiles.profiles[0];
        assert_eq!(
            profile.workspace.as_ref().map(|value| &value.root),
            Some(&root)
        );
        assert_eq!(profile.root_package, None);
        assert_eq!(profile.resolved_graph, None);
        assert_eq!(profile.installation, InstallationState::Unknown);
    }

    #[test]
    fn catalog_search_filters_snapshot_without_destroying_catalog() {
        let mut session = session();
        let _ = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Refresh),
        ));
        let filtered = session.dispatch(request(
            Some(1),
            StudioAction::Catalog(CatalogAction::Search {
                query: "missing".into(),
            }),
        ));
        assert!(filtered.snapshot.catalog.is_empty());
        let restored = session.dispatch(request(
            Some(2),
            StudioAction::Catalog(CatalogAction::Search {
                query: String::new(),
            }),
        ));
        assert_eq!(restored.snapshot.catalog.len(), 1);
    }

    #[test]
    fn stale_action_is_rejected_without_mutating_state() {
        let mut session = session();
        let _ = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Refresh),
        ));
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Inspect {
                content_id: "fixture.engine".to_owned(),
                version: Some("1.0.0".to_owned()),
            }),
        ));
        assert_eq!(response.kind, StudioResponseKind::Rejected);
        assert_eq!(response.error.unwrap().code, StudioErrorCode::StaleRevision);
        assert_eq!(response.revision, StudioRevision(1));
        assert_eq!(response.snapshot.selected_content_id, None);
    }

    #[test]
    fn profile_selection_requires_an_existing_profile() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: None,
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: false,
        });
        let created = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::Create {
                name: "Second".to_owned(),
            }),
        ));
        assert_eq!(created.kind, StudioResponseKind::Applied);
        assert_eq!(created.revision, StudioRevision(1));
        assert_eq!(
            created.snapshot.active_profile_id.as_deref(),
            Some("profile-1")
        );
        assert_eq!(created.snapshot.durable_state.profiles.profiles.len(), 2);
        assert_eq!(created.snapshot.durable_state.profile_revision, 1);
        let rejected = session.dispatch(request(
            Some(1),
            StudioAction::Profile(ProfileAction::Select {
                profile_id: "unknown".to_owned(),
            }),
        ));
        assert_eq!(rejected.kind, StudioResponseKind::Rejected);
        assert_eq!(rejected.revision, StudioRevision(1));
    }

    #[test]
    fn fixture_default_composition_root_is_resolved_and_persisted_by_service() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::ResolveDefaultRoot {
                profile_id: "default".to_owned(),
            }),
        ));

        assert_eq!(response.kind, StudioResponseKind::Applied);
        let profile = &response.snapshot.durable_state.profiles.profiles[0];
        assert_eq!(
            profile.root_package,
            Some(ContentIdentity {
                id: "fixture.pack".to_owned(),
                version: None,
            })
        );
        assert_eq!(profile.installation, InstallationState::Unknown);
    }

    #[test]
    fn profile_launch_arguments_are_validated_and_profile_revision_is_projected() {
        let mut session = session();
        let update = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::SetLaunchArguments {
                profile_id: "default".to_owned(),
                arguments: vec!["--windowed".to_owned()],
            }),
        ));
        assert_eq!(update.kind, StudioResponseKind::Applied);
        assert_eq!(update.snapshot.durable_state.profile_revision, 1);
        assert_eq!(
            update.snapshot.durable_state.profiles.profiles[0]
                .launch
                .arguments,
            ["--windowed"]
        );

        let rejected = session.dispatch(request(
            Some(1),
            StudioAction::Profile(ProfileAction::SetLaunchArguments {
                profile_id: "default".to_owned(),
                arguments: vec!["x".repeat(4097)],
            }),
        ));
        assert_eq!(rejected.kind, StudioResponseKind::Rejected);
        assert_eq!(rejected.snapshot.durable_state.profile_revision, 1);
        assert_eq!(
            rejected.snapshot.durable_state.profiles.profiles[0]
                .launch
                .arguments,
            ["--windowed"]
        );
    }

    #[test]
    fn content_invalidation_clears_stale_graph_and_installation_without_removing_root() {
        let mut session = session();
        let root = ContentIdentity {
            id: "fixture.game".to_owned(),
            version: Some("1.0.0".to_owned()),
        };
        let assigned = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::SetRoot {
                profile_id: "default".to_owned(),
                content_id: Some(root.id.clone()),
                content_version: root.version.clone(),
                workspace_root: None,
            }),
        ));
        assert_eq!(assigned.kind, StudioResponseKind::Applied);

        let invalidated = session.dispatch(request(
            Some(1),
            StudioAction::Profile(ProfileAction::InvalidateContent {
                content_id: root.id.clone(),
                content_version: root.version.clone(),
            }),
        ));
        assert_eq!(invalidated.kind, StudioResponseKind::Applied);
        let profile = &invalidated.snapshot.durable_state.profiles.profiles[0];
        assert_eq!(profile.root_package, Some(root));
        assert_eq!(profile.resolved_graph, None);
        assert_eq!(profile.installation, InstallationState::Unknown);
    }
}
