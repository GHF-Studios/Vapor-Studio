use super::StudioApp;
use super::operation_protocol::WorkspaceOpenSnapshot;
use crate::navigation::Section;
use std::path::PathBuf;
use vapor_studio_core::ContentIdentity;
use vapor_studio_protocol::{AuthoringResult, AuthoringResultKind};

impl StudioApp {
    pub(super) fn apply_service_authoring_result(&mut self, result: AuthoringResult) {
        if !self
            .operation
            .active_operation
            .as_ref()
            .is_some_and(|operation| operation.id.0.to_string() == result.operation_id)
        {
            return;
        }
        match result.result {
            AuthoringResultKind::RegisteredSources { roots, active } => {
                self.workspace.registered_sources = roots.into_iter().map(PathBuf::from).collect();
                self.workspace.active_source = active.map(PathBuf::from);
                self.workspace.registered_sources_loaded = true;
                self.workspace.registered_sources_error = None;
                self.status = format!(
                    "Loaded {} registered workspace{} from this Vapor installation.",
                    self.workspace.registered_sources.len(),
                    if self.workspace.registered_sources.len() == 1 {
                        ""
                    } else {
                        "s"
                    },
                );
            }
            AuthoringResultKind::Composition {
                id,
                version,
                kind,
                manifest,
                source,
                links,
            } => {
                let value = vapor_studio_core::SourceComposition {
                    id,
                    version,
                    kind,
                    manifest: PathBuf::from(manifest),
                    source,
                    links: links
                        .into_iter()
                        .map(|link| vapor_studio_core::SourceCompositionLink {
                            id: link.id,
                            relationship: link.relationship,
                            optional: link.optional,
                            version: link.version,
                            workshop_id: link.workshop_id,
                            reason: link.reason,
                        })
                        .collect(),
                };
                let saved = self
                    .operation
                    .active_operation
                    .as_ref()
                    .is_some_and(|operation| {
                        matches!(
                            &operation.kind,
                            vapor_studio_core::OperationKind::SaveComposition
                        )
                    });
                if saved {
                    self.apply_source_composition_saved(value);
                } else {
                    self.apply_source_composition_loaded(value);
                }
            }
            AuthoringResultKind::WorkshopMetadata {
                id,
                version,
                kind,
                manifest,
                source,
                policy,
            } => {
                let value = vapor_studio_core::SourceWorkshopMetadata {
                    id,
                    version,
                    kind,
                    manifest: PathBuf::from(manifest),
                    source,
                    policy: vapor_studio_core::WorkshopPolicyDraft {
                        app_id: policy.app_id,
                        published_file_id: policy.published_file_id,
                        visibility: policy.visibility,
                        title: policy.title,
                        description: policy.description,
                        tags: policy.tags,
                        change_note: policy.change_note,
                    },
                };
                let saved = self
                    .operation
                    .active_operation
                    .as_ref()
                    .is_some_and(|operation| {
                        matches!(
                            &operation.kind,
                            vapor_studio_core::OperationKind::SaveWorkshopMetadata
                        )
                    });
                if saved {
                    self.apply_source_workshop_saved(value);
                } else {
                    self.apply_source_workshop_loaded(value);
                }
            }
            AuthoringResultKind::RuntimeOutputs {
                id,
                version,
                kind,
                manifest,
                source,
                binaries,
                libraries,
            } => {
                let value = vapor_studio_core::SourceRuntimeOutputs {
                    id,
                    version,
                    kind,
                    manifest: PathBuf::from(manifest),
                    source,
                    binaries,
                    libraries,
                };
                let saved = self
                    .operation
                    .active_operation
                    .as_ref()
                    .is_some_and(|operation| {
                        matches!(
                            &operation.kind,
                            vapor_studio_core::OperationKind::SaveRuntimeOutputs
                        )
                    });
                if saved {
                    self.apply_source_runtime_outputs_saved(value);
                } else {
                    self.apply_source_runtime_outputs_loaded(value);
                }
            }
            AuthoringResultKind::ContentValidated { id } => {
                self.status = format!("Validated {id} through the Studio service.");
            }
            AuthoringResultKind::ContentCreated {
                workspace_root,
                manifest,
                manifest_contents,
                files,
            } => {
                if PathBuf::from(self.workspace.root.trim()) != PathBuf::from(&workspace_root) {
                    self.status =
                        "Content was created, but the active workspace changed before Studio could open its manifest."
                            .to_owned();
                    self.finish_service_result_operation(&result.operation_id);
                    return;
                }
                self.apply_content_workspace_created(
                    PathBuf::from(manifest),
                    manifest_contents,
                    files.into_iter().map(PathBuf::from).collect(),
                );
            }
            AuthoringResultKind::WorkspaceCreated { root, files } => {
                self.activate_workspace(WorkspaceOpenSnapshot {
                    root: PathBuf::from(root),
                    landing_section: Section::Workspace,
                    workspace_files: Ok(files.into_iter().map(PathBuf::from).collect()),
                });
            }
        }
        self.finish_service_result_operation(&result.operation_id);
    }

    pub(super) fn apply_source_composition_loaded(
        &mut self,
        composition: vapor_studio_core::SourceComposition,
    ) {
        self.authoring.pending_composition_role_change = None;
        self.authoring.composition_link_candidate.clear();
        self.authoring.composition_link_search.clear();
        self.authoring.packagepack_engine_search.clear();
        self.authoring.packagepack_game_search.clear();
        self.authoring.composition_edit_selector = composition
            .version
            .as_deref()
            .map(|version| format!("{}@{version}", composition.id))
            .unwrap_or_else(|| composition.id.clone());
        self.authoring.composition_original_links = Some(composition.links.clone());
        self.authoring.composition_edit = Some(composition);
        self.status = format!(
            "Loaded dependencies for this {}. Review them before saving.",
            self.authoring
                .composition_edit
                .as_ref()
                .map(|graph| graph.kind.as_str())
                .unwrap_or("source content"),
        );
    }

    pub(super) fn apply_source_composition_saved(
        &mut self,
        composition: vapor_studio_core::SourceComposition,
    ) {
        self.invalidate_workshop_review();
        self.authoring.pending_composition_role_change = None;
        self.authoring.composition_link_candidate.clear();
        self.authoring.composition_link_search.clear();
        self.authoring.packagepack_engine_search.clear();
        self.authoring.packagepack_game_search.clear();
        let identity = ContentIdentity {
            id: composition.id.clone(),
            version: composition.version.clone(),
        };
        self.authoring.composition_original_links = Some(composition.links.clone());
        self.authoring.composition_edit = Some(composition);
        self.invalidate_profiles_for_source_content(&identity);
        self.status =
            "Saved the composition. Studio is refreshing its content catalog and plan.".to_owned();
    }

    pub(super) fn apply_source_workshop_loaded(
        &mut self,
        metadata: vapor_studio_core::SourceWorkshopMetadata,
    ) {
        self.set_workshop_metadata_fields(&metadata.policy);
        self.authoring.workshop_metadata_edit = Some(metadata);
        self.authoring.runtime_outputs_edit = None;
        self.authoring.manifest_metadata_dirty = false;
        self.authoring.pending_manifest_discard = None;
        self.status =
            "Loaded the source Workshop metadata. Edit the fields, then save through Vapor Shell."
                .to_owned();
    }

    pub(super) fn apply_source_workshop_saved(
        &mut self,
        metadata: vapor_studio_core::SourceWorkshopMetadata,
    ) {
        self.invalidate_workshop_review();
        self.set_workshop_metadata_fields(&metadata.policy);
        self.authoring.workshop_metadata_edit = Some(metadata.clone());
        self.authoring.manifest_metadata_dirty = false;
        self.authoring.pending_manifest_discard = None;
        self.invalidate_profiles_for_source_content(&ContentIdentity {
            id: metadata.id.clone(),
            version: metadata.version.clone(),
        });
        self.sync_saved_workspace_manifest(&metadata.manifest, &metadata.source);
        self.status = "Saved Workshop metadata through Vapor Shell. Studio is refreshing the workspace catalog.".to_owned();
    }

    pub(super) fn apply_source_runtime_outputs_loaded(
        &mut self,
        outputs: vapor_studio_core::SourceRuntimeOutputs,
    ) {
        self.set_runtime_output_fields(&outputs);
        self.authoring.runtime_outputs_edit = Some(outputs);
        self.authoring.workshop_metadata_edit = None;
        self.authoring.manifest_metadata_dirty = false;
        self.authoring.pending_manifest_discard = None;
        self.status = "Loaded runtime output declarations. Edit the executable and library lists, then save through Vapor Shell.".to_owned();
    }

    pub(super) fn apply_source_runtime_outputs_saved(
        &mut self,
        outputs: vapor_studio_core::SourceRuntimeOutputs,
    ) {
        self.invalidate_workshop_review();
        self.set_runtime_output_fields(&outputs);
        self.authoring.runtime_outputs_edit = Some(outputs.clone());
        self.authoring.manifest_metadata_dirty = false;
        self.authoring.pending_manifest_discard = None;
        self.invalidate_profiles_for_source_content(&ContentIdentity {
            id: outputs.id.clone(),
            version: outputs.version.clone(),
        });
        self.sync_saved_workspace_manifest(&outputs.manifest, &outputs.source);
        self.status = "Saved runtime outputs through Vapor Shell. Studio is refreshing the catalog and active profile.".to_owned();
    }
}
