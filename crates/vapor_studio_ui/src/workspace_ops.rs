use super::StudioApp;
use super::catalog::discover_workspace_and_catalog;
use super::operation_protocol::{OperationResult, WorkspaceOpenSnapshot};
use super::workspace_editor::WorkspaceEditorState;
use super::workspace_files::discover_workspace_files;
use crate::navigation::Section;
use std::{fs, path::PathBuf};
use vapor_core::{DevelopmentOperation, VaporWorkspace};
use vapor_studio_core::{OperationKind, WorkspaceAssociation};

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
    pub(super) unregistered_projects: vapor_studio_core::UnregisteredProjectScan,
    pub(super) unregistered_scan_completed: bool,
    pub(super) pending_unregistration: Option<vapor_studio_core::SourceContent>,
    pub(super) registered_sources: Vec<PathBuf>,
    pub(super) active_source: Option<PathBuf>,
    pub(super) overview_panel: WorkspacePanel,
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
            unregistered_projects: vapor_studio_core::UnregisteredProjectScan::default(),
            unregistered_scan_completed: false,
            pending_unregistration: None,
            registered_sources: Vec::new(),
            active_source: None,
            overview_panel: WorkspacePanel::default(),
            editor: WorkspaceEditorState::default(),
        }
    }
    pub(super) fn clear_file_context(&mut self) {
        self.editor.clear_context();
    }
}

impl StudioApp {
    pub(super) fn navigate_to_workspace_content(&mut self) {
        self.section = Section::Workspace;
        self.workspace.editor.files_open = false;
        self.workspace.overview_panel = WorkspacePanel::Content;
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
        let root = self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.workspace.as_ref())
            .map(|workspace| workspace.root.clone());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Discover, move |_context| {
            let (source, catalog, unregistered) =
                discover_workspace_and_catalog(&executable, root.as_deref());
            OperationResult::Discovery(source, catalog, unregistered)
        });
        self.status = "Refreshing the local content catalog through Vapor Shell…".to_owned();
    }

    pub(super) fn activate_workspace(&mut self, selection: WorkspaceOpenSnapshot) {
        let WorkspaceOpenSnapshot {
            root,
            landing_section,
            source_content,
            unregistered_projects,
            local_catalog,
            workspace_files,
        } = selection;
        let workspace_changed = self.profile.items[self.profile.active_index]
            .workspace
            .as_ref()
            .is_none_or(|workspace| workspace.root != root);
        let catalog_error = local_catalog.as_ref().err().cloned();
        let project_scan_error = unregistered_projects.as_ref().err().cloned();
        self.workspace.root = root.display().to_string();
        self.workspace.root_draft = self.workspace.root.clone();
        self.authoring.clear_workspace_context();
        let profile = &mut self.profile.items[self.profile.active_index];
        profile.workspace = Some(WorkspaceAssociation { root });
        if workspace_changed {
            profile.root_package = None;
            profile.resolved_graph = None;
            profile.installation = vapor_studio_core::InstallationState::Unknown;
        }
        self.workspace.clear_file_context();
        self.catalog.sources = source_content;
        self.catalog.items = local_catalog.unwrap_or_default();
        self.workspace.unregistered_scan_completed = unregistered_projects.is_ok();
        self.workspace.unregistered_projects = unregistered_projects.unwrap_or_default();
        self.catalog.selected = None;
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.workspace.editor.files = workspace_files;
        let restored_editor_drafts = self
            .workspace
            .editor
            .restore_editor_drafts(&PathBuf::from(self.workspace.root.trim()));
        self.workspace.pending_unregistration = None;
        self.section = landing_section;
        self.status = catalog_error.map_or_else(
            || format!(
                "Opened workspace {} · {} registered content projects · {} local catalog entries · {} projects ready to adopt.",
                self.workspace.root,
                self.catalog.sources.len(),
                self.catalog.items.len(),
                self.workspace.unregistered_projects.candidates.len(),
            ),
            |error| format!("Opened workspace {} · {} source items. Local cache/install catalog could not load: {error}", self.workspace.root, self.catalog.sources.len()),
        );
        if let Some(error) = project_scan_error {
            self.status.push_str(&format!(
                " Content project scan could not complete: {error}"
            ));
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
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selected = path.clone();
        self.spawn(OperationKind::OpenWorkspace, move |_context| {
            let result = fs::canonicalize(&selected)
                .map_err(|error| format!("{}: {error}", selected.display()))
                .and_then(|root| {
                    VaporWorkspace::load(&root).map_err(|error| {
                        format!("{} is not a valid Vapor Workspace: {error}", root.display())
                    })?;
                    let contents = vapor_studio_core::discover_source_content(&executable, &root)?;
                    let unregistered_projects =
                        vapor_studio_core::scan_unregistered_content_projects(&executable, &root);
                    let catalog =
                        vapor_studio_core::discover_content_catalog(&executable, Some(&root));
                    discover_workspace_files(&root).map(|files| WorkspaceOpenSnapshot {
                        root,
                        landing_section,
                        source_content: contents,
                        unregistered_projects,
                        local_catalog: catalog,
                        workspace_files: files,
                    })
                });
            OperationResult::WorkspaceSelection(result)
        });
        self.status = format!("Opening workspace {}…", path.display());
    }

    pub(super) fn require_saved_workspace(&mut self, action: &str) -> bool {
        if self.authoring.composition_has_unsaved_changes() {
            self.status = format!(
                "Save or discard composition dependency changes before you {action}."
            );
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
