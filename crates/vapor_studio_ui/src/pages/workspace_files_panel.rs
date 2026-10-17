use super::super::StudioApp;
use eframe::egui;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Default)]
struct WorkspaceTreeNode<'a> {
    entries: BTreeMap<String, WorkspaceTreeChild<'a>>,
}

enum WorkspaceTreeChild<'a> {
    Directory(WorkspaceTreeNode<'a>),
    File(&'a std::path::Path),
}

impl StudioApp {
    pub(super) fn draw_workspace_files_panel(&mut self, ui: &mut egui::Ui) {
        let save_shortcut = ui.input(|input| {
            input.modifiers.command && input.key_pressed(egui::Key::S) && !input.modifiers.shift
        });
        let save_all_shortcut = ui.input(|input| {
            input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::S)
        });
        let quick_open_requested = ui.input(|input| {
            input.modifiers.command && input.key_pressed(egui::Key::P)
        });
        let dirty_count = self
            .workspace
            .editor
            .documents
            .values()
            .filter(|document| document.is_dirty())
            .count();
        if !self.operation.running && !self.authoring.manifest_metadata_dirty {
            if save_all_shortcut && dirty_count > 0 {
                self.save_all_workspace_files();
            } else if save_shortcut && self.workspace.editor.active_document_is_dirty() {
                self.save_workspace_file();
            }
        }
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
        ui.horizontal_wrapped(|ui| {
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
                .on_hover_text("Save the current file (Ctrl/Cmd+S).")
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
            if ui
                .add_enabled(
                    !self.operation.running
                        && dirty_count > 0
                        && !self.authoring.manifest_metadata_dirty,
                    egui::Button::new(format!("Save all ({dirty_count})")),
                )
                .on_hover_text("Save every open file (Ctrl/Cmd+Shift+S).")
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
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("file");
                    let label = if *dirty {
                        format!("{name} •")
                    } else {
                        name.to_owned()
                    };
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
        let mut open_file = None;
        egui::Panel::left("workspace-project-tree")
            .resizable(true)
            .default_size(270.0)
            .size_range(190.0..=440.0)
            .show(ui, |ui| {
                ui.heading("Project");
                let filter_response = ui.add(
                    egui::TextEdit::singleline(&mut self.workspace.editor.file_filter)
                        .hint_text("Find file by name or path · Ctrl/Cmd+P"),
                );
                if quick_open_requested {
                    filter_response.request_focus();
                }
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
                    .collect::<Vec<_>>();
                ui.small(format!(
                    "{} of {} files",
                    filtered_files.len(),
                    self.workspace.editor.files.len()
                ));
                let enter_opens_match = filter_response.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    && filtered_files.len() == 1;
                let tree = workspace_tree(filtered_files.iter().map(|path| path.as_path()));
                egui::ScrollArea::vertical().show(ui, |ui| {
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
                    open_file = filtered_files
                        .first()
                        .map(|path| path.as_path().to_path_buf());
                }
            });
        if let Some(path) = open_file {
            self.open_workspace_file(path);
        }

        let selected_path = self.workspace.editor.selected_file.clone();
        let (active_line_count, active_character_count, diagnostic) = self
            .workspace
            .editor
            .active_document()
            .map(|document| {
                let diagnostic = selected_path.as_deref().and_then(|path| {
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default();
                    (name == "Vapor.toml"
                        || name.ends_with(".vapor.toml")
                        || name == "Cargo.toml")
                        .then(|| {
                            document
                                .contents
                                .parse::<toml::Value>()
                                .map(|_| ())
                                .map_err(|error| error.to_string())
                        })
                });
                (
                    document.contents.lines().count(),
                    document.contents.chars().count(),
                    diagnostic,
                )
            })
            .unwrap_or_default();
        let active_dirty = self.workspace.editor.active_document_is_dirty();

        egui::Panel::right("workspace-inspector")
            .resizable(true)
            .default_size(270.0)
            .size_range(210.0..=440.0)
            .show(ui, |ui| {
                ui.heading("Inspector");
                ui.small(format!("Workspace\n{}", self.workspace.root));
                if let Some(path) = &selected_path {
                    ui.separator();
                    ui.label("File");
                    ui.small(path.display().to_string());
                    ui.label(format!("{} file", workspace_file_kind(path)));
                    ui.label(format!(
                        "{} lines · {} characters",
                        active_line_count,
                        active_character_count
                    ));
                    ui.label(if active_dirty { "Unsaved changes" } else { "Saved" });
                    if let Some(diagnostic) = &diagnostic {
                        match diagnostic {
                            Ok(()) => {
                                ui.colored_label(egui::Color32::LIGHT_GREEN, "TOML syntax valid");
                            }
                            Err(error) => {
                                ui.colored_label(egui::Color32::LIGHT_RED, "TOML syntax error")
                                    .on_hover_text(error);
                            }
                        }
                    }
                }
                ui.separator();
                if let Some(path) = &selected_path {
                    ui.weak(workspace_file_guidance(path));
                } else {
                    ui.weak("Select a file to inspect and edit it.");
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(
                selected_path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("Editor"),
            );
            if let Some(document) = self.workspace.editor.active_document_mut() {
                let can_edit = !self.operation.running && !self.authoring.manifest_metadata_dirty;
                ui.push_id(selected_path.as_deref(), |ui| {
                    ui.add_enabled_ui(can_edit, |ui| {
                        egui::TextEdit::multiline(&mut document.contents)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .show(ui)
                    });
                });
            } else {
                ui.weak("Open a file from the project tree to begin editing.");
            }
        });
    }
}

fn workspace_tree<'a>(files: impl IntoIterator<Item = &'a std::path::Path>) -> WorkspaceTreeNode<'a> {
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

fn insert_workspace_tree_entry<'a>(
    node: &mut WorkspaceTreeNode<'a>,
    parts: &[String],
    path: &'a std::path::Path,
) {
    let Some((name, rest)) = parts.split_first() else {
        return;
    };
    if rest.is_empty() {
        node.entries
            .insert(name.clone(), WorkspaceTreeChild::File(path));
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
    node: &WorkspaceTreeNode<'_>,
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
                    egui::Button::selectable(selected.is_some_and(|selected| selected == *path), name),
                );
                if response.clicked() {
                    *open_file = Some(path.to_path_buf());
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
