use super::super::{StudioApp, catalog_matches_filters};
use eframe::egui;

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_catalog_list(&mut self, ui: &mut egui::Ui) {
        if self.catalog.items.is_empty() {
            if self.workspace.unregistered_projects.candidates.iter().all(|candidate| {
                !super::super::catalog::workspace_candidate_matches_filters(
                    candidate,
                    self.catalog.source_filter,
                    self.catalog.kind_filter.as_deref(),
                    &self.catalog.search_query,
                )
            }) {
                ui.weak("No workspace, cached, or installed content matches this search and filter.");
            }
            return;
        }
        let matching_items = self.catalog.items.iter().filter(|content| {
            catalog_matches_filters(
                content,
                self.catalog.source_filter,
                self.catalog.kind_filter.as_deref(),
                &self.catalog.search_query,
            )
        }).count();
        if matching_items == 0 {
            ui.weak("No catalog content matches this search and filter.");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for content in self.catalog.items.clone() {
                let identity = super::super::catalog_content_selector(&content);
                if !catalog_matches_filters(
                    &content,
                    self.catalog.source_filter,
                    self.catalog.kind_filter.as_deref(),
                    &self.catalog.search_query,
                ) { continue; }
                let selected = self.catalog.selected.as_deref() == Some(identity.as_str());
                ui.horizontal(|ui| {
                    if ui.selectable_label(
                        selected,
                        format!(
                            "{} · {} · {}",
                            content.id,
                            content.kind,
                            content.version.as_deref().unwrap_or("unversioned"),
                        ),
                    ).clicked() {
                        self.catalog.selected = Some(identity.clone());
                        self.workshop.preview = None;
                    }
                    if !content.is_unambiguous() {
                        ui.colored_label(egui::Color32::YELLOW, "Conflict");
                    }
                });
                let location_summary = content.locations.iter().map(|location| {
                    let source = match location.kind {
                        vapor_studio_core::CatalogLocationKind::Workspace => "Workspace source",
                        vapor_studio_core::CatalogLocationKind::Cache => "Cached",
                        vapor_studio_core::CatalogLocationKind::Installed if location.enabled => "Installed",
                        vapor_studio_core::CatalogLocationKind::Installed => "Installed · disabled",
                    };
                    source
                }).collect::<Vec<_>>().join(" · ");
                ui.small(location_summary);
                ui.separator();
            }
        });
    }

    pub(in crate::app) fn draw_workspace_candidates(&mut self, ui: &mut egui::Ui) {
        let candidates = self.workspace.unregistered_projects.candidates.iter()
            .filter(|candidate| super::super::catalog::workspace_candidate_matches_filters(
                candidate,
                self.catalog.source_filter,
                self.catalog.kind_filter.as_deref(),
                &self.catalog.search_query,
            ))
            .cloned()
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return;
        }
        ui.heading("Authored in this workspace · needs adoption");
        ui.label("These projects are on this device but are not in the workspace content index yet. Adopt keeps their files in place and makes them available to Compose and profile setup.");
        for candidate in candidates {
            let card_style = ui.style().clone();
            egui::Frame::group(&card_style).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.strong(&candidate.id);
                        ui.small(format!(
                            "{} · {}",
                            candidate.kind,
                            candidate.path.strip_prefix(std::path::Path::new(self.workspace.root.trim()))
                                .unwrap_or(&candidate.path).display(),
                        ));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(
                            !self.operation.running && !self.has_unsaved_workspace_edits(),
                            egui::Button::new("Adopt into workspace"),
                        ).clicked() {
                            self.adopt_content_project(candidate.path.clone());
                        }
                    });
                });
            });
        }
        ui.separator();
    }
}
