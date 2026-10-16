use super::super::{StudioApp, catalog_matches_filters};
use crate::navigation::CatalogSourceFilter;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_discover(&mut self, ui: &mut egui::Ui) {
        ui.heading("Discover content");
        ui.label("Browse workspace, cached, and installed content, or bring a published Workshop item into this app.");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("catalog-source-filter")
                .selected_text(self.catalog.source_filter.label())
                .show_ui(ui, |ui| {
                    for filter in CatalogSourceFilter::ALL {
                        ui.selectable_value(&mut self.catalog.source_filter, filter, filter.label());
                    }
                });
            let kinds = self.catalog.items.iter().map(|content| content.kind.clone())
                .collect::<std::collections::BTreeSet<_>>();
            egui::ComboBox::from_id_salt("catalog-kind-filter")
                .selected_text(self.catalog.kind_filter.as_deref().unwrap_or("All content types"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.catalog.kind_filter, None, "All content types");
                    for kind in kinds {
                        ui.selectable_value(&mut self.catalog.kind_filter, Some(kind.clone()), kind);
                    }
                });
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh local catalog"))
                .on_disabled_hover_text("Save or discard workspace file and structured metadata drafts first; discovery reads saved manifests.")
                .clicked()
            {
                self.discover();
            }
        });
        ui.horizontal(|ui| {
            ui.weak(format!(
                "{} matching content versions",
                self.catalog.items
                    .iter()
                    .filter(|content| {
                        catalog_matches_filters(
                            content,
                            self.catalog.source_filter,
                            self.catalog.kind_filter.as_deref(),
                            &self.catalog.search_query,
                        )
                    })
                    .count()
            ));
        });
        self.draw_workshop_browser(ui);
        ui.separator();
        self.draw_catalog_list(ui);
        self.draw_content_detail(ui);
    }
}
