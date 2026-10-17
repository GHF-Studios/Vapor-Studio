use super::super::catalog::DiscoverTab;
use super::super::components::design::{
    StatusTone, empty_state, page_header, search_field_width, section_header, status_text,
    use_stacked_panels,
};
use super::super::{CatalogFilter, StudioApp};
use crate::navigation::CatalogSourceFilter;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_discover(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Browse",
            "Find a game or add-on",
            "Browse content already on this device or find game setups and add-ons in Steam Workshop.",
        );
        ui.horizontal_wrapped(|ui| {
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
        if let Some(error) = self.catalog.catalog_discovery_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "The on-device content list could not refresh; {}. {error}",
                    if self.catalog.catalog_discovery_loaded {
                        "showing the last successful list"
                    } else {
                        "no successful list is available yet"
                    }
                ),
                StatusTone::Caution,
            );
        }
        if let Some(error) = self.catalog.source_discovery_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "The project-folder list could not refresh; {}. {error}",
                    if self.catalog.source_discovery_loaded {
                        "showing the last successful list"
                    } else {
                        "no successful list is available yet"
                    }
                ),
                StatusTone::Caution,
            );
        }
        if let Some(error) = self.workspace.unregistered_scan_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "The project-folder scan failed; {}. {error}",
                    if self.workspace.unregistered_scan_completed {
                        "showing its last successful results"
                    } else {
                        "no successful results are available yet"
                    }
                ),
                StatusTone::Caution,
            );
        }
        ui.horizontal_wrapped(|ui| {
            let search_width = search_field_width(ui);
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.catalog.search_query)
                    .desired_width(search_width)
                    .hint_text("Search on-device content"),
            );
            if search.changed() {
                self.submit_service_action(vapor_studio_protocol::StudioAction::Catalog(
                    vapor_studio_protocol::CatalogAction::Search {
                        query: self.catalog.search_query.clone(),
                    },
                ));
            }
            egui::ComboBox::from_id_salt("catalog-source-filter")
                .selected_text(self.catalog.source_filter.label())
                .show_ui(ui, |ui| {
                    for filter in CatalogSourceFilter::ALL {
                        ui.selectable_value(
                            &mut self.catalog.source_filter,
                            filter,
                            filter.label(),
                        );
                    }
                });
            let mut kinds = self
                .catalog
                .items
                .iter()
                .map(|content| content.kind.clone())
                .collect::<std::collections::BTreeSet<_>>();
            kinds.extend(
                self.workspace
                    .unregistered_projects
                    .candidates
                    .iter()
                    .map(|candidate| candidate.kind.to_string()),
            );
            egui::ComboBox::from_id_salt("catalog-kind-filter")
                .selected_text(
                    self.catalog
                        .kind_filter
                        .as_deref()
                        .unwrap_or("All content types"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.catalog.kind_filter, None, "All content types");
                    for kind in kinds {
                        ui.selectable_value(
                            &mut self.catalog.kind_filter,
                            Some(kind.clone()),
                            super::super::display_content_kind(&kind),
                        );
                    }
                });
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Refresh on-device content"),
                )
                .on_disabled_hover_text(
                    "Save or discard your open edits first; Studio reads saved project details.",
                )
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
                self.submit_service_action(vapor_studio_protocol::StudioAction::Catalog(
                    vapor_studio_protocol::CatalogAction::Search {
                        query: String::new(),
                    },
                ));
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
        ui.horizontal_wrapped(|ui| {
            ui.weak(format!(
                "{matching_catalog_items} items available here · {matching_workspace_projects} project folders to add"
            ));
        });
        if matching_catalog_items == 0 && matching_workspace_projects == 0 {
            let has_constraints = !self.catalog.search_query.trim().is_empty()
                || self.catalog.source_filter != CatalogSourceFilter::All
                || self.catalog.kind_filter.is_some();
            let discovery_incomplete = self.catalog.catalog_discovery_error.is_some()
                || self.catalog.source_discovery_error.is_some()
                || self.workspace.unregistered_scan_error.is_some();
            let card_style = ui.style().clone();
            egui::Frame::group(&card_style).show(ui, |ui| {
                if has_constraints && discovery_incomplete {
                    ui.heading("Search results may be incomplete");
                    ui.label("A content list failed to refresh. Clear filters or refresh on-device content before treating this as no matches.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Clear search and filters").clicked() {
                            self.catalog.search_query.clear();
                            self.catalog.source_filter = CatalogSourceFilter::All;
                            self.catalog.kind_filter = None;
                        }
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh on-device content")).clicked() {
                            self.discover();
                        }
                    });
                } else if has_constraints {
                    ui.heading("No content matches these filters");
                    ui.label("Clear the search and filters to see everything available on this device.");
                    if ui.button("Clear search and filters").clicked() {
                        self.catalog.search_query.clear();
                        self.catalog.source_filter = CatalogSourceFilter::All;
                        self.catalog.kind_filter = None;
                    }
                } else if self.workspace.root.trim().is_empty() && discovery_incomplete {
                    ui.heading("Local content could not be confirmed");
                    ui.label("One or more on-device content lists failed to refresh. Retry to confirm what is installed or downloaded, or browse Workshop in the meantime.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh on-device content")).clicked() {
                            self.discover();
                        }
                        if ui.button("Browse Steam Workshop").clicked() {
                            self.catalog.discover_tab = DiscoverTab::Workshop;
                        }
                    });
                } else if self.workspace.root.trim().is_empty() {
                    ui.heading("No content on this device yet");
                    ui.label("Open a project folder to create or find your projects, or browse Steam Workshop for existing game content.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Open a project folder").clicked() {
                            self.open_workspace_picker();
                        }
                        if ui.button("Create a project folder").clicked() {
                            self.activate_navigation_section(super::super::Section::Create);
                        }
                        if ui.button("Browse Steam Workshop").clicked() {
                            self.catalog.discover_tab = DiscoverTab::Workshop;
                        }
                    });
                } else if discovery_incomplete {
                    ui.heading("Local content could not be confirmed");
                    ui.label("A project-folder or on-device list failed to refresh. Try again before creating or choosing content.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh on-device content")).clicked() {
                            self.discover();
                        }
                        if ui.button("Browse Steam Workshop").clicked() {
                            self.catalog.discover_tab = DiscoverTab::Workshop;
                        }
                    });
                } else {
                    ui.heading("Nothing else is available locally");
                    ui.label("Create a project in an open folder, or search Workshop for content to add to the active game setup.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Create content")).clicked() {
                            self.activate_navigation_section(super::super::Section::Create);
                        }
                        if ui.button("Browse Steam Workshop").clicked() {
                            self.catalog.discover_tab = DiscoverTab::Workshop;
                        }
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh on-device content")).clicked() {
                            self.discover();
                        }
                    });
                }
            });
            return;
        }
        let selected_is_hidden = selected_exists && !selected_is_visible;
        if use_stacked_panels(ui) {
            section_header(
                ui,
                "On this device",
                "Content already downloaded or included in an open project folder.",
            );
            self.draw_workspace_candidates(ui, &filter);
            self.draw_catalog_list(ui, &filter);
            ui.separator();
            section_header(
                ui,
                "Details",
                "Select an item to see what it needs and what you can do with it.",
            );
            self.draw_selected_content_detail(ui, selected_is_visible, selected_is_hidden);
        } else {
            ui.columns(2, |columns| {
                section_header(
                    &mut columns[0],
                    "On this device",
                    "Content already downloaded or included in an open project folder.",
                );
                egui::ScrollArea::vertical().show(&mut columns[0], |ui| {
                    self.draw_workspace_candidates(ui, &filter);
                    self.draw_catalog_list(ui, &filter);
                });

                section_header(
                    &mut columns[1],
                    "Details",
                    "Select an item to see what it needs and what you can do with it.",
                );
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
