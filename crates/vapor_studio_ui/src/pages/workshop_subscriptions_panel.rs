use super::super::StudioApp;
use super::super::components::design::{StatusTone, empty_state, section_header, status_text};
use super::super::workshop::WorkshopTab;
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Default)]
struct LocalWorkshopContent {
    available: bool,
    conflicted: bool,
    included_in_active_profile: bool,
    selected_as_profile_root: bool,
    packagepack_available: bool,
}

enum SubscriptionAction {
    SetSelected(String, bool),
    SetManySelected(Vec<String>, bool),
    ImportMany(Vec<String>),
    UnsubscribeMany(Vec<String>),
    ReviewLocal(String),
    UseAsProfileRoot(String),
    LaunchAsProfileRoot(String),
    ReviewProfile,
    Import(String),
    Unsubscribe(String),
}

impl StudioApp {
    pub(super) fn draw_workshop_subscriptions_panel(&mut self, ui: &mut egui::Ui) {
        section_header(
            ui,
            "Your Steam subscriptions",
            "Steam keeps track of subscriptions. Download a copy here to make an item available to this installation.",
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.operation.running,
                    egui::Button::new(if self.workshop.subscriptions_loaded {
                        "Refresh subscriptions"
                    } else {
                        "Load subscriptions"
                    }),
                )
                .clicked()
            {
                self.refresh_workshop_subscriptions();
            }
        });
        if self.workshop.subscriptions_stale {
            status_text(
                ui,
                "Showing the last known subscription state. Refresh failed, so Steam changes may not be reflected.",
                StatusTone::Caution,
            );
        }
        if !self.workshop.subscriptions_loaded {
            empty_state(
                ui,
                "Subscriptions have not been loaded",
                "Connect to Steam to review subscribed items, Steam download state, and which items are already available in Vapor.",
                |_| {},
            );
        } else if self.workshop.subscriptions.is_empty() {
            let mut browse_workshop = false;
            empty_state(
                ui,
                "No Vapor Workshop subscriptions",
                "Items you subscribe to in Steam Workshop will appear here.",
                |ui| {
                    browse_workshop = ui.button("Browse Steam Workshop").clicked();
                },
            );
            if browse_workshop {
                self.workshop.tab = WorkshopTab::Browse;
            }
        }
        if !self.workshop.subscriptions.is_empty() {
            // Build the local/profile view once. The old row renderer scanned the
            // whole catalog (and graph) several times for every subscription.
            let active_profile = &self.profile.items[self.profile.active_index];
            let active_content = active_profile
                .resolved_graph
                .as_ref()
                .map(|graph| {
                    graph
                        .nodes
                        .iter()
                        .map(|node| (node.content.id.as_str(), node.content.version.as_deref()))
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            let mut local_by_workshop_id = BTreeMap::<&str, LocalWorkshopContent>::new();
            let results_by_id = self
                .workshop
                .results
                .iter()
                .map(|item| (item.published_file_id.as_str(), item.title.as_str()))
                .collect::<BTreeMap<_, _>>();
            for content in &self.catalog.items {
                let Some(id) = content.published_file_id.as_deref() else {
                    continue;
                };
                let status = local_by_workshop_id.entry(id).or_default();
                status.available = true;
                status.conflicted |= !content.is_unambiguous();
                status.included_in_active_profile |=
                    active_content.contains(&(content.id.as_str(), content.version.as_deref()));
                status.selected_as_profile_root |=
                    self.is_active_profile_root(&content.id, content.version.as_deref());
                status.packagepack_available |= content.kind
                    == vapor_core::ContentKind::Packagepack.as_str()
                    && content.is_unambiguous();
            }
            ui.horizontal_wrapped(|ui| {
                let search_width = super::super::components::design::search_field_width(ui);
                ui.add(
                    egui::TextEdit::singleline(&mut self.workshop.subscription_search_query)
                        .desired_width(search_width)
                        .hint_text("Find a subscription by title or item ID"),
                );
                if !self.workshop.subscription_search_query.is_empty()
                    && ui.small_button("Clear").clicked()
                {
                    self.workshop.subscription_search_query.clear();
                }
            });
            let query = self
                .workshop
                .subscription_search_query
                .trim()
                .to_lowercase();
            let visible_ids = self
                .workshop
                .subscriptions
                .iter()
                .filter(|id| {
                    subscription_matches_query(
                        id,
                        &query,
                        &self.workshop.subscription_details,
                        &results_by_id,
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            let matching_subscriptions = visible_ids.len();
            let visible_selected_ids = visible_ids
                .iter()
                .filter(|id| self.workshop.selected_items.contains(*id))
                .cloned()
                .collect::<Vec<_>>();
            let selected_visible = visible_selected_ids.len();
            let subscription_batch_supported =
                selected_visible <= vapor_studio_core::MAX_WORKSHOP_SUBSCRIPTION_BATCH;
            let all_visible_selected =
                matching_subscriptions > 0 && selected_visible == matching_subscriptions;
            let mut action = None;
            ui.horizontal_wrapped(|ui| {
                ui.small(format!(
                    "{matching_subscriptions} of {} subscriptions · {selected_visible} selected in this view",
                    self.workshop.subscriptions.len()
                ));
                if ui
                    .add_enabled(
                        !self.operation.running && matching_subscriptions > 0,
                        egui::Button::new(if all_visible_selected {
                            "Clear visible"
                        } else {
                            "Select visible"
                        }),
                    )
                    .clicked()
                {
                    action = Some(SubscriptionAction::SetManySelected(
                        visible_ids.clone(),
                        !all_visible_selected,
                    ));
                }
                if selected_visible > 0
                    && ui
                        .add_enabled(
                            !self.operation.running,
                            egui::Button::new(format!("Download selected · {selected_visible}")),
                        )
                        .on_hover_text("Download only the selected items currently visible in this view to this installation.")
                        .clicked()
                {
                    action = Some(SubscriptionAction::ImportMany(
                        visible_selected_ids.clone(),
                    ));
                }
                if selected_visible > 0
                    && ui
                        .add_enabled(
                            !self.operation.running && subscription_batch_supported,
                            egui::Button::new(format!("Unsubscribe visible · {selected_visible}")),
                        )
                        .on_disabled_hover_text(format!(
                            "Select at most {} visible items for one Steam subscription change.",
                            vapor_studio_core::MAX_WORKSHOP_SUBSCRIPTION_BATCH
                        ))
                        .clicked()
                {
                    action = Some(SubscriptionAction::UnsubscribeMany(
                        visible_selected_ids.clone(),
                    ));
                }
            });
            let visible_subscription_ids = visible_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let selected_outside_view = self
                .workshop
                .subscriptions
                .iter()
                .filter(|id| {
                    self.workshop.selected_items.contains(*id)
                        && !visible_subscription_ids.contains(id.as_str())
                })
                .count();
            if selected_outside_view > 0 {
                ui.weak(format!(
                    "{selected_outside_view} additional selections are outside this view. These batch actions affect only visible selections."
                ));
            }
            if matching_subscriptions == 0 {
                empty_state(
                    ui,
                    if query.is_empty() {
                        "No subscriptions are available to show"
                    } else {
                        "No subscriptions match this search"
                    },
                    if query.is_empty() {
                        "Refresh subscriptions from Steam to check for new items."
                    } else {
                        "Clear or change the search to see your other subscriptions."
                    },
                    |ui| {
                        if !query.is_empty() && ui.button("Clear search").clicked() {
                            self.workshop.subscription_search_query.clear();
                        }
                    },
                );
            } else {
                egui::CollapsingHeader::new(format!("My subscriptions ({matching_subscriptions})"))
                    .default_open(true)
                    .show(ui, |ui| {
                for id in self.workshop.subscriptions.iter().filter(|id| {
                    subscription_matches_query(
                        id,
                        &query,
                        &self.workshop.subscription_details,
                        &results_by_id,
                    )
                }) {
                    let details = self.workshop.subscription_details.get(id);
                    let title = details
                        .map(|item| item.title.as_str())
                        .or_else(|| results_by_id.get(id.as_str()).copied())
                        .unwrap_or("Workshop item");
                    let local = local_by_workshop_id
                        .get(id.as_str())
                        .copied()
                        .unwrap_or_default();
                    let steam_state = details.map(|item| {
                        if item.downloading {
                            "Downloading in Steam"
                        } else if item.needs_update {
                            "Steam update available"
                        } else if item.installed {
                            "Downloaded in Steam"
                        } else {
                            "Subscribed"
                        }
                    });
                    ui.group(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            let mut selected = self.workshop.selected_items.contains(id);
                            if ui
                                .add_enabled(
                                    !self.operation.running,
                                    egui::Checkbox::new(&mut selected, ""),
                                )
                                .changed()
                            {
                                action = Some(SubscriptionAction::SetSelected(id.clone(), selected));
                            }
                            ui.strong(title);
                            if let Some(state) = steam_state {
                                ui.weak(state);
                            }
                            if local.available {
                                ui.weak("Downloaded to this app");
                                if local.conflicted {
                                    status_text(ui, "Local conflict", StatusTone::Caution);
                                }
                            }
                        });
                        ui.collapsing("Actions and details", |ui| {
                            if let Some((current, total)) = details
                                .and_then(|item| item.download_progress.filter(|(_, total)| *total > 0))
                            {
                                ui.weak(format!(
                                    "Steam download · {:.1} / {:.1} MiB",
                                    current as f64 / 1_048_576.0,
                                    total as f64 / 1_048_576.0
                                ));
                            }
                            if local.available && local.conflicted {
                                status_text(
                                    ui,
                                    "Local records disagree. Review them in Discover before using this item.",
                                    StatusTone::Caution,
                                );
                                if ui
                                    .add_enabled(
                                        !self.operation.running,
                                        egui::Button::new("Review local records"),
                                    )
                                    .clicked()
                                {
                                    action = Some(SubscriptionAction::ReviewLocal(id.clone()));
                                }
                            }
                            if local.packagepack_available && !local.conflicted {
                                let create_draft_blocks_assignment = self
                                    .workshop_root_change_blocked_by_create_draft(id);
                                let local_catalog_current = self.catalog.local_catalog_current();
                                if local.selected_as_profile_root {
                                    ui.strong("Used as this profile's game setup");
                                } else if ui
                                    .add_enabled(
                                        !self.operation.running
                                            && local_catalog_current
                                            && !self.has_unsaved_workspace_edits()
                                            && !create_draft_blocks_assignment,
                                        egui::Button::new(format!(
                                            "Set {}'s game setup",
                                            self.profile.items[self.profile.active_index].name
                                        )),
                                    )
                                    .on_disabled_hover_text(if !local_catalog_current {
                                        "Refresh the on-device content list before choosing this as your profile's game setup."
                                    } else if create_draft_blocks_assignment {
                                        "Discard the Create draft before changing the profile to this item's workspace."
                                    } else if self.has_unsaved_workspace_edits() {
                                        "Save or discard workspace edits before changing the active profile."
                                    } else {
                                        "Finish the current operation before changing the active profile."
                                    })
                                    .clicked()
                                {
                                    action = Some(SubscriptionAction::UseAsProfileRoot(id.clone()));
                                }
                                if ui
                                    .add_enabled(
                                        !self.operation.running
                                            && local_catalog_current
                                            && !self.has_unsaved_workspace_edits()
                                            && !create_draft_blocks_assignment,
                                        egui::Button::new("Launch this game setup"),
                                    )
                                    .on_disabled_hover_text(
                                        if !local_catalog_current {
                                            "Refresh the on-device content list before launching this game setup."
                                        } else if create_draft_blocks_assignment {
                                            "Discard the Create draft before switching this profile."
                                        } else if self.has_unsaved_workspace_edits() {
                                            "Save or discard project edits before switching this profile."
                                        } else {
                                            "Wait for the current operation to finish."
                                        },
                                    )
                                    .clicked()
                                {
                                    action = Some(SubscriptionAction::LaunchAsProfileRoot(id.clone()));
                                }
                                if (local.included_in_active_profile || local.selected_as_profile_root)
                                    && ui
                                        .add_enabled(
                                            !self.operation.running
                                                && !self.has_unsaved_workspace_edits(),
                                            egui::Button::new("Review game setup"),
                                        )
                                        .on_hover_text(
                                            "Check that every item in this game setup is available here.",
                                        )
                                        .clicked()
                                {
                                    action = Some(SubscriptionAction::ReviewProfile);
                                }
                            }
                            if local.available
                                && self.profile.items[self.profile.active_index]
                                    .root_package
                                    .is_some()
                                && !local.included_in_active_profile
                                && !local.selected_as_profile_root
                            {
                                ui.small(
                                    "Available here · not included in this profile's game setup",
                                );
                            }
                            let import_label = if local.available {
                                "Update local copy"
                            } else {
                                "Download to this app"
                            };
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .add_enabled(
                                        !self.operation.running,
                                        egui::Button::new(import_label),
                                    )
                                    .clicked()
                                {
                                    action = Some(SubscriptionAction::Import(id.clone()));
                                }
                                if ui
                                    .add_enabled(
                                        !self.operation.running,
                                        egui::Button::new("Unsubscribe"),
                                    )
                                    .clicked()
                                {
                                    action = Some(SubscriptionAction::Unsubscribe(id.clone()));
                                }
                            });
                            if let Some(item) = details
                                && !item.description.is_empty()
                            {
                                ui.collapsing("Description", |ui| {
                                    ui.label(&item.description);
                                });
                            }
                            ui.small(format!("Workshop item ID · {id}"));
                            if let Some(url) = details.and_then(|item| item.preview_url.as_ref()) {
                                ui.hyperlink_to("Open Workshop item", url);
                            }
                        });
                    });
                }
            });
            }
            match action {
                Some(SubscriptionAction::SetSelected(id, selected)) => {
                    if selected {
                        self.workshop.selected_items.insert(id);
                    } else {
                        self.workshop.selected_items.remove(&id);
                    }
                }
                Some(SubscriptionAction::SetManySelected(ids, selected)) => {
                    if selected {
                        self.workshop.selected_items.extend(ids);
                    } else {
                        for id in ids {
                            self.workshop.selected_items.remove(&id);
                        }
                    }
                }
                Some(SubscriptionAction::ImportMany(ids)) => {
                    self.import_workshop_items(ids);
                }
                Some(SubscriptionAction::UnsubscribeMany(ids)) => {
                    self.set_workshop_subscriptions(ids, false);
                }
                Some(SubscriptionAction::ReviewLocal(id)) => self.review_workshop_content(&id),
                Some(SubscriptionAction::UseAsProfileRoot(id)) => {
                    self.use_available_workshop_package_as_profile_root(&id);
                }
                Some(SubscriptionAction::LaunchAsProfileRoot(id)) => {
                    self.launch_workshop_package_as_profile_root(&id);
                }
                Some(SubscriptionAction::ReviewProfile) => {
                    self.activate_navigation_section(super::super::Section::Library);
                    self.preview_selected_composition();
                }
                Some(SubscriptionAction::Import(id)) => {
                    self.workshop.published_file_id = id;
                    self.download_workshop_item();
                }
                Some(SubscriptionAction::Unsubscribe(id)) => {
                    self.set_workshop_subscription(id, false);
                }
                None => {}
            }
        }
    }
}

fn subscription_matches_query(
    id: &str,
    query: &str,
    details: &BTreeMap<String, vapor_studio_core::WorkshopSubscription>,
    results: &BTreeMap<&str, &str>,
) -> bool {
    if query.is_empty() {
        return true;
    }
    let detail = details.get(id);
    let title = detail
        .map(|item| item.title.as_str())
        .or_else(|| results.get(id).copied())
        .unwrap_or_default();
    let status = match detail {
        Some(item) if item.downloading => "downloading in steam",
        Some(item) if item.needs_update => "steam update available",
        Some(item) if item.installed => "downloaded in steam",
        Some(_) => "subscribed",
        None => "",
    };
    id.contains(query) || title.to_lowercase().contains(query) || status.contains(query)
}
