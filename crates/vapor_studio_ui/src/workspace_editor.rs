use super::operations::OperationResult;
use super::workspace_files::{
    create_new_workspace_file, discover_workspace_files, read_workspace_file,
    write_workspace_file,
};
use super::StudioApp;
use std::path::PathBuf;
use vapor_studio_core::OperationKind;

impl StudioApp {
    pub(super) fn refresh_workspace_files(&mut self) {
        if !self.require_open_workspace("read workspace files") {
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        self.spawn(OperationKind::DiscoverWorkspace, move |_context| {
            OperationResult::WorkspaceFiles(discover_workspace_files(&root))
        });
        self.status = "Reading workspace files…".to_owned();
    }

    pub(super) fn refresh_workspace_editor_toml_diagnostic(&mut self) {
        let is_manifest = self.workspace.selected_file
            .as_deref()
            .is_some_and(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                name == "Vapor.toml" || name.ends_with(".vapor.toml") || name == "Cargo.toml"
            });
        self.workspace.editor_toml_diagnostic = is_manifest.then(|| {
            self.workspace
                .editor
                .parse::<toml::Value>()
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
    }

    pub(super) fn open_workspace_file(&mut self, path: PathBuf) {
        if !self.require_open_workspace("open a workspace file") {
            return;
        }
        if self.authoring.manifest_metadata_dirty {
            self.status = "Save or discard structured manifest edits before opening another file.".to_owned();
            return;
        }
        if self.workspace.editor_dirty {
            self.status = "Save or discard the current editor changes before opening another file."
                .to_owned();
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        self.spawn(OperationKind::OpenWorkspaceFile, move |_context| {
            OperationResult::FileContents(
                read_workspace_file(&root, &path).map(|contents| (path, contents)),
            )
        });
        self.status = "Opening workspace file…".to_owned();
    }

    pub(super) fn save_workspace_file(&mut self) {
        if !self.require_open_workspace("save a workspace file") {
            return;
        }
        let Some(path) = self.workspace.selected_file.clone() else {
            return;
        };
        let root = PathBuf::from(self.workspace.root.trim());
        let expected_contents = self.workspace.editor_original.clone();
        let contents = self.workspace.editor.clone();
        self.spawn(OperationKind::SaveWorkspaceFile, move |_context| {
            OperationResult::FileSaved(
                write_workspace_file(&root, &path, &expected_contents, &contents).map(|()| path),
            )
        });
        self.status = "Saving workspace file…".to_owned();
    }

    pub(super) fn reload_workspace_file(&mut self) {
        if !self.require_open_workspace("reload a workspace file") {
            return;
        }
        let Some(path) = self.workspace.selected_file.clone() else {
            return;
        };
        let root = PathBuf::from(self.workspace.root.trim());
        self.spawn(OperationKind::OpenWorkspaceFile, move |_context| {
            OperationResult::FileContents(
                read_workspace_file(&root, &path).map(|contents| (path, contents)),
            )
        });
        self.status = "Reloading workspace file from disk…".to_owned();
    }

    pub(super) fn create_workspace_file(&mut self) {
        if !self.require_open_workspace("create a workspace file") {
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        let relative = PathBuf::from(self.workspace.new_file_path.trim());
        if relative.as_os_str().is_empty() {
            self.status = "Enter a file path relative to the workspace root.".to_owned();
            return;
        }
        self.spawn(OperationKind::CreateWorkspaceFile, move |_context| {
            OperationResult::WorkspaceFileCreated(create_new_workspace_file(&root, &relative))
        });
        self.status = "Creating a new workspace file…".to_owned();
    }
}
