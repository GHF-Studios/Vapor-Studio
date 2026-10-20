use super::super::StudioApp;
use super::super::components::design::confirmation_card;
use super::super::workspace_ops::WorkspaceOperation;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workspace_operations_panel(&mut self, ui: &mut egui::Ui) {
        let has_workspace = !self.workspace.root.trim().is_empty();
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!self.operation.running
                    && !self.has_unsaved_workspace_edits()
                    && !self.workspace.root.trim().is_empty()
                    && self.workspace.pending_clean_confirmation.is_none(), egui::Button::new("Validate content"))
                .on_disabled_hover_text("Save or discard your open edits first; checks use the saved project files.")
                .clicked()
            {
                self.validate_workspace_source();
            }
            for (label, operation) in [
                ("Format", WorkspaceOperation::Format),
                ("Check", WorkspaceOperation::Check),
                ("Test", WorkspaceOperation::Test),
                ("Build", WorkspaceOperation::Build),
                ("Clean", WorkspaceOperation::Clean),
            ] {
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && has_workspace
                            && self.workspace.pending_clean_confirmation.is_none(),
                        egui::Button::new(label),
                    )
                    .on_disabled_hover_text(if has_workspace {
                        "Save or discard your open edits first; these actions use the saved project files."
                    } else {
                        "Open a project folder before using these actions."
                    })
                    .clicked()
                {
                    self.run_operation(operation);
                }
            }
        });
        let mut clean_confirmation = None;
        if let Some(clean_root) = self.workspace.pending_clean_confirmation.as_ref() {
            let target_matches =
                clean_root == &std::path::PathBuf::from(self.workspace.root.trim());
            ui.add_space(10.0);
            let title = if target_matches {
                "Clean generated build files?"
            } else {
                "Cleanup target changed"
            };
            let description = format!(
                "Remove generated build files for this project's Rust workspaces. Source files and installed game content are not touched. Reviewed project: {}{}",
                clean_root.display(),
                if target_matches {
                    String::new()
                } else {
                    format!(" · active workspace: {}", self.workspace.root)
                },
            );
            clean_confirmation = confirmation_card(
                ui,
                title,
                &description,
                "Remove generated build files",
                "Cancel",
                target_matches && !self.operation.running && !self.has_unsaved_workspace_edits(),
            );
        }
        match clean_confirmation {
            Some(true) => self.confirm_clean_workspace(),
            Some(false) => self.workspace.pending_clean_confirmation = None,
            None => {}
        }
    }
}
