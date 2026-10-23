use super::super::StudioApp;
use super::super::components::design::{
    LIST_ROW_HEIGHT, StatusTone, contextual_split, empty_state, search_field_width, section_header,
    status_text,
};
use eframe::egui;

enum WorkshopSearchAction {
    ReviewLocal(String),
    Download(String),
    UseAsCompositionRoot(String),
    LaunchAsCompositionRoot(String),
    ReviewComposition,
    ChangeSubscription(String, bool),
    Update(String),
}

impl StudioApp {
    pub(super) fn draw_workshop_search_panel(&mut self, ui: &mut egui::Ui) {
        let steam_available = self.workshop.steam_client_available;
        let search_pending = self.workshop.pending_search.is_some()
            || self
                .operation
                .service_action_id_is_pending_or_active("workshop.search");
        let mut run_search = false;
        ui.horizontal_wrapped(|ui| {
            let search_width = search_field_width(ui);
            let response = ui.add_enabled(
                !search_pending && steam_available,
                egui::TextEdit::singleline(&mut self.workshop.search_query)
                    .hint_text("Search Workshop items")
                    .desired_width(search_width),
            );
            run_search |= (response.has_focus() || response.lost_focus())
                && ui.input(|input| input.key_pressed(egui::Key::Enter));
            run_search |= ui
                .add_enabled(
                    !search_pending && steam_available,
                    egui::Button::new("Search Workshop"),
                )
                .clicked();
            let selected = self
                .workshop
                .kind_filter
                .as_deref()
                .and_then(|kind| kind.parse::<vapor_client_protocol::ContentKind>().ok());
            egui::ComboBox::from_id_salt("workshop-content-kind")
                .selected_text(selected.map_or("All content types", |kind| kind.as_str()))
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut self.workshop.kind_filter, None, "All content types")
                        .clicked()
                    {
                        run_search = true;
                    }
                    for kind in vapor_client_protocol::ContentKind::ALL {
                        if ui
                            .selectable_value(
                                &mut self.workshop.kind_filter,
                                Some(kind.as_str().to_owned()),
                                kind.as_str(),
                            )
                            .clicked()
                        {
                            run_search = true;
                        }
                    }
                });
        });
        ui.weak("Filter by Vapor content type. Subscribe to follow updates, or add an item to your Library when you want to use it. Neither action changes a composition.");
        if run_search && steam_available {
            if search_pending {
                self.workshop.queued_search = true;
            } else {
                self.workshop.queued_search = false;
                self.search_workshop();
            }
        } else if self.workshop.queued_search && !search_pending && steam_available {
            self.workshop.queued_search = false;
            self.search_workshop();
        }
        if let Some((query, page, _content_kind)) = self.workshop.pending_search.as_ref() {
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
            if ![10, 20, 50].contains(&self.workshop.page_size) {
                self.workshop.page_size = 20;
            }
            let result_page_count = self
                .workshop
                .results
                .len()
                .div_ceil(self.workshop.page_size)
                .max(1);
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!(
                    "Steam page {} of {} · results page {} of {} · {} total",
                    self.workshop.page,
                    page_count,
                    self.workshop.results_offset / self.workshop.page_size + 1,
                    result_page_count,
                    self.workshop.total_results,
                ));
                egui::ComboBox::from_label("Rows shown")
                    .selected_text(self.workshop.page_size.to_string())
                    .show_ui(ui, |ui| {
                        for size in [10, 20, 50] {
                            if ui
                                .selectable_value(
                                    &mut self.workshop.page_size,
                                    size,
                                    size.to_string(),
                                )
                                .changed()
                            {
                                self.workshop.results_offset = 0;
                                self.workshop.selected_items.clear();
                            }
                        }
                    });
                let previous = ui
                    .add_enabled(
                        !search_pending && steam_available && self.workshop.page > 1,
                        egui::Button::new("Previous Steam page"),
                    )
                    .clicked();
                let next = ui
                    .add_enabled(
                        !search_pending && steam_available && self.workshop.page < page_count,
                        egui::Button::new("Next Steam page"),
                    )
                    .clicked();
                if previous {
                    self.search_workshop_page(self.workshop.page.saturating_sub(1));
                } else if next {
                    self.search_workshop_page(self.workshop.page.saturating_add(1));
                }
            });
            if result_page_count > 1 {
                ui.horizontal_wrapped(|ui| {
                    let previous = ui
                        .add_enabled(
                            self.workshop.results_offset > 0,
                            egui::Button::new("← Previous results"),
                        )
                        .clicked();
                    let next = ui
                        .add_enabled(
                            self.workshop.results_offset + self.workshop.page_size
                                < self.workshop.results.len(),
                            egui::Button::new("More results →"),
                        )
                        .clicked();
                    if previous {
                        self.workshop.results_offset = self
                            .workshop
                            .results_offset
                            .saturating_sub(self.workshop.page_size);
                    } else if next {
                        self.workshop.results_offset += self.workshop.page_size;
                    }
                });
            }
            let visible_results: Vec<_> = self
                .workshop
                .results
                .iter()
                .skip(self.workshop.results_offset)
                .take(self.workshop.page_size)
                .collect();
            ui.horizontal_wrapped(|ui| {
                let page_is_selected = visible_results.iter().all(|item| {
                    self.workshop
                        .selected_items
                        .contains(&item.published_file_id)
                });
                if ui
                    .add_enabled(
                        !visible_results.is_empty(),
                        egui::Button::new(if page_is_selected {
                            "Clear visible"
                        } else {
                            "Select visible"
                        }),
                    )
                    .clicked()
                {
                    for item in visible_results {
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
                if ui.button("Clear selection").clicked() {
                    self.workshop.selected_items.clear();
                }
                if ui
                    .add_enabled(
                        steam_available,
                        egui::Button::new(format!("Add {count} to Library")),
                    )
                    .clicked()
                {
                    self.download_selected_workshop_items();
                }
                if ui
                    .add_enabled(
                        steam_available && subscription_batch_supported,
                        egui::Button::new(format!("Subscribe {count}")),
                    )
                    .clicked()
                {
                    subscription_action = Some(true);
                }
                if ui
                    .add_enabled(
                        steam_available && subscription_batch_supported,
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
                    "Steam subscription changes handle up to {} items at once. You can still add items to your Library.",
                    vapor_client_application::MAX_WORKSHOP_SUBSCRIPTION_BATCH
                ));
            }
        }
        if let Some(query) = &self.workshop.results_query {
            let type_label = self.workshop.results_content_kind.map_or_else(
                || "all content types".to_owned(),
                |kind| kind.as_str().to_owned(),
            );
            if self.workshop.results.is_empty() {
                ui.weak(if query.is_empty() {
                    format!("No Workshop items found for {type_label}.")
                } else {
                    format!("No {type_label} Workshop items matched ‘{query}’.")
                });
            } else {
                ui.small(format!("Results · {type_label} · {query}"));
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
                    "Find community content in Steam Workshop. Subscribe to follow updates, or add an item to your Library when you want to use it. Neither action changes a composition.",
                    |ui| {
                        if ui.button("Search Workshop").clicked() {
                            run_search = true;
                        }
                    },
                );
                if run_search && !search_pending && steam_available {
                    self.search_workshop();
                }
            }
            return;
        }

        let mut action = None;
        contextual_split(ui, |list, details| {
            section_header(
                list,
                "Workshop results",
                "Choose an item to see its details and actions.",
            );
            list.set_min_width(1.0);
            egui::ScrollArea::vertical()
                .id_salt("workshop-search-results")
                .show(list, |ui| {
                    for item in self
                        .workshop
                        .results
                        .iter()
                        .skip(self.workshop.results_offset)
                        .take(self.workshop.page_size)
                    {
                        ui.horizontal_wrapped(|ui| {
                            let mut selected = self
                                .workshop
                                .selected_items
                                .contains(&item.published_file_id);
                            if ui.checkbox(&mut selected, "").changed() {
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
                            if ui
                                .add_sized(
                                    [ui.available_width(), LIST_ROW_HEIGHT],
                                    egui::Button::selectable(
                                        focused,
                                        egui::RichText::new(&item.title).strong(),
                                    ),
                                )
                                .clicked()
                            {
                                self.workshop.focused_result = Some(item.published_file_id.clone());
                            }
                        });
                        if !item.tags.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for tag in item.tags.iter().take(5) {
                                    ui.small(tag);
                                }
                            });
                        }
                        ui.separator();
                    }
                });
            section_header(
                details,
                "Item details",
                "Subscription and composition actions.",
            );
            let focused_item = self.workshop.results.iter().find(|item| {
                self.workshop.focused_result.as_deref() == Some(item.published_file_id.as_str())
            });
            if let Some(item) = focused_item {
                details.strong(&item.title);
                if !item.tags.is_empty() {
                    details.horizontal_wrapped(|ui| {
                        for tag in &item.tags {
                            ui.small(tag);
                        }
                    });
                }
                details.collapsing("Workshop item ID", |ui| {
                    ui.monospace(&item.published_file_id);
                });
                if !item.description.is_empty() {
                    details.add_space(8.0);
                    details.label(&item.description);
                }
                let subscribed = self
                    .workshop
                    .subscriptions
                    .contains(&item.published_file_id);
                details.weak(if self.workshop.subscriptions_stale {
                    if subscribed {
                        "Last known as subscribed through Steam · refresh status"
                    } else {
                        "Last known as not subscribed through Steam · refresh status"
                    }
                } else if !self.workshop.subscriptions_loaded {
                    "Subscription status has not been checked yet."
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
                    if content.published_file_id.as_deref() != Some(item.published_file_id.as_str())
                    {
                        continue;
                    }
                    locally_available = true;
                    local_conflict |= !content.is_unambiguous();
                    available_packagepack |= content.kind
                        == vapor_client_protocol::ContentKind::Packagepack.as_str()
                        && content.is_unambiguous();
                    selected_as_composition_root |=
                        self.is_active_composition_root(&content.id, content.version.as_deref());
                    included_in_active_composition |= active_graph.is_some_and(|graph| {
                        graph.nodes.iter().any(|node| {
                            node.content.id == content.id && node.content.version == content.version
                        })
                    });
                }
                let create_draft_blocks_assignment =
                    self.workshop_root_change_blocked_by_create_draft(&item.published_file_id);
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
                    details.add_enabled_ui(steam_available, |ui| {
                        clicked =
                            super::super::components::design::primary_button(ui, "Add to Library")
                                .clicked();
                    });
                    if clicked {
                        action = Some(WorkshopSearchAction::Download(
                            item.published_file_id.clone(),
                        ));
                    }
                } else if selected_as_composition_root {
                    details.strong("Used by this composition");
                    let mut clicked = false;
                    details.add_enabled_ui(!self.has_unsaved_workspace_edits(), |ui| {
                        clicked = super::super::components::design::primary_button(
                            ui,
                            "Review composition",
                        )
                        .clicked();
                    });
                    if clicked {
                        action = Some(WorkshopSearchAction::ReviewComposition);
                    }
                } else if available_packagepack {
                    details.label("Available in Library");
                    let mut clicked = false;
                    details.add_enabled_ui(
                        local_catalog_current
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
                        local_catalog_current
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
                    details.add_enabled_ui(!self.has_unsaved_workspace_edits(), |ui| {
                        clicked = super::super::components::design::primary_button(
                            ui,
                            "Review composition",
                        )
                        .clicked();
                    });
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
                            steam_available,
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
                                steam_available,
                                egui::Button::new("Update from Workshop"),
                            )
                            .clicked()
                    {
                        action = Some(WorkshopSearchAction::Update(
                            item.published_file_id.clone(),
                        ));
                    }
                    if let Some(url) = &item.preview_url {
                        ui.hyperlink_to("Open Workshop page", url);
                    }
                });
            } else {
                details.weak("Select an item to review its description and available actions.");
            }
        });
        match action {
            Some(WorkshopSearchAction::ReviewLocal(id)) => self.review_workshop_content(&id),
            Some(WorkshopSearchAction::Download(id) | WorkshopSearchAction::Update(id)) => {
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
