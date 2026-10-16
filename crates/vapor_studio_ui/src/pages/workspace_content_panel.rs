use super::super::StudioApp;
use eframe::egui;
use std::path::{Path, PathBuf};

impl StudioApp {
    pub(super) fn draw_workspace_content_panel(&mut self, ui: &mut egui::Ui) {
        if self.catalog.sources.is_empty() {
            let card_style = ui.style().clone();
            egui::Frame::group(&card_style).show(ui, |ui| {
                if !self.workspace.unregistered_projects.candidates.is_empty() {
                    ui.heading(format!(
                        "{} existing content project{} found",
                        self.workspace.unregistered_projects.candidates.len(),
                        if self.workspace.unregistered_projects.candidates.len() == 1 { "" } else { "s" },
                    ));
                    ui.label("Nothing has been changed. Adopt a project below to add it to this Workspace's content index; its files and Cargo membership stay where they are.");
                } else if self.workspace.unregistered_scan_completed {
                    ui.heading("No content projects in this workspace yet");
                    ui.label("Create a content role from the Create screen, or add an existing role manifest and scan again.");
                } else {
                    ui.heading("Discover authored content");
                    ui.label("Studio checks registered projects first, then scans for existing Engine, Game, pack, and extension manifests you can adopt.");
                }
                ui.horizontal(|ui| {
                    if !self.workspace.unregistered_scan_completed
                        && ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Find existing projects")).clicked()
                    {
                        self.scan_unregistered_content_projects();
                    }
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Create content")).clicked() {
                        self.section = super::super::Section::Create;
                    }
                    if self.workspace.unregistered_scan_completed
                        && ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Scan again")).clicked()
                    {
                        self.scan_unregistered_content_projects();
                    }
                });
            });
        } else {
            ui.heading(format!("Content projects · {}", self.catalog.sources.len()));
            ui.label("Registered authored projects in this workspace. Select a role to validate, edit its dependencies, or manage its Workshop intent.");
            for content in self.catalog.sources.clone() {
                ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&content.id);
                            ui.weak(&content.kind);
                            ui.weak(content.version.as_deref().unwrap_or("workspace version"));
                        });
                        if !content.binaries.is_empty() || !content.libraries.is_empty() {
                            let mut outputs = Vec::new();
                            if !content.binaries.is_empty() {
                                outputs.push(format!("binaries: {}", content.binaries.join(", ")));
                            }
                            if !content.libraries.is_empty() {
                                outputs.push(format!("libraries: {}", content.libraries.join(", ")));
                            }
                            ui.small(format!("Declared runtime outputs · {}", outputs.join(" · ")));
                        }
                        if !content.dependencies.is_empty() {
                            ui.small(format!("Dependencies: {}", content.dependencies.join(" · ")));
                        }
                        let workshop = &content.workshop;
                        if workshop.app_id.is_some()
                            || workshop.published_file_id.is_some()
                            || workshop.visibility.is_some()
                            || workshop.title.is_some()
                            || workshop.description.is_some()
                            || !workshop.tags.is_empty()
                            || workshop.change_note.is_some()
                        {
                            ui.collapsing("Workshop publication metadata", |ui| {
                                if let Some(app_id) = workshop.app_id {
                                    ui.small(format!("Steam AppID: {app_id}"));
                                } else {
                                    ui.colored_label(
                                        egui::Color32::YELLOW,
                                        "Steam AppID not configured",
                                    );
                                }
                                ui.small(format!(
                                    "Published item: {}",
                                    workshop.published_file_id.as_deref().unwrap_or("new item")
                                ));
                                if let Some(visibility) = &workshop.visibility {
                                    ui.small(format!("Visibility: {visibility}"));
                                }
                                if let Some(title) = &workshop.title {
                                    ui.small(format!("Workshop title: {title}"));
                                }
                                if let Some(description) = &workshop.description {
                                    ui.small(format!("Description: {description}"));
                                }
                                if !workshop.tags.is_empty() {
                                    ui.small(format!("Tags: {}", workshop.tags.join(", ")));
                                }
                                if let Some(change_note) = &workshop.change_note {
                                    ui.small(format!("Default change note: {change_note}"));
                                }
                                ui.weak("This is authored intent; staging performs the authoritative package and publication checks.");
                            });
                        } else {
                            ui.small("Workshop publication: not configured");
                        }
                        ui.horizontal_wrapped(|ui| {
                            let relative_manifest = content
                                .manifest
                                .strip_prefix(Path::new(self.workspace.root.trim()))
                                .ok()
                                .map(PathBuf::from);
                            if ui
                                .add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Validate"),
                                )
                                .on_hover_text("Validate this content manifest and its local references through Vapor Shell.")
                                .clicked()
                            {
                                self.validate_source_project(&content);
                            }
                            if let Some(relative_manifest) = relative_manifest {
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                                        egui::Button::new("Edit manifest"),
                                    )
                                    .on_disabled_hover_text("Save or discard the current file before opening a project manifest.")
                                    .clicked()
                                {
                                    self.open_workspace_file(relative_manifest);
                                }
                            }
                            if ui
                                .add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Edit Workshop metadata"),
                                )
                                .on_disabled_hover_text("Save or discard workspace file and structured metadata drafts first; Workshop edits use a fresh Shell-validated manifest snapshot.")
                                .clicked()
                            {
                                self.load_source_workshop_metadata(&content);
                            }
                            if ui
                                .add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Edit runtime outputs"),
                                )
                                .on_disabled_hover_text("Save or discard open edits first; runtime output edits use a fresh Shell-validated manifest snapshot.")
                                .clicked()
                            {
                                self.load_source_runtime_outputs(&content);
                            }
                            if ui
                                    .add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Edit dependencies"))
                                    .clicked()
                            {
                                self.authoring.composition_edit_selector =
                                    super::super::source_content_selector(&content);
                                self.authoring.composition_edit = None;
                                self.authoring.composition_original_links = None;
                                self.section = super::super::Section::Compose;
                                self.load_source_composition();
                            }
                            if content.kind == vapor_core::ContentKind::Packagepack.as_str()
                                && ui
                                    .add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Use in active profile"))
                                    .clicked()
                            {
                                self.assign_profile_root(
                                    vapor_studio_core::ContentIdentity {
                                        id: content.id.clone(),
                                        version: content.version.clone(),
                                    },
                                    Some(PathBuf::from(self.workspace.root.trim())),
                                );
                            }
                            if ui
                                .add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Unregister"),
                                )
                                .on_hover_text("Remove this project from Vapor discovery without deleting its files or changing Cargo membership.")
                                .clicked()
                            {
                                self.workspace.pending_unregistration = Some(content.clone());
                            }
                        });
                    });
            }
        }
        if let Some(project) = self.workspace.pending_unregistration.clone() {
            ui.group(|ui| {
                ui.heading(format!("Unregister {}?", project.id));
                ui.label(format!(
                    "Vapor will stop discovering this {} at {}. The project files and Cargo workspace membership will remain unchanged; you can adopt it again later.",
                    project.kind,
                    project.root.display(),
                ));
                ui.horizontal(|ui| {
                    if ui.add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Unregister from Vapor"),
                    ).clicked() {
                        self.unregister_content_project();
                    }
                    if ui.add_enabled(!self.operation.running, egui::Button::new("Keep registered")).clicked() {
                        self.workspace.pending_unregistration = None;
                    }
                });
            });
        }
    }

    pub(super) fn draw_unregistered_projects_panel(&mut self, ui: &mut egui::Ui) {
        if !self.workspace.root.trim().is_empty() {
            let has_candidates = !self.workspace.unregistered_projects.candidates.is_empty();
            if has_candidates {
                ui.heading(format!(
                    "Ready to adopt · {}",
                    self.workspace.unregistered_projects.candidates.len()
                ));
                ui.weak("Adoption adds a valid project to Vapor discovery. It does not move files, publish anything, or change Cargo membership.");
            }
            for candidate in self.workspace.unregistered_projects.candidates.clone() {
                let card_style = ui.style().clone();
                egui::Frame::group(&card_style).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(&candidate.id);
                            ui.small(format!(
                                "{} · {}",
                                candidate.kind,
                                candidate
                                    .path
                                    .strip_prefix(std::path::Path::new(self.workspace.root.trim()))
                                    .unwrap_or(&candidate.path)
                                    .display()
                            ));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Adopt project"),
                                )
                                .clicked()
                            {
                                self.adopt_content_project(candidate.path.clone());
                            }
                        });
                    });
                });
            }
            if self.workspace.unregistered_scan_completed && !has_candidates {
                ui.weak("No unregistered content manifests were found.");
            }
            if !self.workspace.unregistered_scan_completed
                && !has_candidates
                && !self.catalog.sources.is_empty()
            {
                ui.horizontal(|ui| {
                    ui.weak("A project scan has not completed yet.");
                    if ui
                        .add_enabled(
                            !self.operation.running && !self.has_unsaved_workspace_edits(),
                            egui::Button::new("Find projects"),
                        )
                        .clicked()
                    {
                        self.scan_unregistered_content_projects();
                    }
                });
            }
            for diagnostic in &self.workspace.unregistered_projects.diagnostics {
                ui.colored_label(egui::Color32::YELLOW, diagnostic);
            }
        }
    }
}
