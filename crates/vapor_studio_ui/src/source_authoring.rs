use super::operation_protocol::OperationResult;
use super::{StudioApp, source_content_selector};
use std::path::PathBuf;
use vapor_studio_core::{ContentIdentity, OperationKind};

pub(super) struct AuthoringState {
    pub(super) create: CreateDraft,
    pub(super) composition_edit_selector: String,
    pub(super) composition_edit: Option<vapor_studio_core::SourceComposition>,
    pub(super) composition_original_links: Option<Vec<vapor_studio_core::SourceCompositionLink>>,
    pub(super) composition_link_candidate: String,
    pub(super) workshop_metadata_edit: Option<vapor_studio_core::SourceWorkshopMetadata>,
    pub(super) manifest_metadata_dirty: bool,
    pub(super) runtime_outputs_edit: Option<vapor_studio_core::SourceRuntimeOutputs>,
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

pub(super) struct CreateDraft {
    pub(super) details_open: bool,
    pub(super) content_id: String,
    pub(super) content_version: String,
    pub(super) content_kind: vapor_core::ContentKind,
    pub(super) dependencies: Vec<String>,
    pub(super) workshop_app_id: String,
    pub(super) workshop_published_file_id: String,
    pub(super) workshop_visibility: String,
    pub(super) workshop_title: String,
    pub(super) workshop_description: String,
    pub(super) workshop_tags: String,
    pub(super) workshop_change_note: String,
}

impl Default for CreateDraft {
    fn default() -> Self {
        Self {
            details_open: false,
            content_id: String::new(),
            content_version: "0.1.0".to_owned(),
            content_kind: vapor_core::ContentKind::Packagepack,
            dependencies: Vec::new(),
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

impl Default for AuthoringState {
    fn default() -> Self {
        Self {
            create: CreateDraft::default(),
            composition_edit_selector: String::new(),
            composition_edit: None,
            composition_original_links: None,
            composition_link_candidate: String::new(),
            workshop_metadata_edit: None,
            manifest_metadata_dirty: false,
            runtime_outputs_edit: None,
            runtime_binaries_text: String::new(),
            runtime_libraries_text: String::new(),
            workshop_metadata_app_id: String::new(),
            workshop_metadata_published_file_id: String::new(),
            workshop_metadata_visibility: String::new(),
            workshop_metadata_title: String::new(),
            workshop_metadata_description: String::new(),
            workshop_metadata_tags: String::new(),
            workshop_metadata_change_note: String::new(),
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
        self.composition_edit = None;
        self.composition_original_links = None;
        self.composition_link_candidate.clear();
    }

    pub(super) fn clear_workspace_context(&mut self) {
        self.create.details_open = false;
        self.create.dependencies.clear();
        self.clear_selected_source();
        self.workshop_metadata_edit = None;
        self.runtime_outputs_edit = None;
        self.manifest_metadata_dirty = false;
    }
}

impl CreateDraft {
    pub(super) fn is_dirty(&self) -> bool {
        !self.content_id.is_empty()
            || self.content_version != "0.1.0"
            || self.content_kind != vapor_core::ContentKind::Packagepack
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
}

impl StudioApp {
    pub(super) fn discard_source_composition_changes(&mut self) {
        let Some(original_links) = self.authoring.composition_original_links.clone() else {
            return;
        };
        let Some(composition) = self.authoring.composition_edit.as_mut() else {
            return;
        };
        composition.links = original_links;
        self.authoring.composition_link_candidate.clear();
        self.status = "Discarded unsaved dependency changes. The saved composition is unchanged.".to_owned();
    }

    pub(super) fn load_source_composition(&mut self) {
        if !self.require_open_workspace("edit source content dependencies") {
            return;
        }
        let selector = self.authoring.composition_edit_selector.trim().to_owned();
        if selector.is_empty() {
            self.status = "Select a source content project first.".to_owned();
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let operation_selector = selector.clone();
        self.spawn(OperationKind::LoadComposition, move |_context| {
            OperationResult::SourceCompositionLoaded(vapor_studio_core::load_source_composition(
                &executable,
                &root,
                &operation_selector,
            ))
        });
        self.status = format!("Loading dependencies for {selector} through Vapor Shell…");
    }

    pub(super) fn save_source_composition(&mut self) {
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
        let root = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::SaveComposition, move |_context| {
            OperationResult::SourceCompositionSaved(vapor_studio_core::save_source_composition(
                &executable,
                &root,
                &selector,
                &composition.source,
                &composition.links,
            ))
        });
        self.status =
            "Validating and saving composition dependencies through Vapor Shell…".to_owned();
    }

    pub(super) fn load_source_workshop_metadata(
        &mut self,
        content: &vapor_studio_core::SourceContent,
    ) {
        if !self.require_open_workspace("edit Workshop metadata") {
            return;
        }
        // Both metadata editors share one dirty-state slot and write the same
        // role manifest. Keep only one editor live at a time.
        self.authoring.runtime_outputs_edit = None;
        let selector = source_content_selector(content);
        let root = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let operation_selector = selector.clone();
        self.spawn(OperationKind::LoadWorkshopMetadata, move |_context| {
            OperationResult::SourceWorkshopLoaded(vapor_studio_core::load_source_workshop_metadata(
                &executable,
                &root,
                &operation_selector,
            ))
        });
        self.status = format!("Loading Workshop metadata for {selector} through Vapor Shell…");
    }

    pub(super) fn load_source_runtime_outputs(
        &mut self,
        content: &vapor_studio_core::SourceContent,
    ) {
        if !self.require_open_workspace("edit runtime output declarations") {
            return;
        }
        self.authoring.workshop_metadata_edit = None;
        let selector = source_content_selector(content);
        let root = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let operation_selector = selector.clone();
        self.spawn(OperationKind::LoadRuntimeOutputs, move |_context| {
            OperationResult::SourceRuntimeOutputsLoaded(
                vapor_studio_core::load_source_runtime_outputs(
                    &executable,
                    &root,
                    &operation_selector,
                ),
            )
        });
        self.status =
            format!("Loading runtime output declarations for {selector} through Vapor Shell…");
    }

    pub(super) fn save_source_runtime_outputs(&mut self) {
        if self.authoring.composition_has_unsaved_changes() {
            self.status = "Save or discard dependency changes before saving runtime outputs.".to_owned();
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
        let root = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let operation_selector = selector.clone();
        self.spawn(OperationKind::SaveRuntimeOutputs, move |_context| {
            OperationResult::SourceRuntimeOutputsSaved(
                vapor_studio_core::save_source_runtime_outputs(
                    &executable,
                    &root,
                    &operation_selector,
                    &expected_source,
                    &binaries,
                    &libraries,
                ),
            )
        });
        self.status =
            format!("Validating and saving runtime outputs for {selector} through Vapor Shell…");
    }

    pub(super) fn set_runtime_output_fields(
        &mut self,
        outputs: &vapor_studio_core::SourceRuntimeOutputs,
    ) {
        self.authoring.runtime_binaries_text = outputs.binaries.join("\n");
        self.authoring.runtime_libraries_text = outputs.libraries.join("\n");
    }

    pub(super) fn invalidate_profiles_for_source_content(&mut self, identity: &ContentIdentity) {
        self.invalidate_workshop_review();
        for (index, profile) in self.profile.items.iter_mut().enumerate() {
            let affected = profile
                .root_package
                .as_ref()
                .is_some_and(|root| root == identity)
                || profile
                    .resolved_graph
                    .as_ref()
                    .is_some_and(|graph| graph.nodes.iter().any(|node| &node.content == identity));
            if affected {
                profile.resolved_graph = None;
                profile.installation = vapor_studio_core::InstallationState::NotInstalled;
                if index == self.profile.active_index {
                    self.profile_composition
                        .request_repreview_after_catalog_refresh();
                }
            }
        }
    }

    pub(super) fn save_source_workshop_metadata(&mut self) {
        if self.authoring.composition_has_unsaved_changes() {
            self.status = "Save or discard dependency changes before saving Workshop metadata.".to_owned();
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
                    self.status = "Steam AppID must be a positive number.".to_owned();
                    return;
                }
            },
        };
        let non_empty = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        let policy = vapor_studio_core::WorkshopPolicyDraft {
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
        let root = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let operation_selector = selector.clone();
        self.spawn(OperationKind::SaveWorkshopMetadata, move |_context| {
            OperationResult::SourceWorkshopSaved(vapor_studio_core::save_source_workshop_metadata(
                &executable,
                &root,
                &operation_selector,
                &expected_source,
                &policy,
            ))
        });
        self.status =
            format!("Validating and saving Workshop metadata for {selector} through Vapor Shell…");
    }

    pub(super) fn set_workshop_metadata_fields(
        &mut self,
        policy: &vapor_studio_core::WorkshopPolicyDraft,
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

    pub(super) fn use_edited_composition_for_profile(&mut self) {
        let Some(composition) = self.authoring.composition_edit.as_ref() else {
            self.status = "Load a source Packagepack first.".to_owned();
            return;
        };
        if composition.kind != vapor_core::ContentKind::Packagepack.as_str() {
            self.status = "Only a Packagepack can be selected as a launch profile root.".to_owned();
            return;
        }
        let root = ContentIdentity {
            id: composition.id.clone(),
            version: composition.version.clone(),
        };
        if self.assign_profile_root(root, Some(PathBuf::from(self.workspace.root.trim()))) {
            self.preview_selected_composition();
        }
    }
}
