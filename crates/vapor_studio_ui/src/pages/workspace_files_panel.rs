use super::super::StudioApp;
use super::super::workspace_editor::{WorkspaceTreeChild, WorkspaceTreeNode};
use eframe::egui;
use std::path::PathBuf;

impl StudioApp {
    pub(super) fn draw_workspace_files_panel(&mut self, ui: &mut egui::Ui) {
        let save_shortcut = ui.input(|input| {
            input.modifiers.command && input.key_pressed(egui::Key::S) && !input.modifiers.shift
        });
        let save_all_shortcut = ui.input(|input| {
            input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::S)
        });
        let quick_open_requested =
            ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::P));
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
                if !filter.is_empty() && ui.small_button("Clear").clicked() {
                    self.workspace.editor.file_filter.clear();
                }
                let filtered_count = self.workspace.editor.matching_file_count(&filter);
                ui.small(format!(
                    "{} of {} files",
                    filtered_count,
                    self.workspace.editor.files.len()
                ));
                let enter_opens_match = filter_response.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    && filtered_count == 1;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if filtered_count == 0 {
                        ui.weak(if filter.is_empty() {
                            "No visible files in this workspace."
                        } else {
                            "No files match this path."
                        });
                    } else {
                        draw_workspace_tree(
                            ui,
                            &self.workspace.editor.file_tree,
                            &self.workspace.editor.files,
                            self.workspace.editor.file_search_keys(),
                            &filter,
                            self.workspace.editor.selected_file.as_ref(),
                            !self.operation.running && !self.authoring.manifest_metadata_dirty,
                            &PathBuf::new(),
                            0,
                            &mut open_file,
                        );
                    }
                });
                if enter_opens_match {
                    open_file = self
                        .workspace
                        .editor
                        .files
                        .iter()
                        .zip(self.workspace.editor.file_search_keys())
                        .find_map(|(path, key)| {
                            (filter.is_empty() || key.contains(&filter)).then(|| path.clone())
                        });
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
                (
                    document.line_count,
                    document.character_count,
                    document.toml_diagnostic.clone(),
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
                        active_line_count, active_character_count
                    ));
                    ui.label(if active_dirty {
                        "Unsaved changes"
                    } else {
                        "Saved"
                    });
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
                        let response = egui::TextEdit::multiline(&mut document.contents)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .show(ui);
                        if response.response.changed()
                            && let Some(path) = selected_path.as_deref()
                        {
                            document.refresh_inspection(path);
                        }
                        response
                    });
                });
            } else {
                ui.weak("Open a file from the project tree to begin editing.");
            }
        });
    }
}

fn draw_workspace_tree(
    ui: &mut egui::Ui,
    node: &WorkspaceTreeNode,
    files: &[PathBuf],
    search_keys: &[String],
    filter: &str,
    selected: Option<&PathBuf>,
    enabled: bool,
    parent_path: &std::path::Path,
    depth: usize,
    open_file: &mut Option<PathBuf>,
) {
    for (name, child) in &node.entries {
        match child {
            WorkspaceTreeChild::Directory(directory) => {
                if !filter.is_empty() && !directory.contains_matching_file(search_keys, filter) {
                    continue;
                }
                let directory_path = parent_path.join(name);
                let _ = egui::CollapsingHeader::new(name)
                    .id_salt(("workspace-tree", &directory_path, !filter.is_empty()))
                    .default_open(depth == 0 || !filter.is_empty())
                    .show(ui, |ui| {
                        draw_workspace_tree(
                            ui,
                            directory,
                            files,
                            search_keys,
                            filter,
                            selected,
                            enabled,
                            &directory_path,
                            depth + 1,
                            open_file,
                        )
                    });
            }
            WorkspaceTreeChild::File(index) => {
                let Some(path) = files.get(*index) else {
                    continue;
                };
                if !filter.is_empty()
                    && !search_keys
                        .get(*index)
                        .is_some_and(|key| key.contains(filter))
                {
                    continue;
                }
                let response = ui.add_enabled(
                    enabled,
                    egui::Button::selectable(
                        selected.is_some_and(|selected| selected == path),
                        name,
                    ),
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
