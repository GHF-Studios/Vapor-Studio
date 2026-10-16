use super::super::{StudioApp, catalog_matches_filters};
use crate::navigation::CatalogSourceFilter;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_discover(&mut self, ui: &mut egui::Ui) {
        ui.heading("Discover content");
        ui.label("Browse workspace, cached, and installed content, or bring a published Workshop item into this app.");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("catalog-source-filter")
                .selected_text(self.catalog_source_filter.label())
                .show_ui(ui, |ui| {
                    for filter in CatalogSourceFilter::ALL {
                        ui.selectable_value(&mut self.catalog_source_filter, filter, filter.label());
                    }
                });
            let kinds = self.catalog.iter().map(|content| content.kind.clone())
                .collect::<std::collections::BTreeSet<_>>();
            egui::ComboBox::from_id_salt("catalog-kind-filter")
                .selected_text(self.catalog_kind_filter.as_deref().unwrap_or("All content types"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.catalog_kind_filter, None, "All content types");
                    for kind in kinds {
                        ui.selectable_value(&mut self.catalog_kind_filter, Some(kind.clone()), kind);
                    }
                });
            if ui.add_enabled(!self.running && !self.workspace_editor_dirty && !self.manifest_metadata_dirty, egui::Button::new("Refresh local catalog"))
                .on_disabled_hover_text("Save or reload the open file first; discovery reads saved workspace manifests.")
                .clicked()
            {
                self.discover();
            }
        });
        ui.horizontal(|ui| {
            ui.weak(format!(
                "{} matching content versions",
                self.catalog
                    .iter()
                    .filter(|content| {
                        catalog_matches_filters(
                            content,
                            self.catalog_source_filter,
                            self.catalog_kind_filter.as_deref(),
                            &self.search_query,
                        )
                    })
                    .count()
            ));
        });
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Steam Workshop");
                if ui.add_enabled(!self.running, egui::Button::new("My subscriptions")).clicked() {
                    self.refresh_workshop_subscriptions();
                }
            });
            ui.label("Steam subscriptions are separate from Vapor Library imports. Import an item first, then choose whether it belongs in a profile.");
            ui.weak(format!(
                "Target profile: {} · local catalog remains available when Steam is offline",
                self.profiles[self.active_profile].name
            ));
            if !self.workshop.subscriptions.is_empty() {
                ui.collapsing(format!("My subscriptions ({})", self.workshop.subscriptions.len()), |ui| {
                    for id in self.workshop.subscriptions.iter().cloned().collect::<Vec<_>>() {
                        let details = self.workshop.subscription_details.get(&id).cloned();
                        let title = details
                            .as_ref()
                            .map(|item| item.title.clone())
                            .or_else(|| {
                                self.workshop.results
                                    .iter()
                                    .find(|item| item.published_file_id == id)
                                    .map(|item| item.title.clone())
                            })
                            .unwrap_or_else(|| "Workshop item".to_owned());
                        let locally_available = self.catalog.iter().any(|content| {
                            content.published_file_id.as_deref() == Some(id.as_str())
                        });
                        ui.horizontal(|ui| {
                            ui.strong(title);
                            ui.weak(format!("PublishedFileId {id}"));
                            if let Some(item) = &details {
                                let state = if item.downloading {
                                    "Downloading in Steam"
                                } else if item.needs_update {
                                    "Steam update available"
                                } else if item.installed {
                                    "Downloaded in Steam"
                                } else {
                                    "Subscribed"
                                };
                                ui.weak(state);
                                if let Some((current, total)) = item.download_progress.filter(|(_, total)| *total > 0) {
                                    ui.weak(format!("{:.1} / {:.1} MiB", current as f64 / 1_048_576.0, total as f64 / 1_048_576.0));
                                }
                            }
                            if locally_available {
                                ui.weak("Available in Vapor Library");
                                let available_packagepack = self.catalog.iter().any(|content| {
                                    content.published_file_id.as_deref() == Some(id.as_str())
                                        && content.kind
                                            == vapor_core::ContentKind::Packagepack.as_str()
                                        && content.is_unambiguous()
                                });
                                if available_packagepack
                                    && ui.add_enabled(
                                        !self.running && !self.workspace_editor_dirty,
                                        egui::Button::new(format!(
                                            "Set {} root",
                                            self.profiles[self.active_profile].name
                                        )),
                                    )
                                    .clicked()
                                {
                                    self.use_available_workshop_package_as_profile_root(&id);
                                }
                                if self.profiles[self.active_profile].root_package.is_some()
                                    && ui.add_enabled(
                                        !self.running && !self.workspace_editor_dirty,
                                        egui::Button::new(format!(
                                            "Recheck {} plan",
                                            self.profiles[self.active_profile].name
                                        )),
                                    )
                                    .on_hover_text(
                                        "Resolve the selected Packagepack again using the current local cache.",
                                    )
                                    .clicked()
                                {
                                    self.section = super::super::Section::Library;
                                    self.preview_selected_composition();
                                }
                            } else if ui.add_enabled(!self.running, egui::Button::new("Import to Vapor Library")).clicked() {
                                self.workshop.published_file_id = id.clone();
                                self.download_workshop_item();
                            }
                            if ui.add_enabled(!self.running, egui::Button::new("Unsubscribe")).clicked() {
                                self.set_workshop_subscription(id.clone(), false);
                            }
                        });
                        if let Some(item) = details.as_ref() && !item.description.is_empty() {
                            ui.collapsing("Description", |ui| { ui.label(&item.description); });
                        }
                        if let Some(url) = details.as_ref().and_then(|item| item.preview_url.as_ref()) {
                            ui.hyperlink_to("Open Workshop item", url);
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.workshop.search_query)
                    .hint_text("Search Workshop items")
                    .desired_width(300.0));
                if ui.add_enabled(!self.running, egui::Button::new("Search Workshop")).clicked() {
                    self.workshop.page = 1;
                    self.search_workshop();
                }
            });
            if !self.workshop.results.is_empty() {
                let page_count = self.workshop.total_results.div_ceil(50).max(1);
                ui.horizontal(|ui| {
                    ui.weak(format!("Showing {} of {} results · page {} of {}", self.workshop.results.len(), self.workshop.total_results, self.workshop.page, page_count));
                    let previous = ui.add_enabled(!self.running && self.workshop.page > 1, egui::Button::new("Previous")).clicked();
                    let next = ui.add_enabled(!self.running && self.workshop.page < page_count, egui::Button::new("Next")).clicked();
                    if previous {
                        self.workshop.page -= 1;
                        self.search_workshop();
                    } else if next {
                        self.workshop.page += 1;
                        self.search_workshop();
                    }
                });
            }
            if let Some(query) = &self.workshop.results_query {
                if self.workshop.results.is_empty() {
                    if query.is_empty() {
                        ui.weak("Workshop search returned no items.");
                    } else {
                        ui.weak(format!("No Workshop items matched ‘{query}’."));
                    }
                } else {
                    ui.small(format!("Results for ‘{query}’"));
                }
            }
            for item in self.workshop.results.clone() {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(&item.title);
                    ui.weak(format!("PublishedFileId {}", item.published_file_id));
                });
                if !item.description.is_empty() {
                    ui.collapsing("Description", |ui| { ui.label(&item.description); });
                }
                ui.horizontal(|ui| {
                    let subscribed = self.workshop.subscriptions.contains(&item.published_file_id);
                    let label = if subscribed { "Unsubscribe" } else { "Subscribe" };
                    if ui.add_enabled(!self.running, egui::Button::new(label)).clicked() {
                        self.set_workshop_subscription(item.published_file_id.clone(), !subscribed);
                    }
                    let locally_available = self.catalog.iter().any(|content| {
                        content.published_file_id.as_deref()
                            == Some(item.published_file_id.as_str())
                    });
                    let available_packagepack = self.catalog.iter().any(|content| {
                        content.published_file_id.as_deref()
                            == Some(item.published_file_id.as_str())
                            && content.kind == vapor_core::ContentKind::Packagepack.as_str()
                            && content.is_unambiguous()
                    });
                    if locally_available {
                        ui.weak("In Vapor Library");
                        if available_packagepack
                            && ui.add_enabled(
                                !self.running && !self.workspace_editor_dirty,
                                egui::Button::new(format!(
                                    "Set {} root",
                                    self.profiles[self.active_profile].name
                                )),
                            ).clicked()
                        {
                            self.use_available_workshop_package_as_profile_root(
                                &item.published_file_id,
                            );
                        } else if !available_packagepack
                            && self.profiles[self.active_profile].root_package.is_some()
                            && ui.add_enabled(
                                !self.running && !self.workspace_editor_dirty,
                                egui::Button::new(format!(
                                    "Recheck {} plan",
                                    self.profiles[self.active_profile].name
                                )),
                            ).clicked()
                        {
                            self.section = super::super::Section::Library;
                            self.preview_selected_composition();
                        }
                    }
                    let import_label = if locally_available {
                        "Update Vapor Library copy"
                    } else {
                        "Import to Vapor Library"
                    };
                    if ui.add_enabled(!self.running, egui::Button::new(import_label)).clicked() {
                        self.workshop.published_file_id = item.published_file_id.clone();
                        self.download_workshop_item();
                    }
                    if let Some(url) = &item.preview_url {
                        ui.hyperlink_to("Open item", url);
                    }
                });
            }
            ui.separator();
            ui.label("Download a known public item");
            ui.horizontal(|ui| {
                ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.workshop.published_file_id)
                    .hint_text("PublishedFileId")
                    .desired_width(220.0));
                if ui.add_enabled(!self.running, egui::Button::new("Download item")).clicked() {
                    self.download_workshop_item();
                }
            });
            if let Some(acquisition) = self.workshop.pending_acquisition.clone() {
                ui.separator();
                ui.label(format!("Cached: {}{} · {}", acquisition.id,
                    acquisition.version.as_ref().map(|version| format!("@{version}")).unwrap_or_default(),
                    acquisition.kind));
                ui.label(format!("{} · {}", acquisition.cache_root.display(), acquisition.fingerprint));
                if acquisition.kind == "packagepack" {
                    if ui.add_enabled(!self.running && !self.workspace_editor_dirty,
                        egui::Button::new(format!(
                            "Use as {} root",
                            self.profiles[self.active_profile].name
                        )))
                        .on_disabled_hover_text("Save or discard editor changes before switching the profile's workspace.")
                        .clicked()
                    {
                        self.use_workshop_as_profile_root();
                    }
                    ui.weak("The selected Packagepack will appear in Compositions, where you can preview its dependency plan before installing.");
                } else {
                    ui.weak("Use this item as a dependency of a Packagepack to include it in a composition.");
                    let has_root = self.profiles[self.active_profile].root_package.is_some();
                    if ui
                        .add_enabled(
                            !self.running && !self.workspace_editor_dirty && has_root,
                            egui::Button::new(format!(
                                "Recheck {} composition",
                                self.profiles[self.active_profile].name
                            )),
                        )
                        .on_disabled_hover_text(
                            "Select a Packagepack profile and save or discard open editor changes first.",
                        )
                        .clicked()
                    {
                        self.section = super::super::Section::Library;
                        self.preview_selected_composition();
                    }
                }
            }
        });
        ui.separator();
        self.draw_content_list(ui);
    }
}
