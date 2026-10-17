use super::super::StudioApp;
use super::super::components::design::{empty_state, section_header};
use super::super::workshop::WorkshopTab;
use eframe::egui;

enum AcquisitionAction {
    UseAsProfileRoot(String),
    LaunchAsProfileRoot(String),
    ReviewActiveProfile,
}

impl StudioApp {
    pub(super) fn draw_workshop_acquisition_panel(&mut self, ui: &mut egui::Ui) {
        section_header(
            ui,
            "Downloaded content",
            "Downloaded copies live on this device. Launch a game setup from here, or add other content while editing a profile's game setup.",
        );
        egui::CollapsingHeader::new("Import by Workshop ID").show(ui, |ui| {
            ui.label("Use this if you already have the item's number. Most people can find content by name in Browse Workshop.");
            ui.horizontal_wrapped(|ui| {
                let input_width = ui.available_width().min(280.0);
                ui.add_enabled(
                    !self.operation.running,
                    egui::TextEdit::singleline(&mut self.workshop.published_file_id)
                        .hint_text("Workshop item ID")
                        .desired_width(input_width),
                );
                if ui
                    .add_enabled(!self.operation.running, egui::Button::new("Import"))
                    .clicked()
                {
                    self.download_workshop_item();
                }
            });
        });
        if self.workshop.recent_acquisitions.is_empty() {
            let mut browse_workshop = false;
            empty_state(
                ui,
                "No content downloaded yet",
                "Browse Steam Workshop to find content, or enter an item number if someone shared one with you.",
                |ui| {
                    browse_workshop = ui.button("Browse Steam Workshop").clicked();
                },
            );
            if browse_workshop {
                self.workshop.tab = WorkshopTab::Browse;
            }
        } else {
            section_header(
                ui,
                "Ready to use",
                "These downloads are available on this device. Launch a game setup or add other content to a profile.",
            );
            let mut action = None;
            for acquisition in &self.workshop.recent_acquisitions {
                ui.group(|ui| {
                    ui.label(format!(
                        "{} · {}{}",
                        super::super::display_content_kind(&acquisition.kind),
                        acquisition.id,
                        acquisition
                            .version
                            .as_ref()
                            .map(|version| format!("@{version}"))
                            .unwrap_or_default()
                    ));
                    egui::CollapsingHeader::new("Technical details")
                        .show(ui, |ui| {
                            ui.small(format!(
                                "Cache folder · {}",
                                acquisition.cache_root.display()
                            ));
                            ui.small(format!(
                                "File fingerprint · {}",
                                acquisition.fingerprint
                            ));
                        });
                    if acquisition.kind == "packagepack" {
                        let selected_as_root = self.is_active_profile_root(
                            &acquisition.id,
                            acquisition.version.as_deref(),
                        );
                        if selected_as_root {
                            ui.strong("Already used in this profile");
                            if ui
                                .add_enabled(
                                    !self.operation.running
                                        && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Review game setup"),
                                )
                                .clicked()
                            {
                                action = Some(AcquisitionAction::ReviewActiveProfile);
                            }
                        } else {
                            let create_draft_blocks_assignment = self
                                .profile_root_change_blocked_by_create_draft(None);
                            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && !create_draft_blocks_assignment,
                                egui::Button::new(format!(
                                    "Use as {}'s game setup",
                                    self.profile.items[self.profile.active_index].name
                                )))
                                .on_disabled_hover_text(if create_draft_blocks_assignment {
                                    "Discard the Create draft before changing this profile to cached content."
                                } else if self.has_unsaved_workspace_edits() {
                                    "Save or discard workspace file and structured metadata drafts before changing the profile's workspace."
                                } else {
                                    "Finish the current operation before changing the profile."
                                })
                                .clicked()
                            {
                                action = Some(AcquisitionAction::UseAsProfileRoot(acquisition.id.clone()));
                            }
                        }
                        let create_draft_blocks_assignment =
                            self.profile_root_change_blocked_by_create_draft(None);
                        let launch_blocked_by_profile_state = selected_as_root
                            && matches!(
                                &self.profile.items[self.profile.active_index].installation,
                                vapor_studio_core::InstallationState::OutOfDate { .. }
                            );
                        if ui
                            .add_enabled(
                                !self.operation.running
                                    && !launch_blocked_by_profile_state
                                    && !self.has_unsaved_workspace_edits()
                                    && !create_draft_blocks_assignment,
                                egui::Button::new("Launch this game setup"),
                            )
                            .on_disabled_hover_text(if launch_blocked_by_profile_state {
                                "Review or update this profile before launching."
                            } else {
                                "Finish the current operation and save or discard edits before switching this profile."
                            })
                            .clicked()
                        {
                            action = Some(AcquisitionAction::LaunchAsProfileRoot(
                                acquisition.id.clone(),
                            ));
                        }
                        ui.weak("Review this setup's required content before launch if you want to inspect it first.");
                    } else {
                        let active_profile = &self.profile.items[self.profile.active_index];
                        let included_in_profile = active_profile
                            .resolved_graph
                            .as_ref()
                            .is_some_and(|graph| {
                                graph.nodes.iter().any(|node| {
                                    node.content.id == acquisition.id
                                        && node.content.version == acquisition.version
                                })
                            });
                        ui.weak(if included_in_profile {
                            "This item is included in the active profile's game setup."
                        } else {
                            "This downloaded item is not part of the active profile's game setup."
                        });
                        if included_in_profile && ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Review game setup"),
                            )
                            .clicked()
                        {
                            action = Some(AcquisitionAction::ReviewActiveProfile);
                        }
                    }
                });
            }
            match action {
                Some(AcquisitionAction::UseAsProfileRoot(id)) => {
                    self.use_workshop_as_profile_root(&id);
                }
                Some(AcquisitionAction::LaunchAsProfileRoot(id)) => {
                    self.launch_workshop_acquisition(&id);
                }
                Some(AcquisitionAction::ReviewActiveProfile) => {
                    self.activate_navigation_section(super::super::Section::Library);
                    self.preview_selected_composition();
                }
                None => {}
            }
        }
    }
}
