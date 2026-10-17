use super::StudioApp;
use super::operation_protocol::OperationResult;
use super::workspace_files::{
    create_new_workspace_file, discover_workspace_files, read_workspace_file, write_workspace_file,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use vapor_studio_core::OperationKind;

#[derive(Default)]
pub(super) struct WorkspaceEditorState {
    pub(super) new_file_path: String,
    pub(super) files: Vec<PathBuf>,
    pub(super) file_filter: String,
    pub(super) selected_file: Option<PathBuf>,
    pub(super) documents: BTreeMap<PathBuf, WorkspaceDocument>,
    pub(super) draft_recovery: Option<String>,
    pub(super) files_open: bool,
}

#[derive(Clone)]
pub(super) struct WorkspaceDocument {
    pub(super) original: String,
    pub(super) contents: String,
    dirty: bool,
    pub(super) line_count: usize,
    pub(super) character_count: usize,
    pub(super) toml_diagnostic: Option<Result<(), String>>,
}

impl WorkspaceDocument {
    pub(super) fn new(path: &Path, contents: String) -> Self {
        let original = contents.clone();
        Self::from_parts(path, original, contents)
    }

    fn from_parts(path: &Path, original: String, contents: String) -> Self {
        let mut document = Self {
            original,
            contents,
            dirty: false,
            line_count: 0,
            character_count: 0,
            toml_diagnostic: None,
        };
        document.refresh_inspection(path);
        document
    }

    pub(super) fn refresh_inspection(&mut self, path: &Path) {
        self.dirty = self.contents != self.original;
        self.line_count = self.contents.lines().count();
        self.character_count = self.contents.chars().count();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        self.toml_diagnostic = (name == "Vapor.toml"
            || name.ends_with(".vapor.toml")
            || name == "Cargo.toml")
        .then(|| {
            self.contents
                .parse::<toml::Value>()
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.dirty
    }
}

impl WorkspaceEditorState {
    pub(super) fn active_document(&self) -> Option<&WorkspaceDocument> {
        self.selected_file
            .as_ref()
            .and_then(|path| self.documents.get(path))
    }

    pub(super) fn active_document_mut(&mut self) -> Option<&mut WorkspaceDocument> {
        let path = self.selected_file.clone()?;
        self.documents.get_mut(&path)
    }

    pub(super) fn active_document_is_dirty(&self) -> bool {
        self.active_document()
            .is_some_and(WorkspaceDocument::is_dirty)
    }

    pub(super) fn has_dirty_documents(&self) -> bool {
        self.documents.values().any(WorkspaceDocument::is_dirty)
    }

    pub(super) fn recovery_summary(&self) -> String {
        let open = self.documents.len();
        let dirty = self
            .documents
            .values()
            .filter(|document| document.is_dirty())
            .count();
        format!("Recovered {open} editor tab(s), including {dirty} unsaved draft(s).")
    }

    pub(super) fn save_editor_drafts(&self, root: &str) -> Option<String> {
        if self.documents.is_empty() || root.trim().is_empty() {
            return None;
        }
        let documents = self
            .documents
            .iter()
            .map(|(path, document)| {
                (
                    path.to_string_lossy().into_owned(),
                    document.original.clone(),
                    document.contents.clone(),
                )
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&(
            root.to_owned(),
            self.selected_file
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            documents,
        ))
        .ok()
    }

    pub(super) fn restore_editor_drafts(&mut self, root: &Path) -> bool {
        let Some(encoded) = self.draft_recovery.take() else {
            return false;
        };
        let Ok((saved_root, selected, documents)) =
            serde_json::from_str::<(String, Option<String>, Vec<(String, String, String)>)>(
                &encoded,
            )
        else {
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
                || path
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
                || !self.files.contains(&path)
            {
                return false;
            }
            restored.insert(
                path.clone(),
                WorkspaceDocument::from_parts(&path, original, contents),
            );
        }
        self.documents = restored;
        self.selected_file = selected
            .map(PathBuf::from)
            .filter(|path| self.documents.contains_key(path));
        self.files_open = self.selected_file.is_some();
        self.files_open
    }

    pub(super) fn clear_context(&mut self) {
        self.files.clear();
        self.file_filter.clear();
        self.selected_file = None;
        self.documents.clear();
        self.files_open = false;
        self.new_file_path.clear();
    }
}

impl StudioApp {
    pub(super) fn sync_saved_workspace_manifest(&mut self, manifest: &Path, source: &str) {
        if let Ok(relative) = manifest.strip_prefix(Path::new(self.workspace.root.trim())) {
            if let Some(document) = self.workspace.editor.documents.get_mut(relative) {
                document.contents = source.to_owned();
                document.original = source.to_owned();
                document.refresh_inspection(relative);
            }
        }
    }

    pub(super) fn apply_workspace_files(&mut self, files: Vec<PathBuf>) {
        self.workspace.editor.files = files;
        let restored = self
            .workspace
            .editor
            .restore_editor_drafts(Path::new(self.workspace.root.trim()));
        self.status = format!(
            "Found {} workspace files.",
            self.workspace.editor.files.len()
        );
        if restored {
            self.workspace.editor.files_open = true;
            self.status.push(' ');
            self.status
                .push_str(&self.workspace.editor.recovery_summary());
        }
    }

    pub(super) fn apply_workspace_file_created(&mut self, path: PathBuf) {
        self.invalidate_workshop_review();
        if !self.workspace.editor.files.contains(&path) {
            self.workspace.editor.files.push(path.clone());
            self.workspace.editor.files.sort();
        }
        self.workspace.editor.selected_file = Some(path.clone());
        self.workspace.editor.files_open = true;
        self.workspace.editor.documents.insert(
            path.clone(),
            WorkspaceDocument::new(&path, String::new()),
        );
        self.workspace.editor.new_file_path.clear();
        self.status = format!("Created {}.", path.display());
    }

    pub(super) fn apply_workspace_file_opened(&mut self, path: PathBuf, contents: String) {
        self.workspace.editor.selected_file = Some(path.clone());
        self.workspace.editor.files_open = true;
        self.workspace.editor.documents.insert(
            path.clone(),
            WorkspaceDocument::new(&path, contents),
        );
        self.status = "Opened workspace file.".to_owned();
    }

    pub(super) fn apply_workspace_file_saved(&mut self, path: PathBuf) {
        self.invalidate_workshop_review();
        if let Some(document) = self.workspace.editor.documents.get_mut(&path) {
            document.original.clone_from(&document.contents);
            document.dirty = false;
        }
        self.status = format!("Saved {}.", path.display());
    }

    pub(super) fn apply_workspace_files_saved(&mut self, results: Vec<Result<PathBuf, String>>) {
        let mut saved = 0;
        let mut failures = Vec::new();
        for result in results {
            match result {
                Ok(path) => {
                    if let Some(document) = self.workspace.editor.documents.get_mut(&path) {
                        document.original.clone_from(&document.contents);
                        document.dirty = false;
                        saved += 1;
                    }
                }
                Err(error) => failures.push(error),
            }
        }
        if saved > 0 {
            self.invalidate_workshop_review();
        }
        self.status = if failures.is_empty() {
            format!("Saved {saved} workspace file(s).")
        } else {
            format!(
                "Saved {saved} file(s); {} remain unsaved: {}",
                failures.len(),
                failures.join("; ")
            )
        };
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
            self.status =
                "Save or discard structured manifest edits before opening another file.".to_owned();
            return;
        }
        if self.workspace.editor.documents.contains_key(&path) {
            self.workspace.editor.selected_file = Some(path.clone());
            self.workspace.editor.files_open = true;
            self.status = format!("Switched to {}.", path.display());
            return;
        }
        if self.workspace.editor.documents.len() >= 16 {
            self.status = "Close an editor tab before opening another file (16 open tabs maximum)."
                .to_owned();
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
        let Some(path) = self.workspace.editor.selected_file.clone() else {
            return;
        };
        let Some(document) = self.workspace.editor.active_document().cloned() else {
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

    pub(super) fn save_all_workspace_files(&mut self) {
        if !self.require_open_workspace_for_editor_save("save workspace files") {
            return;
        }
        let documents = self
            .workspace
            .editor
            .documents
            .iter()
            .filter(|(_, document)| document.is_dirty())
            .map(|(path, document)| (path.clone(), document.clone()))
            .collect::<Vec<_>>();
        if documents.is_empty() {
            self.status = "All open workspace files are saved.".to_owned();
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        self.spawn(OperationKind::SaveWorkspaceFile, move |_context| {
            OperationResult::WorkspaceFilesSaved(
                documents
                    .into_iter()
                    .map(|(path, document)| {
                        write_workspace_file(&root, &path, &document.original, &document.contents)
                            .map(|()| path)
                    })
                    .collect(),
            )
        });
        self.status = "Saving all dirty workspace files…".to_owned();
    }

    pub(super) fn reload_workspace_file(&mut self) {
        if !self.require_open_workspace_for_editor_save("reload a workspace file") {
            return;
        }
        let Some(path) = self.workspace.editor.selected_file.clone() else {
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
            .editor
            .documents
            .get(path)
            .is_some_and(WorkspaceDocument::is_dirty)
        {
            self.status =
                "Save the file before closing its tab so the draft stays recoverable.".to_owned();
            return;
        }
        self.workspace.editor.documents.remove(path);
        if self.workspace.editor.selected_file.as_deref() == Some(path) {
            self.workspace.editor.selected_file =
                self.workspace.editor.documents.keys().next().cloned();
        }
        self.workspace.editor.files_open = self.workspace.editor.selected_file.is_some();
    }

    pub(super) fn create_workspace_file(&mut self) {
        if !self.require_open_workspace("create a workspace file") {
            return;
        }
        let root = PathBuf::from(self.workspace.root.trim());
        let relative = PathBuf::from(self.workspace.editor.new_file_path.trim());
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
