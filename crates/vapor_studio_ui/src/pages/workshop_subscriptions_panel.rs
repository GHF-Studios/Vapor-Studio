use super::super::StudioApp;
use super::super::components::design::{empty_state, section_header};
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Default)]
struct LocalWorkshopContent {
    available: bool,
    conflicted: bool,
    included_in_active_profile: bool,
    packagepack_available: bool,
}

enum SubscriptionAction {
    SetSelected(String, bool),
    ReviewLocal(String),
    UseAsProfileRoot(String),
    ReviewProfile,
    Import(String),
    Unsubscribe(String),
}

impl StudioApp {
    pub(super) fn draw_workshop_subscriptions_panel(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Your Steam subscriptions", "Steam subscription downloads and imports to Vapor's local cache are separate steps.");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.operation.running,
                    egui::Button::new(if self.workshop.subscriptions_loaded { "Refresh subscriptions" } else { "Load subscriptions" }),
                )
                .clicked()
            {
                self.refresh_workshop_subscriptions();
            }
        });
        if !self.workshop.subscriptions_loaded {
            empty_state(ui, "Subscriptions have not been loaded", "Connect to Steam to review subscribed items, Steam download state, and which items are already available in Vapor.", |_| {});
        } else if self.workshop.subscriptions.is_empty() {
            empty_state(ui, "No Vapor Workshop subscriptions", "Items you subscribe to in Steam Workshop will appear here.", |_| {});
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
                        .map(|node| node.content.clone())
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            let mut local_by_workshop_id = BTreeMap::<String, LocalWorkshopContent>::new();
            for content in &self.catalog.items {
                let Some(id) = content.published_file_id.as_ref() else {
                    continue;
                };
                let status = local_by_workshop_id.entry(id.clone()).or_default();
                status.available = true;
                status.conflicted |= !content.is_unambiguous();
                status.included_in_active_profile |= active_content.contains(&content.identity());
                status.packagepack_available |= content.kind == vapor_core::ContentKind::Packagepack.as_str()
                    && content.is_unambiguous();
            }
            let mut action = None;
            egui::CollapsingHeader::new(format!("My subscriptions ({})", self.workshop.subscriptions.len()))
                .default_open(true)
                .show(ui, |ui| {
                for id in &self.workshop.subscriptions {
                    let details = self.workshop.subscription_details.get(id);
                    let title = details
                        .map(|item| item.title.as_str())
                        .or_else(|| {
                            self.workshop.results
                                .iter()
                                .find(|item| item.published_file_id.as_str() == id.as_str())
                                .map(|item| item.title.as_str())
                        })
                        .unwrap_or("Workshop item");
                    let local = local_by_workshop_id
                        .get(id)
                        .copied()
                        .unwrap_or_default();
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
                        if let Some(item) = details {
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
                        if local.available {
                            ui.weak("Available in local content");
                            if local.conflicted {
                                ui.colored_label(
                                    egui::Color32::YELLOW,
                                    "Local records conflict; review them in Discover before using this item.",
                                );
                                if ui.add_enabled(
                                    !self.operation.running,
                                    egui::Button::new("Review local records"),
                                ).clicked() {
                                    action = Some(SubscriptionAction::ReviewLocal(id.clone()));
                                }
                            }
                            let create_draft_blocks_assignment = self
                                .workshop_root_change_blocked_by_create_draft(id);
                            if local.packagepack_available && !local.conflicted
                                && ui.add_enabled(
                                    !self.operation.running
                                        && !self.has_unsaved_workspace_edits()
                                        && !create_draft_blocks_assignment,
                                    egui::Button::new(format!(
                                        "Set {} root",
                                        self.profile.items[self.profile.active_index].name
                                    )),
                                )
                                .on_disabled_hover_text(if create_draft_blocks_assignment {
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
                            if local.included_in_active_profile
                                && ui.add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Review active profile plan"),
                                )
                                .on_hover_text(
                                        "Resolve this profile again using the current local cache.",
                                )
                                .clicked()
                            {
                                action = Some(SubscriptionAction::ReviewProfile);
                            }
                        }
                        let import_label = if local.available {
                            "Refresh cached copy"
                        } else {
                            "Import to Vapor cache"
                        };
                        if ui.add_enabled(!self.operation.running, egui::Button::new(import_label)).clicked() {
                            action = Some(SubscriptionAction::Import(id.clone()));
                        }
                        if ui.add_enabled(!self.operation.running, egui::Button::new("Unsubscribe")).clicked() {
                            action = Some(SubscriptionAction::Unsubscribe(id.clone()));
                        }
                    });
                    if local.available
                        && self.profile.items[self.profile.active_index].root_package.is_some()
                        && !local.included_in_active_profile
                    {
                        ui.small("Available locally · not included in the active profile's composition");
                    }
                    if let Some(item) = details && !item.description.is_empty() {
                        ui.collapsing("Description", |ui| { ui.label(&item.description); });
                    }
                    ui.collapsing("Workshop item ID", |ui| {
                        ui.small(format!("PublishedFileId {id}"));
                    });
                    if let Some(url) = details.and_then(|item| item.preview_url.as_ref()) {
                        ui.hyperlink_to("Open Workshop item", url);
                    }
                }
            });
            match action {
                Some(SubscriptionAction::SetSelected(id, selected)) => {
                    if selected {
                        self.workshop.selected_items.insert(id);
                    } else {
                        self.workshop.selected_items.remove(&id);
                    }
                }
                Some(SubscriptionAction::ReviewLocal(id)) => self.review_workshop_content(&id),
                Some(SubscriptionAction::UseAsProfileRoot(id)) => {
                    self.use_available_workshop_package_as_profile_root(&id);
                }
                Some(SubscriptionAction::ReviewProfile) => {
                    self.section = super::super::Section::Library;
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
