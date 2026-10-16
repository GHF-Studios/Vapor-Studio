use super::{StudioApp, catalog_content_selector, content_selector};
use crate::navigation::{CatalogSourceFilter, Section};
use std::path::PathBuf;
use vapor_studio_core::ContentIdentity;

impl StudioApp {
    pub(super) fn use_workshop_as_profile_root(&mut self) {
        let Some(acquisition) = self.workshop.pending_acquisition.as_ref() else {
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

    pub(super) fn select_profile(&mut self, index: usize) {
        if self.running || index >= self.profiles.len() {
            return;
        }
        if self.workspace.editor_dirty || self.manifest_metadata_dirty {
            self.status =
                "Save or discard workspace file or manifest metadata edits before switching profiles.".to_owned();
            return;
        }
        self.active_profile = index;
        self.profile_name_draft = self.profiles[index].name.clone();
        self.confirm_profile_removal = false;
        self.workspace.root = self.profiles[index]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.display().to_string())
            .unwrap_or_default();
        self.workspace.root_draft = if self.workspace.root.is_empty() {
            self.workspace.fallback_root.display().to_string()
        } else {
            self.workspace.root.clone()
        };
        self.draft_dependencies.clear();
        self.contents.clear();
        self.catalog.clear();
        self.selected_catalog_content = None;
        self.composition_preview = None;
        self.composition_edit = None;
        self.composition_original_links = None;
        self.composition_edit_selector.clear();
        self.composition_link_candidate.clear();
        self.repreview_after_catalog_refresh = false;
        self.invalidate_workshop_review();
        self.workspace.pending_unregistration = None;
        self.workshop_metadata_edit = None;
        self.runtime_outputs_edit = None;
        self.manifest_metadata_dirty = false;
        self.workspace.files.clear();
        self.workspace.selected_file = None;
        self.workspace.editor.clear();
        self.workspace.editor_original.clear();
        self.workspace.editor_toml_diagnostic = None;
        self.workspace.editor_dirty = false;
        self.workspace.files_open = false;
        self.workspace.new_file_path.clear();
        self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.discover();
    }

    pub(super) fn assign_profile_root(
        &mut self,
        identity: ContentIdentity,
        workspace: Option<PathBuf>,
    ) {
        if self.manifest_metadata_dirty {
            self.status = "Save or discard structured manifest edits before changing the profile composition.".to_owned();
            return;
        }
        let current_workspace = self.profiles[self.active_profile]
            .workspace
            .as_ref()
            .map(|association| association.root.clone());
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
        if self.workspace.editor_dirty && current_workspace.as_deref() != workspace.as_deref() {
            self.status =
                "Save or discard workspace edits before assigning content from another workspace."
                    .to_owned();
            return;
        }
        let workspace_changed = current_workspace != workspace;
        let profile = &mut self.profiles[self.active_profile];
        profile.root_package = Some(identity.clone());
        profile.workspace = workspace.clone().map(|root| WorkspaceAssociation { root });
        profile.resolved_graph = None;
        profile.installation = vapor_studio_core::InstallationState::NotInstalled;
        self.composition_preview = None;
        if workspace_changed {
            self.composition_edit = None;
            self.composition_original_links = None;
            self.composition_edit_selector.clear();
            self.composition_link_candidate.clear();
            self.repreview_after_catalog_refresh = false;
            self.workspace.root = workspace
                .as_ref()
                .map(|root| root.display().to_string())
                .unwrap_or_default();
            self.workspace.root_draft = if self.workspace.root.is_empty() {
                self.workspace.fallback_root.display().to_string()
            } else {
                self.workspace.root.clone()
            };
            self.contents.clear();
            self.catalog.clear();
            self.selected_catalog_content = None;
            self.workspace.files.clear();
            self.workspace.files_open = false;
            self.workspace.selected_file = None;
            self.workspace.editor.clear();
            self.workspace.editor_original.clear();
            self.workspace.editor_toml_diagnostic = None;
            self.workspace.editor_dirty = false;
            self.workspace.new_file_path.clear();
            self.discover();
        }
        self.status = format!(
            "Selected {} for profile {}. Preview its dependency plan before installing.",
            content_selector(&identity),
            self.profiles[self.active_profile].name,
        );
        self.section = Section::Library;
    }

    pub(super) fn add_profile(&mut self) {
        let name = self.new_profile_name.trim();
        let workspace = (!self.workspace.root.trim().is_empty())
            .then(|| PathBuf::from(self.workspace.root.trim()));
        let index = match vapor_studio_core::add_profile(&mut self.profiles, name, workspace)
        {
            Ok(index) => index,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        self.active_profile = index;
        self.profile_name_draft = self.profiles[index].name.clone();
        self.confirm_profile_removal = false;
        self.new_profile_name.clear();
        self.status = format!(
            "Created profile {}.",
            self.profiles[self.active_profile].name
        );
    }

    pub(super) fn rename_active_profile(&mut self) {
        match vapor_studio_core::rename_profile(
            &mut self.profiles,
            self.active_profile,
            &self.profile_name_draft,
        ) {
            Ok(()) => {
                self.profile_name_draft = self.profiles[self.active_profile].name.clone();
                self.status = format!("Renamed profile to {}.", self.profile_name_draft);
            }
            Err(error) => {
                self.profile_name_draft = self.profiles[self.active_profile].name.clone();
                self.status = error;
            }
        }
    }

    pub(super) fn remove_active_profile(&mut self) {
        if self.running || self.workspace.editor_dirty || self.manifest_metadata_dirty {
            self.status = "Finish or discard the current operation and edits before removing a profile.".to_owned();
            return;
        }
        let index = self.active_profile;
        let removed = match vapor_studio_core::remove_profile(&mut self.profiles, index) {
            Ok(profile) => profile,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        self.confirm_profile_removal = false;
        self.profile_name_draft.clear();
        let next_index = index.min(self.profiles.len() - 1);
        self.select_profile(next_index);
        self.status = format!("Removed profile {}.", removed.name);
    }

    pub(super) fn duplicate_active_profile(&mut self) {
        if self.running || self.workspace.editor_dirty {
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

 }
