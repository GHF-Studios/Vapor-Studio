use super::super::StudioApp;
use super::super::components::design::{StatusTone, status_text, use_stacked_panels};
use eframe::egui;
use std::path::{Path, PathBuf};

impl StudioApp {
    pub(super) fn draw_workspace_registry_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Choose the project folder Studio uses for this play profile and your edits.");
        let card_style = ui.style().clone();
        let workspace_to_open = if use_stacked_panels(ui) {
            egui::Frame::group(&card_style).show(ui, |ui| self.draw_current_workspace_card(ui));
            ui.add_space(8.0);
            egui::Frame::group(&card_style)
                .show(ui, |ui| {
                    self.draw_registered_workspaces_card(ui, &card_style)
                })
                .inner
        } else {
            let mut workspace_to_open = None;
            ui.columns(2, |columns| {
                egui::Frame::group(&card_style).show(&mut columns[0], |ui| {
                    self.draw_current_workspace_card(ui);
                });
                workspace_to_open = egui::Frame::group(&card_style)
                    .show(&mut columns[1], |ui| {
                        self.draw_registered_workspaces_card(ui, &card_style)
                    })
                    .inner;
            });
            workspace_to_open
        };
        if let Some(path) = workspace_to_open {
            self.open_workspace_path(path);
        }
    }

    fn draw_current_workspace_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Current project folder");
        if self.workspace.root.trim().is_empty() {
            ui.weak("No project folder is open for this profile.");
        } else {
            ui.label(
                std::path::Path::new(self.workspace.root.trim())
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&self.workspace.root),
            );
            ui.small(&self.workspace.root);
            ui.weak("Studio uses this folder when you create projects, choose game content, and prepare a local launch.");
        }
        ui.add_space(8.0);
        ui.label("Open a project folder");
        ui.weak("Studio can open a folder even if a project needs repair. Some actions become available after you fix it.");
        ui.horizontal_wrapped(|ui| {
            let path_width = ui.available_width().min(420.0);
            ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workspace.root_draft)
                    .desired_width(path_width)
                    .hint_text("Choose or enter a folder path"),
            );
            if ui
                .add_enabled(!self.operation.running, egui::Button::new("Browse…"))
                .clicked()
            {
                self.open_workspace_picker();
            }
            if ui
                .add_enabled(
                    !self.operation.running && !self.workspace.root_draft.trim().is_empty(),
                    egui::Button::new("Open"),
                )
                .clicked()
            {
                self.open_workspace_path(PathBuf::from(self.workspace.root_draft.trim()));
            }
        });
        if !self.workspace.root.trim().is_empty() {
            let current = Path::new(self.workspace.root.trim());
            let registered = self
                .workspace
                .registered_sources
                .iter()
                .any(|source| source == current);
            if registered {
                if self.workspace.registered_sources_error.is_some() {
                    ui.weak("This folder appeared in the last successful list; its current status could not be refreshed.");
                } else {
                    ui.weak("This folder is also available to other Vapor surfaces.");
                }
            } else if let Some(error) = self.workspace.registered_sources_error.as_deref() {
                ui.weak(format!("Could not confirm whether Vapor already knows this folder: {error}. Refresh before adding it again."));
            } else if !self.workspace.registered_sources_loaded {
                ui.weak("Refresh the project-folder list before adding this folder, so Studio can avoid duplicates.");
            } else {
                ui.weak("The play profile remembers this folder. Making it available to Vapor also lets other Vapor surfaces find its projects.");
                if ui
                    .add_enabled(
                        !self.operation.running,
                        egui::Button::new("Make available in other Vapor apps"),
                    )
                    .clicked()
                {
                    self.remember_open_workspace();
                }
            }
        }
    }

    fn draw_registered_workspaces_card(
        &mut self,
        ui: &mut egui::Ui,
        card_style: &egui::Style,
    ) -> Option<PathBuf> {
        let mut workspace_to_open = None;
        ui.horizontal_wrapped(|ui| {
            ui.heading("Known project folders");
            if ui
                .add_enabled(!self.operation.running, egui::Button::new("Refresh"))
                .clicked()
            {
                self.refresh_registered_sources();
            }
        });
        ui.label("Project folders already known to this Vapor installation.");
        if let Some(error) = self.workspace.registered_sources_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "Could not refresh known project folders. {} {error}",
                    if self.workspace.registered_sources_loaded {
                        "Showing the last successful list."
                    } else {
                        "No successful list is available yet."
                    }
                ),
                StatusTone::Caution,
            );
        }
        if self.workspace.registered_sources.is_empty()
            && self.workspace.registered_sources_loaded
            && self.workspace.registered_sources_error.is_none()
        {
            ui.weak("No project folders are saved in this Vapor installation yet.");
        } else if self.workspace.registered_sources.is_empty()
            && self.workspace.registered_sources_loaded
            && self.workspace.registered_sources_error.is_some()
        {
            ui.weak("The last successful list was empty; refresh to check again.");
        } else if self.workspace.registered_sources.is_empty() {
            ui.weak(
                "No known project folders loaded yet. Refresh to check this installation.",
            );
        }
        for source in &self.workspace.registered_sources {
            let active = self.workspace.active_source.as_ref() == Some(source);
            let selected = Path::new(self.workspace.root.trim()) == source;
            egui::Frame::group(card_style).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.strong(
                            source
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("Workspace"),
                        );
                        ui.small(source.display().to_string());
                    });
                    if active {
                        ui.weak("Active folder");
                    }
                    if ui
                        .add_enabled(
                            !self.operation.running,
                            egui::Button::selectable(selected, "Open"),
                        )
                        .clicked()
                    {
                        workspace_to_open = Some(source.clone());
                    }
                });
            });
        }
        workspace_to_open
    }
}
