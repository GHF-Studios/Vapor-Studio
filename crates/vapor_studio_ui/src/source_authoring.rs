use super::{StudioApp, source_content_kind, source_content_selector};
use std::path::PathBuf;
use vapor_client_application::{ContentReference, OperationKind};
use vapor_client_protocol::{AuthoringAction, ClientAction, CompositionDependency};

#[derive(Clone, Copy)]
pub(super) enum PendingManifestDraftDiscard {
    WorkshopMetadata,
    RuntimeOutputs,
}

#[derive(Default)]
pub(super) struct AuthoringState {
    pub(super) create: CreateDraft,
    pub(super) composition_discard_confirmation_open: bool,
    pub(super) pending_composition_role_change: Option<PendingCompositionRoleChange>,
    pub(super) composition_edit_selector: String,
    pub(super) composition_source_search: String,
    pub(super) composition_edit: Option<vapor_client_application::SourceComposition>,
    pub(super) composition_original_links:
        Option<Vec<vapor_client_application::SourceCompositionLink>>,
    pub(super) composition_link_candidate: String,
    pub(super) composition_link_search: String,
    pub(super) pending_composition_add: Option<ContentReference>,
    pub(super) packagepack_engine_search: String,
    pub(super) packagepack_game_search: String,
    pub(super) workshop_metadata_edit: Option<vapor_client_application::SourceWorkshopMetadata>,
    pub(super) manifest_metadata_dirty: bool,
    pub(super) pending_manifest_discard: Option<PendingManifestDraftDiscard>,
    pub(super) runtime_outputs_edit: Option<vapor_client_application::SourceRuntimeOutputs>,
    pub(super) runtime_binaries_text: String,
    pub(super) runtime_libraries_text: String,
    pub(super) workshop_metadata_app_id: String,
    pub(super) workshop_metadata_published_file_id: String,
    pub(super) workshop_metadata_visibility: String,
    pub(super) workshop_metadata_title: String,
    pub(super) workshop_metadata_description: String,
    pub(super) workshop_metadata_tags: String,
    pub(super) workshop_metadata_change_note: String,
}

#[derive(Clone)]
pub(super) struct PendingCompositionRoleChange {
    pub(super) group: &'static str,
    pub(super) previous: Vec<vapor_client_application::SourceCompositionLink>,
    pub(super) proposed: Option<vapor_client_application::SourceCompositionLink>,
}

pub(super) struct CreateDraft {
    pub(super) details_open: bool,
    pub(super) discard_confirmation_open: bool,
    pub(super) pending_template_change: Option<vapor_client_protocol::ContentKind>,
    pub(super) pending_dependency_clear: bool,
    pub(super) pending_role_replacement: Option<PendingRoleReplacement>,
    pub(super) dependency_search: String,
    pub(super) content_id: String,
    pub(super) content_version: String,
    pub(super) content_kind: vapor_client_protocol::ContentKind,
    pub(super) dependencies: Vec<String>,
    pub(super) optional_dependencies: std::collections::BTreeSet<String>,
    pub(super) exclusive_dependency_groups: std::collections::BTreeMap<String, &'static str>,
    pub(super) workshop_app_id: String,
    pub(super) workshop_published_file_id: String,
    pub(super) workshop_visibility: String,
    pub(super) workshop_title: String,
    pub(super) workshop_description: String,
    pub(super) workshop_tags: String,
    pub(super) workshop_change_note: String,
}

#[derive(Clone)]
pub(super) struct PendingRoleReplacement {
    pub(super) target: String,
    pub(super) replacing: Vec<String>,
    pub(super) group: &'static str,
}

impl Default for CreateDraft {
    fn default() -> Self {
        Self {
            details_open: false,
            discard_confirmation_open: false,
            pending_template_change: None,
            pending_dependency_clear: false,
            pending_role_replacement: None,
            dependency_search: String::new(),
            content_id: String::new(),
            content_version: "0.1.0".to_owned(),
            content_kind: vapor_client_protocol::ContentKind::Packagepack,
            dependencies: Vec::new(),
            optional_dependencies: std::collections::BTreeSet::new(),
            exclusive_dependency_groups: Default::default(),
            workshop_app_id: String::new(),
            workshop_published_file_id: String::new(),
            workshop_visibility: String::new(),
            workshop_title: String::new(),
            workshop_description: String::new(),
            workshop_tags: String::new(),
            workshop_change_note: String::new(),
        }
    }
}

impl AuthoringState {
    pub(super) fn composition_has_unsaved_changes(&self) -> bool {
        self.composition_edit
            .as_ref()
            .zip(self.composition_original_links.as_ref())
            .is_some_and(|(composition, original)| &composition.links != original)
    }

    pub(super) fn clear_selected_source(&mut self) {
        self.composition_edit_selector.clear();
        self.composition_discard_confirmation_open = false;
        self.pending_composition_role_change = None;
        self.composition_source_search.clear();
        self.composition_edit = None;
        self.composition_original_links = None;
        self.composition_link_candidate.clear();
        self.composition_link_search.clear();
        self.packagepack_engine_search.clear();
        self.packagepack_game_search.clear();
    }

    pub(super) fn clear_workspace_context(&mut self) {
        self.create.details_open = false;
        self.create.discard_confirmation_open = false;
        self.create.pending_template_change = None;
        self.create.pending_dependency_clear = false;
        self.create.pending_role_replacement = None;
        self.create.dependency_search.clear();
        self.create.clear_dependencies();
        self.clear_selected_source();
        self.workshop_metadata_edit = None;
        self.runtime_outputs_edit = None;
        self.manifest_metadata_dirty = false;
        self.pending_manifest_discard = None;
        self.pending_composition_add = None;
    }
}

impl CreateDraft {
    pub(super) fn is_dirty(&self) -> bool {
        !self.content_id.is_empty()
            || self.content_version != "0.1.0"
            || self.content_kind != vapor_client_protocol::ContentKind::Packagepack
            || !self.dependencies.is_empty()
            || !self.workshop_app_id.is_empty()
            || !self.workshop_published_file_id.is_empty()
            || !self.workshop_visibility.is_empty()
            || !self.workshop_title.is_empty()
            || !self.workshop_description.is_empty()
            || !self.workshop_tags.is_empty()
            || !self.workshop_change_note.is_empty()
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn request_dependency_selection(
        &mut self,
        id: String,
        role_group: Option<&'static str>,
    ) {
        if self.dependencies.contains(&id) {
            return;
        }
        let Some(role_group) = role_group else {
            self.dependencies.push(id);
            return;
        };
        let replacing = self
            .dependencies
            .iter()
            .filter(|selected| self.exclusive_dependency_groups.get(*selected) == Some(&role_group))
            .cloned()
            .collect::<Vec<_>>();
        if replacing.is_empty() {
            self.dependencies.push(id.clone());
            self.exclusive_dependency_groups.insert(id, role_group);
        } else {
            self.pending_role_replacement = Some(PendingRoleReplacement {
                target: id,
                replacing,
                group: role_group,
            });
        }
    }

    pub(super) fn remove_dependency(&mut self, id: &str) {
        self.dependencies.retain(|selected| selected != id);
        self.optional_dependencies.remove(id);
        self.exclusive_dependency_groups.remove(id);
    }

    pub(super) fn clear_dependencies(&mut self) {
        self.dependencies.clear();
        self.optional_dependencies.clear();
        self.exclusive_dependency_groups.clear();
    }

    pub(super) fn resolve_role_replacement(&mut self, replace: bool) {
        let Some(replacement) = self.pending_role_replacement.take() else {
            return;
        };
        if !replace {
            return;
        }
        self.dependencies
            .retain(|selected| !replacement.replacing.iter().any(|old| old == selected));
        for replaced in &replacement.replacing {
            self.exclusive_dependency_groups.remove(replaced);
        }
        if !self.dependencies.contains(&replacement.target) {
            self.dependencies.push(replacement.target.clone());
        }
        self.exclusive_dependency_groups
            .insert(replacement.target, replacement.group);
    }
}

impl StudioApp {
    pub(super) fn apply_pending_composition_add(&mut self) {
        let Some(identity) = self.authoring.pending_composition_add.take() else {
            return;
        };
        let Some(composition) = self.authoring.composition_edit.as_mut() else {
            self.status = "The selected project could not be opened for adding content.".to_owned();
            return;
        };
        let active = self.composition.items.get(self.composition.active_index);
        if active
            .and_then(|active| active.root_package.as_ref())
            .is_none_or(|root| root.id != composition.id || root.version != composition.version)
        {
            self.status =
                "The selected composition changed. Choose it again before adding content."
                    .to_owned();
            return;
        }
        let candidate =
            self.catalog.sources.iter().find(|candidate| {
                candidate.id == identity.id && candidate.version == identity.version
            });
        let Some(candidate) = candidate else {
            self.status = format!(
                "{} is no longer available in this project folder. Refresh the project list and try again.",
                identity.id
            );
            return;
        };
        let Some(parent_kind) = source_content_kind(&composition.kind) else {
            self.status = "The selected project has an unsupported content type.".to_owned();
            return;
        };
        let Some(child_kind) = source_content_kind(&candidate.kind) else {
            self.status = "The selected content has an unsupported content type.".to_owned();
            return;
        };
        let Some(relationship) =
            vapor_client_domain::dependency_relationship(parent_kind, child_kind)
        else {
            self.status = format!("{} cannot be added to this kind of project.", candidate.id);
            return;
        };
        if composition
            .links
            .iter()
            .any(|link| link.id == candidate.id && link.relationship == relationship)
        {
            self.authoring.composition_link_candidate = source_content_selector(candidate);
            self.status = format!("{} is already included in this project.", candidate.id);
            return;
        }
        composition
            .links
            .push(vapor_client_application::SourceCompositionLink {
                id: candidate.id.clone(),
                relationship: relationship.to_owned(),
                optional: false,
                version: candidate
                    .version
                    .as_ref()
                    .map(|version| format!("={version}")),
                workshop_id: None,
                reason: None,
            });
        self.authoring.composition_link_candidate = source_content_selector(candidate);
        self.status = format!(
            "Added {} to the project draft. Save the content choices to apply it.",
            candidate.id
        );
    }

    pub(super) fn discard_source_composition_changes(&mut self) {
        let Some(original_links) = self.authoring.composition_original_links.clone() else {
            return;
        };
        let Some(composition) = self.authoring.composition_edit.as_mut() else {
            return;
        };
        composition.links = original_links;
        self.authoring.composition_discard_confirmation_open = false;
        self.authoring.composition_link_candidate.clear();
        self.status =
            "Discarded unsaved dependency changes. The saved composition is unchanged.".to_owned();
    }

    pub(super) fn load_source_composition(&mut self) {
        if self.authoring.pending_composition_role_change.is_some() {
            self.status = "Confirm or keep the pending game engine or game choice before loading another project.".to_owned();
            return;
        }
        if !self.require_open_workspace("edit source content dependencies") {
            return;
        }
        let selector = self.authoring.composition_edit_selector.trim().to_owned();
        if selector.is_empty() {
            self.status = "Select a source content project first.".to_owned();
            return;
        }
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::LoadComposition {
                project_id: selector.clone(),
            }),
            None,
            OperationKind::LoadComposition,
            vapor_client_application::OperationPhase::Resolving,
            &format!("Loading dependencies for {selector}…"),
        );
    }

    pub(super) fn save_source_composition(&mut self) {
        if self.authoring.pending_composition_role_change.is_some() {
            self.status = "Confirm or keep the pending game engine or game choice before saving this project.".to_owned();
            return;
        }
        if self.authoring.manifest_metadata_dirty {
            self.status = "Save or discard the open manifest metadata draft before saving dependency changes.".to_owned();
            return;
        }
        if !self.require_open_workspace_for_authoring_save("save a source content composition") {
            return;
        }
        let Some(composition) = self.authoring.composition_edit.clone() else {
            self.status = "Load a source content project before saving.".to_owned();
            return;
        };
        let selector = composition
            .version
            .as_deref()
            .map(|version| format!("{}@{version}", composition.id))
            .unwrap_or_else(|| composition.id.clone());
        let dependencies = composition
            .links
            .iter()
            .map(|link| CompositionDependency {
                project_id: link.id.clone(),
                relationship: link.relationship.clone(),
                version_requirement: link.version.clone(),
                workshop_id: link.workshop_id.clone(),
                optional: link.optional,
                reason: link.reason.clone(),
            })
            .collect();
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::SaveComposition {
                project_id: selector,
                expected_fingerprint: composition.source,
                dependencies,
            }),
            None,
            OperationKind::SaveComposition,
            vapor_client_application::OperationPhase::Building,
            "Checking and saving composition dependencies…",
        );
    }

    pub(super) fn load_source_workshop_metadata(
        &mut self,
        content: &vapor_client_application::SourceContent,
    ) {
        if !self.require_open_workspace("edit Workshop metadata") {
            return;
        }
        // Both metadata editors share one dirty-state slot and write the same
        // role manifest. Keep only one editor live at a time.
        self.authoring.runtime_outputs_edit = None;
        let selector = source_content_selector(content);
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::LoadWorkshopMetadata {
                project_id: selector.clone(),
            }),
            None,
            OperationKind::LoadWorkshopMetadata,
            vapor_client_application::OperationPhase::Resolving,
            &format!("Loading Workshop metadata for {selector}…"),
        );
    }

    pub(super) fn load_source_runtime_outputs(
        &mut self,
        content: &vapor_client_application::SourceContent,
    ) {
        if !self.require_open_workspace("edit runtime output declarations") {
            return;
        }
        self.authoring.workshop_metadata_edit = None;
        let selector = source_content_selector(content);
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::LoadRuntimeOutputs {
                project_id: selector.clone(),
            }),
            None,
            OperationKind::LoadRuntimeOutputs,
            vapor_client_application::OperationPhase::Resolving,
            &format!("Loading runtime output declarations for {selector}…"),
        );
    }

    pub(super) fn save_source_runtime_outputs(&mut self) {
        if self.authoring.composition_has_unsaved_changes() {
            self.status =
                "Save or discard dependency changes before saving runtime outputs.".to_owned();
            return;
        }
        if !self.require_open_workspace_for_authoring_save("save runtime output declarations") {
            return;
        }
        let Some(outputs) = self.authoring.runtime_outputs_edit.clone() else {
            self.status = "Load a source content project's runtime outputs first.".to_owned();
            return;
        };
        let parse_names = |text: &str| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        };
        let binaries = parse_names(&self.authoring.runtime_binaries_text);
        let libraries = parse_names(&self.authoring.runtime_libraries_text);
        let selector = outputs
            .version
            .as_deref()
            .map(|version| format!("{}@{version}", outputs.id))
            .unwrap_or_else(|| outputs.id.clone());
        let expected_source = outputs.source;
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::SaveRuntimeOutputs {
                project_id: selector,
                expected_fingerprint: expected_source,
                binaries,
                libraries,
            }),
            None,
            OperationKind::SaveRuntimeOutputs,
            vapor_client_application::OperationPhase::Building,
            "Checking and saving runtime file declarations…",
        );
    }

    pub(super) fn set_runtime_output_fields(
        &mut self,
        outputs: &vapor_client_application::SourceRuntimeOutputs,
    ) {
        self.authoring.runtime_binaries_text = outputs.binaries.join("\n");
        self.authoring.runtime_libraries_text = outputs.libraries.join("\n");
    }

    pub(super) fn invalidate_compositions_for_source_content(
        &mut self,
        identity: &ContentReference,
    ) {
        self.invalidate_workshop_review();
        let active_composition_affected = self
            .composition
            .items
            .get(self.composition.active_index)
            .is_some_and(|composition| {
                composition
                    .root_package
                    .as_ref()
                    .is_some_and(|root| root == identity)
                    || composition.resolved_graph.as_ref().is_some_and(|graph| {
                        graph.nodes.iter().any(|node| &node.content == identity)
                    })
            });
        if active_composition_affected {
            self.composition_preview
                .request_repreview_after_catalog_refresh();
        }
        if let Err(error) = self.apply_service_action(ClientAction::Composition(
            vapor_client_protocol::CompositionAction::InvalidateContent {
                content_id: identity.id.clone(),
                content_version: identity.version.clone(),
            },
        )) {
            self.status = error;
        }
    }

    pub(super) fn save_source_workshop_metadata(&mut self) {
        if self.authoring.composition_has_unsaved_changes() {
            self.status =
                "Save or discard dependency changes before saving Workshop metadata.".to_owned();
            return;
        }
        if !self.require_open_workspace_for_authoring_save("save Workshop metadata") {
            return;
        }
        let Some(metadata) = self.authoring.workshop_metadata_edit.clone() else {
            self.status = "Load a source content project's Workshop metadata first.".to_owned();
            return;
        };
        let app_id = match self.authoring.workshop_metadata_app_id.trim() {
            "" => None,
            value => match value.parse::<u32>() {
                Ok(value) if value > 0 => Some(value),
                _ => {
                    self.status = "Steam app ID must be a positive number.".to_owned();
                    return;
                }
            },
        };
        let non_empty = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        let policy = vapor_client_application::WorkshopPolicyDraft {
            app_id,
            published_file_id: non_empty(&self.authoring.workshop_metadata_published_file_id),
            visibility: non_empty(&self.authoring.workshop_metadata_visibility),
            title: non_empty(&self.authoring.workshop_metadata_title),
            description: non_empty(&self.authoring.workshop_metadata_description),
            tags: self
                .authoring
                .workshop_metadata_tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            change_note: non_empty(&self.authoring.workshop_metadata_change_note),
        };
        let selector = metadata
            .version
            .as_deref()
            .map(|version| format!("{}@{version}", metadata.id))
            .unwrap_or_else(|| metadata.id.clone());
        let expected_source = metadata.source;
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::SaveWorkshopMetadata {
                project_id: selector,
                expected_fingerprint: expected_source,
                app_id: policy.app_id,
                published_file_id: policy.published_file_id,
                visibility: policy.visibility,
                title: policy.title,
                description: policy.description,
                tags: policy.tags,
                change_note: policy.change_note,
            }),
            None,
            OperationKind::SaveWorkshopMetadata,
            vapor_client_application::OperationPhase::Building,
            "Checking and saving Workshop details…",
        );
    }

    pub(super) fn set_workshop_metadata_fields(
        &mut self,
        policy: &vapor_client_application::WorkshopPolicyDraft,
    ) {
        self.authoring.workshop_metadata_app_id = policy
            .app_id
            .map(|value| value.to_string())
            .unwrap_or_default();
        self.authoring.workshop_metadata_published_file_id =
            policy.published_file_id.clone().unwrap_or_default();
        self.authoring.workshop_metadata_visibility = policy.visibility.clone().unwrap_or_default();
        self.authoring.workshop_metadata_title = policy.title.clone().unwrap_or_default();
        self.authoring.workshop_metadata_description =
            policy.description.clone().unwrap_or_default();
        self.authoring.workshop_metadata_tags = policy.tags.join(", ");
        self.authoring.workshop_metadata_change_note =
            policy.change_note.clone().unwrap_or_default();
    }

    pub(super) fn use_edited_composition_for_composition(&mut self) {
        let Some(composition) = self.authoring.composition_edit.as_ref() else {
            self.status = "Load a playable composition first.".to_owned();
            return;
        };
        if composition.kind != vapor_client_protocol::ContentKind::Packagepack.as_str() {
            self.status = "Only a playable Packagepack can be added to a composition.".to_owned();
            return;
        }
        let root = ContentReference {
            id: composition.id.clone(),
            version: composition.version.clone(),
        };
        if self.assign_composition_root(root, Some(PathBuf::from(self.workspace.root.trim()))) {
            self.preview_selected_composition();
        }
    }
}
