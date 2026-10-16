use super::super::StudioApp;
use eframe::egui;
use std::{
    collections::BTreeMap,
    path::PathBuf,
};

#[derive(Default)]
struct WorkspaceTreeNode {
    entries: BTreeMap<String, WorkspaceTreeChild>,
}

enum WorkspaceTreeChild {
    Directory(WorkspaceTreeNode),
    File(PathBuf),
}

impl StudioApp {
    pub(super) fn draw_workspace_files_panel(&mut self, ui: &mut egui::Ui) {
            ui.horizontal(|ui| {
                ui.label("Workspace files");
                ui.weak(&self.workspace.root);
            });
            ui.horizontal(|ui| {
                ui.label("New file");
                ui.add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::TextEdit::singleline(&mut self.workspace.editor.new_file_path)
                        .desired_width(280.0)
                        .hint_text("scripts/example.rhai"),
                );
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Create file"),
                    )
                    .clicked()
                {
                    self.create_workspace_file();
                }
            });
            ui.weak("Creates a new text file inside this workspace, making missing parent folders as needed; existing files are never overwritten.");
            ui.horizontal(|ui| {
                ui.heading("Files");
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Refresh"),
                    )
                    .clicked()
                {
                    self.refresh_workspace_files();
                }
                if let Some(path) = &self.workspace.editor.selected_file {
                    ui.weak(path.display().to_string());
                }
                if ui
                    .add_enabled(
                        !self.operation.running
                            && self.workspace.editor.active_document_is_dirty()
                            && !self.authoring.manifest_metadata_dirty,
                        egui::Button::new("Save"),
                    )
                    .clicked()
                {
                    self.save_workspace_file();
                }
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.authoring.manifest_metadata_dirty
                            && self.workspace.editor.selected_file.is_some(),
                        egui::Button::new("Reload from disk"),
                    )
                    .on_hover_text("Discard the editor buffer and reopen the current on-disk file.")
                    .clicked()
                {
                    self.reload_workspace_file();
                }
                if self.workspace.editor.active_document_is_dirty() {
                    ui.colored_label(egui::Color32::YELLOW, "Unsaved");
                }
                let dirty_count = self
                    .workspace
                    .editor
                    .documents
                    .values()
                    .filter(|document| document.is_dirty())
                    .count();
                if ui
                    .add_enabled(
                        !self.operation.running
                            && dirty_count > 0
                            && !self.authoring.manifest_metadata_dirty,
                        egui::Button::new(format!("Save all ({dirty_count})")),
                    )
                    .clicked()
                {
                    self.save_all_workspace_files();
                }
            });
            let open_tabs = self
                .workspace
                .editor
                .documents
                .iter()
                .map(|(path, document)| (path.clone(), document.is_dirty()))
                .collect::<Vec<_>>();
            let mut select_tab = None;
            let mut close_tab = None;
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (path, dirty) in &open_tabs {
                        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("file");
                        let label = if *dirty { format!("{name} •") } else { name.to_owned() };
                        if ui
                            .add_enabled(
                                !self.operation.running,
                                egui::Button::selectable(
                                    self.workspace.editor.selected_file.as_ref() == Some(path),
                                    label,
                                ),
                            )
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            select_tab = Some(path.clone());
                        }
                        if ui
                            .add_enabled(!*dirty && !self.operation.running, egui::Button::new("×"))
                            .on_disabled_hover_text("Save this tab before closing it.")
                            .clicked()
                        {
                            close_tab = Some(path.clone());
                        }
                    }
                });
            });
            if let Some(path) = select_tab {
                self.workspace.editor.selected_file = Some(path);
                self.workspace.editor.files_open = true;
            }
            if let Some(path) = close_tab {
                self.close_workspace_document(&path);
            }
            ui.columns(3, |columns| {
                columns[0].heading("Project");
                let filter_response = columns[0].add(
                    egui::TextEdit::singleline(&mut self.workspace.editor.file_filter)
                        .hint_text("Find file by name or path"),
                );
                let filter = self.workspace.editor.file_filter.trim().to_lowercase();
                let filtered_files = self
                    .workspace
                    .editor
                    .files
                    .iter()
                    .filter(|path| {
                        filter.is_empty()
                            || path.to_string_lossy().to_lowercase().contains(&filter)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                columns[0].small(format!(
                    "{} of {} files",
                    filtered_files.len(),
                    self.workspace.editor.files.len()
                ));
                let enter_opens_match = filter_response.lost_focus()
                    && columns[0].input(|input| input.key_pressed(egui::Key::Enter))
                    && filtered_files.len() == 1;
                let tree = workspace_tree(&filtered_files);
                let mut open_file = None;
                egui::ScrollArea::vertical().show(&mut columns[0], |ui| {
                    if tree.entries.is_empty() {
                        ui.weak(if filter.is_empty() {
                            "No visible files in this workspace."
                        } else {
                            "No files match this path."
                        });
                    } else {
                        draw_workspace_tree(
                            ui,
                            &tree,
                            self.workspace.editor.selected_file.as_ref(),
                            !self.operation.running && !self.authoring.manifest_metadata_dirty,
                            &PathBuf::new(),
                            0,
                            &mut open_file,
                        );
                    }
                });
                if enter_opens_match {
                    open_file = filtered_files.first().cloned();
                }
                if let Some(path) = open_file {
                    self.open_workspace_file(path);
                }
                let selected_path = self.workspace.editor.selected_file.clone();
                let can_edit = !self.operation.running && !self.authoring.manifest_metadata_dirty;
                if let Some(document) = self.workspace.editor.active_document_mut() {
                    columns[1].push_id(selected_path.as_deref(), |ui| {
                        ui.add_enabled_ui(can_edit, |ui| {
                            egui::TextEdit::multiline(&mut document.contents)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(24)
                                .show(ui)
                        });
                    });
                } else {
                    columns[1].weak("Open a file from the project tree to begin editing.");
                }
                let active_contents = self
                    .workspace
                    .editor
                    .active_document()
                    .map(|document| document.contents.clone())
                    .unwrap_or_default();
                let active_dirty = self.workspace.editor.active_document_is_dirty();
                let diagnostic = selected_path.as_deref().and_then(|path| {
                    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
                    (name == "Vapor.toml" || name.ends_with(".vapor.toml") || name == "Cargo.toml")
                        .then(|| active_contents.parse::<toml::Value>().map(|_| ()).map_err(|error| error.to_string()))
                });
                columns[2].heading("Inspector");
                columns[2].small(format!("Workspace\n{}", self.workspace.root));
                if let Some(path) = &selected_path {
                    columns[2].separator();
                    columns[2].label("File");
                    columns[2].small(path.display().to_string());
                    columns[2].label(format!("{} file", workspace_file_kind(path)));
                    columns[2].label(format!(
                        "{} lines · {} characters",
                        active_contents.lines().count(),
                        active_contents.chars().count()
                    ));
                    columns[2].label(if active_dirty {
                        "Unsaved changes"
                    } else {
                        "Saved"
                    });
                    if let Some(diagnostic) = &diagnostic {
                        match diagnostic {
                            Ok(()) => {
                                columns[2]
                                    .colored_label(egui::Color32::LIGHT_GREEN, "TOML syntax valid");
                            }
                            Err(error) => {
                                columns[2]
                                    .colored_label(egui::Color32::LIGHT_RED, "TOML syntax error")
                                    .on_hover_text(error);
                            }
                        }
                    }
                }
                columns[2].separator();
                if let Some(path) = &selected_path {
                    columns[2].weak(workspace_file_guidance(path));
                } else {
                    columns[2].weak("Select a file to inspect and edit it.");
                }
        });
    }
}

fn workspace_tree(files: &[PathBuf]) -> WorkspaceTreeNode {
    let mut root = WorkspaceTreeNode::default();
    for file in files {
        let components = file
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>();
        insert_workspace_tree_entry(&mut root, &components, file);
    }
    root
}

fn insert_workspace_tree_entry(
    node: &mut WorkspaceTreeNode,
    parts: &[String],
    path: &std::path::Path,
) {
    let Some((name, rest)) = parts.split_first() else {
        return;
    };
    if rest.is_empty() {
        node.entries
            .insert(name.clone(), WorkspaceTreeChild::File(path.to_path_buf()));
        return;
    }
    let entry = node
        .entries
        .entry(name.clone())
        .or_insert_with(|| WorkspaceTreeChild::Directory(WorkspaceTreeNode::default()));
    if let WorkspaceTreeChild::Directory(child) = entry {
        insert_workspace_tree_entry(child, rest, path);
    }
}

fn draw_workspace_tree(
    ui: &mut egui::Ui,
    node: &WorkspaceTreeNode,
    selected: Option<&PathBuf>,
    enabled: bool,
    parent_path: &std::path::Path,
    depth: usize,
    open_file: &mut Option<PathBuf>,
) {
    for (name, child) in &node.entries {
        match child {
            WorkspaceTreeChild::Directory(directory) => {
                let directory_path = parent_path.join(name);
                let _ = egui::CollapsingHeader::new(name)
                    .id_salt(("workspace-tree", &directory_path))
                    .default_open(depth == 0)
                    .show(ui, |ui| {
                        draw_workspace_tree(
                            ui,
                            directory,
                            selected,
                            enabled,
                            &directory_path,
                            depth + 1,
                            open_file,
                        )
                    });
            }
            WorkspaceTreeChild::File(path) => {
                let response = ui.add_enabled(
                    enabled,
                    egui::Button::selectable(selected == Some(path), name),
                );
                if response.clicked() {
                    *open_file = Some(path.clone());
                }
            }
        }
    }
}

fn workspace_file_kind(path: &std::path::Path) -> &'static str {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name == "Vapor.toml" || name.ends_with(".vapor.toml") || name == "Workspace.vapor.toml" {
        "Vapor manifest"
    } else if name == "Cargo.toml" {
        "Cargo manifest"
    } else if path.extension().and_then(|extension| extension.to_str()) == Some("rhai") {
        "Rhai script"
    } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
        "Rust source"
    } else if matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("md" | "mdx")
    ) {
        "Documentation"
    } else {
        "Text source"
    }
}

fn workspace_file_guidance(path: &std::path::Path) -> &'static str {
    match workspace_file_kind(path) {
        "Vapor manifest" => "Defines Vapor workspace or content identity and lifecycle metadata.",
        "Cargo manifest" => {
            "Defines the Rust package and build targets used by Vapor-managed operations."
        }
        "Rhai script" => {
            "Script content is authored here; runtime capabilities come from the host that invokes it."
        }
        "Rust source" => {
            "Rust source is built only through the selected workspace's Vapor-managed toolchain."
        }
        "Documentation" => "Project documentation stored in this workspace.",
        _ => "Plain text file in the selected workspace.",
    }
}
