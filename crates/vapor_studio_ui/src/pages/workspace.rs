use super::super::StudioApp;
use super::super::components::design::{StatusTone, page_header, status_text};
use super::super::workspace_ops::{WorkspaceContentPanel, WorkspacePanel};
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_workspace(&mut self, ui: &mut egui::Ui) {
        let has_workspace = !self.workspace.root.trim().is_empty();
        if self.workspace.editor.files_open {
            ui.horizontal_wrapped(|ui| {
                if ui.button("← Project overview").clicked() {
                    self.workspace.editor.files_open = false;
                }
                ui.separator();
                ui.strong("Project editor");
                ui.weak(&self.workspace.root);
                if self.has_unsaved_workspace_edits() {
                    status_text(ui, "Unsaved edits", StatusTone::Caution);
                }
            });
            self.draw_workspace_validation_warning(ui);
            self.draw_workspace_files_panel(ui);
            return;
        }

        page_header(
            ui,
            "Author",
            "My projects",
            "Choose the project you are authoring. This stays separate from each composition's launch source.",
        );
        ui.label(if self.workspace.root.trim().is_empty() {
            "Choose a folder to get started. You can open an existing project or create one from a template."
        } else {
            &self.workspace.root
        });
        ui.horizontal_wrapped(|ui| {
            for (panel, label) in [
                (WorkspacePanel::Sources, "Project folders"),
                (WorkspacePanel::Content, "Content projects"),
                (WorkspacePanel::Build, "Build & run"),
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
                self.open_workspace_files();
            }
        });
        self.draw_workspace_validation_warning(ui);
        ui.separator();
        match self.workspace.overview_panel {
            WorkspacePanel::Sources => self.draw_workspace_registry_panel(ui),
            WorkspacePanel::Content => {
                ui.horizontal_wrapped(|ui| {
                    for (panel, label) in [
                        (WorkspaceContentPanel::Projects, "Projects in this folder"),
                        (WorkspaceContentPanel::AddExisting, "Add existing"),
                        (WorkspaceContentPanel::ProjectSettings, "Project settings"),
                    ] {
                        if ui
                            .add(egui::Button::selectable(
                                self.workspace.content_panel == panel,
                                label,
                            ))
                            .clicked()
                        {
                            self.workspace.content_panel = panel;
                        }
                    }
                });
                ui.separator();
                match self.workspace.content_panel {
                    WorkspaceContentPanel::Projects => {
                        let query = self.draw_workspace_content_search(ui);
                        self.draw_workspace_content_panel(ui, &query);
                    }
                    WorkspaceContentPanel::AddExisting => {
                        let query = self.draw_workspace_content_search(ui);
                        self.draw_unregistered_projects_panel(ui, &query);
                    }
                    WorkspaceContentPanel::ProjectSettings => {
                        self.draw_workspace_authoring_panel(ui);
                    }
                }
            }
            WorkspacePanel::Build => self.draw_workspace_operations_panel(ui),
        }
    }

    fn draw_workspace_validation_warning(&self, ui: &mut egui::Ui) {
        if let Some(error) = self.workspace.workspace_validation_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "This project folder needs a repair. Editing remains available; save the fix and refresh the project list before using other actions. {error}"
                ),
                StatusTone::Caution,
            );
        }
    }
}
