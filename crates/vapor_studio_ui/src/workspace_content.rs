use super::{StudioApp, content_selector, source_content_selector};
use std::path::{Path, PathBuf};
use vapor_client_application::OperationKind;

impl StudioApp {
    pub(super) fn acquire_packaged_source(&mut self, selector: String, workspace_root: PathBuf) {
        if !self.require_saved_workspace("add this package to your Library") {
            return;
        }
        if self.workspace.root.trim().is_empty()
            || Path::new(self.workspace.root.trim()) != workspace_root
        {
            self.status = "Reopen the source workspace that produced this package before adding it to your Library.".to_owned();
            return;
        }
        if !self.catalog.source_snapshot_current() {
            self.status =
                "Refresh the source project list before adding this package to your Library."
                    .to_owned();
            return;
        }
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Library(
                vapor_client_protocol::LibraryAction::Acquire {
                    selectors: vec![selector.clone()],
                    workspace_root: Some(workspace_root.to_string_lossy().into_owned()),
                },
            ),
            None,
            OperationKind::Acquire,
            vapor_client_application::OperationPhase::Acquiring,
            &format!("Adding {selector} to your Library…"),
        );
    }

    pub(super) fn apply_remembered_sources(&mut self, known: Vec<PathBuf>) {
        self.workspace.registered_sources = known;
        self.workspace.registered_sources_loaded = true;
        self.workspace.registered_sources_error = None;
        self.status = "Added the open workspace to this Vapor installation. It remains separate from the active source.".to_owned();
    }

    pub(super) fn validate_source_project(
        &mut self,
        content: &vapor_client_application::SourceContent,
    ) {
        if !self.require_open_workspace("validate this content project") {
            return;
        }
        let selector = source_content_selector(content);
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Authoring(
                vapor_client_protocol::AuthoringAction::ValidateContent {
                    project_id: selector.clone(),
                },
            ),
            None,
            OperationKind::Validate,
            vapor_client_application::OperationPhase::Verifying,
            &format!("Checking {selector}…"),
        );
    }

    pub(super) fn package_source_project(
        &mut self,
        content: &vapor_client_application::SourceContent,
        dry_run: bool,
    ) {
        if !self.require_open_workspace("package this content project") {
            return;
        }
        let selector = source_content_selector(content);
        self.workspace.last_package = None;
        let operation_status = if dry_run {
            format!("Building a package preview for {selector}…")
        } else {
            format!("Building and packaging {selector}…")
        };
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Authoring(
                vapor_client_protocol::AuthoringAction::PackageContent {
                    selector: selector.clone(),
                    target_triples: Vec::new(),
                    dry_run,
                },
            ),
            None,
            OperationKind::PackageContent,
            vapor_client_application::OperationPhase::Building,
            &operation_status,
        );
    }

    pub(super) fn validate_selected_source(&mut self) {
        if !self.require_open_workspace("validate source content") {
            return;
        }
        let Some(root_package) = self
            .composition
            .items
            .get(self.composition.active_index)
            .and_then(|composition| composition.root_package.clone())
        else {
            return;
        };
        let selector = content_selector(&root_package);
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Authoring(
                vapor_client_protocol::AuthoringAction::ValidateContent {
                    project_id: selector,
                },
            ),
            None,
            OperationKind::Validate,
            vapor_client_application::OperationPhase::Verifying,
            "Checking the selected project…",
        );
    }

    pub(super) fn validate_workspace_source(&mut self) {
        if !self.require_open_workspace("validate workspace content") {
            return;
        }
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workspace(
                vapor_client_protocol::WorkspaceAction::Validate,
            ),
            None,
            OperationKind::Validate,
            vapor_client_application::OperationPhase::Verifying,
            "Checking project content…",
        );
    }

    pub(super) fn scan_unregistered_content_projects(&mut self) {
        if !self.require_open_workspace("scan for unregistered content") {
            return;
        }
        self.submit_service_action(vapor_client_protocol::ClientAction::Catalog(
            vapor_client_protocol::CatalogAction::DiscoverContentProjects,
        ));
        self.status = "Finding unregistered content manifests in this Workspace…".to_owned();
    }

    pub(super) fn adopt_content_project(&mut self, project_path: PathBuf) {
        if !self.require_open_workspace("adopt a content project") {
            return;
        }
        let display_path = project_path.display().to_string();
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workspace(
                vapor_client_protocol::WorkspaceAction::AdoptProject {
                    path: project_path.to_string_lossy().into_owned(),
                },
            ),
            None,
            OperationKind::AdoptContentProject,
            vapor_client_application::OperationPhase::Resolving,
            &format!("Checking and adding {display_path}…"),
        );
    }

    pub(super) fn unregister_content_project(&mut self) {
        if !self.require_open_workspace("unregister a content project") {
            return;
        }
        let Some(project) = self.workspace.pending_unregistration.clone() else {
            self.status = "Select a registered content project to unregister.".to_owned();
            return;
        };
        let id = project.id.clone();
        let display = format!("{} · {}", project.id, project.root.display());
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workspace(
                vapor_client_protocol::WorkspaceAction::UnregisterProject { project_id: id },
            ),
            None,
            OperationKind::UnregisterContentProject,
            vapor_client_application::OperationPhase::Building,
            &format!("Removing {display} from Vapor's content registry…"),
        );
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
        if self
            .workspace
            .registered_sources
            .iter()
            .any(|source| source == &root)
        {
            self.status = "This workspace is already registered with Vapor.".to_owned();
            return;
        }
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workspace(
                vapor_client_protocol::WorkspaceAction::Remember,
            ),
            None,
            OperationKind::RememberWorkspace,
            vapor_client_application::OperationPhase::Building,
            "Adding the open workspace to this Vapor installation…",
        );
    }

    pub(super) fn refresh_registered_sources(&mut self) {
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Authoring(
                vapor_client_protocol::AuthoringAction::ListRegisteredSources,
            ),
            None,
            OperationKind::Discover,
            vapor_client_application::OperationPhase::Resolving,
            "Loading your project folders…",
        );
    }
}
