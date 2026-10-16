use super::operation_protocol::OperationResult;
use super::workspace_files::{
    create_new_workspace_file, discover_workspace_files, read_workspace_file,
    write_workspace_file,
};
use super::workspace_ops::WorkspaceDocument;
use super::StudioApp;
use std::path::{Path, PathBuf};
use vapor_studio_core::OperationKind;

impl StudioApp {
    pub(super) fn sync_saved_workspace_manifest(&mut self, manifest: &Path, source: &str) {
        if let Ok(relative) = manifest.strip_prefix(Path::new(self.workspace.root.trim())) {
            if let Some(document) = self.workspace.editor_documents.get_mut(relative) {
                document.contents = source.to_owned();
                document.original = source.to_owned();
            }
        }
    }

    pub(super) fn apply_workspace_files(&mut self, files: Vec<PathBuf>) {
        self.workspace.files = files;
        let restored = self
            .workspace
            .restore_editor_drafts(&PathBuf::from(self.workspace.root.trim()));
        self.status = format!("Found {} workspace files.", self.workspace.files.len());
        if restored {
            self.workspace.files_open = true;
            self.status.push(' ');
            self.status.push_str(&self.workspace.editor_recovery_summary());
        }
    }

    pub(super) fn apply_workspace_file_created(&mut self, path: PathBuf) {
        self.invalidate_workshop_review();
        if !self.workspace.files.contains(&path) {
            self.workspace.files.push(path.clone());
            self.workspace.files.sort();
        }
        self.workspace.selected_file = Some(path.clone());
        self.workspace.files_open = true;
        self.workspace.editor_documents.insert(
            path.clone(),
            WorkspaceDocument {
                original: String::new(),
                contents: String::new(),
            },
        );
        self.workspace.new_file_path.clear();
        self.status = format!("Created {}.", path.display());
    }

    pub(super) fn apply_workspace_file_opened(&mut self, path: PathBuf, contents: String) {
        self.workspace.selected_file = Some(path.clone());
        self.workspace.files_open = true;
        self.workspace.editor_documents.insert(
            path,
            WorkspaceDocument {
                original: contents.clone(),
                contents,
            },
        );
        self.status = "Opened workspace file.".to_owned();
    }

    pub(super) fn apply_workspace_file_saved(&mut self, path: PathBuf) {
        self.invalidate_workshop_review();
        if let Some(document) = self.workspace.editor_documents.get_mut(&path) {
            document.original.clone_from(&document.contents);
        }
        self.status = format!("Saved {}.", path.display());
    }

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

    pub(super) fn open_workspace_file(&mut self, path: PathBuf) {
        if self.authoring.manifest_metadata_dirty {
            self.status = "Save or discard structured manifest edits before opening another file.".to_owned();
            return;
        }
        if self.workspace.editor_documents.contains_key(&path) {
            self.workspace.selected_file = Some(path.clone());
            self.workspace.files_open = true;
            self.status = format!("Switched to {}.", path.display());
            return;
        }
        if self.workspace.editor_documents.len() >= 16 {
            self.status = "Close an editor tab before opening another file (16 open tabs maximum).".to_owned();
            return;
        }
        if !self.require_workspace_association("open a workspace file") {
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
        if !self.require_open_workspace_for_editor_save("save a workspace file") {
            return;
        }
        let Some(path) = self.workspace.selected_file.clone() else {
            return;
        };
        let Some(document) = self.workspace.active_document().cloned() else {
            return;
        };
        let root = PathBuf::from(self.workspace.root.trim());
        self.spawn(OperationKind::SaveWorkspaceFile, move |_context| {
            OperationResult::FileSaved(
                write_workspace_file(&root, &path, &document.original, &document.contents)
                    .map(|()| path),
            )
        });
        self.status = "Saving workspace file…".to_owned();
    }

    pub(super) fn reload_workspace_file(&mut self) {
        if !self.require_open_workspace_for_editor_save("reload a workspace file") {
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

    pub(super) fn close_workspace_document(&mut self, path: &Path) {
        if self
            .workspace
            .editor_documents
            .get(path)
            .is_some_and(WorkspaceDocument::is_dirty)
        {
            self.status = "Save the file before closing its tab so the draft stays recoverable.".to_owned();
            return;
        }
        self.workspace.editor_documents.remove(path);
        if self.workspace.selected_file.as_deref() == Some(path) {
            self.workspace.selected_file = self.workspace.editor_documents.keys().next().cloned();
        }
        self.workspace.files_open = self.workspace.selected_file.is_some();
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
