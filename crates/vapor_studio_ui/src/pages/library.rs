use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Compositions");
        ui.label("Profiles keep their own content choice, workspace, launch settings, and installed graph.");
        self.draw_profile_selector_panel(ui);

        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        ui.separator();
        ui.heading(&profile.name);
        self.draw_profile_management_panel(ui, &profile);
        self.draw_profile_composition_panel(ui, &profile);
        self.draw_profile_launch_panel(ui);
        self.draw_profile_installation_panel(ui, &profile);
    }
}
