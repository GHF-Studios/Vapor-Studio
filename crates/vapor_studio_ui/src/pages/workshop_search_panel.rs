use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_search_panel(&mut self, ui: &mut egui::Ui) {
        let mut run_search = false;
        ui.horizontal_wrapped(|ui| {
            let response = ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workshop.search_query)
                    .hint_text("Search Workshop items")
                    .desired_width(300.0),
            );
            run_search |= (response.has_focus() || response.lost_focus())
                && ui.input(|input| input.key_pressed(egui::Key::Enter));
            run_search |= ui
                .add_enabled(
                    !self.operation.running,
                    egui::Button::new("Search Workshop"),
                )
                .clicked();
        });
        if run_search && !self.operation.running {
            self.workshop.page = 1;
            self.search_workshop();
        }
        if !self.workshop.results.is_empty() {
            let page_count = self.workshop.total_results.div_ceil(50).max(1);
            ui.horizontal_wrapped(|ui| {
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
            ui.horizontal_wrapped(|ui| {
                let page_is_selected = self.workshop.results.iter().all(|item| {
                    self.workshop
                        .selected_items
                        .contains(&item.published_file_id)
                });
                if ui
                    .add_enabled(
                        !self.operation.running,
                        egui::Button::new(if page_is_selected {
                            "Clear this page"
                        } else {
                            "Select this page"
                        }),
                    )
                    .clicked()
                {
                    for item in &self.workshop.results {
                        if page_is_selected {
                            self.workshop
                                .selected_items
                                .remove(&item.published_file_id);
                        } else {
                            self.workshop
                                .selected_items
                                .insert(item.published_file_id.clone());
                        }
                    }
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
            ui.horizontal_wrapped(|ui| {
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

        let narrow_layout = ui.available_width() < 820.0;
        ui.columns(if narrow_layout { 1 } else { 2 }, |columns| {
            {
                let list = &mut columns[0];
                list.set_min_width(if narrow_layout { 1.0 } else { 320.0 });
                list.heading("Items");
                egui::ScrollArea::vertical()
                    .id_salt("workshop-search-results")
                    .show(list, |ui| {
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
                        ui.separator();
                    }
                });
            }

            let details = if narrow_layout {
                columns[0].separator();
                &mut columns[0]
            } else {
                &mut columns[1]
            };
            details.heading("Item details");
            let focused_item = self.workshop.results.iter().find(|item|
                self.workshop.focused_result.as_deref() == Some(item.published_file_id.as_str())
            ).cloned();
            if let Some(item) = focused_item {
                details.strong(&item.title);
                details.collapsing("Workshop item ID", |ui| {
                    ui.small(format!("PublishedFileId {}", item.published_file_id));
                });
                if !item.description.is_empty() {
                    details.add_space(8.0);
                    details.label(&item.description);
                }
                let subscribed = self.workshop.subscriptions.contains(&item.published_file_id);
                details.weak(if !self.workshop.subscriptions_loaded {
                    "Steam subscription status has not been loaded."
                } else if subscribed {
                    "Subscribed through Steam"
                } else {
                    "Not subscribed through Steam"
                });
                details.separator();
                let local_identities = self.catalog.items.iter()
                    .filter(|content| content.published_file_id.as_deref() == Some(item.published_file_id.as_str()))
                    .map(|content| content.identity())
                    .collect::<Vec<_>>();
                let locally_available = !local_identities.is_empty();
                let local_conflict = self.catalog.items.iter().any(|content| {
                    content.published_file_id.as_deref() == Some(item.published_file_id.as_str())
                        && !content.is_unambiguous()
                });
                let available_packagepack = self.catalog.items.iter().any(|content| {
                    content.published_file_id.as_deref() == Some(item.published_file_id.as_str())
                        && content.kind == vapor_core::ContentKind::Packagepack.as_str()
                        && content.is_unambiguous()
                });
                let create_draft_blocks_assignment = self
                    .workshop_root_change_blocked_by_create_draft(&item.published_file_id);
                let included_in_active_profile = local_identities.iter().any(|identity| {
                    self.profile.items[self.profile.active_index]
                        .resolved_graph
                        .as_ref()
                        .is_some_and(|graph| graph.nodes.iter().any(|node| &node.content == identity))
                });
                if local_conflict {
                    details.colored_label(
                        egui::Color32::YELLOW,
                        "Local records disagree about this Workshop item. Review them in Discover before assigning it to a profile.",
                    );
                    if details.button("Review local records").clicked() {
                        self.review_workshop_content(&item.published_file_id);
                    }
                } else if !locally_available {
                    let mut clicked = false;
                    details.add_enabled_ui(!self.operation.running, |ui| {
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
                    details.label("Available in local content");
                    let mut clicked = false;
                    details.add_enabled_ui(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && !create_draft_blocks_assignment,
                        |ui| {
                            clicked = super::super::components::design::primary_button(
                                ui,
                                &format!("Use in {}", self.profile.items[self.profile.active_index].name),
                            )
                            .clicked();
                        },
                    )
                    .response
                    .on_disabled_hover_text(if create_draft_blocks_assignment {
                        "Discard the Create draft before changing the profile to this item's workspace."
                    } else if self.has_unsaved_workspace_edits() {
                        "Save or discard workspace edits before changing the active profile."
                    } else {
                        "Finish the current operation before changing the active profile."
                    });
                    if clicked {
                        self.use_available_workshop_package_as_profile_root(&item.published_file_id);
                    }
                } else if included_in_active_profile {
                    details.label("Available in local content");
                    let mut clicked = false;
                    details.add_enabled_ui(
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
                    details.label("Available in local content");
                    details.weak("This item is not included in the active play profile.");
                }
                details.horizontal_wrapped(|ui| {
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
                details.weak("Select a Workshop item to review its description, subscription, and import options.");
            }
        });
    }
}
