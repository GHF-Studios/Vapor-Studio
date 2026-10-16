use super::super::StudioApp;
use super::super::workshop::WorkshopTab;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_browser(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (WorkshopTab::Browse, "Browse items"),
                (WorkshopTab::Subscriptions, "Subscriptions"),
                (WorkshopTab::Imports, "Vapor Library"),
            ] {
                if ui
                    .add(egui::Button::selectable(self.workshop.tab == tab, label))
                    .clicked()
                {
                    self.workshop.tab = tab;
                }
            }
        });
        ui.separator();
        match self.workshop.tab {
            WorkshopTab::Browse => self.draw_workshop_search_panel(ui),
            WorkshopTab::Subscriptions => self.draw_workshop_subscriptions_panel(ui),
            WorkshopTab::Imports => self.draw_workshop_acquisition_panel(ui),
        }
    }
}
