use super::super::StudioApp;
use super::super::components::design::{page_header, section_header};
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_library(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Play", "Play profiles", "Choose a composition, prepare its installation, and manage how it launches.");
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        if ui.available_width() < 820.0 {
            section_header(ui, "Launch profiles", "Each profile keeps its own content selection and launch arguments.");
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Duplicate selected profile"),
                )
                .clicked()
            {
                self.duplicate_active_profile();
            }
            self.draw_profile_selector_panel(ui);
            ui.separator();
            section_header(ui, &profile.name, "Selected composition and local installation state.");
            self.draw_profile_management_panel(ui, &profile);
            self.draw_profile_composition_panel(ui, &profile);
            self.draw_profile_installation_panel(ui, &profile);
            self.draw_profile_launch_panel(ui);
            return;
        }
        ui.columns(2, |columns| {
            columns[0].set_min_width(300.0);
            section_header(&mut columns[0], "Launch profiles", "Each profile keeps its own content selection and launch arguments.");
            if columns[0]
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Duplicate selected profile"),
                )
                .on_disabled_hover_text("Save or discard workspace edits before duplicating a profile.")
                .clicked()
            {
                self.duplicate_active_profile();
            }
            self.draw_profile_selector_panel(&mut columns[0]);

            columns[1].set_min_width(420.0);
            section_header(&mut columns[1], &profile.name, "Selected composition and local installation state.");
            self.draw_profile_management_panel(&mut columns[1], &profile);
            self.draw_profile_composition_panel(&mut columns[1], &profile);
            self.draw_profile_installation_panel(&mut columns[1], &profile);
            self.draw_profile_launch_panel(&mut columns[1]);
        });
    }
}
