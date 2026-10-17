use super::super::StudioApp;
use super::super::components::design::{StatusTone, status_text};
use super::super::workshop::WorkshopTab;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_browser(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in [
                (WorkshopTab::Browse, "Browse items"),
                (WorkshopTab::Subscriptions, "Subscriptions"),
                (WorkshopTab::Imports, "Downloaded"),
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
        if let Some(error) = self.catalog.catalog_discovery_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "The list of content on this device could not refresh, so availability details may be out of date. {error}"
                ),
                StatusTone::Caution,
            );
        }
        match self.workshop.tab {
            WorkshopTab::Browse => self.draw_workshop_search_panel(ui),
            WorkshopTab::Subscriptions => self.draw_workshop_subscriptions_panel(ui),
            WorkshopTab::Imports => self.draw_workshop_acquisition_panel(ui),
        }
    }
}
