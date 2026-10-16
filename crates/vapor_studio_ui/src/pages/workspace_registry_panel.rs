use super::super::StudioApp;
use eframe::egui;
use std::path::{Path, PathBuf};

impl StudioApp {
    pub(super) fn draw_workspace_registry_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Operations use the workspace pin and Vapor-managed toolchain and cache.");
        ui.horizontal(|ui| {
            if ui.button("Refresh registered workspaces").clicked() {
                self.refresh_registered_sources();
            }
            if self.workspace.registered_sources.is_empty() {
                ui.weak("No workspaces loaded from this Vapor installation yet.");
            }
        });
        if !self.workspace.registered_sources.is_empty() {
            ui.label("Registered workspaces");
            for source in self.workspace.registered_sources.clone() {
                let active = self.workspace.active_source.as_ref() == Some(&source);
                let selected = Path::new(self.workspace.root.trim()) == source;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.operation.running,
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
        if !self.workspace.root.trim().is_empty() {
            let current = Path::new(self.workspace.root.trim());
            let already_registered = self.workspace.registered_sources.iter().any(|source| source == current);
            if !already_registered {
                ui.horizontal(|ui| {
                    ui.label("This workspace is open for the current profile only.");
                    if ui.add_enabled(!self.operation.running, egui::Button::new("Add to Vapor sources")).clicked() {
                        self.remember_open_workspace();
                    }
                });
            }
        }
        ui.horizontal(|ui| {
            ui.label("Workspace");
            ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workspace.root_draft)
                    .desired_width(360.0)
                    .hint_text("Path to a Vapor workspace"),
            );
            if ui
                .add_enabled(!self.operation.running, egui::Button::new("Open workspace"))
                .clicked()
            {
                self.open_workspace_path(PathBuf::from(self.workspace.root_draft.trim()));
            }
        });
    }
}
