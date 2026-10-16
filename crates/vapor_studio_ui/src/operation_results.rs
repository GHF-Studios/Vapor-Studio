use super::{StudioApp, catalog_content_selector};
use super::operations::OperationResult;
use crate::navigation::Section;
use std::path::Path;
use vapor_studio_core::{ContentIdentity, InstalledArtifactRecord};

impl StudioApp {
    pub(super) fn apply_operation_result(&mut self, result: OperationResult) {
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
                self.workspace.registered_sources = known;
                self.status = "Added the open workspace to this Vapor installation. It remains separate from the active source.".to_owned();
            }
            OperationResult::AdoptedContentProject(Ok((summary, contents))) => {
                self.contents = contents;
                self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
                self.status = summary;
            }
            OperationResult::UnregisteredContentProject(Ok((summary, contents, identity))) => {
                self.contents = contents;
                self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
                self.workspace.pending_unregistration = None;
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
                self.workspace.unregistered_projects = scan;
            }
            OperationResult::WorkspaceSelection(Ok((root, contents, catalog, files))) => {
                self.workshop_metadata_edit = None;
                self.runtime_outputs_edit = None;
                self.manifest_metadata_dirty = false;
                self.workspace.pending_unregistration = None;
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
                self.workshop.pending_acquisition = Some(acquisition);
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
                self.workshop.preview = Some(preview);
            }
            OperationResult::WorkshopSearch(Ok(page), query) => {
                self.workshop.page = page.page;
                self.workshop.total_results = page.total_results;
                self.workshop.results = page.items;
                self.workshop.results_query = Some(query);
                self.status = format!(
                    "Steam Workshop returned {} items on page {} of {}.",
                    self.workshop.results.len(),
                    self.workshop.page,
                    self.workshop.total_results
                );
            }
            OperationResult::WorkshopSubscriptions(Ok(subscriptions)) => {
                self.workshop.subscriptions = subscriptions
                    .iter()
                    .map(|item| item.published_file_id.clone())
                    .collect();
                self.workshop.subscription_details = subscriptions
                    .into_iter()
                    .map(|item| (item.published_file_id.clone(), item))
                    .collect();
                self.status = format!(
                    "Loaded {} Steam Workshop subscriptions.",
                    self.workshop.subscriptions.len()
                );
            }
            OperationResult::WorkshopSubscriptionChange(Ok(()), id, subscribed) => {
                if subscribed {
                    self.workshop.subscriptions.insert(id.clone());
                } else {
                    self.workshop.subscriptions.remove(&id);
                    self.workshop.subscription_details.remove(&id);
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
                self.workspace.files = files;
                self.status = format!("Found {} workspace files.", self.workspace.files.len());
            }
            OperationResult::WorkspaceFileCreated(Ok(path)) => {
                self.invalidate_workshop_review();
                if !self.workspace.files.contains(&path) {
                    self.workspace.files.push(path.clone());
                    self.workspace.files.sort();
                }
                self.workspace.selected_file = Some(path.clone());
                self.workspace.files_open = true;
                self.workspace.editor.clear();
                self.workspace.editor_original.clear();
                self.refresh_workspace_editor_toml_diagnostic();
                self.workspace.editor_dirty = false;
                self.workspace.new_file_path.clear();
                self.status = format!("Created {}.", path.display());
            }
            OperationResult::FileContents(Ok((path, contents))) => {
                self.workspace.selected_file = Some(path);
                self.workspace.files_open = true;
                self.workspace.editor = contents.clone();
                self.workspace.editor_original = contents;
                self.refresh_workspace_editor_toml_diagnostic();
                self.workspace.editor_dirty = false;
                self.status = "Opened workspace file.".to_owned();
            }
            OperationResult::FileSaved(Ok(path)) => {
                self.invalidate_workshop_review();
                if self.workspace.selected_file.as_ref() == Some(&path) {
                    self.workspace.editor_original
                        .clone_from(&self.workspace.editor);
                    self.refresh_workspace_editor_toml_diagnostic();
                    self.workspace.editor_dirty = false;
                }
                self.status = format!("Saved {}.", path.display());
            }
            OperationResult::ContentWorkspace(Ok((manifest, contents, files))) => {
                self.invalidate_workshop_review();
                self.workspace.files = files;
                self.workspace.selected_file = Some(manifest);
                self.workspace.files_open = true;
                self.workspace.editor = contents.clone();
                self.workspace.editor_original = contents;
                self.refresh_workspace_editor_toml_diagnostic();
                self.workspace.editor_dirty = false;
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
                if let Ok(relative) = metadata.manifest.strip_prefix(Path::new(self.workspace.root.trim())) {
                    let relative = relative.to_path_buf();
                    if self.workspace.selected_file.as_ref() == Some(&relative) {
                        self.workspace.editor = metadata.source.clone();
                        self.workspace.editor_original = metadata.source.clone();
                        self.workspace.editor_dirty = false;
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
                if let Ok(relative) = outputs.manifest.strip_prefix(Path::new(self.workspace.root.trim())) {
                    let relative = relative.to_path_buf();
                    if self.workspace.selected_file.as_ref() == Some(&relative) {
                        self.workspace.editor = outputs.source.clone();
                        self.workspace.editor_original = outputs.source.clone();
                        self.workspace.editor_dirty = false;
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
