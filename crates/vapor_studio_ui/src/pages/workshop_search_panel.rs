use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_search_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_enabled(!self.operation.running, egui::TextEdit::singleline(&mut self.workshop.search_query)
                .hint_text("Search Workshop items")
                .desired_width(300.0));
            if ui.add_enabled(!self.operation.running, egui::Button::new("Search Workshop")).clicked() {
                self.workshop.page = 1;
                self.search_workshop();
            }
        });
        if !self.workshop.results.is_empty() {
            let page_count = self.workshop.total_results.div_ceil(50).max(1);
            ui.horizontal(|ui| {
                ui.weak(format!("Showing {} of {} results · page {} of {}", self.workshop.results.len(), self.workshop.total_results, self.workshop.page, page_count));
                let previous = ui.add_enabled(!self.operation.running && self.workshop.page > 1, egui::Button::new("Previous")).clicked();
                let next = ui.add_enabled(!self.operation.running && self.workshop.page < page_count, egui::Button::new("Next")).clicked();
                if previous {
                    self.workshop.page -= 1;
                    self.search_workshop();
                } else if next {
                    self.workshop.page += 1;
                    self.search_workshop();
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.operation.running,
                        egui::Button::new("Select this page"),
                    )
                    .clicked()
                {
                    self.workshop.selected_items.extend(
                        self.workshop.results.iter().map(|item| item.published_file_id.clone()),
                    );
                }
            });
        }
        if !self.workshop.selected_items.is_empty() {
            let count = self.workshop.selected_items.len();
            let subscription_batch_supported =
                count <= vapor_studio_core::MAX_WORKSHOP_SUBSCRIPTION_BATCH;
            let selected_ids = self.workshop.selected_items.iter().cloned().collect::<Vec<_>>();
            ui.horizontal(|ui| {
                ui.small(format!("Selected across pages and searches · {count} item(s)"));
                if ui
                    .add_enabled(
                        !self.operation.running,
                        egui::Button::new("Clear selection"),
                    )
                    .clicked()
                {
                    self.workshop.selected_items.clear();
                }
                if ui
                    .add_enabled(
                        !self.operation.running,
                        egui::Button::new(format!("Import {count} to Vapor Library")),
                    )
                    .clicked()
                {
                    self.download_selected_workshop_items();
                }
                if ui
                    .add_enabled(
                        !self.operation.running && subscription_batch_supported,
                        egui::Button::new("Subscribe selected"),
                    )
                    .clicked()
                {
                    self.set_workshop_subscriptions(selected_ids.clone(), true);
                }
                if ui
                    .add_enabled(
                        !self.operation.running && subscription_batch_supported,
                        egui::Button::new("Unsubscribe selected"),
                    )
                    .clicked()
                {
                    self.set_workshop_subscriptions(selected_ids, false);
                }
            });
            if !subscription_batch_supported {
                ui.small(format!(
                    "Steam subscription changes handle up to {} items at once. Import selection remains available.",
                    vapor_studio_core::MAX_WORKSHOP_SUBSCRIPTION_BATCH
                ));
            }
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
                let mut selected = self.workshop.selected_items.contains(&item.published_file_id);
                if ui
                    .add_enabled(
                        !self.operation.running,
                        egui::Checkbox::new(&mut selected, ""),
                    )
                    .changed()
                {
                    if selected {
                        self.workshop.selected_items.insert(item.published_file_id.clone());
                    } else {
                        self.workshop.selected_items.remove(&item.published_file_id);
                    }
                }
                ui.strong(&item.title);
                ui.weak(format!("PublishedFileId {}", item.published_file_id));
            });
            if !item.description.is_empty() {
                ui.collapsing("Description", |ui| { ui.label(&item.description); });
            }
            ui.horizontal(|ui| {
                let subscribed = self.workshop.subscriptions.contains(&item.published_file_id);
                let label = if subscribed { "Unsubscribe" } else { "Subscribe" };
                if ui.add_enabled(!self.operation.running, egui::Button::new(label)).clicked() {
                    self.set_workshop_subscription(item.published_file_id.clone(), !subscribed);
                }
                let locally_available = self.catalog.items.iter().any(|content| {
                    content.published_file_id.as_deref()
                        == Some(item.published_file_id.as_str())
                });
                let available_packagepack = self.catalog.items.iter().any(|content| {
                    content.published_file_id.as_deref()
                        == Some(item.published_file_id.as_str())
                        && content.kind == vapor_core::ContentKind::Packagepack.as_str()
                        && content.is_unambiguous()
                });
                if locally_available {
                    ui.weak("In Vapor Library");
                    if available_packagepack
                        && ui.add_enabled(
                            !self.operation.running && !self.has_unsaved_workspace_edits(),
                            egui::Button::new(format!(
                                "Set {} root",
                                self.profile.items[self.profile.active_index].name
                            )),
                        ).clicked()
                    {
                        self.use_available_workshop_package_as_profile_root(
                            &item.published_file_id,
                        );
                    } else if !available_packagepack
                        && self.profile.items[self.profile.active_index].root_package.is_some()
                        && ui.add_enabled(
                            !self.operation.running && !self.has_unsaved_workspace_edits(),
                            egui::Button::new(format!(
                                "Recheck {} plan",
                                self.profile.items[self.profile.active_index].name
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
                if ui.add_enabled(!self.operation.running, egui::Button::new(import_label)).clicked() {
                    self.workshop.published_file_id = item.published_file_id.clone();
                    self.download_workshop_item();
                }
                if let Some(url) = &item.preview_url {
                    ui.hyperlink_to("Open item", url);
                }
            });
        }
    }
}
