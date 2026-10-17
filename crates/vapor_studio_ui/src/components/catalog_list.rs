use super::super::{CatalogFilter, StudioApp};
use eframe::egui;

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_catalog_list(&mut self, ui: &mut egui::Ui, filter: &CatalogFilter) {
        if self.catalog.items.is_empty() {
            if self
                .workspace
                .unregistered_projects
                .candidates
                .iter()
                .all(|candidate| !filter.matches_workspace(candidate))
            {
                ui.weak(
                    "No workspace, cached, or installed content matches this search and filter.",
                );
            }
            return;
        }
        let mut found_matching_item = false;
        let mut newly_selected = None;
        for content in self
            .catalog
            .items
            .iter()
            .filter(|content| filter.matches_catalog(content))
        {
            found_matching_item = true;
            let selected = self.catalog.selected.as_deref().is_some_and(|identity| {
                super::super::catalog_content_selector_matches(content, identity)
            });
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        selected,
                        format!(
                            "{} · {} · {}",
                            content.id,
                            content.kind,
                            content.version.as_deref().unwrap_or("unversioned"),
                        ),
                    )
                    .clicked()
                {
                    newly_selected = Some(super::super::catalog_content_selector(content));
                }
                if !content.is_unambiguous() {
                    ui.colored_label(egui::Color32::YELLOW, "Conflict");
                }
            });
            ui.horizontal_wrapped(|ui| {
                for location in &content.locations {
                    ui.small(super::super::catalog::catalog_location_label(location));
                }
            });
            ui.separator();
        }
        if !found_matching_item {
            ui.weak("No catalog content matches this search and filter.");
            return;
        }
        if let Some(identity) = newly_selected {
            self.catalog.selected = Some(identity);
            self.workshop.preview = None;
        }
    }

    pub(in crate::app) fn draw_workspace_candidates(
        &mut self,
        ui: &mut egui::Ui,
        filter: &CatalogFilter,
    ) {
        let mut adoption_path = None;
        let mut found_candidate = false;
        let card_style = ui.style().clone();
        for candidate in self
            .workspace
            .unregistered_projects
            .candidates
            .iter()
            .filter(|candidate| filter.matches_workspace(candidate))
        {
            if !found_candidate {
                ui.heading("Authored in this workspace · needs adoption");
                ui.label("These projects are on this device but are not in the workspace content index yet. Adopt keeps their files in place and makes them available to Compose and profile setup.");
                found_candidate = true;
            }
            egui::Frame::group(&card_style).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.strong(&candidate.id);
                        ui.small(format!(
                            "{} · {}",
                            candidate.kind,
                            candidate
                                .path
                                .strip_prefix(std::path::Path::new(self.workspace.root.trim()))
                                .unwrap_or(&candidate.path)
                                .display(),
                        ));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Adopt into workspace"),
                            )
                            .clicked()
                        {
                            adoption_path = Some(candidate.path.clone());
                        }
                    });
                });
            });
        }
        if let Some(path) = adoption_path {
            self.adopt_content_project(path);
        }
        if found_candidate {
            ui.separator();
        }
    }
}
