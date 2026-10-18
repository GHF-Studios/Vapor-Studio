use super::super::{CatalogFilter, StudioApp};
use super::design::{LIST_ROW_HEIGHT, StatusTone, status_text};
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
                    "Nothing on this device or in the open project folder matches this search and filter.",
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
            let conflicted = !content.is_unambiguous();
            ui.horizontal_wrapped(|ui| {
                let marker_width = if conflicted { 92.0 } else { 0.0 };
                let row_width = (ui.available_width() - marker_width).max(1.0);
                if ui
                    .add_sized(
                        egui::vec2(row_width, LIST_ROW_HEIGHT),
                        egui::Button::selectable(
                            selected,
                            super::super::display_catalog_name(content),
                        ),
                    )
                    .clicked()
                {
                    newly_selected = Some(super::super::catalog_content_selector(content));
                }
                if conflicted {
                    status_text(ui, "Conflict", StatusTone::Caution);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.small(super::super::display_content_kind(&content.kind));
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
            self.select_catalog_identity(identity);
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
                ui.heading("Projects in this folder · not added yet");
                ui.label("Add a project to make it available in Vapor Studio. Its files stay where they are.");
                found_candidate = true;
            }
            egui::Frame::group(&card_style).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.strong(&candidate.id);
                        ui.small(format!(
                            "{} · {}",
                            super::super::display_content_kind(&candidate.kind.to_string()),
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
                                !self.operation.running
                                    && self.workspace.unregistered_scan_error.is_none()
                                    && self.catalog.source_snapshot_current()
                                    && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Add this project"),
                            )
                            .on_hover_text("Adds this project to Studio without moving or changing its files.")
                            .on_disabled_hover_text(if self.workspace.unregistered_scan_error.is_some() {
                                "Refresh the project list before adding one; the current results may be out of date."
                            } else if !self.catalog.source_snapshot_current() {
                                "Refresh the project list before adding one; it may already be available in Studio."
                            } else {
                                "Save or discard your unsaved edits before adding this project."
                            })
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
