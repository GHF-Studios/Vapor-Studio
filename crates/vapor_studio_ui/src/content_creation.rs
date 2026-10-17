use super::StudioApp;
use super::operation_protocol::{OperationResult, WorkspaceOpenSnapshot};
use super::workspace_editor::WorkspaceDocument;
use super::workspace_files::discover_workspace_files;
use crate::navigation::Section;
use std::{fs, path::PathBuf};
use vapor_studio_core::OperationKind;

impl StudioApp {
    pub(super) fn apply_content_workspace_created(
        &mut self,
        manifest: PathBuf,
        contents: String,
        files: Vec<PathBuf>,
    ) {
        self.invalidate_workshop_review();
        self.authoring.create_details_open = false;
        self.workspace.editor.files = files;
        self.workspace.editor.selected_file = Some(manifest.clone());
        self.workspace.editor.files_open = true;
        self.workspace.editor.documents.insert(
            manifest,
            WorkspaceDocument {
                original: contents.clone(),
                contents,
            },
        );
        self.section = Section::Workspace;
        self.status = "Created content and opened its manifest.".to_owned();
    }

    pub(super) fn create_content_workspace(&mut self) {
        if !self.require_open_workspace("create content in this workspace") {
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
        let id = self.authoring.draft_content_id.trim().to_owned();
        let version = self.authoring.draft_content_version.trim().to_owned();
        let selected = self.authoring.draft_dependencies.clone();
        let app_id = match self.authoring.draft_workshop_app_id.trim() {
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
        let workshop = vapor_studio_core::WorkshopPolicyDraft {
            app_id,
            published_file_id: non_empty(&self.authoring.draft_workshop_published_file_id),
            visibility: non_empty(&self.authoring.draft_workshop_visibility),
            title: non_empty(&self.authoring.draft_workshop_title),
            description: non_empty(&self.authoring.draft_workshop_description),
            tags: self
                .authoring
                .draft_workshop_tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            change_note: non_empty(&self.authoring.draft_workshop_change_note),
        };
        let kind = self.authoring.draft_content_kind;
        self.spawn(OperationKind::CreateContent, move |_context| {
            let result = vapor_studio_core::create_content_workspace_with_workshop(
                &executable,
                &root,
                &id,
                &version,
                kind,
                &selected,
                &workshop,
            )
            .and_then(|output| {
                let workspace = fs::canonicalize(&root)
                    .map_err(|error| format!("{}: {error}", root.display()))?;
                let manifest = output;
                let relative = manifest
                    .strip_prefix(&workspace)
                    .map(PathBuf::from)
                    .map_err(|error| {
                        format!("created manifest is outside the workspace: {error}")
                    })?;
                let contents = fs::read_to_string(&manifest)
                    .map_err(|error| format!("{}: {error}", manifest.display()))?;
                let files = discover_workspace_files(&workspace)?;
                Ok((relative, contents, files))
            });
            OperationResult::ContentWorkspace(result)
        });
        self.status = "Creating content and opening its manifest…".to_owned();
    }

    pub(super) fn create_starter_workspace(&mut self) {
        if !self.require_saved_workspace("create and open a starter workspace") {
            return;
        }
        let location = PathBuf::from(self.workspace.starter_path.trim());
        if location.as_os_str().is_empty() {
            self.status = "Choose a parent folder for the new starter workspace.".to_owned();
            return;
        }
        let organization = self.workspace.starter_organization.trim().to_owned();
        let name = self.workspace.starter_name.trim().to_owned();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || !name
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || !name
                .bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            self.status = "Workspace name must use lowercase letters, digits, and internal hyphens.".to_owned();
            return;
        }
        let destination = location.join(&name);
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let open_executable = executable.clone();
        self.spawn(OperationKind::CreateWorkspace, move |_context| {
            let result = vapor_studio_core::create_basic_content_workspace(
                &executable,
                &destination,
                &organization,
                &name,
            )
            .and_then(|root| {
                vapor_studio_core::discover_source_content(&open_executable, &root)
                    .and_then(|contents| {
                        let unregistered_projects =
                            vapor_studio_core::scan_unregistered_content_projects(
                                &open_executable,
                                &root,
                            );
                        let catalog = vapor_studio_core::discover_content_catalog(
                            &open_executable,
                            Some(&root),
                        );
                        discover_workspace_files(&root).map(|files| WorkspaceOpenSnapshot {
                            root: root.clone(),
                            landing_section: crate::navigation::Section::Workspace,
                            source_content: contents,
                            unregistered_projects,
                            local_catalog: catalog,
                            workspace_files: files,
                        })
                    })
                    .map_err(|error| {
                        format!(
                            "Created workspace at {}, but Studio could not open it: {error}",
                            root.display()
                        )
                    })
            });
            OperationResult::WorkspaceSelection(result)
        });
        self.status =
            "Creating the maintained Engine, Game, and Packagepack starter workspace…".to_owned();
    }
}
