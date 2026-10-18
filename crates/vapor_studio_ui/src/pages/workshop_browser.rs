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
        if !self.workshop.steam_client_available {
            status_text(
                ui,
                "Steam is unavailable. Your on-device Library still works; Workshop search, subscriptions, and downloads need the Steam client.",
                StatusTone::Caution,
            );
        }
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
        }
    }
}
