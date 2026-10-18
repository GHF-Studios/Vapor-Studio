use super::StudioApp;
use super::operation_protocol::{OperationResult, WorkspaceOpenSnapshot};
use super::workspace_editor::WorkspaceEditorState;
use crate::navigation::Section;
use std::path::{Path, PathBuf};
use vapor_core::{DevelopmentOperation, VaporWorkspace};
use vapor_studio_core::OperationKind;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum WorkspacePanel {
    #[default]
    Sources,
    Content,
    Build,
}

pub(super) struct WorkspaceState {
    pub(super) root: String,
    pub(super) root_draft: String,
    pub(super) fallback_root: PathBuf,
    pub(super) starter_path: String,
    pub(super) starter_organization: String,
    pub(super) starter_name: String,
    pub(super) workspace_validation_error: Option<String>,
    pub(super) unregistered_projects: vapor_studio_core::UnregisteredProjectScan,
    pub(super) unregistered_scan_completed: bool,
    pub(super) unregistered_scan_error: Option<String>,
    pub(super) pending_unregistration: Option<vapor_studio_core::SourceContent>,
    pub(super) pending_clean_confirmation: Option<PathBuf>,
    pub(super) registered_sources: Vec<PathBuf>,
    pub(super) active_source: Option<PathBuf>,
    pub(super) registered_sources_loaded: bool,
    pub(super) registered_sources_error: Option<String>,
    pub(super) overview_panel: WorkspacePanel,
    pub(super) content_search: String,
    pub(super) editor: WorkspaceEditorState,
}

impl WorkspaceState {
    pub(super) fn new(root: String, root_draft: String, fallback_root: PathBuf) -> Self {
        Self {
            root,
            root_draft,
            fallback_root,
            starter_path: String::new(),
            starter_organization: "my-workshop".to_owned(),
            starter_name: "my-game".to_owned(),
            workspace_validation_error: None,
            unregistered_projects: vapor_studio_core::UnregisteredProjectScan::default(),
            unregistered_scan_completed: false,
            unregistered_scan_error: None,
            pending_unregistration: None,
            pending_clean_confirmation: None,
            registered_sources: Vec::new(),
            active_source: None,
            registered_sources_loaded: false,
            registered_sources_error: None,
            overview_panel: WorkspacePanel::default(),
            content_search: String::new(),
            editor: WorkspaceEditorState::default(),
        }
    }
    pub(super) fn clear_file_context(&mut self) {
        self.content_search.clear();
        self.pending_clean_confirmation = None;
        self.editor.clear_context();
    }
}

impl StudioApp {
    pub(super) fn navigate_to_workspace_content(&mut self) {
        self.open_workspace_panel(WorkspacePanel::Content);
        if !self.workspace.root.trim().is_empty()
            && !self.workspace.unregistered_scan_completed
            && !self.operation.running
        {
            self.scan_unregistered_content_projects();
        }
    }

    pub(super) fn apply_workspace_loaded(&mut self, workspace: VaporWorkspace) {
        self.status = format!(
            "Completed for {}/{} · {} projects",
            workspace.manifest.workspace.organization,
            workspace.manifest.workspace.name,
            workspace.projects.len()
        );
    }

    pub(super) fn run_operation(&mut self, operation: DevelopmentOperation) {
        if self.workspace.pending_clean_confirmation.is_some() {
            return;
        }
        if operation == DevelopmentOperation::Clean {
            if !self.require_open_workspace("clean generated build files") {
                return;
            }
            self.workspace.pending_clean_confirmation =
                Some(PathBuf::from(self.workspace.root.trim()));
            self.open_workspace_panel(WorkspacePanel::Build);
            self.status =
                "Review the cleanup scope before removing generated build files.".to_owned();
            return;
        }
        self.start_workspace_operation(operation);
    }

    pub(super) fn confirm_clean_workspace(&mut self) {
        let Some(confirmed_root) = self.workspace.pending_clean_confirmation.take() else {
            return;
        };
        if confirmed_root != Path::new(self.workspace.root.trim()) {
            self.status = "The workspace changed; review the cleanup target again.".to_owned();
            return;
        }
        self.start_workspace_operation(DevelopmentOperation::Clean);
    }

    fn start_workspace_operation(&mut self, operation: DevelopmentOperation) {
        if !self.require_open_workspace("run a workspace operation") {
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
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

    pub(super) fn discover(&mut self) {
        if !self.require_saved_workspace("refresh workspace content") {
            return;
        }
        self.invalidate_workshop_review();
        self.refresh_content_catalog();
        self.status = "Refreshing the local content catalog through the Studio service…".to_owned();
    }

    pub(super) fn activate_workspace(&mut self, selection: WorkspaceOpenSnapshot) {
        let previous_root = self.workspace.root.clone();
        self.activate_workspace_from_root(selection, &previous_root, false);
    }

    fn activate_workspace_from_root(
        &mut self,
        selection: WorkspaceOpenSnapshot,
        previous_root: &str,
        service_already_opened: bool,
    ) {
        if self.authoring.create.is_dirty() {
            self.workspace.root_draft = self.workspace.root.clone();
            self.status =
                "The workspace was read, but not opened: discard the Create draft before switching workspaces."
                    .to_owned();
            return;
        }
        let WorkspaceOpenSnapshot {
            root,
            landing_section,
            workspace_files,
        } = selection;
        let workspace_changed = previous_root != root.to_string_lossy().as_ref();
        let files_error = workspace_files.as_ref().err().cloned();
        let previous_files =
            (!workspace_changed && files_error.is_some() && self.workspace.editor.files_loaded)
                .then(|| self.workspace.editor.files.clone());
        let requested_root = root.display().to_string();
        if !service_already_opened
            && let Err(error) =
                self.apply_service_action(vapor_studio_protocol::StudioAction::Workspace(
                    vapor_studio_protocol::WorkspaceAction::Open {
                        root: requested_root.clone(),
                    },
                ))
        {
            self.status = format!("Could not open this workspace in the Studio service: {error}");
            return;
        }
        self.workspace.root = requested_root;
        self.workspace.root_draft = self.workspace.root.clone();
        self.authoring.clear_workspace_context();
        self.workspace.clear_file_context();
        if workspace_changed {
            self.catalog.clear_workspace_content();
            self.workspace.workspace_validation_error = None;
            self.workspace.unregistered_projects =
                vapor_studio_core::UnregisteredProjectScan::default();
            self.workspace.unregistered_scan_completed = false;
            self.workspace.unregistered_scan_error = None;
        }
        self.catalog.selected = None;
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.workspace.editor.files = match workspace_files {
            Ok(files) => files,
            Err(_) => previous_files.clone().unwrap_or_default(),
        };
        self.workspace.editor.files_loaded = files_error.is_none() || previous_files.is_some();
        self.workspace.editor.files_refresh_error = files_error
            .as_ref()
            .map(|error| format!("Could not read workspace files: {error}"));
        self.workspace.editor.rebuild_file_index();
        let restored_editor_drafts = self.workspace.editor.files_loaded
            && self
                .workspace
                .editor
                .restore_editor_drafts(&PathBuf::from(self.workspace.root.trim()));
        self.workspace.pending_unregistration = None;
        self.activate_navigation_section(landing_section);
        self.status = format!("Opened project folder {}", self.workspace.root);
        if self.catalog.local_catalog_current() {
            self.status.push_str(&format!(
                " · {} local catalog entries",
                self.catalog.items.len()
            ));
        }
        if let Some(error) = files_error {
            if previous_files.is_some() {
                self.status.push_str(&format!(
                    " Workspace file listing could not complete; showing its last successful list: {error}"
                ));
            } else {
                self.status.push_str(&format!(
                    " Workspace file listing could not complete; the workspace remains open, but the editor list is unavailable: {error}"
                ));
            }
        }
        if restored_editor_drafts {
            self.status.push(' ');
            self.status
                .push_str(&self.workspace.editor.recovery_summary());
        }
    }

    pub(super) fn open_workspace_path(&mut self, path: PathBuf) {
        self.open_workspace_path_to(path, Section::Workspace);
    }

    pub(super) fn open_workspace_path_to(&mut self, path: PathBuf, landing_section: Section) {
        if self.authoring.create.is_dirty() {
            self.workspace.root_draft = self.workspace.root.clone();
            self.status =
                "Finish creating this content or discard its Create draft before changing workspaces."
                    .to_owned();
            return;
        }
        if self.authoring.composition_has_unsaved_changes() {
            self.workspace.root_draft = self.workspace.root.clone();
            self.status =
                "Save or discard composition dependency changes before changing workspaces."
                    .to_owned();
            return;
        }
        if self.authoring.manifest_metadata_dirty {
            self.workspace.root_draft = self.workspace.root.clone();
            self.status =
                "Save or discard the structured manifest edits before changing workspaces."
                    .to_owned();
            return;
        }
        if self.workspace.editor.has_dirty_documents() {
            self.workspace.root_draft = self.workspace.root.clone();
            self.status =
                "Save or discard the current editor changes before changing workspaces.".to_owned();
            return;
        }
        if path.as_os_str().is_empty() {
            self.status = "Enter a workspace folder path.".to_owned();
            return;
        }
        if self.operation.running {
            self.status =
                "Wait for the current operation to finish before changing workspaces.".to_owned();
            return;
        }
        let previous_root = self.workspace.root.clone();
        if let Err(error) =
            self.apply_service_action(vapor_studio_protocol::StudioAction::Workspace(
                vapor_studio_protocol::WorkspaceAction::Open {
                    root: path.to_string_lossy().into_owned(),
                },
            ))
        {
            self.status = format!("Could not open {}: {error}", path.display());
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        if root.as_os_str().is_empty() {
            self.status = "The Studio service did not return an opened workspace.".to_owned();
            return;
        }
        self.activate_workspace_from_root(
            WorkspaceOpenSnapshot {
                root,
                landing_section,
                workspace_files: Err("Workspace file listing is loading.".to_owned()),
            },
            &previous_root,
            true,
        );
        self.start_service_operation(
            vapor_studio_protocol::StudioAction::Workspace(
                vapor_studio_protocol::WorkspaceAction::ListFiles {
                    path: ".".to_owned(),
                },
            ),
            None,
            OperationKind::DiscoverWorkspace,
            vapor_studio_core::OperationPhase::Resolving,
            "Loading workspace files through the Studio service…",
        );
    }

    pub(super) fn require_saved_workspace(&mut self, action: &str) -> bool {
        if self.authoring.composition_has_unsaved_changes() {
            self.status =
                format!("Save or discard composition dependency changes before you {action}.");
            return false;
        }
        if !self.has_unsaved_workspace_edits() {
            return true;
        }
        self.status = if self.authoring.manifest_metadata_dirty {
            format!("Save or discard structured manifest edits before you {action}.")
        } else {
            format!(
                "Save or reload every dirty editor tab before you {action}; Vapor operations read saved files from disk.",
            )
        };
        false
    }

    pub(super) fn has_unsaved_workspace_edits(&self) -> bool {
        self.workspace.editor.has_dirty_documents()
            || self.authoring.manifest_metadata_dirty
            || self.authoring.composition_has_unsaved_changes()
    }

    pub(super) fn has_unsaved_context_switch_edits(&self) -> bool {
        self.has_unsaved_workspace_edits()
            || self.authoring.create.is_dirty()
            || self.authoring.pending_composition_role_change.is_some()
    }

    pub(super) fn require_open_workspace(&mut self, action: &str) -> bool {
        if !self.require_saved_workspace(action) {
            return false;
        }
        self.require_workspace_association(action)
    }

    pub(super) fn require_open_workspace_for_authoring_save(&mut self, action: &str) -> bool {
        if self.workspace.editor.has_dirty_documents() {
            self.status = format!(
                "Save or reload every dirty editor tab before you {action}; authoring saves must not overwrite unsaved file edits.",
            );
            return false;
        }
        self.require_workspace_association(action)
    }

    pub(super) fn require_workspace_association(&mut self, action: &str) -> bool {
        if self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.workspace.as_ref())
            .is_some()
            && !self.workspace.root.trim().is_empty()
        {
            return true;
        }
        self.status = format!("Open a Vapor Workspace before you {action}.");
        false
    }

    pub(super) fn require_open_workspace_for_editor_save(&mut self, action: &str) -> bool {
        if self.authoring.manifest_metadata_dirty {
            self.status = format!(
                "Save or discard structured manifest edits before you {action}; they may target the same file.",
            );
            return false;
        }
        self.require_workspace_association(action)
    }
}
