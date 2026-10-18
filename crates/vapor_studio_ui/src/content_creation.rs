use super::StudioApp;
use super::workspace_editor::WorkspaceDocument;
use crate::navigation::Section;
use std::path::PathBuf;
use vapor_client_protocol::{AuthoringAction, ClientAction, WorkshopMetadataDraft};
use vapor_client_application::OperationKind;

pub(super) fn starter_workspace_slug_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

impl StudioApp {
    pub(super) fn apply_content_workspace_created(
        &mut self,
        manifest: PathBuf,
        contents: String,
        files: Vec<PathBuf>,
    ) {
        self.invalidate_workshop_review();
        self.authoring.create.reset();
        self.workspace.editor.files = files;
        self.workspace.editor.files_loaded = true;
        self.workspace.editor.files_refresh_error = None;
        self.workspace.editor.rebuild_file_index();
        self.workspace.editor.selected_file = Some(manifest.clone());
        self.workspace.editor.files_open = true;
        self.workspace.editor.documents.insert(
            manifest.clone(),
            WorkspaceDocument::new(&manifest, contents),
        );
        self.activate_navigation_section(Section::Workspace);
        self.status = "Created content and opened its manifest.".to_owned();
    }

    pub(super) fn create_content_workspace(&mut self) {
        if !self.require_open_workspace("create content in this workspace") {
            return;
        }
        let root = self.workspace.root.trim().to_owned();
        let id = self.authoring.create.content_id.trim().to_owned();
        let version = self.authoring.create.content_version.trim().to_owned();
        let selected = self.authoring.create.dependencies.clone();
        let app_id = match self.authoring.create.workshop_app_id.trim() {
            "" => None,
            value => match value.parse::<u32>() {
                Ok(value) if value > 0 => Some(value),
                _ => {
                    self.status = "Steam AppID must be a positive number.".to_owned();
                    return;
                }
            },
        };
        let non_empty = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        let workshop = WorkshopMetadataDraft {
            app_id,
            published_file_id: non_empty(&self.authoring.create.workshop_published_file_id),
            visibility: non_empty(&self.authoring.create.workshop_visibility),
            title: non_empty(&self.authoring.create.workshop_title),
            description: non_empty(&self.authoring.create.workshop_description),
            tags: self
                .authoring
                .create
                .workshop_tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            change_note: non_empty(&self.authoring.create.workshop_change_note),
        };
        let kind = self.authoring.create.content_kind;
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::CreateContent {
                workspace_root: root,
                kind: kind.as_str().to_owned(),
                id,
                version,
                dependencies: selected,
                workshop,
            }),
            None,
            OperationKind::CreateContent,
            vapor_client_application::OperationPhase::Building,
            "Creating content and opening its manifest…",
        );
    }

    pub(super) fn create_starter_workspace(&mut self) {
        if !self.require_saved_workspace("create and open a starter workspace") {
            return;
        }
        if self.authoring.create.is_dirty() {
            self.status =
                "Discard the Create draft before creating and opening a new workspace.".to_owned();
            return;
        }
        let location = PathBuf::from(self.workspace.starter_path.trim());
        if location.as_os_str().is_empty() {
            self.status = "Choose a parent folder for the new starter workspace.".to_owned();
            return;
        }
        let organization = self.workspace.starter_organization.trim().to_owned();
        let name = self.workspace.starter_name.trim().to_owned();
        if !starter_workspace_slug_is_valid(&name) {
            self.status =
                "Workspace name must use lowercase letters, digits, and internal hyphens."
                    .to_owned();
            return;
        }
        if !starter_workspace_slug_is_valid(&organization) {
            self.status =
                "Organization must use lowercase letters, digits, and internal hyphens.".to_owned();
            return;
        }
        self.start_service_operation(
            ClientAction::Authoring(AuthoringAction::CreateWorkspace {
                parent: location.to_string_lossy().into_owned(),
                organization,
                name,
            }),
            None,
            OperationKind::CreateWorkspace,
            vapor_client_application::OperationPhase::Building,
            "Creating a playable sample game and its project folders…",
        );
    }
}
