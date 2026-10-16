use super::{StudioApp, content_selector, source_content_selector};
use super::operations::{OperationResult, discover_workspace_and_catalog};
use super::workspace_files::discover_workspace_files;
use crate::navigation::Section;
use std::{fs, path::PathBuf};
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
    pub(super) selected_file: Option<PathBuf>,
    pub(super) editor: String,
    pub(super) editor_original: String,
    pub(super) editor_toml_diagnostic: Option<Result<(), String>>,
    pub(super) editor_dirty: bool,
    pub(super) files_open: bool,
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
            selected_file: None,
            editor: String::new(),
            editor_original: String::new(),
            editor_toml_diagnostic: None,
            editor_dirty: false,
            files_open: false,
        }
    }

    pub(super) fn clear_file_context(&mut self) {
        self.files.clear();
        self.selected_file = None;
        self.editor.clear();
        self.editor_original.clear();
        self.editor_toml_diagnostic = None;
        self.editor_dirty = false;
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

    pub(super) fn apply_remembered_sources(&mut self, known: Vec<PathBuf>) {
        self.workspace.registered_sources = known;
        self.status = "Added the open workspace to this Vapor installation. It remains separate from the active source.".to_owned();
    }

    pub(super) fn apply_adopted_content_project(
        &mut self,
        summary: String,
        contents: Vec<vapor_studio_core::SourceContent>,
    ) {
        self.catalog.sources = contents;
        self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.status = summary;
    }

    pub(super) fn apply_unregistered_content_project(
        &mut self,
        summary: String,
        contents: Vec<vapor_studio_core::SourceContent>,
        identity: vapor_studio_core::ContentIdentity,
    ) {
        self.catalog.sources = contents;
        self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.workspace.pending_unregistration = None;
        self.invalidate_profiles_for_source_content(&identity);
        self.status = summary;
    }

    pub(super) fn apply_unregistered_project_scan(
        &mut self,
        scan: vapor_studio_core::UnregisteredProjectScan,
    ) {
        self.status = format!(
            "Found {} adoptable content project{} and {} diagnostic{}.",
            scan.candidates.len(),
            if scan.candidates.len() == 1 { "" } else { "s" },
            scan.diagnostics.len(),
            if scan.diagnostics.len() == 1 { "" } else { "s" },
        );
        self.workspace.unregistered_projects = scan;
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
        root: PathBuf,
        contents: Vec<vapor_studio_core::SourceContent>,
        catalog: Result<Vec<vapor_studio_core::CatalogContent>, String>,
        files: Vec<PathBuf>,
    ) {
        let workspace_changed = self.profile.items[self.profile.active_index]
            .workspace
            .as_ref()
            .is_none_or(|workspace| workspace.root != root);
        let catalog_error = catalog.as_ref().err().cloned();
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
        self.catalog.sources = contents;
        self.catalog.items = catalog.unwrap_or_default();
        self.catalog.selected = None;
        self.profile_composition.clear_context();
        self.invalidate_workshop_review();
        self.workspace.files = files;
        self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.workspace.pending_unregistration = None;
        self.section = Section::Workspace;
        self.status = catalog_error.map_or_else(
            || format!("Opened workspace {} · {} source items · {} local catalog entries.", self.workspace.root, self.catalog.sources.len(), self.catalog.items.len()),
            |error| format!("Opened workspace {} · {} source items. Local cache/install catalog could not load: {error}", self.workspace.root, self.catalog.sources.len()),
        );
    }

    pub(super) fn open_workspace_path(&mut self, path: PathBuf) {
        if self.authoring.manifest_metadata_dirty {
            self.workspace.root_draft = self.workspace.root.clone();
            self.status = "Save or discard the structured manifest edits before changing workspaces.".to_owned();
            return;
        }
        if self.workspace.editor_dirty {
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
                    discover_workspace_files(&root).map(|files| (root, contents, catalog, files))
                });
            OperationResult::WorkspaceSelection(result)
        });
        self.status = format!("Opening workspace {}…", path.display());
    }

    pub(super) fn validate_source_project(&mut self, content: &vapor_studio_core::SourceContent) {
        if !self.require_open_workspace("validate this content project") {
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        let selector = source_content_selector(content);
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_profile_source(
                &executable, &root, &selector,
            ))
        });
        self.status = format!("Validating {selector} through Vapor Shell…");
    }

    pub(super) fn validate_selected_source(&mut self) {
        if !self.require_open_workspace("validate source content") {
            return;
        }
        let Some(root_package) = self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let root = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_profile_source(
                &executable,
                &root,
                &selector,
            ))
        });
        self.status = "Validating the selected source through Vapor Shell…".to_owned();
    }

    pub(super) fn validate_workspace_source(&mut self) {
        if !self.require_open_workspace("validate workspace content") {
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
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_workspace_source(
                &executable,
                &root,
            ))
        });
        self.status = "Validating authored content through Vapor…".to_owned();
    }

    pub(super) fn require_saved_workspace(&mut self, action: &str) -> bool {
        if self.authoring.manifest_metadata_dirty {
            self.status = format!(
                "Save or discard structured manifest edits before you {action}.",
            );
            return false;
        }
        if !self.workspace.editor_dirty {
            return true;
        }
        self.status = format!(
            "Save or reload the open file before you {action}; Vapor operations read saved files from disk.",
        );
        false
    }

    pub(super) fn require_open_workspace(&mut self, action: &str) -> bool {
        if !self.require_saved_workspace(action) {
            return false;
        }
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

    pub(super) fn scan_unregistered_content_projects(&mut self) {
        if !self.require_open_workspace("scan for unregistered content") {
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace.root.trim());
        self.spawn(OperationKind::DiscoverContentProjects, move |_context| {
            OperationResult::UnregisteredProjectScan(
                vapor_studio_core::scan_unregistered_content_projects(&executable, &root),
            )
        });
        self.status = "Finding unregistered content manifests in this Workspace…".to_owned();
    }

    pub(super) fn adopt_content_project(&mut self, project_path: PathBuf) {
        if !self.require_open_workspace("adopt a content project") {
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace.root.trim());
        let display_path = project_path.display().to_string();
        self.spawn(OperationKind::AdoptContentProject, move |_context| {
            OperationResult::AdoptedContentProject(
                vapor_studio_core::adopt_workspace_content_project(
                    &executable, &root, &project_path,
                ),
            )
        });
        self.status = format!("Validating and adopting {display_path}…");
    }

    pub(super) fn unregister_content_project(&mut self) {
        if !self.require_open_workspace("unregister a content project") {
            return;
        }
        let Some(project) = self.workspace.pending_unregistration.clone() else {
            self.status = "Select a registered content project to unregister.".to_owned();
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace.root.trim());
        let id = project.id.clone();
        let display = format!("{} · {}", project.id, project.root.display());
        self.spawn(OperationKind::UnregisterContentProject, move |_context| {
            OperationResult::UnregisteredContentProject(
                vapor_studio_core::unregister_workspace_content_project(&executable, &root, &id),
            )
        });
        self.status = format!("Removing {display} from Vapor's content registry…");
    }

    pub(super) fn remember_open_workspace(&mut self) {
        if !self.require_open_workspace("add this workspace to Vapor sources") {
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        if root.as_os_str().is_empty() {
            self.status = "Open a workspace before adding it to Vapor sources.".to_owned();
            return;
        }
        if self.workspace.registered_sources.iter().any(|source| source == &root) {
            self.status = "This workspace is already registered with Vapor.".to_owned();
            return;
        }
        self.spawn(OperationKind::RememberWorkspace, move |_context| {
            OperationResult::RememberedSources(vapor_studio_core::remember_workspace(&root))
        });
        self.status = "Adding the open workspace to this Vapor installation…".to_owned();
    }

    pub(super) fn refresh_registered_sources(&mut self) {
        let result = vapor_core::VaporInstallation::discover()
            .map_err(|error| error.to_string())
            .and_then(|installation| {
                vapor_core::source_state(&installation)
                    .map(|state| (state.known, state.active))
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok((known, active)) => {
                self.workspace.registered_sources = known;
                self.workspace.active_source = active;
                self.status = format!(
                    "Loaded {} registered workspace{} from this Vapor installation.",
                    self.workspace.registered_sources.len(),
                    if self.workspace.registered_sources.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            }
            Err(error) => self.status = format!("Could not read registered workspaces: {error}"),
        }
    }
}
