use super::super::operation_center::operation_kind_label;
use super::super::{Section, StudioApp, content_selector};
use eframe::egui;
use vapor_studio_core::InstallationState;

impl StudioApp {
    pub(in crate::app) fn draw_home(&mut self, ui: &mut egui::Ui) {
        let profile = self.profile.items[self.profile.active_index].clone();
        ui.heading("Your space");
        ui.label("Play a composition or pick up where you left off creating.");
        ui.add_space(8.0);

        let card_style = ui.style().clone();
        ui.columns(2, |columns| {
            egui::Frame::group(&card_style).show(&mut columns[0], |ui| {
                ui.small("READY TO PLAY");
                ui.heading(&profile.name);
                if let Some(root) = profile.root_package.as_ref() {
                    ui.label(format!("{} · {}", content_selector(root), installation_label(&profile.installation)));
                    let installed = matches!(profile.installation, InstallationState::Installed { .. });
                    let source_ready = !self.has_unsaved_workspace_edits()
                        && self.profile_composition.preview().is_some_and(|preview| {
                            preview.selector == content_selector(root)
                                && preview.ready()
                                && preview.nodes.last().is_some_and(|node| {
                                    node.source == vapor_studio_core::CompositionPreviewSource::Workspace
                                })
                        });
                    let action = if installed {
                        "Play composition"
                    } else if source_ready {
                        "Build and play"
                    } else {
                        "Set up composition"
                    };
                    if ui.add_enabled(
                        !self.operation.running
                            && !matches!(profile.installation,
                                InstallationState::OutOfDate { .. }
                                    | InstallationState::Partial { .. }
                                    | InstallationState::NeedsRepair { .. })
                            && (installed || source_ready || !self.has_unsaved_workspace_edits()),
                        egui::Button::new(action),
                    ).clicked() {
                        if installed {
                            self.run_installed_selected();
                        } else if source_ready {
                            self.run_selected();
                        } else {
                            self.section = Section::Library;
                            self.preview_selected_composition();
                        }
                    }
                    if matches!(profile.installation,
                        InstallationState::OutOfDate { .. }
                            | InstallationState::Partial { .. }
                            | InstallationState::NeedsRepair { .. }) {
                        ui.colored_label(egui::Color32::YELLOW,
                            "This installation needs review before it can be played. Open Library for verification or repair.");
                    }
                } else {
                    ui.weak("Choose a Packagepack to create your first launchable composition.");
                    if ui.add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Browse content"),
                    ).clicked() {
                        self.section = Section::Discover;
                        self.discover();
                    }
                }
                if ui.button("Manage play profiles").clicked() {
                    self.section = Section::Library;
                }
            });

            egui::Frame::group(&card_style).show(&mut columns[1], |ui| {
                ui.small("CONTINUE CREATING");
                if self.workspace.root.trim().is_empty() {
                    ui.heading("Start a workspace");
                    ui.label("Create a starter workspace or open existing source to author content.");
                    if ui.button("Create a workspace").clicked() {
                        self.section = Section::Create;
                    }
                    if ui.button("Open a workspace").clicked() {
                        self.section = Section::Workspace;
                    }
                } else {
                    let workspace_name = std::path::Path::new(&self.workspace.root)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&self.workspace.root);
                    ui.heading(workspace_name);
                    ui.small(&self.workspace.root);
                    if self.has_unsaved_workspace_edits() {
                        ui.colored_label(egui::Color32::YELLOW, "Unsaved edits need attention");
                    } else {
                        ui.weak("Workspace open");
                    }
                    ui.small(if self.workspace.unregistered_scan_completed {
                        format!(
                            "{} registered · {} found to adopt",
                            self.catalog.sources.len(),
                            self.workspace.unregistered_projects.candidates.len(),
                        )
                    } else {
                        "Content discovery has not run yet".to_owned()
                    });
                    if ui.add_enabled(!self.operation.running, egui::Button::new("Open workspace")).clicked() {
                        self.section = Section::Workspace;
                        if self.workspace.editor.files.is_empty() && !self.operation.running {
                            self.refresh_workspace_files();
                        }
                    }
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Review workspace content")).clicked() {
                        self.navigate_to_workspace_content();
                    }
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Create content in this workspace")).clicked() {
                        self.section = Section::Create;
                    }
                }
                if ui.add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Browse content"),
                ).clicked() {
                    self.section = Section::Discover;
                    self.discover();
                }
            });
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.heading("Recent activity");
            if self.operation.running {
                ui.spinner();
                ui.label("An operation is in progress. Details and progress stay available below.");
            }
        });
        if self.operation.operation_history.is_empty() {
            ui.weak("Completed installs, builds, validation, and publishing actions will appear here.");
        } else {
            for entry in self.operation.operation_history.iter().take(5) {
                ui.horizontal(|ui| {
                    ui.small(operation_kind_label(&entry.event.kind));
                    match &entry.event.status {
                        vapor_studio_core::OperationStatus::Succeeded { summary } => {
                            ui.colored_label(egui::Color32::from_rgb(120, 210, 150), summary);
                        }
                        vapor_studio_core::OperationStatus::Failed { message } => {
                            ui.colored_label(egui::Color32::LIGHT_RED, message);
                        }
                        vapor_studio_core::OperationStatus::Cancelled => {
                            ui.weak("Cancelled");
                        }
                        vapor_studio_core::OperationStatus::Running => {
                            ui.weak("In progress");
                        }
                    }
                    ui.weak(super::super::operations::elapsed_label(entry.completed_at_unix));
                });
            }
        }
    }
}

fn installation_label(state: &InstallationState) -> &'static str {
    match state {
        InstallationState::Unknown => "Status not checked",
        InstallationState::NotInstalled => "Not installed",
        InstallationState::Installed { .. } => "Installed",
        InstallationState::OutOfDate { .. } => "Out of date",
        InstallationState::Partial { .. } => "Partially installed",
        InstallationState::NeedsRepair { .. } => "Needs repair",
    }
}
