use super::super::StudioApp;
use super::super::components::design::{empty_state, section_header};
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_workshop_acquisition_panel(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Recent imports", "These Workshop downloads were added to Vapor's local cache during this session. Browse all available content in Discover.");
        ui.horizontal(|ui| {
            ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workshop.published_file_id)
                    .hint_text("PublishedFileId")
                    .desired_width(220.0),
            );
            if ui
                .add_enabled(!self.operation.running, egui::Button::new("Download to cache"))
                .clicked()
            {
                self.download_workshop_item();
            }
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
                    ui.small(format!(
                        "{} · {}",
                        acquisition.cache_root.display(),
                        acquisition.fingerprint
                    ));
                    if acquisition.kind == "packagepack" {
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(),
                            egui::Button::new(format!(
                                "Use as {} root",
                                self.profile.items[self.profile.active_index].name
                            )))
                            .on_disabled_hover_text("Save or discard workspace file and structured metadata drafts before changing the profile's workspace.")
                            .clicked()
                        {
                            self.use_workshop_as_profile_root(&acquisition.id);
                        }
                        ui.weak("Preview this Packagepack's dependency plan before installing it.");
                    } else {
                        ui.weak("Use this item as a dependency of a Packagepack to include it in a composition.");
                        let has_root = self.profile.items[self.profile.active_index].root_package.is_some();
                        if ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits() && has_root,
                                egui::Button::new(format!(
                                    "Recheck {} composition",
                                    self.profile.items[self.profile.active_index].name
                                )),
                            )
                            .on_disabled_hover_text(
                                "Select a Packagepack profile and save or discard open editor changes first.",
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
