use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_subscriptions_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Steam Workshop");
            if ui.add_enabled(!self.operation.running, egui::Button::new("My subscriptions")).clicked() {
                self.refresh_workshop_subscriptions();
            }
        });
        ui.label("Steam subscriptions are separate from Vapor cache imports. A subscribed item is not playable until its content is imported and composed.");
        if !self.workshop.subscriptions_loaded {
            ui.weak("Load your Steam Workshop subscriptions to review download state and import them into Vapor Library.");
        } else if self.workshop.subscriptions.is_empty() {
            ui.weak("This Steam account has no Workshop subscriptions for Vapor.");
        }
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
                    let locally_available = self.catalog.items.iter().any(|content| {
                        content.published_file_id.as_deref() == Some(id.as_str())
                    });
                    ui.horizontal(|ui| {
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
                            ui.weak("Available in Vapor Library");
                            let available_packagepack = self.catalog.items.iter().any(|content| {
                                content.published_file_id.as_deref() == Some(id.as_str())
                                    && content.kind
                                        == vapor_core::ContentKind::Packagepack.as_str()
                                    && content.is_unambiguous()
                            });
                            if available_packagepack
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
                            if self.profile.items[self.profile.active_index].root_package.is_some()
                                && ui.add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new(format!(
                                        "Recheck {} plan",
                                        self.profile.items[self.profile.active_index].name
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
                        }
                        let import_label = if locally_available {
                            "Update Vapor Library copy"
                        } else {
                            "Import to Vapor Library"
                        };
                        if ui.add_enabled(!self.operation.running, egui::Button::new(import_label)).clicked() {
                            self.workshop.published_file_id = id.clone();
                            self.download_workshop_item();
                        }
                        if ui.add_enabled(!self.operation.running, egui::Button::new("Unsubscribe")).clicked() {
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
    }
}
