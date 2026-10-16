use super::{StudioApp, catalog_content_selector, content_selector};
use std::{
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
    ContentIdentity, InstalledArtifactRecord, OperationEvent, OperationId, OperationKind,
    OperationPhase, OperationRequest, OperationStatus,
};

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
                OperationResult::UnregisteredContentProject(Ok((summary, contents, identity))) => {
                    self.contents = contents;
                    self.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
                    self.pending_content_unregistration = None;
                    self.invalidate_profiles_for_source_content(&identity);
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
                    self.runtime_outputs_edit = None;
                    self.manifest_metadata_dirty = false;
                    self.pending_content_unregistration = None;
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
                | OperationResult::SourceRuntimeOutputsLoaded(Err(error))
                | OperationResult::SourceRuntimeOutputsSaved(Err(error))
                | OperationResult::UnregisteredContentProject(Err(error))
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
                OperationResult::WorkshopStage(Err(error)) => {
                    self.invalidate_workshop_review();
                    self.status = error;
                }
                OperationResult::WorkshopAcquisition(Err(error))
                | OperationResult::WorkshopPublish(Err(error))
                | OperationResult::WorkshopSearch(Err(error), _)
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
                    self.invalidate_workshop_review();
                }
                OperationResult::WorkshopStage(Ok(preview)) => {
                    self.status = format!(
                        "Staged {} · {} files · {} bytes.",
                        preview.id, preview.files, preview.bytes
                    );
                    self.workshop_preview = Some(preview);
                }
                OperationResult::WorkshopSearch(Ok(page), query) => {
                    self.workshop_page = page.page;
                    self.workshop_result_total = page.total_results;
                    self.workshop_results = page.items;
                    self.workshop_results_query = Some(query);
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
                    self.invalidate_workshop_review();
                    if !self.workspace_files.contains(&path) {
                        self.workspace_files.push(path.clone());
                        self.workspace_files.sort();
                    }
                    self.selected_workspace_file = Some(path.clone());
                    self.workspace_files_open = true;
                    self.workspace_editor.clear();
                    self.workspace_editor_original.clear();
                    self.refresh_workspace_editor_toml_diagnostic();
                    self.workspace_editor_dirty = false;
                    self.new_workspace_file_path.clear();
                    self.status = format!("Created {}.", path.display());
                }
                OperationResult::FileContents(Ok((path, contents))) => {
                    self.selected_workspace_file = Some(path);
                    self.workspace_files_open = true;
                    self.workspace_editor = contents.clone();
                    self.workspace_editor_original = contents;
                    self.refresh_workspace_editor_toml_diagnostic();
                    self.workspace_editor_dirty = false;
                    self.status = "Opened workspace file.".to_owned();
                }
                OperationResult::FileSaved(Ok(path)) => {
                    self.invalidate_workshop_review();
                    if self.selected_workspace_file.as_ref() == Some(&path) {
                        self.workspace_editor_original
                            .clone_from(&self.workspace_editor);
                        self.refresh_workspace_editor_toml_diagnostic();
                        self.workspace_editor_dirty = false;
                    }
                    self.status = format!("Saved {}.", path.display());
                }
                OperationResult::ContentWorkspace(Ok((manifest, contents, files))) => {
                    self.invalidate_workshop_review();
                    self.workspace_files = files;
                    self.selected_workspace_file = Some(manifest);
                    self.workspace_files_open = true;
                    self.workspace_editor = contents.clone();
                    self.workspace_editor_original = contents;
                    self.refresh_workspace_editor_toml_diagnostic();
                    self.workspace_editor_dirty = false;
                    self.section = Section::Workspace;
                    self.status = "Created content and opened its manifest.".to_owned();
                }
                OperationResult::SourceCompositionLoaded(Ok(composition)) => {
                    self.composition_edit_selector = composition
                        .version
                        .as_deref()
                        .map(|version| format!("{}@{version}", composition.id))
                        .unwrap_or_else(|| composition.id.clone());
                    self.composition_original_links = Some(composition.links.clone());
                    self.composition_edit = Some(composition);
                    self.status = format!(
                        "Loaded the {} dependency graph. Review its edges before saving.",
                        self.composition_edit.as_ref().map(|graph| graph.kind.as_str()).unwrap_or("source content"),
                    );
                }
                OperationResult::SourceCompositionSaved(Ok(composition)) => {
                    self.invalidate_workshop_review();
                    let identity = ContentIdentity {
                        id: composition.id.clone(),
                        version: composition.version.clone(),
                    };
                    self.composition_original_links = Some(composition.links.clone());
                    self.composition_edit = Some(composition);
                    self.invalidate_profiles_for_source_content(&identity);
                    self.status = "Saved the source content graph. Studio is refreshing its catalog and composition plan.".to_owned();
                }
                OperationResult::SourceWorkshopLoaded(Ok(metadata)) => {
                    self.set_workshop_metadata_fields(&metadata.policy);
                    self.workshop_metadata_edit = Some(metadata);
                    self.runtime_outputs_edit = None;
                    self.manifest_metadata_dirty = false;
                    self.status = "Loaded the source Workshop metadata. Edit the fields, then save through Vapor Shell.".to_owned();
                }
                OperationResult::SourceWorkshopSaved(Ok(metadata)) => {
                    self.invalidate_workshop_review();
                    self.set_workshop_metadata_fields(&metadata.policy);
                    self.workshop_metadata_edit = Some(metadata.clone());
                    self.manifest_metadata_dirty = false;
                    self.invalidate_profiles_for_source_content(&ContentIdentity {
                        id: metadata.id.clone(),
                        version: metadata.version.clone(),
                    });
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
                OperationResult::SourceRuntimeOutputsLoaded(Ok(outputs)) => {
                    self.set_runtime_output_fields(&outputs);
                    self.runtime_outputs_edit = Some(outputs);
                    self.workshop_metadata_edit = None;
                    self.manifest_metadata_dirty = false;
                    self.status = "Loaded runtime output declarations. Edit the executable and library lists, then save through Vapor Shell.".to_owned();
                }
                OperationResult::SourceRuntimeOutputsSaved(Ok(outputs)) => {
                    self.invalidate_workshop_review();
                    self.set_runtime_output_fields(&outputs);
                    self.runtime_outputs_edit = Some(outputs.clone());
                    self.manifest_metadata_dirty = false;
                    self.invalidate_profiles_for_source_content(&ContentIdentity {
                        id: outputs.id.clone(),
                        version: outputs.version.clone(),
                    });
                    if let Ok(relative) = outputs.manifest.strip_prefix(Path::new(self.workspace_root.trim())) {
                        let relative = relative.to_path_buf();
                        if self.selected_workspace_file.as_ref() == Some(&relative) {
                            self.workspace_editor = outputs.source.clone();
                            self.workspace_editor_original = outputs.source.clone();
                            self.workspace_editor_dirty = false;
                            self.refresh_workspace_editor_toml_diagnostic();
                        }
                    }
                    self.status = "Saved runtime outputs through Vapor Shell. Studio is refreshing the catalog and active profile.".to_owned();
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

    pub(super) fn spawn(
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

