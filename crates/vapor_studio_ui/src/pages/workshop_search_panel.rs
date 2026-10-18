use super::super::StudioApp;
use super::super::components::design::{
    LIST_ROW_HEIGHT, StatusTone, empty_state, search_field_width, status_text, use_stacked_panels,
};
use eframe::egui;

enum WorkshopSearchAction {
    ReviewLocal(String),
    Import(String),
    UseAsCompositionRoot(String),
    LaunchAsCompositionRoot(String),
    ReviewComposition,
    ChangeSubscription(String, bool),
    RefreshCache(String),
}

impl StudioApp {
    pub(super) fn draw_workshop_search_panel(&mut self, ui: &mut egui::Ui) {
        let mut run_search = false;
        ui.horizontal_wrapped(|ui| {
            let search_width = search_field_width(ui);
            let response = ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workshop.search_query)
                    .hint_text("Search Workshop items")
                    .desired_width(search_width),
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
        ui.weak("Subscribe to add content to your Library and receive Steam updates. Add-ons can be added to a compatible composition later.");
        if run_search && !self.operation.running {
            self.search_workshop();
        }
        if let Some((query, page)) = self.workshop.pending_search.as_ref() {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.label(if query.is_empty() {
                    format!("Searching all Workshop items · page {page}…")
                } else {
                    format!("Searching for ‘{query}’ · page {page}…")
                });
                if let Some(previous_query) = self.workshop.results_query.as_deref() {
                    ui.weak(format!(
                        "Previous results for ‘{previous_query}’ stay available until this search succeeds."
                    ));
                }
            });
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
                    self.search_workshop_page(self.workshop.page.saturating_sub(1));
                } else if next {
                    self.search_workshop_page(self.workshop.page.saturating_add(1));
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
                            self.workshop.selected_items.remove(&item.published_file_id);
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
                count <= vapor_client_application::MAX_WORKSHOP_SUBSCRIPTION_BATCH;
            let mut subscription_action = None;
            ui.horizontal_wrapped(|ui| {
                ui.small(format!(
                    "Selected across pages and searches · {count} item{}",
                    if count == 1 { "" } else { "s" }
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
                        egui::Button::new(format!("Add {count} to Library")),
                    )
                    .clicked()
                {
                    self.download_selected_workshop_items();
                }
                if ui
                    .add_enabled(
                        !self.operation.running && subscription_batch_supported,
                        egui::Button::new("Subscribe and add to Library"),
                    )
                    .clicked()
                {
                    subscription_action = Some(true);
                }
                if ui
                    .add_enabled(
                        !self.operation.running && subscription_batch_supported,
                        egui::Button::new("Unsubscribe selected"),
                    )
                    .clicked()
                {
                    subscription_action = Some(false);
                }
            });
            if let Some(subscribe) = subscription_action {
                let selected_ids = self.workshop.selected_items.iter().cloned().collect();
                self.set_workshop_subscriptions(selected_ids, subscribe);
            }
            if !subscription_batch_supported {
                ui.small(format!(
                    "Steam subscription changes handle up to {} items at once. Import selection remains available.",
                    vapor_client_application::MAX_WORKSHOP_SUBSCRIPTION_BATCH
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
            if self.workshop.pending_search.is_some() {
                ui.add_space(8.0);
                ui.weak(
                    "No previous results to show. The first page will appear here when this search completes.",
                );
            } else if self.workshop.results_query.is_none() {
                empty_state(
                    ui,
                    "Find content in Steam Workshop",
                    "Search Steam Workshop for community content. Subscribe to add an item to your Library and receive updates. This does not change a composition.",
                    |ui| {
                        if ui.button("Search Workshop").clicked() {
                            run_search = true;
                        }
                    },
                );
                if run_search && !self.operation.running {
                    self.search_workshop();
                }
            }
            return;
        }

        let narrow_layout = use_stacked_panels(ui);
        let mut action = None;
        ui.columns(if narrow_layout { 1 } else { 2 }, |columns| {
            {
                let list = &mut columns[0];
                list.set_min_width(if narrow_layout { 1.0 } else { 320.0 });
                list.heading("Items");
                egui::ScrollArea::vertical()
                    .id_salt("workshop-search-results")
                    .show(list, |ui| {
                    for item in &self.workshop.results {
                        ui.horizontal_wrapped(|ui| {
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
                                [ui.available_width(), LIST_ROW_HEIGHT],
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
            );
            if let Some(item) = focused_item {
                details.strong(&item.title);
                details.collapsing("Workshop item ID", |ui| {
                    ui.monospace(&item.published_file_id);
                });
                if !item.description.is_empty() {
                    details.add_space(8.0);
                    details.label(&item.description);
                }
                let subscribed = self.workshop.subscriptions.contains(&item.published_file_id);
                details.weak(if self.workshop.subscriptions_stale {
                    if subscribed {
                        "Last known as subscribed through Steam · refresh status"
                    } else {
                        "Last known as not subscribed through Steam · refresh status"
                    }
                } else if !self.workshop.subscriptions_loaded {
                    "Steam subscription status has not been loaded."
                } else if subscribed {
                    "Subscribed through Steam"
                } else {
                    "Not subscribed through Steam"
                });
                details.separator();
                let mut locally_available = false;
                let mut local_conflict = false;
                let mut available_packagepack = false;
                let mut included_in_active_composition = false;
                let mut selected_as_composition_root = false;
                let active_composition = &self.composition.items[self.composition.active_index];
                let active_graph = active_composition.resolved_graph.as_ref();
                for content in &self.catalog.items {
                    if content.published_file_id.as_deref()
                        != Some(item.published_file_id.as_str())
                    {
                        continue;
                    }
                    locally_available = true;
                    local_conflict |= !content.is_unambiguous();
                    available_packagepack |= content.kind
                        == vapor_core::ContentKind::Packagepack.as_str()
                        && content.is_unambiguous();
                    selected_as_composition_root |= self.is_active_composition_root(
                        &content.id,
                        content.version.as_deref(),
                    );
                    included_in_active_composition |= active_graph.is_some_and(|graph| {
                        graph.nodes.iter().any(|node| {
                            node.content.id == content.id
                                && node.content.version == content.version
                        })
                    });
                }
                let create_draft_blocks_assignment = self
                    .workshop_root_change_blocked_by_create_draft(&item.published_file_id);
                let local_catalog_current = self.catalog.local_catalog_current();
                if local_conflict {
                    status_text(
                        details,
                        "This item has conflicting copies on this device. Review them in App Status before using it.",
                        StatusTone::Caution,
                    );
                    if details.button("Open App Status").clicked() {
                        action = Some(WorkshopSearchAction::ReviewLocal(
                            item.published_file_id.clone(),
                        ));
                    }
                } else if !locally_available {
                    let mut clicked = false;
                    details.add_enabled_ui(!self.operation.running, |ui| {
                        clicked = super::super::components::design::primary_button(
                            ui,
                            "Add to Library",
                        )
                        .clicked();
                    });
                    if clicked {
                        action = Some(WorkshopSearchAction::Import(
                            item.published_file_id.clone(),
                        ));
                    }
                } else if selected_as_composition_root {
                    details.strong("Used by this composition");
                    let mut clicked = false;
                    details.add_enabled_ui(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        |ui| {
                            clicked = super::super::components::design::primary_button(
                                ui,
                                "Review composition",
                            )
                            .clicked();
                        },
                    );
                    if clicked {
                        action = Some(WorkshopSearchAction::ReviewComposition);
                    }
                } else if available_packagepack {
                    details.label("Available in Library");
                    let mut clicked = false;
                    details.add_enabled_ui(
                        !self.operation.running
                            && local_catalog_current
                            && !self.has_unsaved_workspace_edits()
                            && !create_draft_blocks_assignment,
                        |ui| {
                            clicked = super::super::components::design::primary_button(
                                ui,
                                &format!("Use in {}", self.composition.items[self.composition.active_index].name),
                            )
                            .clicked();
                        },
                    )
                    .response
                    .on_disabled_hover_text(if !local_catalog_current {
                        "Refresh the content list before choosing this for your composition."
                    } else if create_draft_blocks_assignment {
                        "Discard the Create draft before changing the composition to this item's workspace."
                    } else if self.has_unsaved_workspace_edits() {
                        "Save or discard workspace edits before changing the active composition."
                    } else {
                        "Finish the current operation before changing the active composition."
                    });
                    if clicked {
                        action = Some(WorkshopSearchAction::UseAsCompositionRoot(
                            item.published_file_id.clone(),
                        ));
                    }
                    let mut launch_clicked = false;
                    details.add_enabled_ui(
                        !self.operation.running
                            && local_catalog_current
                            && !local_conflict
                            && !self.has_unsaved_workspace_edits()
                            && !create_draft_blocks_assignment,
                        |ui| {
                            launch_clicked = super::super::components::design::primary_button(
                                ui,
                                "Launch this composition",
                            )
                            .clicked();
                        },
                    )
                    .response
                    .on_disabled_hover_text(if !local_catalog_current {
                        "Refresh the on-device content list before launching this composition."
                    } else if local_conflict {
                        "Resolve the conflicting content copies before launching this composition."
                    } else if create_draft_blocks_assignment {
                        "Discard the Create draft before switching this composition."
                    } else if self.has_unsaved_workspace_edits() {
                        "Save or discard project edits before switching this composition."
                    } else if self.operation.running {
                        "Wait for the current operation to finish."
                    } else {
                        "Launch this composition."
                    });
                    if launch_clicked {
                        action = Some(WorkshopSearchAction::LaunchAsCompositionRoot(
                            item.published_file_id.clone(),
                        ));
                    }
                } else if included_in_active_composition {
                    details.label("Available in Library");
                    let mut clicked = false;
                    details.add_enabled_ui(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        |ui| {
                            clicked = super::super::components::design::primary_button(
                                ui,
                                "Review composition",
                            )
                            .clicked();
                        },
                    );
                    if clicked {
                        action = Some(WorkshopSearchAction::ReviewComposition);
                    }
                } else {
                    details.label("Available in Library");
                    details.weak("This item is not included in the active composition.");
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
                        action = Some(WorkshopSearchAction::ChangeSubscription(
                            item.published_file_id.clone(),
                            !subscribed,
                        ));
                    }
                    if locally_available
                        && ui
                            .add_enabled(
                                !self.operation.running,
                                egui::Button::new("Update from Workshop"),
                            )
                            .clicked()
                    {
                        action = Some(WorkshopSearchAction::RefreshCache(
                            item.published_file_id.clone(),
                        ));
                    }
                    if let Some(url) = &item.preview_url {
                        ui.hyperlink_to("Open Workshop page", url);
                    }
                });
            } else {
                details.weak("Select a Workshop item to review its description, subscription, and import options.");
            }
        });
        match action {
            Some(WorkshopSearchAction::ReviewLocal(id)) => self.review_workshop_content(&id),
            Some(WorkshopSearchAction::Import(id) | WorkshopSearchAction::RefreshCache(id)) => {
                self.workshop.published_file_id = id;
                self.download_workshop_item();
            }
            Some(WorkshopSearchAction::UseAsCompositionRoot(id)) => {
                self.use_available_workshop_package_as_composition_root(&id);
            }
            Some(WorkshopSearchAction::LaunchAsCompositionRoot(id)) => {
                self.launch_workshop_package_as_composition_root(&id);
            }
            Some(WorkshopSearchAction::ReviewComposition) => {
                self.activate_navigation_section(super::super::Section::Compositions);
                self.preview_selected_composition();
            }
            Some(WorkshopSearchAction::ChangeSubscription(id, subscribe)) => {
                self.set_workshop_subscription(id, subscribe);
            }
            None => {}
        }
    }
}
