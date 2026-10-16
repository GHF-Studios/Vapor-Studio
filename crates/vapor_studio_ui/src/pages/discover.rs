use super::super::{StudioApp, catalog_matches_filters};
use super::super::catalog::DiscoverTab;
use crate::navigation::CatalogSourceFilter;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_discover(&mut self, ui: &mut egui::Ui) {
        ui.label("Find something to play, install, or bring into your workspace.");
        ui.horizontal(|ui| {
            self.draw_profile_target_selector(ui);
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.catalog.discover_tab, DiscoverTab::Local, "On this device");
            ui.selectable_value(&mut self.catalog.discover_tab, DiscoverTab::Workshop, "Steam Workshop");
        });
        ui.separator();
        if self.catalog.discover_tab == DiscoverTab::Workshop {
            self.draw_workshop_browser(ui);
            return;
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.catalog.search_query)
                .desired_width(320.0)
                .hint_text("Search local content"));
            egui::ComboBox::from_id_salt("catalog-source-filter")
                .selected_text(self.catalog.source_filter.label())
                .show_ui(ui, |ui| {
                    for filter in CatalogSourceFilter::ALL {
                        ui.selectable_value(&mut self.catalog.source_filter, filter, filter.label());
                    }
                });
            let mut kinds = self.catalog.items.iter().map(|content| content.kind.clone())
                .collect::<std::collections::BTreeSet<_>>();
            kinds.extend(self.workspace.unregistered_projects.candidates.iter()
                .map(|candidate| candidate.kind.to_string()));
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
        let matching_catalog_items = self.catalog.items
            .iter()
            .filter(|content| catalog_matches_filters(
                content,
                self.catalog.source_filter,
                self.catalog.kind_filter.as_deref(),
                &self.catalog.search_query,
            ))
            .count();
        let matching_workspace_projects = self.workspace.unregistered_projects.candidates
            .iter()
            .filter(|candidate| super::super::catalog::workspace_candidate_matches_filters(
                candidate,
                self.catalog.source_filter,
                self.catalog.kind_filter.as_deref(),
                &self.catalog.search_query,
            ))
            .count();
        ui.horizontal(|ui| {
            ui.weak(format!(
                "{matching_catalog_items} content versions · {matching_workspace_projects} authored projects to adopt"
            ));
        });
        if matching_catalog_items == 0 && matching_workspace_projects == 0 {
            let has_constraints = !self.catalog.search_query.trim().is_empty()
                || self.catalog.source_filter != CatalogSourceFilter::All
                || self.catalog.kind_filter.is_some();
            let card_style = ui.style().clone();
            egui::Frame::group(&card_style).show(ui, |ui| {
                if has_constraints {
                    ui.heading("No content matches these filters");
                    ui.label("Clear the search and filters to see everything available on this device.");
                    if ui.button("Clear search and filters").clicked() {
                        self.catalog.search_query.clear();
                        self.catalog.source_filter = CatalogSourceFilter::All;
                        self.catalog.kind_filter = None;
                        self.catalog.selected = None;
                    }
                } else if self.workspace.root.trim().is_empty() {
                    ui.heading("No local content yet");
                    ui.label("Open a workspace to create or discover authored content, or browse Steam Workshop for existing packages.");
                    ui.horizontal(|ui| {
                        if ui.button("Open a workspace").clicked() {
                            self.section = super::super::Section::Workspace;
                        }
                        if ui.button("Create a workspace").clicked() {
                            self.section = super::super::Section::Create;
                        }
                        if ui.button("Browse Steam Workshop").clicked() {
                            self.catalog.discover_tab = DiscoverTab::Workshop;
                        }
                    });
                } else {
                    ui.heading("Nothing else is available locally");
                    ui.label("Create a project in this workspace, or search Workshop to acquire content for the active profile.");
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Create content")).clicked() {
                            self.section = super::super::Section::Create;
                        }
                        if ui.button("Browse Steam Workshop").clicked() {
                            self.catalog.discover_tab = DiscoverTab::Workshop;
                        }
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh local content")).clicked() {
                            self.discover();
                        }
                    });
                }
            });
            return;
        }
        self.draw_workspace_candidates(ui);
        self.draw_catalog_list(ui);
        self.draw_content_detail(ui);
    }
}
