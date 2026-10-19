use super::{StudioApp, content_selector, source_content_selector};
use std::path::PathBuf;
use vapor_client_application::OperationKind;

impl StudioApp {
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
            &format!("Validating {selector} through the Studio service…"),
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
            "Validating the selected source through the Studio service…",
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
            "Validating authored content through the Studio service…",
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
            "Loading registered workspaces through the Studio service…",
        );
    }
}
