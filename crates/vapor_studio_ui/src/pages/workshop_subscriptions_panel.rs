use super::super::StudioApp;
use super::super::components::design::{empty_state, section_header};
use eframe::egui;

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
            egui::CollapsingHeader::new(format!("My subscriptions ({})", self.workshop.subscriptions.len()))
                .default_open(true)
                .show(ui, |ui| {
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
                    let locally_available = self.catalog.items.iter().any(|content| {
                        content.published_file_id.as_deref() == Some(id.as_str())
                    });
                    let local_conflict = self.catalog.items.iter().any(|content| {
                        content.published_file_id.as_deref() == Some(id.as_str())
                            && !content.is_unambiguous()
                    });
                    let included_in_active_profile = self.catalog.items.iter()
                        .filter(|content| content.published_file_id.as_deref() == Some(id.as_str()))
                        .map(|content| content.identity())
                        .any(|identity| {
                        self.profile.items[self.profile.active_index]
                            .resolved_graph
                            .as_ref()
                            .is_some_and(|graph| graph.nodes.iter().any(|node| node.content == identity))
                    });
                    ui.horizontal_wrapped(|ui| {
                        let mut selected = self.workshop.selected_items.contains(&id);
                        if ui
                            .add_enabled(
                                !self.operation.running,
                                egui::Checkbox::new(&mut selected, ""),
                            )
                            .changed()
                        {
                            if selected {
                                self.workshop.selected_items.insert(id.clone());
                            } else {
                                self.workshop.selected_items.remove(&id);
                            }
                        }
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
                            ui.weak("Available in local content");
                            if local_conflict {
                                ui.colored_label(
                                    egui::Color32::YELLOW,
                                    "Local records conflict; review them in Discover before using this item.",
                                );
                                if ui.add_enabled(
                                    !self.operation.running,
                                    egui::Button::new("Review local records"),
                                ).clicked() {
                                    self.review_workshop_content(&id);
                                }
                            }
                            let available_packagepack = self.catalog.items.iter().any(|content| {
                                content.published_file_id.as_deref() == Some(id.as_str())
                                    && content.kind
                                        == vapor_core::ContentKind::Packagepack.as_str()
                                    && content.is_unambiguous()
                            });
                            if available_packagepack && !local_conflict
                                && ui.add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new(format!(
                                        "Set {} root",
                                        self.profile.items[self.profile.active_index].name
                                    )),
                                )
                                .clicked()
                            {
                                self.use_available_workshop_package_as_profile_root(&id);
                            }
                            if included_in_active_profile
                                && ui.add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Review active profile plan"),
                                )
                                .on_hover_text(
                                        "Resolve this profile again using the current local cache.",
                                )
                                .clicked()
                            {
                                self.section = super::super::Section::Library;
                                self.preview_selected_composition();
                            }
                        }
                        let import_label = if locally_available {
                            "Refresh cached copy"
                        } else {
                            "Import to Vapor cache"
                        };
                        if ui.add_enabled(!self.operation.running, egui::Button::new(import_label)).clicked() {
                            self.workshop.published_file_id = id.clone();
                            self.download_workshop_item();
                        }
                        if ui.add_enabled(!self.operation.running, egui::Button::new("Unsubscribe")).clicked() {
                            self.set_workshop_subscription(id.clone(), false);
                        }
                    });
                    if locally_available
                        && self.profile.items[self.profile.active_index].root_package.is_some()
                        && !included_in_active_profile
                    {
                        ui.small("Available locally · not included in the active profile's composition");
                    }
                    if let Some(item) = details.as_ref() && !item.description.is_empty() {
                        ui.collapsing("Description", |ui| { ui.label(&item.description); });
                    }
                    if let Some(url) = details.as_ref().and_then(|item| item.preview_url.as_ref()) {
                        ui.hyperlink_to("Open Workshop item", url);
                    }
                }
            });
        }
    }
}
