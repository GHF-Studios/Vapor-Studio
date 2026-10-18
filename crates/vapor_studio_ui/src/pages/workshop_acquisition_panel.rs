use super::super::StudioApp;
use super::super::components::design::{empty_state, section_header};
use super::super::workshop::WorkshopTab;
use eframe::egui;

enum AcquisitionAction {
    UseAsCompositionRoot(String),
    LaunchAsCompositionRoot(String),
    ReviewActiveComposition,
}

impl StudioApp {
    pub(super) fn draw_workshop_acquisition_panel(&mut self, ui: &mut egui::Ui) {
        section_header(
            ui,
            "Library",
            "Content you have added is ready to use here. Launch a composition or add compatible content while customizing it.",
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
                "Your Library is empty",
                "Find a composition or add-on in Steam Workshop, then subscribe to add it here.",
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
                "Available content",
                "Launch a complete composition or add compatible content to one.",
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
                    if acquisition.kind == "packagepack" {
                        let selected_as_root = self.is_active_composition_root(
                            &acquisition.id,
                            acquisition.version.as_deref(),
                        );
                        if selected_as_root {
                            ui.strong("Already used in this composition");
                            if ui
                                .add_enabled(
                                    !self.operation.running
                                        && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Review composition"),
                                )
                                .clicked()
                            {
                                action = Some(AcquisitionAction::ReviewActiveComposition);
                            }
                        } else {
                            let create_draft_blocks_assignment = self
                                .composition_root_change_blocked_by_create_draft(None);
                            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && !create_draft_blocks_assignment,
                                egui::Button::new(format!(
                                    "Use as {}'s composition",
                                    self.composition.items[self.composition.active_index].name
                                )))
                                .on_disabled_hover_text(if create_draft_blocks_assignment {
                                    "Discard the Create draft before changing this composition to downloaded content."
                                } else if self.has_unsaved_workspace_edits() {
                                    "Save or discard workspace file and structured metadata drafts before changing the composition's workspace."
                                } else {
                                    "Finish the current operation before changing the composition."
                                })
                                .clicked()
                            {
                                action = Some(AcquisitionAction::UseAsCompositionRoot(acquisition.id.clone()));
                            }
                        }
                        let create_draft_blocks_assignment =
                            self.composition_root_change_blocked_by_create_draft(None);
                        let launch_blocked_by_composition_preview = selected_as_root
                            && matches!(
                                &self.composition.items[self.composition.active_index].installation,
                                vapor_client_application::InstallationState::OutOfDate { .. }
                            );
                        if ui
                            .add_enabled(
                                !self.operation.running
                                    && !launch_blocked_by_composition_preview
                                    && !self.has_unsaved_workspace_edits()
                                    && !create_draft_blocks_assignment,
                                egui::Button::new("Launch this composition"),
                            )
                            .on_disabled_hover_text(if launch_blocked_by_composition_preview {
                                "Review or update this composition before launching."
                            } else {
                                "Finish the current operation and save or discard edits before switching this composition."
                            })
                            .clicked()
                        {
                            action = Some(AcquisitionAction::LaunchAsCompositionRoot(
                                acquisition.id.clone(),
                            ));
                        }
                        ui.weak("Review this setup's required content before launch if you want to inspect it first.");
                    } else {
                        let active_composition = &self.composition.items[self.composition.active_index];
                        let included_in_composition = active_composition
                            .resolved_graph
                            .as_ref()
                            .is_some_and(|graph| {
                                graph.nodes.iter().any(|node| {
                                    node.content.id == acquisition.id
                                        && node.content.version == acquisition.version
                                })
                            });
                        ui.weak(if included_in_composition {
                            "This item is included in the active composition."
                        } else {
                            "This downloaded item is not part of the active composition."
                        });
                        if included_in_composition && ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Review composition"),
                            )
                            .clicked()
                        {
                            action = Some(AcquisitionAction::ReviewActiveComposition);
                        }
                    }
                });
            }
            match action {
                Some(AcquisitionAction::UseAsCompositionRoot(id)) => {
                    self.use_workshop_as_composition_root(&id);
                }
                Some(AcquisitionAction::LaunchAsCompositionRoot(id)) => {
                    self.launch_workshop_acquisition(&id);
                }
                Some(AcquisitionAction::ReviewActiveComposition) => {
                    self.activate_navigation_section(super::super::Section::Compositions);
                    self.preview_selected_composition();
                }
                None => {}
            }
        }
    }
}
