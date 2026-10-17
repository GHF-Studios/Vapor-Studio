use super::super::StudioApp;
use super::super::components::design::{empty_state, section_header};
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_acquisition_panel(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Recent imports", "These Workshop downloads were added to Vapor's local cache during this session. Browse all available content in Discover.");
        egui::CollapsingHeader::new("Import by Workshop ID")
            .show(ui, |ui| {
                ui.label("Use this when you already know the item's numeric Workshop ID.");
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.workshop.published_file_id)
                            .hint_text("Workshop item ID")
                            .desired_width(220.0),
                    );
                    if ui
                        .add_enabled(!self.operation.running, egui::Button::new("Import"))
                        .clicked()
                    {
                        self.download_workshop_item();
                    }
                });
        });
        if self.workshop.pending_acquisitions.is_empty() {
            empty_state(ui, "No recent imports", "Items imported from Workshop are added to Vapor's local cache. Browse Discover to find cached and installed content.", |_| {});
        } else {
            section_header(ui, "Downloaded this session", "Cached items are available to dependency plans. Installing a composition into a play profile is a separate step.");
            for acquisition in self.workshop.pending_acquisitions.clone() {
                ui.group(|ui| {
                    ui.label(format!(
                        "Cached: {}{} · {}",
                        acquisition.id,
                        acquisition
                            .version
                            .as_ref()
                            .map(|version| format!("@{version}"))
                            .unwrap_or_default(),
                        acquisition.kind
                    ));
                    egui::CollapsingHeader::new("Cache and file details")
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
                        let create_draft_blocks_assignment = self
                            .profile_root_change_blocked_by_create_draft(None);
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && !create_draft_blocks_assignment,
                            egui::Button::new(format!(
                                "Use as {} root",
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
                            self.use_workshop_as_profile_root(&acquisition.id);
                        }
                        ui.weak("Preview this Packagepack's dependency plan before installing it.");
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
                            "This item is part of the active profile's resolved composition."
                        } else {
                            "This cached item is not currently part of the active profile's composition."
                        });
                        if included_in_profile && ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Review active profile plan"),
                            )
                            .clicked()
                        {
                            self.section = super::super::Section::Library;
                            self.preview_selected_composition();
                        }
                    }
                });
            }
        }
    }
}
