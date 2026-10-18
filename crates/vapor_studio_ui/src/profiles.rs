use super::{StudioApp, catalog_content_selector, content_selector};
use crate::navigation::{CatalogSourceFilter, Section};
use std::path::{Path, PathBuf};
use vapor_studio_core::ContentIdentity;
use vapor_studio_protocol::StudioAction;

pub(super) struct ProfileState {
    pub(super) items: Vec<vapor_studio_core::CompositionProfile>,
    pub(super) service_revision: Option<u64>,
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
            service_revision: None,
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
    pub(super) fn is_active_profile_root(&self, id: &str, version: Option<&str>) -> bool {
        self.profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.root_package.as_ref())
            .is_some_and(|root| root.id == id && root.version.as_deref() == version)
    }

    pub(super) fn use_workshop_as_profile_root(&mut self, content_id: &str) -> bool {
        self.assign_workshop_acquisition_to_profile(content_id, false)
    }

    pub(super) fn launch_workshop_acquisition(&mut self, content_id: &str) -> bool {
        self.assign_workshop_acquisition_to_profile(content_id, true)
    }

    fn assign_workshop_acquisition_to_profile(
        &mut self,
        content_id: &str,
        launch_after_assignment: bool,
    ) -> bool {
        let Some(acquisition) = self
            .workshop
            .recent_acquisitions
            .iter()
            .find(|acquisition| acquisition.id == content_id)
        else {
            self.status = "This Workshop download is no longer in the recent imports list. Import it again to use it.".to_owned();
            return false;
        };
        if acquisition.kind != "packagepack" {
            self.status = format!(
                "{} is a {}, so it cannot be a composition root.",
                acquisition.id, acquisition.kind
            );
            return false;
        }
        let identity = ContentIdentity {
            id: acquisition.id.clone(),
            version: acquisition.version.clone(),
        };
        let assigned = if launch_after_assignment {
            self.assign_profile_root_without_catalog_refresh(identity, None)
        } else {
            self.assign_profile_root(identity, None)
        };
        if assigned {
            if launch_after_assignment {
                self.run_installed_selected();
            } else {
                self.activate_navigation_section(Section::Library);
            }
        }
        assigned
    }

    pub(super) fn use_available_workshop_package_as_profile_root(
        &mut self,
        published_file_id: &str,
    ) -> bool {
        self.assign_available_workshop_package_to_profile(published_file_id, false)
    }

    pub(super) fn launch_workshop_package_as_profile_root(
        &mut self,
        published_file_id: &str,
    ) -> bool {
        self.assign_available_workshop_package_to_profile(published_file_id, true)
    }

    fn assign_available_workshop_package_to_profile(
        &mut self,
        published_file_id: &str,
        launch_after_assignment: bool,
    ) -> bool {
        if !self.catalog.local_catalog_current() {
            self.status = if launch_after_assignment {
                "Refresh the on-device content list before launching this Workshop game setup."
            } else {
                "Refresh the on-device content list before selecting this Workshop game setup."
            }
            .to_owned();
            return false;
        }
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
                "Workshop item {published_file_id} is available here, but it is not a playable game setup."
            );
            return false;
        };
        if matches.len() > 1 {
            self.select_catalog_identity(catalog_content_selector(&content));
            self.catalog.source_filter = CatalogSourceFilter::Workshop;
            self.catalog.kind_filter =
                Some(vapor_core::ContentKind::Packagepack.as_str().to_owned());
            self.catalog.search_query = published_file_id.to_owned();
            self.open_discover_tab(super::catalog::DiscoverTab::Local);
            self.status = if launch_after_assignment {
                format!(
                    "Workshop item {published_file_id} has multiple game-setup versions here. Choose the exact version from Browse before launching it."
                )
            } else {
                format!(
                    "Workshop item {published_file_id} has multiple game-setup versions here. Choose the exact version from Browse before adding it to a profile."
                )
            };
            return false;
        }
        let workspace = content
            .locations
            .iter()
            .find(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace)
            .map(|location| location.root.clone());
        let identity = ContentIdentity {
            id: content.id,
            version: content.version,
        };
        let assigned = if launch_after_assignment {
            self.assign_profile_root_without_catalog_refresh(identity, workspace)
        } else {
            self.assign_profile_root(identity, workspace)
        };
        if assigned {
            if launch_after_assignment {
                self.run_installed_selected();
            } else {
                self.activate_navigation_section(Section::Library);
            }
        }
        assigned
    }

    pub(super) fn select_profile(&mut self, index: usize) {
        if index >= self.profile.items.len() || index == self.profile.active_index {
            return;
        }
        if self.operation.running {
            self.status = "Finish the current operation before switching profiles.".to_owned();
            return;
        }
        if self.has_unsaved_context_switch_edits() {
            self.status =
                "Discard the Create draft or save or discard workspace, composition, or manifest edits before switching profiles.".to_owned();
            return;
        }
        self.submit_service_action(vapor_studio_protocol::StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::Select {
                profile_id: self.profile.items[index].id.clone(),
            },
        ));
        self.activate_profile_context(index);
    }

    pub(super) fn persist_active_profile_composition(&mut self) {
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        if let Err(error) = self.apply_service_action(StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::SetCompositionState {
                profile_id: profile.id,
                root_package: profile.root_package,
                resolved_graph: profile.resolved_graph.map(Box::new),
                installation: Box::new(profile.installation),
            },
        )) {
            self.service_error = Some(error.clone());
            self.status = error;
        }
    }

    fn activate_profile_context(&mut self, index: usize) {
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
        self.workspace.workspace_validation_error = None;
        self.authoring.clear_workspace_context();
        self.catalog.clear_workspace_content();
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.workspace.pending_unregistration = None;
        self.workspace.clear_file_context();
        self.workspace.unregistered_projects =
            vapor_studio_core::UnregisteredProjectScan::default();
        self.workspace.unregistered_scan_completed = false;
        self.workspace.unregistered_scan_error = None;
        self.discover();
    }

    pub(super) fn assign_profile_root(
        &mut self,
        identity: ContentIdentity,
        workspace: Option<PathBuf>,
    ) -> bool {
        self.assign_profile_root_inner(identity, workspace)
    }

    fn assign_profile_root_without_catalog_refresh(
        &mut self,
        identity: ContentIdentity,
        workspace: Option<PathBuf>,
    ) -> bool {
        self.assign_profile_root_inner(identity, workspace)
    }

    fn assign_profile_root_inner(
        &mut self,
        identity: ContentIdentity,
        workspace: Option<PathBuf>,
    ) -> bool {
        if self.authoring.manifest_metadata_dirty
            || self.authoring.composition_has_unsaved_changes()
        {
            self.status = "Save or discard dependency or structured manifest edits before changing the profile composition.".to_owned();
            return false;
        }
        let current_workspace = self.profile.items[self.profile.active_index]
            .workspace
            .as_ref()
            .map(|association| association.root.clone());
        let workspace_changed = current_workspace != workspace;
        if self.profile_root_change_blocked_by_create_draft(workspace.as_deref()) {
            self.status = "Finish creating this content or discard its Create draft before changing the profile workspace.".to_owned();
            return false;
        }
        if self
            .catalog
            .items
            .iter()
            .any(|content| content.identity() == identity && !content.is_unambiguous())
        {
            self.status = format!(
                "Cannot select {}: its local workspace, cache, or installed records disagree. Review the catalog conflicts first.",
                content_selector(&identity)
            );
            return false;
        }
        if self.workspace.editor.has_dirty_documents()
            && current_workspace.as_deref() != workspace.as_deref()
        {
            self.status =
                "Save or discard workspace edits before assigning content from another workspace."
                    .to_owned();
            return false;
        }
        let already_selected =
            self.is_active_profile_root(&identity.id, identity.version.as_deref());
        if already_selected && !workspace_changed {
            self.status = format!(
                "{} is already selected for profile {}.",
                content_selector(&identity),
                self.profile.items[self.profile.active_index].name,
            );
            self.activate_navigation_section(Section::Library);
            return true;
        }
        let profile_id = self.profile.items[self.profile.active_index].id.clone();
        if let Err(error) = self.apply_service_action(StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::SetRoot {
                profile_id,
                content_id: Some(identity.id.clone()),
                content_version: identity.version.clone(),
                workspace_root: workspace.as_ref().map(|root| root.display().to_string()),
            },
        )) {
            self.status = error;
            return false;
        }
        if workspace_changed {
            self.authoring.clear_selected_source();
            self.profile_composition.clear_context();
            self.workspace.workspace_validation_error = None;
            self.workspace.clear_file_context();
        } else {
            self.profile_composition.invalidate_preview();
        }
        self.status = format!(
            "Selected {} for profile {}. Preview its dependency plan before installing.",
            content_selector(&identity),
            self.profile.items[self.profile.active_index].name,
        );
        self.activate_navigation_section(Section::Library);
        true
    }

    pub(super) fn profile_root_change_blocked_by_create_draft(
        &self,
        candidate_workspace: Option<&Path>,
    ) -> bool {
        let current_workspace = self.profile.items[self.profile.active_index]
            .workspace
            .as_ref()
            .map(|association| association.root.as_path());
        self.authoring.create.is_dirty() && current_workspace != candidate_workspace
    }

    pub(super) fn workshop_root_change_blocked_by_create_draft(
        &self,
        published_file_id: &str,
    ) -> bool {
        let matches = self
            .catalog
            .items
            .iter()
            .filter(|content| {
                content.published_file_id.as_deref() == Some(published_file_id)
                    && content.kind == vapor_core::ContentKind::Packagepack.as_str()
                    && content.is_unambiguous()
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return false;
        }
        let content = matches[0];
        let workspace = content
            .locations
            .iter()
            .find(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace)
            .map(|location| location.root.as_path());
        self.profile_root_change_blocked_by_create_draft(workspace)
    }

    pub(super) fn add_profile(&mut self) {
        if self.has_unsaved_context_switch_edits() {
            self.status = "Discard the Create draft or save or discard workspace, composition, or manifest edits before creating a profile.".to_owned();
            return;
        }
        let name = self.profile.new_name.trim().to_owned();
        if let Err(error) = self.apply_service_action(vapor_studio_protocol::StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::Create { name },
        )) {
            self.status = error;
            return;
        }
        self.profile.confirm_removal = false;
        self.profile.new_name.clear();
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.status = format!(
            "Created profile {}.",
            self.profile.items[self.profile.active_index].name
        );
    }

    pub(super) fn rename_active_profile(&mut self) {
        let profile_id = self.profile.items[self.profile.active_index].id.clone();
        let name = self.profile.name_draft.clone();
        if let Err(error) = self.apply_service_action(vapor_studio_protocol::StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::Rename { profile_id, name },
        )) {
            self.profile.name_draft = self.profile.items[self.profile.active_index].name.clone();
            self.status = error;
            return;
        }
        self.status = format!("Renamed profile to {}.", self.profile.name_draft);
    }

    pub(super) fn remove_active_profile(&mut self) {
        if self.operation.running || self.has_unsaved_context_switch_edits() {
            self.status =
                "Finish or discard the current operation and edits before removing a profile."
                    .to_owned();
            return;
        }
        let removed = self.profile.items[self.profile.active_index].name.clone();
        let profile_id = self.profile.items[self.profile.active_index].id.clone();
        if let Err(error) = self.apply_service_action(vapor_studio_protocol::StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::Remove { profile_id },
        )) {
            self.status = error;
            return;
        }
        self.profile.confirm_removal = false;
        self.profile.name_draft.clear();
        self.activate_profile_context(self.profile.active_index);
        self.status = format!("Removed profile {}.", removed);
    }

    pub(super) fn duplicate_active_profile(&mut self) {
        if self.operation.running || self.has_unsaved_context_switch_edits() {
            self.status = "Discard the Create draft or save or discard workspace, composition, or manifest edits before duplicating a profile.".to_owned();
            return;
        }
        let source_name = self.profile.items[self.profile.active_index].name.clone();
        let profile_id = self.profile.items[self.profile.active_index].id.clone();
        if let Err(error) = self.apply_service_action(vapor_studio_protocol::StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::Duplicate {
                profile_id,
                name: String::new(),
            },
        )) {
            self.status = error;
            return;
        }
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.status = format!(
            "Duplicated {source_name} as {}.",
            self.profile.items[self.profile.active_index].name
        );
    }
}
