use super::super::components::design::{
    contextual_split, empty_state, page_header, search_field_width, section_header, status_text,
    use_stacked_panels,
};
use super::super::{CatalogFilter, StudioApp};
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_library(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Library",
            "Your content",
            "Downloaded or installed runtime content. Packagepacks are under Compositions; Rust Crates stay in Creator projects.",
        );
        if let Some(error) = self.catalog.catalog_discovery_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "The Library could not refresh; {}. {error}",
                    if self.catalog.catalog_discovery_loaded {
                        "showing the last successful list"
                    } else {
                        "no successful list is available yet"
                    }
                ),
                super::super::components::design::StatusTone::Caution,
            );
        }

        if self.catalog.catalog_omitted > 0 {
            status_text(
                ui,
                &format!(
                    "Showing a bounded content list; {} additional items are omitted. Results are incomplete.",
                    self.catalog.catalog_omitted,
                ),
                super::super::components::design::StatusTone::Caution,
            );
        }

        ui.horizontal_wrapped(|ui| {
            let search_width = search_field_width(ui);
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.catalog.library_search_query)
                    .desired_width(search_width)
                    .hint_text("Search your Library"),
            );
            if search.changed() {
                self.submit_service_action(vapor_client_protocol::ClientAction::Catalog(
                    vapor_client_protocol::CatalogAction::Search {
                        query: self.catalog.library_search_query.clone(),
                    },
                ));
            }
            let kinds = self
                .catalog
                .items
                .iter()
                .filter(|content| {
                    let role = self
                        .catalog
                        .service_item_roles
                        .get(&(content.id.clone(), content.version.clone()))
                        .copied();
                    CatalogFilter::library(None, "")
                        .matches_catalog_with_role(content, role)
                })
                .map(|content| content.kind.clone())
                .collect::<std::collections::BTreeSet<_>>();
            egui::ComboBox::from_id_salt("library-kind-filter")
                .selected_text(
                    self.catalog
                        .library_kind_filter
                        .as_deref()
                        .unwrap_or("All content types"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.catalog.library_kind_filter,
                        None,
                        "All content types",
                    );
                    for kind in kinds {
                        ui.selectable_value(
                            &mut self.catalog.library_kind_filter,
                            Some(kind.clone()),
                            super::super::display_content_kind(&kind),
                        );
                    }
                });
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Refresh Library"),
                )
                .on_disabled_hover_text(
                    "Save or discard your open edits before refreshing saved content details.",
                )
                .clicked()
            {
                self.discover();
            }
            if (!self.catalog.library_search_query.trim().is_empty()
                || self.catalog.library_kind_filter.is_some())
                && ui.button("Clear filters").clicked()
            {
                self.catalog.library_search_query.clear();
                self.catalog.library_kind_filter = None;
                self.submit_service_action(vapor_client_protocol::ClientAction::Catalog(
                    vapor_client_protocol::CatalogAction::Search {
                        query: String::new(),
                    },
                ));
            }
        });

        let filter = CatalogFilter::library(
            self.catalog.library_kind_filter.clone(),
            &self.catalog.library_search_query,
        );
        let matching = self
            .catalog
            .items
            .iter()
            .filter(|content| {
                let role = self
                    .catalog
                    .service_item_roles
                    .get(&(content.id.clone(), content.version.clone()))
                    .copied();
                filter.matches_catalog_with_role(content, role)
            })
            .count();
        let selected_identity = self.catalog.selected.as_deref();
        let mut selected_exists = false;
        let mut selected_is_visible = false;
        for content in &self.catalog.items {
            let is_selected = selected_identity.is_some_and(|identity| {
                super::super::catalog_content_selector_matches(content, identity)
            });
            if is_selected {
                selected_exists = true;
                let role = self
                    .catalog
                    .service_item_roles
                    .get(&(content.id.clone(), content.version.clone()))
                    .copied();
                selected_is_visible = filter.matches_catalog_with_role(content, role);
            }
        }
        ui.weak(format!("{matching} items in your Library"));

        if matching == 0 {
            let has_filters = !self.catalog.library_search_query.trim().is_empty()
                || self.catalog.library_kind_filter.is_some();
            let needs_refresh = !self.catalog.catalog_discovery_loaded
                || self.catalog.catalog_discovery_error.is_some();
            empty_state(
                ui,
                if has_filters {
                    "No Library items match"
                } else if needs_refresh {
                    "Library is not up to date"
                } else {
                    "Your Library is empty"
                },
                if has_filters {
                    "Clear the search and content type filter to see everything in your Library."
                } else if needs_refresh {
                    "Refresh the Library to check which content is available on this device."
                } else {
                    "Browse content to find items, or visit Steam Workshop to download something new."
                },
                |ui| {
                    if has_filters {
                        if ui.button("Clear filters").clicked() {
                            self.catalog.library_search_query.clear();
                            self.catalog.library_kind_filter = None;
                        }
                    } else if needs_refresh {
                        if ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Refresh Library"),
                            )
                            .clicked()
                        {
                            self.discover();
                        }
                    } else {
                        if ui.button("Browse content").clicked() {
                            self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                        }
                        if ui.button("Browse Steam Workshop").clicked() {
                            self.open_workshop_tab(super::super::workshop::WorkshopTab::Browse);
                        }
                    }
                },
            );
            return;
        }

        if use_stacked_panels(ui) {
            section_header(
                ui,
                "Available content",
                "Downloaded or installed items, ready to review.",
            );
            self.draw_catalog_list(ui, &filter);
            ui.separator();
            section_header(ui, "Details", "Select an item to review it.");
            draw_selected_details(self, ui, selected_is_visible, selected_exists);
        } else {
            contextual_split(ui, |items, details| {
                section_header(
                    items,
                    "Available content",
                    "Downloaded or installed items, ready to review.",
                );
                egui::ScrollArea::vertical().show(items, |ui| {
                    self.draw_catalog_list(ui, &filter);
                });
                section_header(details, "Details", "Selected item");
                egui::ScrollArea::vertical().show(details, |ui| {
                    draw_selected_details(self, ui, selected_is_visible, selected_exists);
                });
            });
        }
    }
}

fn draw_selected_details(
    app: &mut StudioApp,
    ui: &mut egui::Ui,
    selected_is_visible: bool,
    selected_exists: bool,
) {
    if selected_is_visible {
        app.draw_content_detail(ui);
    } else if selected_exists {
        empty_state(
            ui,
            "Selected item is outside this Library",
            "It may be a project-folder item or a playable Packagepack. Browse content or Compositions to find it.",
            |ui| {
                if ui.button("Browse content").clicked() {
                    app.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                }
                if ui.button("Open Compositions").clicked() {
                    app.activate_navigation_section(super::super::Section::Compositions);
                }
            },
        );
    } else {
        empty_state(
            ui,
            "Choose content",
            "Select an item to review its details and available actions.",
            |_| {},
        );
    }
}
