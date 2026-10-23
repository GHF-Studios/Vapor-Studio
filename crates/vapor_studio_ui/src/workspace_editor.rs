use super::StudioApp;
use std::collections::VecDeque;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use vapor_client_application::{OperationKind, OperationPhase, workspace_file_fingerprint};
use vapor_client_protocol::{ClientAction, WorkspaceAction, WorkspaceFileResult};
use vapor_gui_core::workspace_tree::WorkspaceFileTree;

const DOCUMENT_INSPECTION_DELAY: Duration = Duration::from_millis(350);

#[derive(Default)]
pub(super) struct WorkspaceEditorState {
    pub(super) new_file_path: String,
    pub(super) files: Vec<PathBuf>,
    pub(super) files_loaded: bool,
    pub(super) files_refresh_error: Option<String>,
    pub(super) file_tree: WorkspaceFileTree,
    pub(super) file_filter: String,
    pub(super) selected_file: Option<PathBuf>,
    pub(super) pending_reload_confirmation: Option<PathBuf>,
    pub(super) documents: BTreeMap<PathBuf, WorkspaceDocument>,
    pub(super) save_queue: VecDeque<PendingWorkspaceSave>,
    pub(super) save_in_flight: Option<PendingWorkspaceSave>,
    pub(super) save_batch: Option<WorkspaceSaveBatch>,
    pub(super) draft_recovery: Option<String>,
    pub(super) files_open: bool,
    pub(super) show_project_tree: Option<bool>,
    pub(super) show_inspector: Option<bool>,
}

#[derive(Clone)]
pub(super) struct PendingWorkspaceSave {
    path: PathBuf,
    expected_fingerprint: String,
    contents: String,
}

pub(super) struct WorkspaceSaveBatch {
    total: usize,
    saved: usize,
    failures: Vec<String>,
}

impl WorkspaceEditorState {
    pub(super) fn project_tree_visible(&self, editor_width: f32) -> bool {
        self.show_project_tree.unwrap_or(editor_width >= 760.0)
    }

    pub(super) fn inspector_visible(&self, editor_width: f32) -> bool {
        self.show_inspector.unwrap_or(editor_width >= 1120.0)
    }

    pub(super) fn toggle_project_tree(&mut self, editor_width: f32) {
        self.show_project_tree = Some(!self.project_tree_visible(editor_width));
    }

    pub(super) fn toggle_inspector(&mut self, editor_width: f32) {
        self.show_inspector = Some(!self.inspector_visible(editor_width));
    }
}

impl WorkspaceEditorState {
    pub(super) fn rebuild_file_index(&mut self) {
        self.file_tree = WorkspaceFileTree::new(&self.files);
    }

    pub(super) fn matching_file_count(&self, filter: &str) -> usize {
        self.file_tree.matching_file_count(filter)
    }
}

#[derive(Clone)]
pub(super) struct WorkspaceDocument {
    pub(super) original: String,
    pub(super) contents: String,
    pub(super) original_fingerprint: String,
    dirty: bool,
    pub(super) line_count: usize,
    pub(super) character_count: usize,
    pub(super) toml_diagnostic: Option<Result<(), String>>,
    pub(super) inspection_due: Option<Instant>,
}

impl WorkspaceDocument {
    pub(super) fn new(path: &Path, contents: String) -> Self {
        let original = contents.clone();
        let fingerprint = workspace_file_fingerprint(&original);
        Self::from_parts(path, original, contents, fingerprint)
    }

    pub(super) fn opened(path: &Path, contents: String, fingerprint: String) -> Self {
        Self::from_parts(path, contents.clone(), contents, fingerprint)
    }

    fn from_parts(
        path: &Path,
        original: String,
        contents: String,
        original_fingerprint: String,
    ) -> Self {
        let mut document = Self {
            original,
            contents,
            original_fingerprint,
            dirty: false,
            line_count: 0,
            character_count: 0,
            toml_diagnostic: None,
            inspection_due: None,
        };
        document.refresh_inspection(path);
        document
    }

    pub(super) fn refresh_inspection(&mut self, path: &Path) {
        self.dirty = self.contents != self.original;
        self.line_count = self.contents.lines().count();
        self.character_count = self.contents.chars().count();
        self.inspect_toml_now(path);
    }

    pub(super) fn schedule_inspection(&mut self, _path: &Path) {
        // Keep the editor responsive for large files: all document-wide work is
        // deferred until typing pauses. Mark dirty immediately so save and
        // navigation safeguards remain conservative while the counts/diagnostic
        // catch up.
        self.dirty = true;
        self.toml_diagnostic = None;
        self.inspection_due = Some(Instant::now() + DOCUMENT_INSPECTION_DELAY);
    }

    pub(super) fn refresh_inspection_if_due(&mut self, path: &Path, now: Instant) {
        if self.inspection_due.is_some_and(|deadline| deadline <= now) {
            self.refresh_inspection(path);
        }
    }

    fn inspect_toml_now(&mut self, path: &Path) {
        self.inspection_due = None;
        self.toml_diagnostic = is_toml_file(path).then(|| {
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

fn is_toml_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name == "Vapor.toml" || name.ends_with(".vapor.toml") || name == "Cargo.toml"
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
        format!(
            "Recovered {open} editor tab{}, including {dirty} unsaved draft{}.",
            if open == 1 { "" } else { "s" },
            if dirty == 1 { "" } else { "s" }
        )
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
                WorkspaceDocument::from_parts(
                    &path,
                    original.clone(),
                    contents,
                    workspace_file_fingerprint(&original),
                ),
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
        self.files_loaded = false;
        self.files_refresh_error = None;
        self.file_tree = WorkspaceFileTree::default();
        self.file_filter.clear();
        self.selected_file = None;
        self.pending_reload_confirmation = None;
        self.documents.clear();
        self.save_queue.clear();
        self.save_in_flight = None;
        self.save_batch = None;
        self.files_open = false;
        self.new_file_path.clear();
    }
}

impl StudioApp {
    pub(super) fn sync_saved_workspace_manifest(&mut self, manifest: &Path, source: &str) {
        if let Ok(relative) = manifest.strip_prefix(Path::new(self.workspace.root.trim()))
            && let Some(document) = self.workspace.editor.documents.get_mut(relative)
        {
            document.contents = source.to_owned();
            document.original = source.to_owned();
            document.refresh_inspection(relative);
        }
    }

    pub(super) fn apply_workspace_files(&mut self, files: Vec<PathBuf>) {
        self.workspace.editor.files = files;
        self.workspace.editor.files_loaded = true;
        self.workspace.editor.files_refresh_error = None;
        self.workspace.editor.rebuild_file_index();
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
        self.workspace.editor.files_loaded = true;
        if !self.workspace.editor.files.contains(&path) {
            self.workspace.editor.files.push(path.clone());
            self.workspace.editor.files.sort();
            self.workspace.editor.rebuild_file_index();
        }
        self.workspace.editor.selected_file = Some(path.clone());
        self.workspace.editor.files_open = true;
        self.workspace
            .editor
            .documents
            .insert(path.clone(), WorkspaceDocument::new(&path, String::new()));
        self.workspace.editor.new_file_path.clear();
        self.status = format!("Created {}.", path.display());
    }

    pub(super) fn apply_workspace_file_opened(
        &mut self,
        path: PathBuf,
        contents: String,
        fingerprint: String,
    ) {
        self.workspace.editor.selected_file = Some(path.clone());
        self.workspace.editor.files_open = true;
        self.workspace.editor.documents.insert(
            path.clone(),
            WorkspaceDocument::opened(&path, contents, fingerprint),
        );
        self.status = "Opened workspace file.".to_owned();
    }

    pub(super) fn apply_workspace_file_saved(
        &mut self,
        path: PathBuf,
        contents: String,
        fingerprint: String,
    ) {
        self.invalidate_workshop_review();
        if let Some(document) = self.workspace.editor.documents.get_mut(&path) {
            document.original = contents;
            document.original_fingerprint = fingerprint;
            document.refresh_inspection(&path);
        }
        self.status = format!("Saved {}.", path.display());
    }

    pub(super) fn refresh_workspace_files(&mut self) {
        if self.operation.running {
            self.status =
                "Wait for the current operation to finish before refreshing files.".to_owned();
            return;
        }
        if !self.require_open_workspace("read workspace files") {
            return;
        }
        self.start_service_operation(
            ClientAction::Workspace(WorkspaceAction::ListFiles { path: ".".into() }),
            None,
            OperationKind::DiscoverWorkspace,
            OperationPhase::Resolving,
            "Reading workspace files…",
        );
    }

    pub(super) fn open_workspace_file(&mut self, path: PathBuf) {
        if self.operation.running {
            self.status =
                "Wait for the current operation to finish before opening a file.".to_owned();
            return;
        }
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
        self.start_service_operation(
            ClientAction::Workspace(WorkspaceAction::OpenFile {
                path: path.to_string_lossy().into_owned(),
            }),
            None,
            OperationKind::OpenWorkspaceFile,
            OperationPhase::Resolving,
            "Opening workspace file…",
        );
    }

    pub(super) fn save_workspace_file(&mut self) {
        if self.operation.running {
            self.status =
                "Wait for the current operation to finish before saving files.".to_owned();
            return;
        }
        if !self.require_open_workspace_for_editor_save("save a workspace file") {
            return;
        }
        let Some(path) = self.workspace.editor.selected_file.clone() else {
            return;
        };
        let Some(document) = self.workspace.editor.active_document().cloned() else {
            return;
        };
        self.workspace.editor.save_in_flight = Some(PendingWorkspaceSave {
            path: path.clone(),
            expected_fingerprint: document.original_fingerprint.clone(),
            contents: document.contents.clone(),
        });
        self.start_workspace_file_save();
    }

    pub(super) fn save_all_workspace_files(&mut self) {
        if self.operation.running {
            self.status =
                "Wait for the current operation to finish before saving files.".to_owned();
            return;
        }
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
        let total = documents.len();
        self.workspace.editor.save_batch = Some(WorkspaceSaveBatch {
            total,
            saved: 0,
            failures: Vec::new(),
        });
        self.workspace.editor.save_queue = documents
            .into_iter()
            .map(|(path, document)| PendingWorkspaceSave {
                path,
                expected_fingerprint: document.original_fingerprint,
                contents: document.contents,
            })
            .collect();
        self.start_next_workspace_file_save();
    }

    pub(super) fn reload_workspace_file(&mut self) {
        if self.operation.running {
            self.status =
                "Wait for the current operation to finish before reloading a file.".to_owned();
            return;
        }
        if !self.require_open_workspace_for_editor_save("reload a workspace file") {
            return;
        }
        let Some(path) = self.workspace.editor.selected_file.clone() else {
            return;
        };
        self.start_service_operation(
            ClientAction::Workspace(WorkspaceAction::OpenFile {
                path: path.to_string_lossy().into_owned(),
            }),
            None,
            OperationKind::OpenWorkspaceFile,
            OperationPhase::Resolving,
            "Reloading workspace file from disk…",
        );
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
        if self.operation.running {
            self.status =
                "Wait for the current operation to finish before creating a file.".to_owned();
            return;
        }
        if !self.require_open_workspace("create a workspace file") {
            return;
        }
        let relative = PathBuf::from(self.workspace.editor.new_file_path.trim());
        if relative.as_os_str().is_empty() {
            self.status = "Enter a file path relative to the workspace root.".to_owned();
            return;
        }
        self.start_service_operation(
            ClientAction::Workspace(WorkspaceAction::CreateFile {
                path: relative.to_string_lossy().into_owned(),
                contents: String::new(),
            }),
            None,
            OperationKind::CreateWorkspaceFile,
            OperationPhase::Building,
            "Creating a new workspace file…",
        );
    }

    pub(super) fn apply_service_workspace_file_result(&mut self, result: WorkspaceFileResult) {
        match result {
            WorkspaceFileResult::Listed {
                operation_id,
                files,
            } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                self.apply_workspace_files(files.into_iter().map(PathBuf::from).collect());
                self.finish_service_result_operation(&operation_id);
            }
            WorkspaceFileResult::Opened {
                operation_id,
                path,
                contents,
                fingerprint,
            } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                self.apply_workspace_file_opened(PathBuf::from(path), contents, fingerprint);
                self.finish_service_result_operation(&operation_id);
            }
            WorkspaceFileResult::Saved {
                operation_id,
                path,
                fingerprint,
            } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                let path = PathBuf::from(path);
                if let Some(saved) = self.workspace.editor.save_in_flight.take()
                    && saved.path == path
                {
                    self.apply_workspace_file_saved(path, saved.contents, fingerprint);
                    if let Some(batch) = self.workspace.editor.save_batch.as_mut() {
                        batch.saved += 1;
                    }
                }
                self.finish_service_result_operation(&operation_id);
                self.start_next_workspace_file_save();
            }
            WorkspaceFileResult::Created { operation_id, path } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                self.apply_workspace_file_created(PathBuf::from(path));
                self.finish_service_result_operation(&operation_id);
            }
            WorkspaceFileResult::CanonicalRustModuleCreated {
                operation_id,
                module_path,
                module_contents,
                module_fingerprint,
                parent_path,
                parent_contents,
                parent_fingerprint,
            } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                let module_path = PathBuf::from(module_path);
                let parent_path = PathBuf::from(parent_path);
                if let Some(document) = self.workspace.editor.documents.get_mut(&parent_path) {
                    let mut current = document.contents.clone();
                    let module_name = module_path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default();
                    let declaration = format!("pub mod {module_name};");
                    if !current.lines().any(|line| line.trim() == declaration) {
                        if !current.is_empty() && !current.ends_with('\n') {
                            current.push('\n');
                        }
                        if !current.is_empty() && !current.ends_with("\n\n") {
                            current.push('\n');
                        }
                        current.push_str(&declaration);
                        current.push('\n');
                    }
                    document.original = parent_contents;
                    document.original_fingerprint = parent_fingerprint;
                    document.contents = current;
                    document.refresh_inspection(&parent_path);
                }
                self.apply_workspace_file_created(module_path.clone());
                self.apply_workspace_file_opened(module_path, module_contents, module_fingerprint);
                self.finish_service_result_operation(&operation_id);
            }
            WorkspaceFileResult::Completed { operation_id } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                self.finish_service_result_operation(&operation_id);
            }
            WorkspaceFileResult::RememberedSources {
                operation_id,
                roots,
            } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                self.apply_remembered_sources(roots.into_iter().map(PathBuf::from).collect());
                self.finish_service_result_operation(&operation_id);
            }
            WorkspaceFileResult::UnregisteredContent {
                operation_id,
                content_id,
                version,
            } => {
                if !self.workspace_file_result_matches_active(&operation_id) {
                    return;
                }
                self.workspace.pending_unregistration = None;
                self.invalidate_compositions_for_source_content(
                    &vapor_client_application::ContentReference {
                        id: content_id,
                        version,
                    },
                );
                self.finish_service_result_operation(&operation_id);
            }
        }
    }

    fn workspace_file_result_matches_active(&self, operation_id: &str) -> bool {
        self.operation
            .active_operation
            .as_ref()
            .is_some_and(|operation| operation.id.0.to_string() == operation_id)
    }

    pub(super) fn fail_service_workspace_save(&mut self, message: String) {
        if let Some(save) = self.workspace.editor.save_in_flight.take() {
            if let Some(batch) = self.workspace.editor.save_batch.as_mut() {
                batch
                    .failures
                    .push(format!("{}: {message}", save.path.display()));
            } else {
                self.status = message;
            }
        }
        self.start_next_workspace_file_save();
    }

    fn start_workspace_file_save(&mut self) {
        let Some(save) = self.workspace.editor.save_in_flight.as_ref() else {
            return;
        };
        self.start_service_operation(
            ClientAction::Workspace(WorkspaceAction::SaveFile {
                path: save.path.to_string_lossy().into_owned(),
                expected_fingerprint: save.expected_fingerprint.clone(),
                contents: save.contents.clone(),
            }),
            None,
            OperationKind::SaveWorkspaceFile,
            OperationPhase::Building,
            "Saving workspace file…",
        );
        if !self.operation.service_owned
            && let Some(save) = self.workspace.editor.save_in_flight.take()
        {
            if let Some(batch) = self.workspace.editor.save_batch.as_mut() {
                batch.failures.push(format!(
                    "{}: the Studio service could not start the save",
                    save.path.display()
                ));
                self.start_next_workspace_file_save();
            } else {
                self.status = "The Studio service could not start the save.".to_owned();
            }
        }
    }

    fn start_next_workspace_file_save(&mut self) {
        if self.operation.running || self.workspace.editor.save_in_flight.is_some() {
            return;
        }
        if let Some(save) = self.workspace.editor.save_queue.pop_front() {
            self.workspace.editor.save_in_flight = Some(save);
            self.start_workspace_file_save();
            return;
        }
        let Some(batch) = self.workspace.editor.save_batch.take() else {
            return;
        };
        if batch.failures.is_empty() {
            self.status = format!("Saved all {} workspace files.", batch.saved);
        } else {
            self.status = format!(
                "Saved {} of {} workspace files; {} remain unsaved: {}",
                batch.saved,
                batch.total,
                batch.failures.len(),
                batch.failures.join("; ")
            );
        }
    }
}
