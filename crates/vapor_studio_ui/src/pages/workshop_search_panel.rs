use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_search_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workshop.search_query)
                    .hint_text("Search Workshop items")
                    .desired_width(300.0),
            );
            if ui
                .add_enabled(
                    !self.operation.running,
                    egui::Button::new("Search Workshop"),
                )
                .clicked()
            {
                self.workshop.page = 1;
                self.search_workshop();
            }
        });
        if !self.workshop.results.is_empty() {
            let page_count = self.workshop.total_results.div_ceil(50).max(1);
            ui.horizontal(|ui| {
                ui.weak(format!(
                    "Showing {} of {} results · page {} of {}",
                    self.workshop.results.len(),
                    self.workshop.total_results,
                    self.workshop.page,
                    page_count
                ));
                let previous = ui
                    .add_enabled(
                        !self.operation.running && self.workshop.page > 1,
                        egui::Button::new("Previous"),
                    )
                    .clicked();
                let next = ui
                    .add_enabled(
                        !self.operation.running && self.workshop.page < page_count,
                        egui::Button::new("Next"),
                    )
                    .clicked();
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
                        self.workshop
                            .results
                            .iter()
                            .map(|item| item.published_file_id.clone()),
                    );
                }
            });
        }
        if !self.workshop.selected_items.is_empty() {
            let count = self.workshop.selected_items.len();
            let subscription_batch_supported =
                count <= vapor_studio_core::MAX_WORKSHOP_SUBSCRIPTION_BATCH;
            let selected_ids = self
                .workshop
                .selected_items
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            ui.horizontal(|ui| {
                ui.small(format!(
                    "Selected across pages and searches · {count} item(s)"
                ));
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
                        egui::Button::new(format!("Import {count} to Vapor cache")),
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
        if self.workshop.results.is_empty() {
            return;
        }

        ui.columns(2, |columns| {
            columns[0].set_min_width(320.0);
            columns[0].heading("Items");
            egui::ScrollArea::vertical()
                .id_salt("workshop-search-results")
                .show(&mut columns[0], |ui| {
                    for item in &self.workshop.results {
                        ui.horizontal(|ui| {
                            let mut selected = self
                                .workshop
                                .selected_items
                                .contains(&item.published_file_id);
                            if ui
                                .add_enabled(
                                    !self.operation.running,
                                    egui::Checkbox::new(&mut selected, ""),
                                )
                                .changed()
                            {
                                if selected {
                                    self.workshop
                                        .selected_items
                                        .insert(item.published_file_id.clone());
                                } else {
                                    self.workshop.selected_items.remove(&item.published_file_id);
                                }
                            }
                            let focused = self.workshop.focused_result.as_deref()
                                == Some(item.published_file_id.as_str());
                            if ui.add_sized(
                                [ui.available_width(), 42.0],
                                egui::Button::selectable(focused, egui::RichText::new(&item.title).strong()),
                            ).clicked() {
                                self.workshop.focused_result = Some(item.published_file_id.clone());
                            }
                        });
                        ui.small(format!("Workshop item · {}", item.published_file_id));
                        ui.separator();
                    }
                });

            columns[1].heading("Item details");
            let focused_item = self.workshop.results.iter().find(|item|
                self.workshop.focused_result.as_deref() == Some(item.published_file_id.as_str())
            ).cloned();
            if let Some(item) = focused_item {
                columns[1].strong(&item.title);
                columns[1].small(format!("PublishedFileId {}", item.published_file_id));
                if !item.description.is_empty() {
                    columns[1].add_space(8.0);
                    columns[1].label(&item.description);
                }
                let subscribed = self.workshop.subscriptions.contains(&item.published_file_id);
                columns[1].weak(if !self.workshop.subscriptions_loaded {
                    "Steam subscription status has not been loaded."
                } else if subscribed {
                    "Subscribed through Steam"
                } else {
                    "Not subscribed through Steam"
                });
                columns[1].separator();
                let locally_available = self.catalog.items.iter().any(|content| {
                    content.published_file_id.as_deref() == Some(item.published_file_id.as_str())
                });
                let available_packagepack = self.catalog.items.iter().any(|content| {
                    content.published_file_id.as_deref() == Some(item.published_file_id.as_str())
                        && content.kind == vapor_core::ContentKind::Packagepack.as_str()
                        && content.is_unambiguous()
                });
                let can_review_profile = self.profile.items[self.profile.active_index]
                    .root_package
                    .is_some();
                if !locally_available {
                    let mut clicked = false;
                    columns[1].add_enabled_ui(!self.operation.running, |ui| {
                        clicked = super::super::components::design::primary_button(
                            ui,
                            "Import to Vapor cache",
                        )
                        .clicked();
                    });
                    if clicked {
                        self.workshop.published_file_id = item.published_file_id.clone();
                        self.download_workshop_item();
                    }
                } else if available_packagepack {
                    columns[1].label("Available in local content");
                    let mut clicked = false;
                    columns[1].add_enabled_ui(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        |ui| {
                            clicked = super::super::components::design::primary_button(
                                ui,
                                &format!("Use in {}", self.profile.items[self.profile.active_index].name),
                            )
                            .clicked();
                        },
                    );
                    if clicked {
                        self.use_available_workshop_package_as_profile_root(&item.published_file_id);
                    }
                } else if can_review_profile {
                    columns[1].label("Available in local content");
                    let mut clicked = false;
                    columns[1].add_enabled_ui(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        |ui| {
                            clicked = super::super::components::design::primary_button(
                                ui,
                                "Review active profile plan",
                            )
                            .clicked();
                        },
                    );
                    if clicked {
                        self.section = super::super::Section::Library;
                        self.preview_selected_composition();
                    }
                } else {
                    columns[1].label("Available in local content");
                }
                columns[1].horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !self.operation.running,
                            egui::Button::new(if subscribed { "Unsubscribe" } else { "Subscribe" }),
                        )
                        .on_hover_text(if self.workshop.subscriptions_loaded {
                            "Change this item's Steam Workshop subscription."
                        } else {
                            "Subscription status has not been checked. Load Subscriptions first if you want to confirm its current Steam state."
                        })
                        .clicked()
                    {
                        self.set_workshop_subscription(item.published_file_id.clone(), !subscribed);
                    }
                    if locally_available {
                        if ui
                            .add_enabled(
                                !self.operation.running,
                                egui::Button::new("Refresh cached copy"),
                            )
                            .clicked()
                        {
                            self.workshop.published_file_id = item.published_file_id.clone();
                            self.download_workshop_item();
                        }
                    }
                    if let Some(url) = &item.preview_url {
                        ui.hyperlink_to("Open Workshop page", url);
                    }
                });
            } else {
                columns[1].weak("Select a Workshop item to review its description, subscription, and import options.");
            }
        });
    }
}
