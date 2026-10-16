use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_workspace(&mut self, ui: &mut egui::Ui) {
        ui.heading("Workspace");
        ui.label("Author content and maintain the selected Vapor workspace.");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.workspace.files_open, false, "Overview");
            let has_workspace = !self.workspace.root.trim().is_empty();
            if ui
                .add_enabled(
                    has_workspace,
                    egui::Button::selectable(self.workspace.files_open, "Files"),
                )
                .clicked()
            {
                self.workspace.files_open = true;
            }
        });
        ui.separator();
        if !self.workspace.files_open {
            self.draw_workspace_registry_panel(ui);
            self.draw_workspace_content_panel(ui);
            self.draw_workspace_authoring_panel(ui);
            self.draw_unregistered_projects_panel(ui);
            self.draw_workspace_operations_panel(ui);
            ui.separator();
        } else if self.workspace.root.trim().is_empty() {
            ui.weak("Open a Vapor Workspace from Overview before editing its files.");
        } else {
            self.draw_workspace_files_panel(ui);
        }
    }
}
