#[path = "components/mod.rs"]
mod components;
#[path = "operation_center.rs"]
mod operation_center;
#[path = "pages/mod.rs"]
mod pages;

use crate::navigation::{CatalogSourceFilter, Section};
use eframe::egui;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use vapor_core::{
    ContentKind, DevelopmentOperation, OperationEvent as CoreOperationEvent,
    OperationRequest as CoreOperationRequest, OperationService, VaporWorkspace,
};
use vapor_studio_core::{
    CURRENT_SCHEMA_VERSION, CompositionProfile, ContentIdentity, InstalledArtifactRecord,
    OperationEvent, OperationId, OperationKind, OperationPhase, OperationRequest, OperationStatus,
    ProfileDocument, WorkspaceAssociation, decode_profiles,
};

enum OperationResult {
    Workspace(Result<VaporWorkspace, String>),
    WorkspaceSelection(Result<WorkspaceSelectionData, String>),
    RememberedSources(Result<Vec<PathBuf>, String>),
    AdoptedContentProject(Result<(String, Vec<vapor_studio_core::SourceContent>), String>),
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
    Install(Result<vapor_studio_core::InstalledComposition, String>),
    Update(Result<vapor_studio_core::InstalledComposition, String>),
    RunProfile(Result<vapor_studio_core::InstalledComposition, String>),
    Verify(Result<vapor_studio_core::InstallationCheck, String>),
    Repair(Result<vapor_studio_core::InstalledComposition, String>),
    WorkshopStage(Result<vapor_studio_core::WorkshopPackagePreview, String>),
    WorkshopAcquisition(Result<vapor_studio_core::WorkshopAcquisition, String>),
    WorkshopPublish(Result<vapor_studio_core::WorkshopPublication, String>),
    WorkshopSearch(Result<vapor_studio_core::WorkshopPage, String>),
    WorkshopSubscriptions(Result<Vec<vapor_studio_core::WorkshopSubscription>, String>),
    WorkshopSubscriptionChange(Result<(), String>, String, bool),
}

type WorkspaceSelectionData = (
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
        | OperationResult::Install(Err(error))
        | OperationResult::Update(Err(error))
        | OperationResult::RunProfile(Err(error))
        | OperationResult::Verify(Err(error))
        | OperationResult::Repair(Err(error))
        | OperationResult::WorkshopStage(Err(error))
        | OperationResult::WorkshopAcquisition(Err(error))
        | OperationResult::WorkshopPublish(Err(error))
        | OperationResult::WorkshopSearch(Err(error))
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

pub struct StudioApp {
    section: Section,
    workspace_root: String,
    workspace_root_draft: String,
    fallback_workspace_root: PathBuf,
    profiles: Vec<CompositionProfile>,
    profile_recovery: Option<String>,
    profile_recovery_backup: Option<String>,
    active_profile: usize,
    new_profile_name: String,
    search_query: String,
    draft_content_id: String,
    draft_content_version: String,
    draft_content_kind: ContentKind,
    draft_dependencies: Vec<String>,
    draft_workshop_app_id: String,
    draft_workshop_published_file_id: String,
    draft_workshop_visibility: String,
    draft_workshop_title: String,
    draft_workshop_description: String,
    draft_workshop_tags: String,
    draft_workshop_change_note: String,
    composition_edit_selector: String,
    composition_edit: Option<vapor_studio_core::SourceComposition>,
    composition_original_links: Option<Vec<vapor_studio_core::SourceCompositionLink>>,
    composition_link_candidate: String,
    workshop_metadata_edit: Option<vapor_studio_core::SourceWorkshopMetadata>,
    workshop_metadata_dirty: bool,
    workshop_metadata_app_id: String,
    workshop_metadata_published_file_id: String,
    workshop_metadata_visibility: String,
    workshop_metadata_title: String,
    workshop_metadata_description: String,
    workshop_metadata_tags: String,
    workshop_metadata_change_note: String,
    repreview_after_catalog_refresh: bool,
    starter_workspace_path: String,
    starter_workspace_organization: String,
    starter_workspace_name: String,
    new_workspace_file_path: String,
    unregistered_projects: vapor_studio_core::UnregisteredProjectScan,
    registered_sources: Vec<PathBuf>,
    active_source: Option<PathBuf>,
    contents: Vec<vapor_studio_core::SourceContent>,
    catalog: Vec<vapor_studio_core::CatalogContent>,
    catalog_source_filter: CatalogSourceFilter,
    catalog_kind_filter: Option<String>,
    selected_catalog_content: Option<String>,
    workshop_preview: Option<vapor_studio_core::WorkshopPackagePreview>,
    workshop_id: String,
    workshop_account: String,
    workshop_change_note: String,
    workshop_confirmation: String,
    pending_workshop: Option<vapor_studio_core::WorkshopAcquisition>,
    workshop_search_text: String,
    workshop_results: Vec<vapor_studio_core::WorkshopItem>,
    workshop_subscriptions: std::collections::BTreeSet<String>,
    workshop_subscription_details:
        std::collections::BTreeMap<String, vapor_studio_core::WorkshopSubscription>,
    workshop_result_total: u32,
    workshop_page: u32,
    composition_preview: Option<vapor_studio_core::CompositionPreview>,
    workspace_files: Vec<PathBuf>,
    selected_workspace_file: Option<PathBuf>,
    workspace_editor: String,
    workspace_editor_original: String,
    workspace_editor_toml_diagnostic: Option<Result<(), String>>,
    workspace_editor_dirty: bool,
    status: String,
    running: bool,
    receiver: Option<Receiver<(OperationId, OperationResult)>>,
    latest_operation_event: Arc<Mutex<Option<CoreOperationEvent>>>,
    active_operation: Option<OperationEvent>,
    operation_history: VecDeque<OperationEvent>,
    diagnostics: vapor_devtools::app::DevtoolsApp,
}

impl StudioApp {
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        workspace: Option<PathBuf>,
        play_mode: bool,
    ) -> Self {
        let saved_workspace = creation
            .storage
            .and_then(|storage| storage.get_string("workspace_root"));
        let root = workspace
            .or_else(|| saved_workspace.map(PathBuf::from))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let saved_profiles = creation
            .storage
            .and_then(|storage| storage.get_string("profiles"));
        let (profile_document, profile_recovery) = decode_profiles(saved_profiles.as_deref());
        let profiles = profile_document.profiles;
        let saved_profile_id = creation
            .storage
            .and_then(|storage| storage.get_string("active_profile_id"));
        let active_profile = saved_profile_id
            .as_deref()
            .and_then(|id| {
                profiles
                    .iter()
                    .position(|profile| profile.id.as_str() == id)
            })
            .or_else(|| {
                creation
                    .storage
                    .and_then(|storage| storage.get_string("active_profile"))
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|index| *index < profiles.len())
            })
            .unwrap_or(0);
        let workspace_root = profiles[active_profile]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.display().to_string())
            .unwrap_or_else(|| root.display().to_string());
        let workspace_root_draft = workspace_root.clone();
        let mut app = Self {
            section: if play_mode {
                Section::Library
            } else {
                Section::Home
            },
            workspace_root,
            workspace_root_draft,
            fallback_workspace_root: root,
            profiles,
            profile_recovery: profile_recovery
                .as_ref()
                .map(|recovery| recovery.message.clone()),
            profile_recovery_backup: if profile_recovery.is_some() {
                saved_profiles
            } else {
                None
            },
            active_profile,
            new_profile_name: String::new(),
            search_query: String::new(),
            contents: Vec::new(),
            catalog: Vec::new(),
            catalog_source_filter: CatalogSourceFilter::All,
            catalog_kind_filter: None,
            selected_catalog_content: None,
            workshop_preview: None,
            workshop_id: String::new(),
            workshop_account: String::new(),
            workshop_change_note: String::new(),
            workshop_confirmation: String::new(),
            pending_workshop: None,
            workshop_search_text: String::new(),
            workshop_results: Vec::new(),
            workshop_subscriptions: std::collections::BTreeSet::new(),
            workshop_subscription_details: std::collections::BTreeMap::new(),
            workshop_result_total: 0,
            workshop_page: 1,
            composition_preview: None,
            composition_edit_selector: String::new(),
            composition_edit: None,
            composition_original_links: None,
            composition_link_candidate: String::new(),
            workshop_metadata_edit: None,
            workshop_metadata_dirty: false,
            workshop_metadata_app_id: String::new(),
            workshop_metadata_published_file_id: String::new(),
            workshop_metadata_visibility: String::new(),
            workshop_metadata_title: String::new(),
            workshop_metadata_description: String::new(),
            workshop_metadata_tags: String::new(),
            workshop_metadata_change_note: String::new(),
            repreview_after_catalog_refresh: false,
            workspace_files: Vec::new(),
            selected_workspace_file: None,
            workspace_editor: String::new(),
            workspace_editor_original: String::new(),
            workspace_editor_toml_diagnostic: None,
            workspace_editor_dirty: false,
            draft_content_id: String::new(),
            draft_content_version: "0.1.0".to_owned(),
            draft_content_kind: ContentKind::Packagepack,
            draft_dependencies: Vec::new(),
            draft_workshop_app_id: String::new(),
            draft_workshop_published_file_id: String::new(),
            draft_workshop_visibility: String::new(),
            draft_workshop_title: String::new(),
            draft_workshop_description: String::new(),
            draft_workshop_tags: String::new(),
            draft_workshop_change_note: String::new(),
            starter_workspace_path: String::new(),
            starter_workspace_organization: "my-workshop".to_owned(),
            starter_workspace_name: "my-game".to_owned(),
            new_workspace_file_path: String::new(),
            unregistered_projects: vapor_studio_core::UnregisteredProjectScan::default(),
            registered_sources: Vec::new(),
            active_source: None,
            status: "Choose a workspace or discover content in this folder.".to_owned(),
            running: false,
            receiver: None,
            latest_operation_event: Arc::new(Mutex::new(None)),
            active_operation: None,
            operation_history: VecDeque::new(),
            diagnostics: vapor_devtools::connect(None),
        };
        app.refresh_registered_sources();
        if play_mode {
            app.launch_default_profile();
        } else {
            app.discover();
        }
        app
    }

    fn poll_operation(&mut self) {
        let latest_event = self
            .latest_operation_event
            .lock()
            .ok()
            .and_then(|mut event| event.take());
        if let (Some(event), Some(active)) = (latest_event.as_ref(), self.active_operation.as_mut())
        {
            apply_core_operation_event(event, active);
        }
        let received = self.receiver.as_ref().map(|receiver| receiver.try_recv());
        let result = match received {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => {
                self.active_operation.as_ref().map(|operation| {
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
            self.receiver = None;
            self.running = false;
            let refresh_after_content_change = matches!(
                &result,
                OperationResult::WorkshopPublish(Ok(_))
                    | OperationResult::WorkshopAcquisition(Ok(_))
                    | OperationResult::ContentWorkspace(Ok(_))
                    | OperationResult::AdoptedContentProject(Ok(_))
                    | OperationResult::SourceCompositionSaved(Ok(_))
                    | OperationResult::SourceWorkshopSaved(Ok(_))
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
            match result {
                OperationResult::Workspace(Ok(workspace)) => {
                    self.status = format!(
                        "Completed for {}/{} · {} projects",
                        workspace.manifest.workspace.organization,
                        workspace.manifest.workspace.name,
                        workspace.projects.len()
                    )
                }
                OperationResult::Workspace(Err(error)) => self.status = error,
                OperationResult::WorkspaceSelection(Err(error)) => self.status = error,
                OperationResult::UnregisteredProjectScan(Err(error)) => self.status = error,
                OperationResult::RememberedSources(Err(error)) => self.status = error,
                OperationResult::RememberedSources(Ok(known)) => {
                    self.registered_sources = known;
                    self.status = "Added the open workspace to this Vapor installation. It remains separate from the active source.".to_owned();
                }
                OperationResult::AdoptedContentProject(Ok((summary, contents))) => {
                    self.contents = contents;
                    self.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
                    self.status = summary;
                }
                OperationResult::UnregisteredProjectScan(Ok(scan)) => {
                    self.status = format!(
                        "Found {} adoptable content project{} and {} diagnostic{}.",
                        scan.candidates.len(),
                        if scan.candidates.len() == 1 { "" } else { "s" },
                        scan.diagnostics.len(),
                        if scan.diagnostics.len() == 1 { "" } else { "s" },
                    );
                    self.unregistered_projects = scan;
                }
                OperationResult::WorkspaceSelection(Ok((root, contents, catalog, files))) => {
                    self.workshop_metadata_edit = None;
                    self.workshop_metadata_dirty = false;
                    self.activate_workspace(root, contents, catalog, files)
                }
                OperationResult::Discovery(source_result, catalog_result) => {
                    let source_error = match source_result {
                        Ok(contents) => {
                            self.contents = contents;
                            None
                        }
                        Err(error) => {
                            self.contents.clear();
                            Some(error)
                        }
                    };
                    match catalog_result {
                        Ok(catalog) => {
                            let count = catalog.len();
                            self.catalog = catalog;
                            self.status = if self.profiles[self.active_profile].workspace.is_some()
                            {
                                source_error.map(|error| format!("Found {count} local items, but workspace sources could not load: {error}"))
                                    .unwrap_or_else(|| format!("Discovered {count} local content identities across workspace, cache, and installation."))
                            } else {
                                format!(
                                    "Discovered {count} cached and installed content identities."
                                )
                            };
                        }
                        Err(error) => {
                            self.catalog.clear();
                            self.selected_catalog_content = None;
                            self.status = source_error.map_or_else(
                                || format!("Could not refresh the local content catalog: {error}"),
                                |source_error| format!("Workspace sources failed: {source_error}. Local catalog failed: {error}"),
                            );
                        }
                    }
                    if self.repreview_after_catalog_refresh {
                        self.repreview_after_catalog_refresh = false;
                        self.preview_selected_composition();
                    }
                    if self
                        .selected_catalog_content
                        .as_ref()
                        .is_some_and(|identity| {
                            !self
                                .catalog
                                .iter()
                                .any(|content| catalog_content_selector(content) == *identity)
                        })
                    {
                        self.selected_catalog_content = None;
                    }
                }
                OperationResult::Message(Err(error))
                | OperationResult::CompositionPreview(Err(error))
                | OperationResult::WorkspaceFiles(Err(error))
                | OperationResult::WorkspaceFileCreated(Err(error))
                | OperationResult::FileContents(Err(error))
                | OperationResult::FileSaved(Err(error))
                | OperationResult::ContentWorkspace(Err(error))
                | OperationResult::SourceCompositionLoaded(Err(error))
                | OperationResult::SourceCompositionSaved(Err(error))
                | OperationResult::SourceWorkshopLoaded(Err(error))
                | OperationResult::SourceWorkshopSaved(Err(error))
                | OperationResult::AdoptedContentProject(Err(error)) => self.status = error,
                OperationResult::Verify(Err(error)) => {
                    let artifacts = match self.profiles[self.active_profile].installation.clone() {
                        vapor_studio_core::InstallationState::Installed { artifacts, .. }
                        | vapor_studio_core::InstallationState::NeedsRepair { artifacts, .. } => {
                            artifacts
                        }
                        _ => Vec::new(),
                    };
                    self.profiles[self.active_profile].installation =
                        vapor_studio_core::InstallationState::NeedsRepair {
                            detail: error.clone(),
                            artifacts,
                        };
                    self.status = format!("Could not verify the full composition: {error}");
                }
                OperationResult::Install(Err(error))
                | OperationResult::Update(Err(error))
                | OperationResult::RunProfile(Err(error))
                | OperationResult::Repair(Err(error)) => {
                    self.profiles[self.active_profile].installation =
                        vapor_studio_core::InstallationState::Partial {
                            detail: error.clone(),
                        };
                    self.status = error;
                }
                OperationResult::Message(Ok(message)) => self.status = message,
                OperationResult::CompositionPreview(Ok(preview)) => {
                    self.status = if preview.ready() {
                        format!(
                            "Resolved {} content nodes. This composition is ready to install.",
                            preview.nodes.len()
                        )
                    } else {
                        format!(
                            "Resolved {} available nodes with {} blocker(s). Review the plan before installing.",
                            preview.nodes.len(),
                            preview.blockers.len()
                        )
                    };
                    self.composition_preview = Some(preview);
                }
                OperationResult::WorkshopStage(Err(error))
                | OperationResult::WorkshopAcquisition(Err(error))
                | OperationResult::WorkshopPublish(Err(error))
                | OperationResult::WorkshopSearch(Err(error))
                | OperationResult::WorkshopSubscriptions(Err(error)) => self.status = error,
                OperationResult::WorkshopAcquisition(Ok(acquisition)) => {
                    self.status = format!(
                        "Downloaded {} ({}) into Vapor's cache.",
                        acquisition.id, acquisition.kind
                    );
                    self.pending_workshop = Some(acquisition);
                }
                OperationResult::WorkshopPublish(Ok(publication)) => {
                    self.status = format!(
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
                        publication.receipt.display(),
                    );
                    self.workshop_preview = None;
                    self.workshop_confirmation.clear();
                }
                OperationResult::WorkshopStage(Ok(preview)) => {
                    self.status = format!(
                        "Staged {} · {} files · {} bytes.",
                        preview.id, preview.files, preview.bytes
                    );
                    self.workshop_preview = Some(preview);
                }
                OperationResult::WorkshopSearch(Ok(page)) => {
                    self.workshop_page = page.page;
                    self.workshop_result_total = page.total_results;
                    self.workshop_results = page.items;
                    self.status = format!(
                        "Steam Workshop returned {} items on page {} of {}.",
                        self.workshop_results.len(),
                        self.workshop_page,
                        self.workshop_result_total
                    );
                }
                OperationResult::WorkshopSubscriptions(Ok(subscriptions)) => {
                    self.workshop_subscriptions = subscriptions
                        .iter()
                        .map(|item| item.published_file_id.clone())
                        .collect();
                    self.workshop_subscription_details = subscriptions
                        .into_iter()
                        .map(|item| (item.published_file_id.clone(), item))
                        .collect();
                    self.status = format!(
                        "Loaded {} Steam Workshop subscriptions.",
                        self.workshop_subscriptions.len()
                    );
                }
                OperationResult::WorkshopSubscriptionChange(Ok(()), id, subscribed) => {
                    if subscribed {
                        self.workshop_subscriptions.insert(id.clone());
                    } else {
                        self.workshop_subscriptions.remove(&id);
                        self.workshop_subscription_details.remove(&id);
                    }
                    self.status = format!(
                        "{} Steam Workshop item {id}.",
                        if subscribed {
                            "Subscribed to"
                        } else {
                            "Unsubscribed from"
                        }
                    );
                }
                OperationResult::WorkshopSubscriptionChange(Err(error), _, _) => {
                    self.status = error
                }
                OperationResult::WorkspaceFiles(Ok(files)) => {
                    self.workspace_files = files;
                    self.status = format!("Found {} workspace files.", self.workspace_files.len());
                }
                OperationResult::WorkspaceFileCreated(Ok(path)) => {
                    if !self.workspace_files.contains(&path) {
                        self.workspace_files.push(path.clone());
                        self.workspace_files.sort();
                    }
                    self.selected_workspace_file = Some(path.clone());
                    self.workspace_editor.clear();
                    self.workspace_editor_original.clear();
                    self.refresh_workspace_editor_toml_diagnostic();
                    self.workspace_editor_dirty = false;
                    self.new_workspace_file_path.clear();
                    self.status = format!("Created {}.", path.display());
                }
                OperationResult::FileContents(Ok((path, contents))) => {
                    self.selected_workspace_file = Some(path);
                    self.workspace_editor = contents.clone();
                    self.workspace_editor_original = contents;
                    self.refresh_workspace_editor_toml_diagnostic();
                    self.workspace_editor_dirty = false;
                    self.status = "Opened workspace file.".to_owned();
                }
                OperationResult::FileSaved(Ok(path)) => {
                    if self.selected_workspace_file.as_ref() == Some(&path) {
                        self.workspace_editor_original
                            .clone_from(&self.workspace_editor);
                        self.refresh_workspace_editor_toml_diagnostic();
                        self.workspace_editor_dirty = false;
                    }
                    self.status = format!("Saved {}.", path.display());
                }
                OperationResult::ContentWorkspace(Ok((manifest, contents, files))) => {
                    self.workspace_files = files;
                    self.selected_workspace_file = Some(manifest);
                    self.workspace_editor = contents.clone();
                    self.workspace_editor_original = contents;
                    self.refresh_workspace_editor_toml_diagnostic();
                    self.workspace_editor_dirty = false;
                    self.section = Section::Workspace;
                    self.status = "Created content and opened its manifest.".to_owned();
                }
                OperationResult::SourceCompositionLoaded(Ok(composition)) => {
                    self.composition_edit_selector = composition.id.clone();
                    self.composition_original_links = Some(composition.links.clone());
                    self.composition_edit = Some(composition);
                    self.status = "Loaded the source Packagepack graph. Review its edges before saving.".to_owned();
                }
                OperationResult::SourceCompositionSaved(Ok(composition)) => {
                    let selected_root = self.profiles[self.active_profile].root_package.as_ref()
                        .is_some_and(|root| root.id == composition.id);
                    self.composition_original_links = Some(composition.links.clone());
                    self.composition_edit = Some(composition);
                    if selected_root {
                        self.profiles[self.active_profile].resolved_graph = None;
                        self.profiles[self.active_profile].installation = vapor_studio_core::InstallationState::NotInstalled;
                        self.repreview_after_catalog_refresh = true;
                    }
                    self.status = "Saved the Packagepack graph. Studio is refreshing its catalog and composition plan.".to_owned();
                }
                OperationResult::SourceWorkshopLoaded(Ok(metadata)) => {
                    self.set_workshop_metadata_fields(&metadata.policy);
                    self.workshop_metadata_edit = Some(metadata);
                    self.workshop_metadata_dirty = false;
                    self.status = "Loaded the source Workshop metadata. Edit the fields, then save through Vapor Shell.".to_owned();
                }
                OperationResult::SourceWorkshopSaved(Ok(metadata)) => {
                    self.set_workshop_metadata_fields(&metadata.policy);
                    self.workshop_metadata_edit = Some(metadata.clone());
                    self.workshop_metadata_dirty = false;
                    if let Ok(relative) = metadata.manifest.strip_prefix(Path::new(self.workspace_root.trim())) {
                        let relative = relative.to_path_buf();
                        if self.selected_workspace_file.as_ref() == Some(&relative) {
                            self.workspace_editor = metadata.source.clone();
                            self.workspace_editor_original = metadata.source.clone();
                            self.workspace_editor_dirty = false;
                            self.refresh_workspace_editor_toml_diagnostic();
                        }
                    }
                    self.status = "Saved Workshop metadata through Vapor Shell. Studio is refreshing the workspace catalog.".to_owned();
                }
                OperationResult::Install(Ok(composition)) => {
                    self.composition_preview = None;
                    let graph_fingerprint =
                        vapor_studio_core::fingerprint_resolved_graph(&composition.graph);
                    self.profiles[self.active_profile].resolved_graph = Some(composition.graph);
                    self.profiles[self.active_profile].installation =
                        vapor_studio_core::InstallationState::Installed {
                            artifacts: installed_records(composition.artifacts),
                            graph_fingerprint,
                        };
                    self.status =
                        "Installed the complete resolved composition through Vapor Shell."
                            .to_owned();
                }
                OperationResult::Update(Ok(composition)) => {
                    self.composition_preview = None;
                    let graph_fingerprint =
                        vapor_studio_core::fingerprint_resolved_graph(&composition.graph);
                    self.profiles[self.active_profile].resolved_graph = Some(composition.graph);
                    self.profiles[self.active_profile].installation =
                        vapor_studio_core::InstallationState::Installed {
                            artifacts: installed_records(composition.artifacts),
                            graph_fingerprint,
                        };
                    self.status = "Updated the selected composition from its current workspace or cache sources.".to_owned();
                }
                OperationResult::RunProfile(Ok(composition)) => {
                    self.composition_preview = None;
                    let graph_fingerprint =
                        vapor_studio_core::fingerprint_resolved_graph(&composition.graph);
                    self.profiles[self.active_profile].resolved_graph = Some(composition.graph);
                    self.profiles[self.active_profile].installation =
                        vapor_studio_core::InstallationState::Installed {
                            artifacts: installed_records(composition.artifacts),
                            graph_fingerprint,
                        };
                    self.status = "Finished playing the selected source composition.".to_owned();
                }
                OperationResult::Repair(Ok(composition)) => {
                    self.composition_preview = None;
                    let graph_fingerprint =
                        vapor_studio_core::fingerprint_resolved_graph(&composition.graph);
                    self.profiles[self.active_profile].resolved_graph = Some(composition.graph);
                    self.profiles[self.active_profile].installation =
                        vapor_studio_core::InstallationState::Installed {
                            artifacts: installed_records(composition.artifacts),
                            graph_fingerprint,
                        };
                    self.status =
                        "Repaired the selected composition through Vapor Shell.".to_owned();
                }
                OperationResult::Verify(Ok(check)) => {
                    let graph_fingerprint = check
                        .graph
                        .as_ref()
                        .and_then(vapor_studio_core::fingerprint_resolved_graph);
                    if let Some(graph) = check.graph {
                        self.profiles[self.active_profile].resolved_graph = Some(graph);
                    }
                    let checked = check.detail.len();
                    let detail = check.detail.join("; ");
                    let artifacts = installed_records(check.artifacts);
                    if check.ok {
                        self.profiles[self.active_profile].installation =
                            vapor_studio_core::InstallationState::Installed {
                                artifacts,
                                graph_fingerprint,
                            };
                        self.status = format!(
                            "Installed composition verified. {checked} graph nodes checked."
                        );
                    } else {
                        self.profiles[self.active_profile].installation =
                            vapor_studio_core::InstallationState::NeedsRepair {
                                detail: detail.clone(),
                                artifacts,
                            };
                        self.status = format!("Installed composition needs repair: {detail}");
                    }
                }
            }
            if let Some(event) = self
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
                self.operation_history.push_front(event.clone());
                self.operation_history.truncate(20);
            }
            self.active_operation = None;
            if refresh_after_content_change || refresh_after_manifest_save {
                self.refresh_content_catalog();
            }
            if refresh_after_subscription_change {
                self.refresh_workshop_subscriptions();
            }
        }
    }

    fn refresh_content_catalog(&mut self) {
        self.composition_preview = None;
        let root = self
            .profiles
            .get(self.active_profile)
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

    fn run_operation(&mut self, operation: DevelopmentOperation) {
        if !self.require_saved_workspace("run a workspace operation") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let kind = match operation {
            DevelopmentOperation::Fmt => OperationKind::Fmt,
            DevelopmentOperation::Check => OperationKind::Check,
            DevelopmentOperation::Test => OperationKind::Test,
            DevelopmentOperation::Build => OperationKind::Build,
            DevelopmentOperation::Clean => OperationKind::Clean,
        };
        self.spawn(kind, move |context| {
            OperationResult::Workspace(
                vapor_core::run_workspace_operation_at_with_context(&root, operation, &context)
                    .map_err(|error| error.to_string()),
            )
        });
        self.status = format!("Running Vapor-managed {operation}…");
    }

    fn discover(&mut self) {
        if !self.require_saved_workspace("refresh workspace content") {
            return;
        }
        self.workshop_preview = None;
        let root = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.workspace.as_ref())
            .map(|workspace| workspace.root.clone());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Discover, move |_context| {
            let (source, catalog) = discover_workspace_and_catalog(&executable, root.as_deref());
            OperationResult::Discovery(source, catalog)
        });
        self.status = "Refreshing the local content catalog through Vapor Shell…".to_owned();
    }

    fn activate_workspace(
        &mut self,
        root: PathBuf,
        contents: Vec<vapor_studio_core::SourceContent>,
        catalog: Result<Vec<vapor_studio_core::CatalogContent>, String>,
        files: Vec<PathBuf>,
    ) {
        let catalog_error = catalog.as_ref().err().cloned();
        self.workspace_root = root.display().to_string();
        self.workspace_root_draft = self.workspace_root.clone();
        self.profiles[self.active_profile].workspace = Some(WorkspaceAssociation { root });
        self.profiles[self.active_profile].root_package = None;
        self.profiles[self.active_profile].resolved_graph = None;
        self.profiles[self.active_profile].installation =
            vapor_studio_core::InstallationState::Unknown;
        self.contents = contents;
        self.catalog = catalog.unwrap_or_default();
        self.selected_catalog_content = None;
        self.composition_preview = None;
        self.composition_edit = None;
        self.composition_original_links = None;
        self.composition_edit_selector.clear();
        self.composition_link_candidate.clear();
        self.repreview_after_catalog_refresh = false;
        self.workshop_preview = None;
        self.workspace_files = files;
        self.selected_workspace_file = None;
        self.workspace_editor.clear();
        self.workspace_editor_original.clear();
        self.workspace_editor_toml_diagnostic = None;
        self.workspace_editor_dirty = false;
        self.new_workspace_file_path.clear();
        self.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.section = Section::Workspace;
        self.status = catalog_error.map_or_else(
            || format!("Opened workspace {} · {} source items · {} local catalog entries.", self.workspace_root, self.contents.len(), self.catalog.len()),
            |error| format!("Opened workspace {} · {} source items. Local cache/install catalog could not load: {error}", self.workspace_root, self.contents.len()),
        );
    }

    pub(super) fn open_workspace_path(&mut self, path: PathBuf) {
        if self.workshop_metadata_dirty {
            self.workspace_root_draft = self.workspace_root.clone();
            self.status = "Save or discard the Workshop metadata draft before changing workspaces.".to_owned();
            return;
        }
        if self.workspace_editor_dirty {
            self.workspace_root_draft = self.workspace_root.clone();
            self.status =
                "Save or discard the current editor changes before changing workspaces.".to_owned();
            return;
        }
        if path.as_os_str().is_empty() {
            self.status = "Enter a workspace folder path.".to_owned();
            return;
        }
        if path == Path::new(self.workspace_root.trim()) {
            self.status = "That workspace is already open for this profile.".to_owned();
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selected = path.clone();
        self.spawn(OperationKind::OpenWorkspace, move |_context| {
            let result = fs::canonicalize(&selected)
                .map_err(|error| format!("{}: {error}", selected.display()))
                .and_then(|root| {
                    VaporWorkspace::load(&root)
                        .map_err(|error| format!("{} is not a valid Vapor Workspace: {error}", root.display()))?;
                    let contents = vapor_studio_core::discover_source_content(&executable, &root)?;
                    let catalog =
                        vapor_studio_core::discover_content_catalog(&executable, Some(&root));
                    discover_workspace_files(&root).map(|files| (root, contents, catalog, files))
                });
            OperationResult::WorkspaceSelection(result)
        });
        self.status = format!("Opening workspace {}…", path.display());
    }

    pub(super) fn validate_source_project(&mut self, content: &vapor_studio_core::SourceContent) {
        if !self.require_saved_workspace("validate this content project") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let selector = source_content_selector(content);
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_profile_source(
                &executable, &root, &selector,
            ))
        });
        self.status = format!("Validating {selector} through Vapor Shell…");
    }

    fn validate_selected_source(&mut self) {
        if !self.require_saved_workspace("validate source content") {
            return;
        }
        let Some(root_package) = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_profile_source(
                &executable,
                &root,
                &selector,
            ))
        });
        self.status = "Validating the selected source through Vapor Shell…".to_owned();
    }

    pub(super) fn validate_workspace_source(&mut self) {
        if !self.require_saved_workspace("validate workspace content") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_workspace_source(
                &executable,
                &root,
            ))
        });
        self.status = "Validating authored content through Vapor…".to_owned();
    }

    fn install_selected(&mut self) {
        if !self.require_saved_workspace("install this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = root_package
            .version
            .as_deref()
            .map(|version| format!("{}@{}", root_package.id, version))
            .unwrap_or_else(|| root_package.id.clone());
        self.spawn(OperationKind::Install, move |_context| {
            OperationResult::Install(vapor_studio_core::install_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = "Installing the selected composition and dependencies…".to_owned();
    }

    fn update_selected(&mut self) {
        if !self.require_saved_workspace("update this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        self.spawn(OperationKind::Update, move |_context| {
            OperationResult::Update(vapor_studio_core::update_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = "Updating the selected composition through Vapor Shell…".to_owned();
    }

    pub(super) fn preview_selected_composition(&mut self) {
        if !self.require_saved_workspace("resolve this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let selector = content_selector(&root_package);
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.composition_preview = None;
        self.spawn(OperationKind::Resolve, move |_context| {
            OperationResult::CompositionPreview(vapor_studio_core::preview_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status =
            "Resolving this profile against workspace, cache, and installed content…".to_owned();
    }

    fn catalog_identity_is_conflicted(&self, selector: &str) -> bool {
        self.catalog.iter().any(|content| {
            catalog_content_selector(content) == selector && !content.is_unambiguous()
        })
    }

    pub(super) fn stage_workshop_content(&mut self, selector: String) {
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot stage {selector}: local records for this exact version disagree. Review the catalog conflict first."
            );
            return;
        }
        if !self.require_saved_workspace("stage this Workshop item") {
            return;
        }
        let workspace = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        self.spawn(OperationKind::StageWorkshop, move |_context| {
            OperationResult::WorkshopStage(vapor_studio_core::stage_workshop_preview(
                &executable,
                &workspace,
                &selector,
            ))
        });
        self.status = "Building and staging a Workshop review package…".to_owned();
    }

    pub(super) fn publish_reviewed_workshop_content(
        &mut self,
        selector: String,
        expected_fingerprint: String,
    ) {
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot publish {selector}: local records for this exact version disagree. Review the catalog conflict first."
            );
            return;
        }
        if !self.require_saved_workspace("publish this Workshop item") {
            return;
        }
        let account = self.workshop_account.trim().to_owned();
        if account.is_empty() {
            self.status =
                "Enter the Steam publisher account used by the local SteamCMD provider.".to_owned();
            return;
        }
        let expected_confirmation = format!("PUBLISH {selector}");
        if self.workshop_confirmation.trim() != expected_confirmation {
            self.status =
                format!("Type {expected_confirmation} exactly to authorize this Workshop upload.");
            return;
        }
        let workspace = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let change_note = self.workshop_change_note.trim().to_owned();
        self.spawn(OperationKind::PublishWorkshop, move |_context| {
            OperationResult::WorkshopPublish(vapor_studio_core::publish_reviewed_workshop_content(
                &executable,
                &workspace,
                &selector,
                &expected_fingerprint,
                &account,
                (!change_note.is_empty()).then_some(change_note.as_str()),
            ))
        });
        self.status = "Uploading the reviewed Workshop artifact through Vapor Shell…".to_owned();
    }

    pub(super) fn download_workshop_item(&mut self) {
        let published_file_id = self.workshop_id.trim().to_owned();
        if published_file_id.is_empty() {
            self.status = "Enter a numeric Steam Workshop item ID.".to_owned();
            return;
        }
        if !published_file_id.bytes().all(|byte| byte.is_ascii_digit()) {
            self.status =
                "Workshop downloads currently require a numeric PublishedFileId.".to_owned();
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Acquire, move |_context| {
            OperationResult::WorkshopAcquisition(vapor_studio_core::download_workshop_item(
                &executable,
                &published_file_id,
            ))
        });
        self.status = "Downloading the public Workshop item into Vapor's cache…".to_owned();
    }

    pub(super) fn use_workshop_as_profile_root(&mut self) {
        let Some(acquisition) = self.pending_workshop.as_ref() else {
            return;
        };
        if acquisition.kind != "packagepack" {
            self.status = format!(
                "{} is a {}, so it cannot be a composition root.",
                acquisition.id, acquisition.kind
            );
            return;
        }
        let identity = ContentIdentity {
            id: acquisition.id.clone(),
            version: acquisition.version.clone(),
        };
        self.assign_profile_root(identity, None);
        self.section = Section::Library;
    }

    pub(super) fn use_available_workshop_package_as_profile_root(&mut self, published_file_id: &str) {
        let matches = self
            .catalog
            .iter()
            .filter(|content| {
                content.published_file_id.as_deref() == Some(published_file_id)
                    && content.kind == vapor_core::ContentKind::Packagepack.as_str()
                    && content.is_unambiguous()
            })
            .cloned()
            .collect::<Vec<_>>();
        let Some(content) = matches.first().cloned() else {
            self.status = format!(
                "Workshop item {published_file_id} is available locally, but no Packagepack was found."
            );
            return;
        };
        if matches.len() > 1 {
            self.selected_catalog_content = Some(catalog_content_selector(&content));
            self.catalog_source_filter = CatalogSourceFilter::Workshop;
            self.catalog_kind_filter = Some(vapor_core::ContentKind::Packagepack.as_str().to_owned());
            self.search_query = published_file_id.to_owned();
            self.section = Section::Discover;
            self.status = format!(
                "Workshop item {published_file_id} has multiple local Packagepack versions. Choose the exact version in the catalog before assigning it."
            );
            return;
        }
        let workspace = content.locations.iter()
            .find(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace)
            .map(|location| location.root.clone());
        let identity = ContentIdentity {
            id: content.id,
            version: content.version,
        };
        self.assign_profile_root(identity, workspace);
        self.section = Section::Library;
    }

    fn verify_selected_installation(&mut self) {
        let Some(root_package) = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let selectors = self.profiles[self.active_profile]
            .resolved_graph
            .as_ref()
            .map(|graph| {
                graph
                    .nodes
                    .iter()
                    .map(|node| content_selector(&node.content))
                    .collect()
            })
            .unwrap_or_else(|| vec![content_selector(&root_package)]);
        self.spawn(OperationKind::Verify, move |_context| {
            OperationResult::Verify(vapor_studio_core::verify_installation(
                &executable,
                &content_selector(&root_package),
                &selectors,
            ))
        });
        self.status = "Verifying installed composition…".to_owned();
    }

    fn repair_selected_installation(&mut self) {
        if !self.require_saved_workspace("repair this workspace composition") {
            return;
        }
        let Some(root_package) = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let workspace = self.profiles[self.active_profile]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.clone());
        let selector = content_selector(&root_package);
        let graph_selectors = self.profiles[self.active_profile]
            .resolved_graph
            .as_ref()
            .map(|graph| {
                graph
                    .nodes
                    .iter()
                    .map(|node| content_selector(&node.content))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.spawn(OperationKind::Repair, move |_context| {
            OperationResult::Repair(vapor_studio_core::repair_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
                &graph_selectors,
            ))
        });
        self.status = "Repairing installed composition…".to_owned();
    }

    fn run_installed_selected(&mut self) {
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let arguments = profile.launch.arguments;
        self.spawn(OperationKind::RunInstalled, move |_context| {
            OperationResult::Message(vapor_studio_core::run_installed_profile(
                &executable,
                workspace.as_deref(),
                &selector,
                &arguments,
            ))
        });
        self.status = "Launching the installed composition for this profile…".to_owned();
    }

    fn launch_default_profile(&mut self) {
        if self.profiles[self.active_profile].root_package.is_none() {
            let executable = match std::env::current_exe() {
                Ok(value) => value,
                Err(error) => {
                    self.status = format!("Could not locate the Studio executable: {error}");
                    return;
                }
            };
            match vapor_studio_core::default_packagepack_selector(&executable, "default") {
                Ok(id) => {
                    self.profiles[self.active_profile].root_package =
                        Some(ContentIdentity { id, version: None })
                }
                Err(error) => {
                    self.status = format!("This app has no default playable composition: {error}");
                    return;
                }
            }
        }
        self.run_installed_selected();
    }

    fn run_selected(&mut self) {
        if !self.require_saved_workspace("build and play this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let Some(workspace) = profile.workspace.map(|workspace| workspace.root) else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        let arguments = profile.launch.arguments;
        self.spawn(OperationKind::Run, move |_context| {
            OperationResult::RunProfile(vapor_studio_core::run_source_profile(
                &executable,
                &workspace,
                &selector,
                &arguments,
            ))
        });
        self.status =
            "Installing source changes and launching this profile through Vapor Shell…".to_owned();
    }

    fn require_saved_workspace(&mut self, action: &str) -> bool {
        if self.workshop_metadata_dirty {
            self.status = format!(
                "Save or discard the Workshop metadata draft before you {action}.",
            );
            return false;
        }
        if !self.workspace_editor_dirty {
            return true;
        }
        self.status = format!(
            "Save or reload the open file before you {action}; Vapor operations read saved files from disk.",
        );
        false
    }

    fn refresh_workspace_files(&mut self) {
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::DiscoverWorkspace, move |_context| {
            OperationResult::WorkspaceFiles(discover_workspace_files(&root))
        });
        self.status = "Reading workspace files…".to_owned();
    }

    fn refresh_workspace_editor_toml_diagnostic(&mut self) {
        self.workspace_editor_toml_diagnostic = self
            .selected_workspace_file
            .as_deref()
            .filter(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                name == "Vapor.toml" || name.ends_with(".vapor.toml") || name == "Cargo.toml"
            })
            .map(|_| {
                self.workspace_editor
                    .parse::<toml::Value>()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            });
    }

    fn open_workspace_file(&mut self, path: PathBuf) {
        if self.workshop_metadata_dirty {
            self.status = "Save or discard the Workshop metadata draft before opening another file.".to_owned();
            return;
        }
        if self.workspace_editor_dirty {
            self.status = "Save or discard the current editor changes before opening another file."
                .to_owned();
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::OpenWorkspaceFile, move |_context| {
            OperationResult::FileContents(
                read_workspace_file(&root, &path).map(|contents| (path, contents)),
            )
        });
        self.status = "Opening workspace file…".to_owned();
    }

    fn save_workspace_file(&mut self) {
        let Some(path) = self.selected_workspace_file.clone() else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        let expected_contents = self.workspace_editor_original.clone();
        let contents = self.workspace_editor.clone();
        self.spawn(OperationKind::SaveWorkspaceFile, move |_context| {
            OperationResult::FileSaved(
                write_workspace_file(&root, &path, &expected_contents, &contents).map(|()| path),
            )
        });
        self.status = "Saving workspace file…".to_owned();
    }

    pub(super) fn reload_workspace_file(&mut self) {
        let Some(path) = self.selected_workspace_file.clone() else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::OpenWorkspaceFile, move |_context| {
            OperationResult::FileContents(
                read_workspace_file(&root, &path).map(|contents| (path, contents)),
            )
        });
        self.status = "Reloading workspace file from disk…".to_owned();
    }

    pub(super) fn create_workspace_file(&mut self) {
        let root = PathBuf::from(self.workspace_root.trim());
        let relative = PathBuf::from(self.new_workspace_file_path.trim());
        if relative.as_os_str().is_empty() {
            self.status = "Enter a file path relative to the workspace root.".to_owned();
            return;
        }
        self.spawn(OperationKind::CreateWorkspaceFile, move |_context| {
            OperationResult::WorkspaceFileCreated(create_new_workspace_file(&root, &relative))
        });
        self.status = "Creating a new workspace file…".to_owned();
    }

    pub(in crate::app) fn search_workshop(&mut self) {
        let query = self.workshop_search_text.trim().to_owned();
        let page = self.workshop_page;
        self.spawn(OperationKind::Discover, move |_context| {
            OperationResult::WorkshopSearch(vapor_studio_core::search_workshop(
                vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                &query,
                page,
            ))
        });
        self.status = "Searching Steam Workshop…".to_owned();
    }

    pub(in crate::app) fn refresh_workshop_subscriptions(&mut self) {
        self.spawn(OperationKind::Discover, move |_context| {
            OperationResult::WorkshopSubscriptions(vapor_studio_core::workshop_subscriptions(
                vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
            ))
        });
        self.status = "Loading Steam Workshop subscriptions…".to_owned();
    }

    pub(in crate::app) fn set_workshop_subscription(&mut self, id: String, subscribed: bool) {
        let parsed_id = match id.parse::<u64>() {
            Ok(id) if id != 0 => id,
            _ => {
                self.status = "Steam returned an invalid PublishedFileId.".to_owned();
                return;
            }
        };
        self.spawn(OperationKind::Discover, move |_context| {
            OperationResult::WorkshopSubscriptionChange(
                if subscribed {
                    vapor_studio_core::subscribe_workshop_item(
                        vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                        parsed_id,
                    )
                } else {
                    vapor_studio_core::unsubscribe_workshop_item(
                        vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                        parsed_id,
                    )
                },
                id,
                subscribed,
            )
        });
        self.status = if subscribed {
            "Subscribing to Steam Workshop item…"
        } else {
            "Unsubscribing from Steam Workshop item…"
        }
        .to_owned();
    }

    fn spawn(
        &mut self,
        kind: OperationKind,
        operation: impl FnOnce(vapor_core::OperationContext) -> OperationResult + Send + 'static,
    ) {
        let request = OperationRequest {
            id: OperationId::fresh(),
            profile_id: self
                .profiles
                .get(self.active_profile)
                .map(|profile| profile.id.clone()),
            kind,
        };
        let phase = match &request.kind {
            OperationKind::Resolve
            | OperationKind::LoadComposition
            | OperationKind::LoadWorkshopMetadata
            | OperationKind::Discover
            | OperationKind::OpenWorkspace
            | OperationKind::RememberWorkspace
            | OperationKind::AdoptContentProject
            | OperationKind::DiscoverContentProjects
            | OperationKind::DiscoverWorkspace => OperationPhase::Resolving,
            OperationKind::CreateWorkspace => OperationPhase::Building,
            OperationKind::CreateWorkspaceFile
            | OperationKind::SaveComposition
            | OperationKind::SaveWorkshopMetadata => OperationPhase::Building,
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
        self.active_operation = Some(OperationEvent {
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
        if let Ok(mut latest) = self.latest_operation_event.lock() {
            *latest = None;
        }
        let latest_event = Arc::clone(&self.latest_operation_event);
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
        self.receiver = Some(receiver);
        self.running = true;
    }

    fn draw_navigation(&mut self, ui: &mut egui::Ui) {
        ui.heading("VAPOR");
        ui.weak("Studio");
        ui.add_space(14.0);
        for section in Section::ALL {
            if ui
                .selectable_label(self.section == section, section.label())
                .clicked()
            {
                self.section = section;
                if section == Section::Workspace && self.workspace_files.is_empty() && !self.running
                {
                    self.refresh_workspace_files();
                }
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            if ui
                .selectable_label(self.section == Section::Diagnostics, "Diagnostics")
                .clicked()
            {
                self.section = Section::Diagnostics;
            }
            ui.separator();
            ui.small("Vapor Studio");
        });
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let previous_profile = self.active_profile;
            ui.heading(self.section.label());
            ui.separator();
            ui.label("Profile");
            egui::ComboBox::from_id_salt("active-profile")
                .selected_text(&self.profiles[self.active_profile].name)
                .show_ui(ui, |ui| {
                    for (index, profile) in self.profiles.iter().enumerate() {
                        ui.selectable_value(&mut self.active_profile, index, &profile.name);
                    }
                });
            if self.active_profile != previous_profile {
                let requested_profile = self.active_profile;
                self.active_profile = previous_profile;
                self.select_profile(requested_profile);
            }
            ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Search content"))
                .on_hover_text("Search the current catalog");
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty,
                    egui::Button::new("+ Profile"),
                )
                .clicked()
            {
                self.new_profile_name = format!("Composition {}", self.profiles.len() + 1);
                self.add_profile();
            }
            if self.running {
                ui.separator();
                ui.spinner();
                ui.label("Working…");
            }
        });
    }

    pub(super) fn select_profile(&mut self, index: usize) {
        if self.running || index >= self.profiles.len() {
            return;
        }
        if self.workspace_editor_dirty {
            self.status =
                "Save or discard the current editor changes before switching profiles.".to_owned();
            return;
        }
        self.active_profile = index;
        self.workspace_root = self.profiles[index]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.display().to_string())
            .unwrap_or_else(|| self.fallback_workspace_root.display().to_string());
        self.workspace_root_draft = self.workspace_root.clone();
        self.contents.clear();
        self.catalog.clear();
        self.selected_catalog_content = None;
        self.composition_preview = None;
        self.workshop_preview = None;
        self.workspace_files.clear();
        self.selected_workspace_file = None;
        self.workspace_editor.clear();
        self.workspace_editor_original.clear();
        self.workspace_editor_toml_diagnostic = None;
        self.workspace_editor_dirty = false;
        self.new_workspace_file_path.clear();
        self.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.discover();
    }

    pub(super) fn assign_profile_root(
        &mut self,
        identity: ContentIdentity,
        workspace: Option<PathBuf>,
    ) {
        let next_workspace = workspace
            .clone()
            .unwrap_or_else(|| self.fallback_workspace_root.clone());
        if self.catalog.iter().any(|content| {
            content.identity() == identity
                && !content.is_unambiguous()
        }) {
            self.status = format!(
                "Cannot select {}: its local workspace, cache, or installed records disagree. Review the catalog conflicts first.",
                content_selector(&identity)
            );
            return;
        }
        if self.workspace_editor_dirty && Path::new(self.workspace_root.trim()) != next_workspace {
            self.status =
                "Save or discard editor changes before assigning content from another workspace."
                    .to_owned();
            return;
        }
        let workspace_changed = Path::new(self.workspace_root.trim()) != next_workspace;
        let profile = &mut self.profiles[self.active_profile];
        profile.root_package = Some(identity.clone());
        profile.workspace = workspace.map(|root| WorkspaceAssociation { root });
        profile.resolved_graph = None;
        profile.installation = vapor_studio_core::InstallationState::NotInstalled;
        self.composition_preview = None;
        if workspace_changed {
            self.composition_edit = None;
            self.composition_original_links = None;
            self.composition_edit_selector.clear();
            self.composition_link_candidate.clear();
            self.repreview_after_catalog_refresh = false;
            self.workspace_root = next_workspace.display().to_string();
            self.workspace_root_draft = self.workspace_root.clone();
            self.contents.clear();
            self.catalog.clear();
            self.selected_catalog_content = None;
            self.workspace_files.clear();
            self.selected_workspace_file = None;
            self.workspace_editor.clear();
            self.workspace_editor_original.clear();
            self.workspace_editor_toml_diagnostic = None;
            self.workspace_editor_dirty = false;
            self.new_workspace_file_path.clear();
            self.discover();
        }
        self.status = format!(
            "Selected {} for profile {}. Preview its dependency plan before installing.",
            content_selector(&identity),
            self.profiles[self.active_profile].name,
        );
        self.section = Section::Library;
    }

    fn add_profile(&mut self) {
        let name = self.new_profile_name.trim();
        let workspace = PathBuf::from(self.workspace_root.trim());
        let index = match vapor_studio_core::add_profile(&mut self.profiles, name, Some(workspace))
        {
            Ok(index) => index,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        self.active_profile = index;
        self.new_profile_name.clear();
        self.status = format!(
            "Created profile {}.",
            self.profiles[self.active_profile].name
        );
    }

    pub(super) fn duplicate_active_profile(&mut self) {
        if self.running || self.workspace_editor_dirty {
            return;
        }
        let source_name = self.profiles[self.active_profile].name.clone();
        let index =
            match vapor_studio_core::duplicate_profile(&mut self.profiles, self.active_profile) {
                Ok(index) => index,
                Err(error) => {
                    self.status = error;
                    return;
                }
            };
        self.select_profile(index);
        self.status = format!("Duplicated {source_name} as {}.", self.profiles[index].name);
    }

    fn preview_steam_package(&mut self) {
        self.spawn(OperationKind::StageRootDistribution, |_context| {
            let result = vapor_sdk_core::root::package::root_package(
                &vapor_sdk_core::root::types::RootPackageRequest {
                    plan: true,
                    app_id: vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                    depot_id: vapor_sdk_core::root::types::ROOT_STEAM_DEPOT_ID,
                    description: "Vapor Studio package preview".to_owned(),
                    set_live: None,
                },
            )
            .map(|report| {
                format!(
                    "Steam root preview · App {} · depot {} · {} source roots · {} files · output {}",
                    report.app_id,
                    report.depot_id,
                    report.included_roots.len(),
                    report.copied_files,
                    report.package_root.display(),
                )
            })
            .map_err(|error| error.to_string());
            OperationResult::Message(result)
        });
        self.status = "Preparing a read-only Steam root package plan…".to_owned();
    }

    pub(super) fn load_source_composition(&mut self) {
        if !self.require_saved_workspace("edit a Packagepack composition") { return; }
        let selector = self.composition_edit_selector.trim().to_owned();
        if selector.is_empty() { self.status = "Select a source Packagepack first.".to_owned(); return; }
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() { Ok(value) => value, Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; } };
        self.spawn(OperationKind::LoadComposition, move |_context| {
            OperationResult::SourceCompositionLoaded(vapor_studio_core::load_source_composition(&executable, &root, &selector))
        });
        self.status = format!("Loading {selector} through Vapor Shell…");
    }

    pub(super) fn save_source_composition(&mut self) {
        if !self.require_saved_workspace("save a Packagepack composition") { return; }
        let Some(composition) = self.composition_edit.clone() else { self.status = "Load a source Packagepack before saving.".to_owned(); return; };
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() { Ok(value) => value, Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; } };
        self.spawn(OperationKind::SaveComposition, move |_context| {
            OperationResult::SourceCompositionSaved(vapor_studio_core::save_source_composition(
                &executable, &root, &composition.id, &composition.source, &composition.links,
            ))
        });
        self.status = "Validating and saving the Packagepack graph through Vapor Shell…".to_owned();
    }

    pub(super) fn load_source_workshop_metadata(&mut self, content: &vapor_studio_core::SourceContent) {
        if !self.require_saved_workspace("edit Workshop metadata") { return; }
        let selector = source_content_selector(content);
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; }
        };
        self.spawn(OperationKind::LoadWorkshopMetadata, move |_context| {
            OperationResult::SourceWorkshopLoaded(vapor_studio_core::load_source_workshop_metadata(
                &executable, &root, &selector,
            ))
        });
        self.status = format!("Loading Workshop metadata for {selector} through Vapor Shell…");
    }

    pub(super) fn save_source_workshop_metadata(&mut self) {
        if self.workspace_editor_dirty {
            self.status = "Save or reload the open file before saving Workshop metadata.".to_owned();
            return;
        }
        let Some(metadata) = self.workshop_metadata_edit.clone() else {
            self.status = "Load a source content project's Workshop metadata first.".to_owned();
            return;
        };
        let app_id = match self.workshop_metadata_app_id.trim() {
            "" => None,
            value => match value.parse::<u32>() {
                Ok(value) if value > 0 => Some(value),
                _ => { self.status = "Steam AppID must be a positive number.".to_owned(); return; }
            },
        };
        let non_empty = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        let policy = vapor_studio_core::WorkshopPolicyDraft {
            app_id,
            published_file_id: non_empty(&self.workshop_metadata_published_file_id),
            visibility: non_empty(&self.workshop_metadata_visibility),
            title: non_empty(&self.workshop_metadata_title),
            description: non_empty(&self.workshop_metadata_description),
            tags: self.workshop_metadata_tags.split(',').map(str::trim)
                .filter(|tag| !tag.is_empty()).map(ToOwned::to_owned).collect(),
            change_note: non_empty(&self.workshop_metadata_change_note),
        };
        let selector = metadata.version.as_deref()
            .map(|version| format!("{}@{version}", metadata.id))
            .unwrap_or_else(|| metadata.id.clone());
        let expected_source = metadata.source;
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; }
        };
        self.spawn(OperationKind::SaveWorkshopMetadata, move |_context| {
            OperationResult::SourceWorkshopSaved(vapor_studio_core::save_source_workshop_metadata(
                &executable, &root, &selector, &expected_source, &policy,
            ))
        });
        self.status = format!("Validating and saving Workshop metadata for {selector} through Vapor Shell…");
    }

    fn set_workshop_metadata_fields(&mut self, policy: &vapor_studio_core::WorkshopPolicyDraft) {
        self.workshop_metadata_app_id = policy.app_id.map(|value| value.to_string()).unwrap_or_default();
        self.workshop_metadata_published_file_id = policy.published_file_id.clone().unwrap_or_default();
        self.workshop_metadata_visibility = policy.visibility.clone().unwrap_or_default();
        self.workshop_metadata_title = policy.title.clone().unwrap_or_default();
        self.workshop_metadata_description = policy.description.clone().unwrap_or_default();
        self.workshop_metadata_tags = policy.tags.join(", ");
        self.workshop_metadata_change_note = policy.change_note.clone().unwrap_or_default();
    }

    pub(super) fn use_edited_composition_for_profile(&mut self) {
        let Some(composition) = self.composition_edit.as_ref() else { self.status = "Load a source Packagepack first.".to_owned(); return; };
        let root = ContentIdentity { id: composition.id.clone(), version: composition.version.clone() };
        self.assign_profile_root(root, Some(PathBuf::from(self.workspace_root.trim())));
        self.preview_selected_composition();
    }

    fn create_content_workspace(&mut self) {
        if !self.require_saved_workspace("create content in this workspace") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let id = self.draft_content_id.trim().to_owned();
        let version = self.draft_content_version.trim().to_owned();
        let selected = self.draft_dependencies.clone();
        let app_id = match self.draft_workshop_app_id.trim() {
            "" => None,
            value => match value.parse::<u32>() {
                Ok(value) if value > 0 => Some(value),
                _ => {
                    self.status = "Steam AppID must be a positive number.".to_owned();
                    return;
                }
            },
        };
        let non_empty = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        let workshop = vapor_studio_core::WorkshopPolicyDraft {
            app_id,
            published_file_id: non_empty(&self.draft_workshop_published_file_id),
            visibility: non_empty(&self.draft_workshop_visibility),
            title: non_empty(&self.draft_workshop_title),
            description: non_empty(&self.draft_workshop_description),
            tags: self
                .draft_workshop_tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            change_note: non_empty(&self.draft_workshop_change_note),
        };
        let kind = self.draft_content_kind;
        self.spawn(OperationKind::CreateContent, move |_context| {
            let result = vapor_studio_core::create_content_workspace_with_workshop(
                &executable,
                &root,
                &id,
                &version,
                kind,
                &selected,
                &workshop,
            )
            .and_then(|output| {
                let workspace = fs::canonicalize(&root)
                    .map_err(|error| format!("{}: {error}", root.display()))?;
                let manifest = output;
                let relative = manifest
                    .strip_prefix(&workspace)
                    .map(PathBuf::from)
                    .map_err(|error| {
                        format!("created manifest is outside the workspace: {error}")
                    })?;
                let contents = fs::read_to_string(&manifest)
                    .map_err(|error| format!("{}: {error}", manifest.display()))?;
                let files = discover_workspace_files(&workspace)?;
                Ok((relative, contents, files))
            });
            OperationResult::ContentWorkspace(result)
        });
        self.status = "Creating content and opening its manifest…".to_owned();
    }

    pub(super) fn create_starter_workspace(&mut self) {
        if !self.require_saved_workspace("create and open a starter workspace") {
            return;
        }
        let destination = PathBuf::from(self.starter_workspace_path.trim());
        if destination.as_os_str().is_empty() {
            self.status = "Choose an empty folder for the new starter workspace.".to_owned();
            return;
        }
        let organization = self.starter_workspace_organization.trim().to_owned();
        let name = self.starter_workspace_name.trim().to_owned();
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let open_executable = executable.clone();
        self.spawn(OperationKind::CreateWorkspace, move |_context| {
            let result = vapor_studio_core::create_basic_content_workspace(
                &executable,
                &destination,
                &organization,
                &name,
            )
            .and_then(|root| {
                vapor_studio_core::discover_source_content(&open_executable, &root)
                    .and_then(|contents| {
                        let catalog = vapor_studio_core::discover_content_catalog(
                            &open_executable,
                            Some(&root),
                        );
                        discover_workspace_files(&root)
                            .map(|files| (root.clone(), contents, catalog, files))
                    })
                    .map_err(|error| {
                        format!(
                            "Created workspace at {}, but Studio could not open it: {error}",
                            root.display()
                        )
                    })
            });
            OperationResult::WorkspaceSelection(result)
        });
        self.status =
            "Creating the maintained Engine, Game, and Packagepack starter workspace…".to_owned();
    }

    pub(super) fn scan_unregistered_content_projects(&mut self) {
        if !self.require_saved_workspace("scan for unregistered content") {
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::DiscoverContentProjects, move |_context| {
            OperationResult::UnregisteredProjectScan(
                vapor_studio_core::scan_unregistered_content_projects(&executable, &root),
            )
        });
        self.status = "Finding unregistered content manifests in this Workspace…".to_owned();
    }

    pub(super) fn adopt_content_project(&mut self, project_path: PathBuf) {
        if !self.require_saved_workspace("adopt a content project") {
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace_root.trim());
        let display_path = project_path.display().to_string();
        self.spawn(OperationKind::AdoptContentProject, move |_context| {
            OperationResult::AdoptedContentProject(
                vapor_studio_core::adopt_workspace_content_project(
                    &executable, &root, &project_path,
                ),
            )
        });
        self.status = format!("Validating and adopting {display_path}…");
    }

    pub(super) fn remember_open_workspace(&mut self) {
        let root = PathBuf::from(self.workspace_root.trim());
        if root.as_os_str().is_empty() {
            self.status = "Open a workspace before adding it to Vapor sources.".to_owned();
            return;
        }
        if self.registered_sources.iter().any(|source| source == &root) {
            self.status = "This workspace is already registered with Vapor.".to_owned();
            return;
        }
        self.spawn(OperationKind::RememberWorkspace, move |_context| {
            OperationResult::RememberedSources(vapor_studio_core::remember_workspace(&root))
        });
        self.status = "Adding the open workspace to this Vapor installation…".to_owned();
    }

    fn refresh_registered_sources(&mut self) {
        let result = vapor_core::VaporInstallation::discover()
            .map_err(|error| error.to_string())
            .and_then(|installation| {
                vapor_core::source_state(&installation)
                    .map(|state| (state.known, state.active))
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok((known, active)) => {
                self.registered_sources = known;
                self.active_source = active;
                self.status = format!(
                    "Loaded {} registered workspace{} from this Vapor installation.",
                    self.registered_sources.len(),
                    if self.registered_sources.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            }
            Err(error) => self.status = format!("Could not read registered workspaces: {error}"),
        }
    }
}

fn installed_records(
    artifacts: Vec<vapor_studio_core::InstalledArtifact>,
) -> Vec<InstalledArtifactRecord> {
    artifacts
        .into_iter()
        .map(|artifact| InstalledArtifactRecord {
            id: artifact.id,
            version: artifact.version,
            root: artifact.root,
            fingerprint: artifact.fingerprint,
        })
        .collect()
}

fn discover_workspace_and_catalog(
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

fn content_selector(identity: &ContentIdentity) -> String {
    identity
        .version
        .as_deref()
        .map(|version| format!("{}@{}", identity.id, version))
        .unwrap_or_else(|| identity.id.clone())
}

pub(crate) fn source_content_selector(content: &vapor_studio_core::SourceContent) -> String {
    content
        .version
        .as_deref()
        .map(|version| format!("{}@{version}", content.id))
        .unwrap_or_else(|| content.id.clone())
}

pub(crate) fn catalog_content_selector(content: &vapor_studio_core::CatalogContent) -> String {
    content_selector(&content.identity())
}

pub(crate) fn source_content_kind(kind: &str) -> Option<ContentKind> {
    Some(match kind {
        "packagepack" => ContentKind::Packagepack,
        "enginepack" => ContentKind::Enginepack,
        "gamepack" => ContentKind::Gamepack,
        "modpack" => ContentKind::Modpack,
        "engine" => ContentKind::Engine,
        "game" => ContentKind::Game,
        "engine-mod" => ContentKind::EngineMod,
        "game-mod" => ContentKind::GameMod,
        "extension-mod" => ContentKind::ExtensionMod,
        _ => return None,
    })
}

fn discover_workspace_files(root: &std::path::Path) -> Result<Vec<PathBuf>, String> {
    let root =
        std::fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    let mut pending = vec![root.clone()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.')
                || matches!(
                    name.as_ref(),
                    "target" | "node_modules" | "vendor" | "output"
                )
            {
                continue;
            }
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(&root)
                    .map_err(|error| error.to_string())?
                    .to_path_buf();
                files.push(relative);
                if files.len() > 10_000 {
                    return Err(
                        "workspace contains more than 10,000 visible files; narrow the source root"
                            .to_owned(),
                    );
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

fn safe_workspace_file(
    root: &std::path::Path,
    relative: &std::path::Path,
) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    let path = fs::canonicalize(root.join(relative))
        .map_err(|error| format!("{}: {error}", relative.display()))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err("selected path is not a regular file inside the workspace".to_owned());
    }
    Ok(path)
}

fn read_workspace_file(
    root: &std::path::Path,
    relative: &std::path::Path,
) -> Result<String, String> {
    let path = safe_workspace_file(root, relative)?;
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > 4 * 1024 * 1024 {
        return Err(
            "files larger than 4 MiB open as preview-only in a future Studio editor".to_owned(),
        );
    }
    fs::read_to_string(&path).map_err(|error| {
        format!(
            "{} is not a readable UTF-8 text file: {error}",
            relative.display()
        )
    })
}

fn write_workspace_file(
    root: &std::path::Path,
    relative: &std::path::Path,
    expected_contents: &str,
    contents: &str,
) -> Result<(), String> {
    let path = safe_workspace_file(root, relative)?;
    let current =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", relative.display()))?;
    if current != expected_contents {
        return Err(format!(
            "{} changed on disk after it was opened; reload it before saving to avoid overwriting those edits",
            relative.display()
        ));
    }
    fs::write(&path, contents).map_err(|error| format!("{}: {error}", relative.display()))
}

fn create_new_workspace_file(
    root: &std::path::Path,
    relative: &std::path::Path,
) -> Result<PathBuf, String> {
    use std::path::Component;

    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(
            "file path must be relative and cannot contain '.', '..', or root components"
                .to_owned(),
        );
    }
    let root = fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    let requested = root.join(relative);
    let parent = requested
        .parent()
        .ok_or_else(|| "file path has no parent directory".to_owned())?;
    let parent =
        fs::canonicalize(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    if !parent.starts_with(&root) || !parent.is_dir() {
        return Err("file path must stay inside an existing workspace directory".to_owned());
    }
    let name = requested
        .file_name()
        .ok_or_else(|| "file path must include a filename".to_owned())?;
    let target = parent.join(name);
    let _file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|error| {
            format!(
                "could not create {} without overwriting: {error}",
                relative.display()
            )
        })?;
    Ok(relative.to_path_buf())
}

pub(super) fn catalog_matches_filters(
    content: &vapor_studio_core::CatalogContent,
    source_filter: CatalogSourceFilter,
    kind_filter: Option<&str>,
    query: &str,
) -> bool {
    let source_matches = match source_filter {
        CatalogSourceFilter::All => true,
        CatalogSourceFilter::Workspace => content
            .locations
            .iter()
            .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace),
        CatalogSourceFilter::Cache => content
            .locations
            .iter()
            .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Cache),
        CatalogSourceFilter::Installed => content
            .locations
            .iter()
            .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Installed),
        CatalogSourceFilter::Workshop => content.published_file_id.is_some(),
    };
    if !source_matches || kind_filter.is_some_and(|kind| content.kind != kind) {
        return false;
    }
    let query = query.trim().to_lowercase();
    query.is_empty()
        || content.id.to_lowercase().contains(&query)
        || content.kind.to_lowercase().contains(&query)
        || content
            .version
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(&query)
        || content
            .published_file_id
            .as_deref()
            .unwrap_or_default()
            .contains(&query)
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_operation();
        egui::Panel::left("studio-navigation")
            .resizable(false)
            .default_size(190.0)
            .show(ui, |ui| self.draw_navigation(ui));
        egui::Panel::top("studio-header").show(ui, |ui| self.draw_header(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            match self.section {
                Section::Home => self.draw_home(ui),
                Section::Discover => self.draw_discover(ui),
                Section::Library => self.draw_library(ui),
                Section::Compose => self.draw_composition(ui),
                Section::Create => self.draw_create(ui),
                Section::Workspace => self.draw_workspace(ui),
                Section::Diagnostics => {
                    self.diagnostics.show(ui);
                    egui::CollapsingHeader::new("Steam root package tools").show(ui, |ui| {
                        ui.label("Root app/depot publishing is separate from Workshop content publishing.");
                        if ui.add_enabled(!self.running, egui::Button::new("Preview root package")).clicked() {
                            self.preview_steam_package();
                        }
                    });
                }
            }
            self.draw_operation_center(ui);
            if let Some(recovery) = &self.profile_recovery {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, recovery);
            }
        });
        if self.running {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("workspace_root", self.workspace_root.clone());
        let document = ProfileDocument {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: self.profiles.clone(),
        };
        if let Ok(encoded) = serde_json::to_string(&document) {
            storage.set_string("profiles", encoded);
        }
        if let Some(original) = &self.profile_recovery_backup {
            storage.set_string("profiles.recovery_backup", original.clone());
        }
        storage.set_string("active_profile", self.active_profile.to_string());
        if let Some(profile) = self.profiles.get(self.active_profile) {
            storage.set_string("active_profile_id", profile.id.clone());
        }
    }
}
