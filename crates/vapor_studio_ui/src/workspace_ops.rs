use super::StudioApp;
use super::catalog::discover_workspace_and_catalog;
use super::operation_protocol::{OperationResult, WorkspaceOpenSnapshot};
use super::workspace_files::discover_workspace_files;
use crate::navigation::Section;
use std::{collections::BTreeMap, fs, path::PathBuf};
use vapor_core::{DevelopmentOperation, VaporWorkspace};
use vapor_studio_core::{OperationKind, WorkspaceAssociation};

pub(super) struct WorkspaceState {
    pub(super) root: String,
    pub(super) root_draft: String,
    pub(super) fallback_root: PathBuf,
    pub(super) starter_path: String,
    pub(super) starter_organization: String,
    pub(super) starter_name: String,
    pub(super) new_file_path: String,
    pub(super) unregistered_projects: vapor_studio_core::UnregisteredProjectScan,
    pub(super) pending_unregistration: Option<vapor_studio_core::SourceContent>,
    pub(super) registered_sources: Vec<PathBuf>,
    pub(super) active_source: Option<PathBuf>,
    pub(super) files: Vec<PathBuf>,
    pub(super) file_filter: String,
    pub(super) selected_file: Option<PathBuf>,
    pub(super) editor_documents: BTreeMap<PathBuf, WorkspaceDocument>,
    pub(super) editor_draft_recovery: Option<String>,
    pub(super) files_open: bool,
}

#[derive(Clone)]
pub(super) struct WorkspaceDocument {
    pub(super) original: String,
    pub(super) contents: String,
}

impl WorkspaceDocument {
    pub(super) fn is_dirty(&self) -> bool {
        self.contents != self.original
    }
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
            new_file_path: String::new(),
            unregistered_projects: vapor_studio_core::UnregisteredProjectScan::default(),
            pending_unregistration: None,
            registered_sources: Vec::new(),
            active_source: None,
            files: Vec::new(),
            file_filter: String::new(),
            selected_file: None,
            editor_documents: BTreeMap::new(),
            editor_draft_recovery: None,
            files_open: false,
        }
    }

    pub(super) fn active_document(&self) -> Option<&WorkspaceDocument> {
        self.selected_file
            .as_ref()
            .and_then(|path| self.editor_documents.get(path))
    }

    pub(super) fn active_document_mut(&mut self) -> Option<&mut WorkspaceDocument> {
        let path = self.selected_file.clone()?;
        self.editor_documents.get_mut(&path)
    }

    pub(super) fn active_document_is_dirty(&self) -> bool {
        self.active_document().is_some_and(WorkspaceDocument::is_dirty)
    }

    pub(super) fn has_dirty_documents(&self) -> bool {
        self.editor_documents.values().any(WorkspaceDocument::is_dirty)
    }

    pub(super) fn editor_recovery_summary(&self) -> String {
        let open = self.editor_documents.len();
        let dirty = self.editor_documents.values().filter(|document| document.is_dirty()).count();
        format!("Recovered {open} editor tab(s), including {dirty} unsaved draft(s).")
    }

    pub(super) fn save_editor_drafts(&self) -> Option<String> {
        if self.editor_documents.is_empty() || self.root.trim().is_empty() {
            return None;
        }
        let documents = self.editor_documents.iter().map(|(path, document)| {
            (
                path.to_string_lossy().into_owned(),
                document.original.clone(),
                document.contents.clone(),
            )
        }).collect::<Vec<_>>();
        serde_json::to_string(&(
            self.root.clone(),
            self.selected_file
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            documents,
        )).ok()
    }

    pub(super) fn restore_editor_drafts(&mut self, root: &PathBuf) -> bool {
        let Some(encoded) = self.editor_draft_recovery.take() else {
            return false;
        };
        let Ok((saved_root, selected, documents)) = serde_json::from_str::<(
            String,
            Option<String>,
            Vec<(String, String, String)>,
        )>(&encoded) else {
            return false;
        };
        let same_root = matches!(
            (fs::canonicalize(&saved_root), fs::canonicalize(root)),
            (Ok(saved), Ok(current)) if saved == current
        );
        if !same_root || documents.len() > 16 {
            return false;
        }
        let mut restored = BTreeMap::new();
        for (path, original, contents) in documents {
            let path = PathBuf::from(path);
            if path.is_absolute()
                || path.components().any(|part| !matches!(part, std::path::Component::Normal(_)))
                || !self.files.contains(&path)
            {
                return false;
            }
            restored.insert(path, WorkspaceDocument { original, contents });
        }
        self.editor_documents = restored;
        self.selected_file = selected
            .map(PathBuf::from)
            .filter(|path| self.editor_documents.contains_key(path));
        self.files_open = self.selected_file.is_some();
        self.files_open
    }

    pub(super) fn clear_file_context(&mut self) {
        self.files.clear();
        self.file_filter.clear();
        self.selected_file = None;
        self.editor_documents.clear();
        self.files_open = false;
        self.new_file_path.clear();
    }
}

impl StudioApp {
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
            let (source, catalog) = discover_workspace_and_catalog(&executable, root.as_deref());
            OperationResult::Discovery(source, catalog)
        });
        self.status = "Refreshing the local content catalog through Vapor Shell…".to_owned();
    }

    pub(super) fn activate_workspace(
        &mut self,
        selection: WorkspaceOpenSnapshot,
    ) {
        let WorkspaceOpenSnapshot {
            root,
            source_content,
            local_catalog,
            workspace_files,
        } = selection;
        let workspace_changed = self.profile.items[self.profile.active_index]
            .workspace
            .as_ref()
            .is_none_or(|workspace| workspace.root != root);
        let catalog_error = local_catalog.as_ref().err().cloned();
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
        self.catalog.selected = None;
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.workspace.files = workspace_files;
        let restored_editor_drafts = self
            .workspace
            .restore_editor_drafts(&PathBuf::from(self.workspace.root.trim()));
        self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.workspace.pending_unregistration = None;
        self.section = Section::Workspace;
        self.status = catalog_error.map_or_else(
            || format!("Opened workspace {} · {} source items · {} local catalog entries.", self.workspace.root, self.catalog.sources.len(), self.catalog.items.len()),
            |error| format!("Opened workspace {} · {} source items. Local cache/install catalog could not load: {error}", self.workspace.root, self.catalog.sources.len()),
        );
        if restored_editor_drafts {
            self.status.push(' ');
            self.status.push_str(&self.workspace.editor_recovery_summary());
        }
    }

    pub(super) fn open_workspace_path(&mut self, path: PathBuf) {
        if self.authoring.manifest_metadata_dirty {
            self.workspace.root_draft = self.workspace.root.clone();
            self.status = "Save or discard the structured manifest edits before changing workspaces.".to_owned();
            return;
        }
        if self.workspace.has_dirty_documents() {
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
                    VaporWorkspace::load(&root)
                        .map_err(|error| format!("{} is not a valid Vapor Workspace: {error}", root.display()))?;
                    let contents = vapor_studio_core::discover_source_content(&executable, &root)?;
                    let catalog =
                        vapor_studio_core::discover_content_catalog(&executable, Some(&root));
                    discover_workspace_files(&root).map(|files| WorkspaceOpenSnapshot {
                        root,
                        source_content: contents,
                        local_catalog: catalog,
                        workspace_files: files,
                    })
                });
            OperationResult::WorkspaceSelection(result)
        });
        self.status = format!("Opening workspace {}…", path.display());
    }







    pub(super) fn require_saved_workspace(&mut self, action: &str) -> bool {
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
        self.workspace.has_dirty_documents() || self.authoring.manifest_metadata_dirty
    }

    pub(super) fn require_open_workspace(&mut self, action: &str) -> bool {
        if !self.require_saved_workspace(action) {
            return false;
        }
        self.require_workspace_association(action)
    }

    pub(super) fn require_open_workspace_for_authoring_save(&mut self, action: &str) -> bool {
        if self.workspace.has_dirty_documents() {
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
