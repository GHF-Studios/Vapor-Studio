use super::{StudioApp, content_selector, source_content_selector};
use super::operation_protocol::OperationResult;
use std::path::PathBuf;
use vapor_studio_core::OperationKind;

impl StudioApp {
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
        self.workspace.unregistered_scan_completed = false;
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
        self.workspace.unregistered_scan_completed = false;
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
        self.workspace.unregistered_scan_completed = true;
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
        let operation_selector = selector.clone();
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_profile_source(
                &executable, &root, &operation_selector,
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
