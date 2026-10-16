use super::super::StudioApp;
use super::super::content_selector;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_profile_selector_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
    ) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Duplicate active profile"),
                )
                .on_disabled_hover_text(
                    "Save or discard workspace file and structured metadata drafts before duplicating a profile.",
                )
                .clicked()
            {
                self.duplicate_active_profile();
            }
            ui.weak(
                "Copies the selected root and launch settings; installed artifacts remain shared.",
            );
        });

        for index in 0..self.profile.items.len() {
            let profile = &self.profile.items[index];
            let name = profile.name.clone();
            let root = profile
                .root_package
                .as_ref()
                .map(content_selector)
                .unwrap_or_else(|| "No Packagepack selected".to_owned());
            let selected = index == self.profile.active_index;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::selectable(selected, name),
                    )
                    .on_disabled_hover_text(
                        "Save or discard workspace file and structured metadata drafts before switching profiles.",
                    )
                    .clicked()
                {
                    self.select_profile(index);
                }
                ui.label(root);
            });
        }
    }
}

use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_profile_management_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
        profile: &vapor_studio_core::CompositionProfile,
    ) {
        ui.group(|ui| {
            ui.label("Manage profile");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.profile.name_draft);
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && self.profile.name_draft.trim() != profile.name,
                        egui::Button::new("Rename"),
                    )
                    .clicked()
                {
                    self.rename_active_profile();
                }
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && self.profile.items.len() > 1,
                        egui::Button::new("Remove profile"),
                    )
                    .clicked()
                {
                    self.profile.confirm_removal = true;
                }
            });
            if self.profile.confirm_removal {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!(
                        "Remove {} from Studio? Installed content and workspace files remain untouched.",
                        profile.name
                    ),
                );
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.operation.running, egui::Button::new("Confirm removal"))
                        .clicked()
                    {
                        self.remove_active_profile();
                    }
                    if ui.button("Cancel").clicked() {
                        self.profile.confirm_removal = false;
                    }
                });
            }
        });
    }
}
