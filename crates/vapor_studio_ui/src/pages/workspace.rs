use super::super::StudioApp;
use super::super::components::design::page_header;
use super::super::workspace_ops::WorkspacePanel;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_workspace(&mut self, ui: &mut egui::Ui) {
        let has_workspace = !self.workspace.root.trim().is_empty();
        if self.workspace.editor.files_open {
            ui.horizontal(|ui| {
                if ui.button("← Workspace overview").clicked() {
                    self.workspace.editor.files_open = false;
                }
                ui.separator();
                ui.strong("Project editor");
                ui.weak(&self.workspace.root);
                if self.has_unsaved_workspace_edits() {
                    ui.colored_label(egui::Color32::from_rgb(242, 193, 91), "Unsaved edits");
                }
            });
            self.draw_workspace_files_panel(ui);
            return;
        }

        page_header(ui, "Author", "Workspace", "Manage source projects, edit files, and run managed development actions.");
        ui.label(if self.workspace.root.trim().is_empty() {
            "Open or create a source workspace to author, validate, and run content."
        } else {
            &self.workspace.root
        });
        ui.horizontal(|ui| {
            for (panel, label) in [
                (WorkspacePanel::Sources, "Sources"),
                (WorkspacePanel::Content, "Content"),
                (WorkspacePanel::Build, "Build"),
            ] {
                if ui
                    .add_enabled(
                        !self.workspace.editor.files_open,
                        egui::Button::selectable(self.workspace.overview_panel == panel, label),
                    )
                    .clicked()
                {
                    self.workspace.overview_panel = panel;
                }
            }
            if ui
                .add_enabled(
                    has_workspace,
                    egui::Button::selectable(self.workspace.editor.files_open, "Files"),
                )
                .clicked()
            {
                self.workspace.editor.files_open = true;
                if self.workspace.editor.files.is_empty() && !self.operation.running {
                    self.refresh_workspace_files();
                }
            }
        });
        ui.separator();
        match self.workspace.overview_panel {
            WorkspacePanel::Sources => self.draw_workspace_registry_panel(ui),
            WorkspacePanel::Content => {
                self.draw_workspace_content_panel(ui);
                self.draw_unregistered_projects_panel(ui);
                self.draw_workspace_authoring_panel(ui);
            }
            WorkspacePanel::Build => self.draw_workspace_operations_panel(ui),
        }
    }
}
