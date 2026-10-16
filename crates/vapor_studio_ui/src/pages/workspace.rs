use super::super::StudioApp;
use eframe::egui;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use vapor_core::DevelopmentOperation;

#[derive(Default)]
struct WorkspaceTreeNode {
    entries: BTreeMap<String, WorkspaceTreeChild>,
}

enum WorkspaceTreeChild {
    Directory(WorkspaceTreeNode),
    File(PathBuf),
}

impl StudioApp {
    pub(in crate::app) fn draw_workspace(&mut self, ui: &mut egui::Ui) {
        ui.heading("Workspace operations");
        ui.label("Operations use the workspace pin and Vapor-managed toolchain and cache.");
        ui.horizontal(|ui| {
            if ui.button("Refresh registered workspaces").clicked() {
                self.refresh_registered_sources();
            }
            if self.registered_sources.is_empty() {
                ui.weak("No workspaces loaded from this Vapor installation yet.");
            }
        });
        if !self.registered_sources.is_empty() {
            ui.label("Registered workspaces");
            for source in self.registered_sources.clone() {
                let active = self.active_source.as_ref() == Some(&source);
                let selected = Path::new(self.workspace_root.trim()) == source;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.running,
                            egui::Button::selectable(selected, source.display().to_string()),
                        )
                        .clicked()
                    {
                        self.open_workspace_path(source.clone());
                    }
                    if active {
                        ui.weak("Vapor active source");
                    }
                });
            }
        }
        if !self.workspace_root.trim().is_empty() {
            let current = Path::new(self.workspace_root.trim());
            let already_registered = self.registered_sources.iter().any(|source| source == current);
            if !already_registered {
                ui.horizontal(|ui| {
                    ui.label("This workspace is open for the current profile only.");
                    if ui.add_enabled(!self.running, egui::Button::new("Add to Vapor sources")).clicked() {
                        self.remember_open_workspace();
                    }
                });
            }
        }
        ui.horizontal(|ui| {
            ui.label("Workspace");
            ui.add_enabled(
                !self.running,
                egui::TextEdit::singleline(&mut self.workspace_root_draft)
                    .desired_width(360.0)
                    .hint_text("Path to a Vapor workspace"),
            );
            if ui
                .add_enabled(!self.running, egui::Button::new("Open workspace"))
                .clicked()
            {
                self.open_workspace_path(PathBuf::from(self.workspace_root_draft.trim()));
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!self.running && !self.workspace_editor_dirty && !self.workspace_root.trim().is_empty(), egui::Button::new("Validate content"))
                .on_disabled_hover_text("Save or reload the open file first; validation reads workspace files from disk.")
                .clicked()
            {
                self.validate_workspace_source();
            }
            for (label, operation) in [
                ("Format", DevelopmentOperation::Fmt),
                ("Check", DevelopmentOperation::Check),
                ("Test", DevelopmentOperation::Test),
                ("Build", DevelopmentOperation::Build),
                ("Clean", DevelopmentOperation::Clean),
            ] {
                if ui
                    .add_enabled(!self.running && !self.workspace_editor_dirty, egui::Button::new(label))
                    .on_disabled_hover_text("Save or reload the open file first; workspace operations read files from disk.")
                    .clicked()
                {
                    self.run_operation(operation);
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("New file");
            ui.add_enabled(
                !self.running && !self.workspace_editor_dirty,
                egui::TextEdit::singleline(&mut self.new_workspace_file_path)
                    .desired_width(280.0)
                    .hint_text("scripts/example.rhai"),
            );
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty,
                    egui::Button::new("Create file"),
                )
                .clicked()
            {
                self.create_workspace_file();
            }
        });
        ui.weak("Creates a new text file inside an existing workspace folder; existing files are never overwritten.");
        ui.horizontal(|ui| {
            ui.heading("Files");
            if ui
                .add_enabled(!self.running, egui::Button::new("Refresh"))
                .clicked()
            {
                self.refresh_workspace_files();
            }
            if let Some(path) = &self.selected_workspace_file {
                ui.weak(path.display().to_string());
            }
            if ui
                .add_enabled(
                    !self.running && self.workspace_editor_dirty,
                    egui::Button::new("Save"),
                )
                .clicked()
            {
                self.save_workspace_file();
            }
            if ui
                .add_enabled(
                    !self.running && self.selected_workspace_file.is_some(),
                    egui::Button::new("Reload from disk"),
                )
                .on_hover_text("Discard the editor buffer and reopen the current on-disk file.")
                .clicked()
            {
                self.reload_workspace_file();
            }
            if self.workspace_editor_dirty {
                ui.colored_label(egui::Color32::YELLOW, "Unsaved");
            }
        });
        ui.columns(3, |columns| {
            columns[0].heading("Project");
            let tree = workspace_tree(&self.workspace_files);
            let mut open_file = None;
            egui::ScrollArea::vertical().show(&mut columns[0], |ui| {
                if tree.entries.is_empty() {
                    ui.weak("No visible files in this workspace.");
                } else {
                    draw_workspace_tree(
                        ui,
                        &tree,
                        self.selected_workspace_file.as_ref(),
                        !self.running,
                        &PathBuf::new(),
                        0,
                        &mut open_file,
                    );
                }
            });
            if let Some(path) = open_file {
                self.open_workspace_file(path);
            }
            let changed = columns[1]
                .add_enabled_ui(
                    self.selected_workspace_file.is_some() && !self.running,
                    |ui| {
                        egui::TextEdit::multiline(&mut self.workspace_editor)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .show(ui)
                    },
                )
                .inner
                .response
                .changed();
            self.workspace_editor_dirty |= changed;
            if changed {
                self.refresh_workspace_editor_toml_diagnostic();
            }
            columns[2].heading("Inspector");
            columns[2].small(format!("Workspace\n{}", self.workspace_root));
            if let Some(path) = &self.selected_workspace_file {
                columns[2].separator();
                columns[2].label("File");
                columns[2].small(path.display().to_string());
                columns[2].label(format!("{} file", workspace_file_kind(path)));
                columns[2].label(format!(
                    "{} lines · {} characters",
                    self.workspace_editor.lines().count(),
                    self.workspace_editor.chars().count()
                ));
                columns[2].label(if self.workspace_editor_dirty {
                    "Unsaved changes"
                } else {
                    "Saved"
                });
                if let Some(diagnostic) = &self.workspace_editor_toml_diagnostic {
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
                columns[2].separator();
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
