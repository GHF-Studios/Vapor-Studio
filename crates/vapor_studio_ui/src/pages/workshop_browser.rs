use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_browser(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            self.draw_workshop_subscriptions_panel(ui);
            self.draw_workshop_search_panel(ui);
            self.draw_workshop_acquisition_panel(ui);
        });
    }
}
