use super::super::catalog::DiscoverTab;
use super::super::components::design::{empty_state, page_header, section_header};
use super::super::{CatalogFilter, StudioApp};
use crate::navigation::CatalogSourceFilter;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_discover(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Content",
            "Discover",
            "Browse content on this device or find packages in Steam Workshop.",
        );
        ui.horizontal(|ui| {
            ui.add(egui::Button::selectable(
                self.catalog.discover_tab == DiscoverTab::Local,
                "On this device",
            ))
            .clicked()
            .then(|| self.catalog.discover_tab = DiscoverTab::Local);
            ui.add(egui::Button::selectable(
                self.catalog.discover_tab == DiscoverTab::Workshop,
                "Steam Workshop",
            ))
            .clicked()
            .then(|| self.catalog.discover_tab = DiscoverTab::Workshop);
        });
        ui.separator();
        if self.catalog.discover_tab == DiscoverTab::Workshop {
            self.draw_workshop_browser(ui);
            return;
        }
        ui.horizontal_wrapped(|ui| {
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
            let mut kinds = self
                .catalog
                .items
                .iter()
                .map(|content| content.kind.clone())
                .collect::<std::collections::BTreeSet<_>>();
            kinds.extend(self.workspace.unregistered_projects.candidates.iter()
                .map(|candidate| candidate.kind.to_string()));
            egui::ComboBox::from_id_salt("catalog-kind-filter")
                .selected_text(self.catalog.kind_filter.as_deref().unwrap_or("All content types"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.catalog.kind_filter, None, "All content types");
                    for kind in kinds {
                        if ui
                            .selectable_label(
                                self.catalog.kind_filter.as_deref() == Some(kind.as_str()),
                                &kind,
                            )
                            .clicked()
                        {
                            self.catalog.kind_filter = Some(kind);
                        }
                    }
                });
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh local catalog"))
                .on_disabled_hover_text("Save or discard workspace file and structured metadata drafts first; discovery reads saved manifests.")
                .clicked()
            {
                self.discover();
            }
            if (!self.catalog.search_query.trim().is_empty()
                || self.catalog.source_filter != CatalogSourceFilter::All
                || self.catalog.kind_filter.is_some())
                && ui.button("Clear filters").clicked()
            {
                self.catalog.search_query.clear();
                self.catalog.source_filter = CatalogSourceFilter::All;
                self.catalog.kind_filter = None;
            }
        });
        let filter = CatalogFilter::new(
            self.catalog.source_filter,
            self.catalog.kind_filter.clone(),
            &self.catalog.search_query,
        );
        let selected_identity = self.catalog.selected.as_deref();
        let mut matching_catalog_items = 0;
        let mut selected_exists = false;
        let mut selected_is_visible = false;
        for content in &self.catalog.items {
            let is_selected = selected_identity.is_some_and(|identity| {
                super::super::catalog_content_selector_matches(content, identity)
            });
            let matches_filters = filter.matches_catalog(content);
            if matches_filters {
                matching_catalog_items += 1;
            }
            if is_selected {
                selected_exists = true;
                selected_is_visible = matches_filters;
            }
        }
        let matching_workspace_projects = self
            .workspace
            .unregistered_projects
            .candidates
            .iter()
            .filter(|candidate| filter.matches_workspace(candidate))
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
        let selected_is_hidden = selected_exists && !selected_is_visible;
        if ui.available_width() < 820.0 {
            section_header(
                ui,
                "Available content",
                "Local sources and authored projects in the active workspace.",
            );
            self.draw_workspace_candidates(ui, &filter);
            self.draw_catalog_list(ui, &filter);
            ui.separator();
            section_header(
                ui,
                "Details",
                "Select a content version to review its source, dependencies, and available actions.",
            );
            self.draw_selected_content_detail(ui, selected_is_visible, selected_is_hidden);
        } else {
            ui.columns(2, |columns| {
                columns[0].set_min_width(360.0);
                section_header(&mut columns[0], "Available content", "Local sources and authored projects in the active workspace.");
                egui::ScrollArea::vertical().show(&mut columns[0], |ui| {
                    self.draw_workspace_candidates(ui, &filter);
                    self.draw_catalog_list(ui, &filter);
                });

                columns[1].set_min_width(360.0);
                section_header(&mut columns[1], "Details", "Select a content version to review its source, dependencies, and available actions.");
                self.draw_selected_content_detail(
                    &mut columns[1],
                    selected_is_visible,
                    selected_is_hidden,
                );
            });
        }
    }

    fn draw_selected_content_detail(
        &mut self,
        ui: &mut egui::Ui,
        selected_is_visible: bool,
        selected_is_hidden: bool,
    ) {
        if selected_is_visible {
            self.draw_content_detail(ui);
        } else if !selected_is_hidden {
            empty_state(
                ui,
                "Choose content to inspect",
                "The detail panel will show where a version came from and whether it can be selected, installed, or published.",
                |_| {},
            );
        } else {
            let selected = self.catalog.selected.clone().unwrap_or_default();
            empty_state(
                ui,
                "Selected content is filtered out",
                &format!(
                    "{selected} is still selected, but the current search or filters hide it."
                ),
                |ui| {
                    if ui.button("Clear search and filters").clicked() {
                        self.catalog.search_query.clear();
                        self.catalog.source_filter = CatalogSourceFilter::All;
                        self.catalog.kind_filter = None;
                    }
                },
            );
        }
    }
}
