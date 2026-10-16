use super::{StudioApp, catalog_content_selector, content_selector};
use crate::navigation::{CatalogSourceFilter, Section};
use std::path::PathBuf;
use vapor_studio_core::ContentIdentity;


pub(super) struct ProfileState {
    pub(super) items: Vec<vapor_studio_core::CompositionProfile>,
    pub(super) active_index: usize,
    pub(super) recovery: Option<String>,
    pub(super) recovery_backup: Option<String>,
    pub(super) name_draft: String,
    pub(super) confirm_removal: bool,
    pub(super) new_name: String,
}

impl ProfileState {
    pub(super) fn new(
        items: Vec<vapor_studio_core::CompositionProfile>,
        active_index: usize,
        name_draft: String,
        recovery: Option<String>,
        recovery_backup: Option<String>,
    ) -> Self {
        Self {
            items,
            active_index,
            recovery,
            recovery_backup,
            name_draft,
            confirm_removal: false,
            new_name: String::new(),
        }
    }
}

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
            .items
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
            self.catalog.selected = Some(catalog_content_selector(&content));
            self.catalog.source_filter = CatalogSourceFilter::Workshop;
            self.catalog.kind_filter = Some(vapor_core::ContentKind::Packagepack.as_str().to_owned());
            self.catalog.search_query = published_file_id.to_owned();
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
        if self.operation.running || index >= self.profile.items.len() {
            return;
        }
        if self.workspace.editor_dirty || self.authoring.manifest_metadata_dirty {
            self.status =
                "Save or discard workspace file or manifest metadata edits before switching profiles.".to_owned();
            return;
        }
        self.profile.active_index = index;
        self.profile.name_draft = self.profile.items[index].name.clone();
        self.profile.confirm_removal = false;
        self.workspace.root = self.profile.items[index]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.display().to_string())
            .unwrap_or_default();
        self.workspace.root_draft = if self.workspace.root.is_empty() {
            self.workspace.fallback_root.display().to_string()
        } else {
            self.workspace.root.clone()
        };
        self.authoring.clear_workspace_context();
        self.catalog.clear_workspace_content();
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.workspace.pending_unregistration = None;
        self.workspace.clear_file_context();
        self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.discover();
    }

    pub(super) fn assign_profile_root(
        &mut self,
        identity: ContentIdentity,
        workspace: Option<PathBuf>,
    ) {
        if self.authoring.manifest_metadata_dirty {
            self.status = "Save or discard structured manifest edits before changing the profile composition.".to_owned();
            return;
        }
        let current_workspace = self.profile.items[self.profile.active_index]
            .workspace
            .as_ref()
            .map(|association| association.root.clone());
        if self.catalog.items.iter().any(|content| {
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
        let profile = &mut self.profile.items[self.profile.active_index];
        profile.root_package = Some(identity.clone());
        profile.workspace = workspace.clone().map(|root| WorkspaceAssociation { root });
        profile.resolved_graph = None;
        profile.installation = vapor_studio_core::InstallationState::NotInstalled;
        if workspace_changed {
            self.authoring.clear_selected_source();
            self.profile_composition.clear_context();
            self.workspace.root = workspace
                .as_ref()
                .map(|root| root.display().to_string())
                .unwrap_or_default();
            self.workspace.root_draft = if self.workspace.root.is_empty() {
                self.workspace.fallback_root.display().to_string()
            } else {
                self.workspace.root.clone()
            };
            self.catalog.clear_workspace_content();
            self.workspace.clear_file_context();
            self.discover();
        } else {
            self.profile_composition.invalidate_preview();
        }
        self.status = format!(
            "Selected {} for profile {}. Preview its dependency plan before installing.",
            content_selector(&identity),
            self.profile.items[self.profile.active_index].name,
        );
        self.section = Section::Library;
    }

    pub(super) fn add_profile(&mut self) {
        let name = self.profile.new_name.trim();
        let workspace = (!self.workspace.root.trim().is_empty())
            .then(|| PathBuf::from(self.workspace.root.trim()));
        let index = match vapor_studio_core::add_profile(&mut self.profile.items, name, workspace)
        {
            Ok(index) => index,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        self.profile.active_index = index;
        self.profile.name_draft = self.profile.items[index].name.clone();
        self.profile.confirm_removal = false;
        self.profile.new_name.clear();
        self.status = format!(
            "Created profile {}.",
            self.profile.items[self.profile.active_index].name
        );
    }

    pub(super) fn rename_active_profile(&mut self) {
        match vapor_studio_core::rename_profile(
            &mut self.profile.items,
            self.profile.active_index,
            &self.profile.name_draft,
        ) {
            Ok(()) => {
                self.profile.name_draft = self.profile.items[self.profile.active_index].name.clone();
                self.status = format!("Renamed profile to {}.", self.profile.name_draft);
            }
            Err(error) => {
                self.profile.name_draft = self.profile.items[self.profile.active_index].name.clone();
                self.status = error;
            }
        }
    }

    pub(super) fn remove_active_profile(&mut self) {
        if self.operation.running || self.workspace.editor_dirty || self.authoring.manifest_metadata_dirty {
            self.status = "Finish or discard the current operation and edits before removing a profile.".to_owned();
            return;
        }
        let index = self.profile.active_index;
        let removed = match vapor_studio_core::remove_profile(&mut self.profile.items, index) {
            Ok(profile) => profile,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        self.profile.confirm_removal = false;
        self.profile.name_draft.clear();
        let next_index = index.min(self.profile.items.len() - 1);
        self.select_profile(next_index);
        self.status = format!("Removed profile {}.", removed.name);
    }

    pub(super) fn duplicate_active_profile(&mut self) {
        if self.operation.running || self.workspace.editor_dirty {
            return;
        }
        let source_name = self.profile.items[self.profile.active_index].name.clone();
        let index =
            match vapor_studio_core::duplicate_profile(&mut self.profile.items, self.profile.active_index) {
                Ok(index) => index,
                Err(error) => {
                    self.status = error;
                    return;
                }
            };
        self.select_profile(index);
        self.status = format!("Duplicated {source_name} as {}.", self.profile.items[index].name);
    }

 }
