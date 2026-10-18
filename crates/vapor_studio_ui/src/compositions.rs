use super::{StudioApp, catalog_content_selector, content_selector};
use crate::navigation::{CatalogSourceFilter, Section};
use std::path::{Path, PathBuf};
use vapor_client_protocol::ClientAction;
use vapor_client_application::ContentReference;

pub(super) struct CompositionState {
    pub(super) items: Vec<vapor_client_application::Composition>,
    pub(super) service_revision: Option<u64>,
    pub(super) active_index: usize,
    pub(super) recovery: Option<String>,
    pub(super) recovery_backup: Option<String>,
    pub(super) name_draft: String,
    pub(super) confirm_removal: bool,
    pub(super) new_name: String,
}

impl CompositionState {
    pub(super) fn new(
        items: Vec<vapor_client_application::Composition>,
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
    pub(super) fn is_active_composition_root(&self, id: &str, version: Option<&str>) -> bool {
        self.composition
            .items
            .get(self.composition.active_index)
            .and_then(|composition| composition.root_package.as_ref())
            .is_some_and(|root| root.id == id && root.version.as_deref() == version)
    }

    pub(super) fn use_available_workshop_package_as_composition_root(
        &mut self,
        published_file_id: &str,
    ) -> bool {
        self.assign_available_workshop_package_to_composition(published_file_id, false)
    }

    pub(super) fn launch_workshop_package_as_composition_root(
        &mut self,
        published_file_id: &str,
    ) -> bool {
        self.assign_available_workshop_package_to_composition(published_file_id, true)
    }

    fn assign_available_workshop_package_to_composition(
        &mut self,
        published_file_id: &str,
        launch_after_assignment: bool,
    ) -> bool {
        if !self.catalog.local_catalog_current() {
            self.status = if launch_after_assignment {
                "Refresh the on-device content list before launching this Workshop composition."
            } else {
                "Refresh the on-device content list before selecting this Workshop composition."
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
                "Workshop item {published_file_id} is available here, but it is not a playable composition."
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
                    "Workshop item {published_file_id} has multiple game-setup versions here. Choose the exact version from Browse before adding it to a composition."
                )
            };
            return false;
        }
        let workspace = content
            .locations
            .iter()
            .find(|location| location.kind == vapor_client_application::CatalogLocationKind::Workspace)
            .map(|location| location.root.clone());
        let identity = ContentReference {
            id: content.id,
            version: content.version,
        };
        let assigned = if launch_after_assignment {
            self.assign_composition_root_without_catalog_refresh(identity, workspace)
        } else {
            self.assign_composition_root(identity, workspace)
        };
        if assigned {
            if launch_after_assignment {
                self.run_installed_selected();
            } else {
                self.activate_navigation_section(Section::Compositions);
            }
        }
        assigned
    }

    pub(super) fn select_composition(&mut self, index: usize) {
        if index >= self.composition.items.len() || index == self.composition.active_index {
            return;
        }
        if self.operation.running {
            self.status = "Finish the current operation before switching compositions.".to_owned();
            return;
        }
        if self.has_unsaved_context_switch_edits() {
            self.status =
                "Discard the Create draft or save or discard workspace, composition, or manifest edits before switching compositions.".to_owned();
            return;
        }
        self.submit_service_action(vapor_client_protocol::ClientAction::Composition(
            vapor_client_protocol::CompositionAction::Select {
                composition_id: self.composition.items[index].id.clone(),
            },
        ));
        self.activate_composition_context(index);
    }

    fn activate_composition_context(&mut self, index: usize) {
        self.composition.active_index = index;
        self.composition.name_draft = self.composition.items[index].name.clone();
        self.composition.confirm_removal = false;
        self.authoring.clear_workspace_context();
        self.composition_preview.clear_context();
        self.invalidate_workshop_review();
        self.workspace.pending_unregistration = None;
        self.discover();
    }

    pub(super) fn assign_composition_root(
        &mut self,
        identity: ContentReference,
        workspace: Option<PathBuf>,
    ) -> bool {
        self.assign_composition_root_inner(identity, workspace)
    }

    pub(super) fn create_composition_from_content(
        &mut self,
        identity: ContentReference,
        workspace: Option<PathBuf>,
    ) -> bool {
        if self.has_unsaved_context_switch_edits() {
            self.status = "Save or discard open edits before creating a composition.".to_owned();
            return false;
        }
        let short_name = identity
            .id
            .rsplit(['/', '.'])
            .next()
            .unwrap_or(identity.id.as_str())
            .replace('-', " ");
        if let Err(error) = self.apply_service_action(ClientAction::Composition(
            vapor_client_protocol::CompositionAction::Create {
                name: format!("{short_name} composition"),
            },
        )) {
            self.status = error;
            return false;
        }
        if !self.assign_composition_root(identity, workspace) {
            return false;
        }
        self.preview_selected_composition();
        true
    }

    fn assign_composition_root_without_catalog_refresh(
        &mut self,
        identity: ContentReference,
        workspace: Option<PathBuf>,
    ) -> bool {
        self.assign_composition_root_inner(identity, workspace)
    }

    fn assign_composition_root_inner(
        &mut self,
        identity: ContentReference,
        workspace: Option<PathBuf>,
    ) -> bool {
        if self.authoring.manifest_metadata_dirty
            || self.authoring.composition_has_unsaved_changes()
        {
            self.status =
                "Save or discard dependency or manifest edits before changing the composition."
                    .to_owned();
            return false;
        }
        let current_workspace = self.composition.items[self.composition.active_index]
            .workspace
            .as_ref()
            .map(|association| association.root.clone());
        let workspace_changed = current_workspace != workspace;
        if self.composition_root_change_blocked_by_create_draft(workspace.as_deref()) {
            self.status = "Finish creating this content or discard its Create draft before changing the composition workspace.".to_owned();
            return false;
        }
        if self
            .catalog
            .items
            .iter()
            .any(|content| content.identity() == identity && !content.is_unambiguous())
        {
            self.status = format!(
                "Cannot select {} because copies on this device disagree. Review the conflicting copies first.",
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
            self.is_active_composition_root(&identity.id, identity.version.as_deref());
        if already_selected && !workspace_changed {
            self.status = format!(
                "{} is already selected for composition {}.",
                content_selector(&identity),
                self.composition.items[self.composition.active_index].name,
            );
            self.activate_navigation_section(Section::Compositions);
            return true;
        }
        let composition_id = self.composition.items[self.composition.active_index]
            .id
            .clone();
        if let Err(error) = self.apply_service_action(ClientAction::Composition(
            vapor_client_protocol::CompositionAction::SetRoot {
                composition_id,
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
            self.composition_preview.clear_context();
            self.workspace.workspace_validation_error = None;
            self.workspace.clear_file_context();
        } else {
            self.composition_preview.invalidate_preview();
        }
        self.status = format!(
            "Selected {} for composition {}. Preview its dependency plan before installing.",
            content_selector(&identity),
            self.composition.items[self.composition.active_index].name,
        );
        self.activate_navigation_section(Section::Compositions);
        true
    }

    pub(super) fn composition_root_change_blocked_by_create_draft(
        &self,
        candidate_workspace: Option<&Path>,
    ) -> bool {
        let current_workspace = self.composition.items[self.composition.active_index]
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
            .find(|location| location.kind == vapor_client_application::CatalogLocationKind::Workspace)
            .map(|location| location.root.as_path());
        self.composition_root_change_blocked_by_create_draft(workspace)
    }

    pub(super) fn add_composition(&mut self) {
        if self.has_unsaved_context_switch_edits() {
            self.status = "Discard the Create draft or save or discard workspace, composition, or manifest edits before creating a composition.".to_owned();
            return;
        }
        let name = self.composition.new_name.trim().to_owned();
        if let Err(error) =
            self.apply_service_action(vapor_client_protocol::ClientAction::Composition(
                vapor_client_protocol::CompositionAction::Create { name },
            ))
        {
            self.status = error;
            return;
        }
        self.composition.confirm_removal = false;
        self.composition.new_name.clear();
        self.composition_preview.clear_context();
        self.invalidate_workshop_review();
        self.status = format!(
            "Created composition {}.",
            self.composition.items[self.composition.active_index].name
        );
    }

    pub(super) fn rename_active_composition(&mut self) {
        let composition_id = self.composition.items[self.composition.active_index]
            .id
            .clone();
        let name = self.composition.name_draft.clone();
        if let Err(error) =
            self.apply_service_action(vapor_client_protocol::ClientAction::Composition(
                vapor_client_protocol::CompositionAction::Rename {
                    composition_id,
                    name,
                },
            ))
        {
            self.composition.name_draft = self.composition.items[self.composition.active_index]
                .name
                .clone();
            self.status = error;
            return;
        }
        self.status = format!("Renamed composition to {}.", self.composition.name_draft);
    }

    pub(super) fn remove_active_composition(&mut self) {
        if self.operation.running || self.has_unsaved_context_switch_edits() {
            self.status =
                "Finish or discard the current operation and edits before removing a composition."
                    .to_owned();
            return;
        }
        let removed = self.composition.items[self.composition.active_index]
            .name
            .clone();
        let composition_id = self.composition.items[self.composition.active_index]
            .id
            .clone();
        if let Err(error) =
            self.apply_service_action(vapor_client_protocol::ClientAction::Composition(
                vapor_client_protocol::CompositionAction::Remove { composition_id },
            ))
        {
            self.status = error;
            return;
        }
        self.composition.confirm_removal = false;
        self.composition.name_draft.clear();
        self.activate_composition_context(self.composition.active_index);
        self.status = format!("Removed composition {}.", removed);
    }

    pub(super) fn duplicate_active_composition(&mut self) {
        if self.operation.running || self.has_unsaved_context_switch_edits() {
            self.status = "Discard the Create draft or save or discard workspace, composition, or manifest edits before duplicating a composition.".to_owned();
            return;
        }
        let source_name = self.composition.items[self.composition.active_index]
            .name
            .clone();
        let composition_id = self.composition.items[self.composition.active_index]
            .id
            .clone();
        if let Err(error) =
            self.apply_service_action(vapor_client_protocol::ClientAction::Composition(
                vapor_client_protocol::CompositionAction::Duplicate {
                    composition_id,
                    name: String::new(),
                },
            ))
        {
            self.status = error;
            return;
        }
        self.composition_preview.clear_context();
        self.invalidate_workshop_review();
        self.status = format!(
            "Duplicated {source_name} as {}.",
            self.composition.items[self.composition.active_index].name
        );
    }
}
