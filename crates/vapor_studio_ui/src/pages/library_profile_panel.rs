use super::super::StudioApp;
use super::super::components::design::{installation_status, status_text};
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_profile_selector_panel(&mut self, ui: &mut eframe::egui::Ui) {
        let can_switch = !self.operation.running && !self.has_unsaved_context_switch_edits();
        let card_style = ui.style().clone();
        let mut requested_profile = None;
        egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
            for index in 0..self.profile.items.len() {
                let profile = &self.profile.items[index];
                let (status, tone) = installation_status(&profile.installation);
                let selected = index == self.profile.active_index;
                egui::Frame::group(&card_style).show(ui, |ui| {
                    let response = ui.add_enabled(
                        can_switch,
                        egui::Button::selectable(
                            selected,
                            egui::RichText::new(&profile.name).strong(),
                        ),
                    );
                    if response
                        .on_disabled_hover_text("Discard the Create draft or save/discard workspace edits before switching profiles.")
                        .clicked()
                    {
                        requested_profile = Some(index);
                    }
                    if let Some(root) = &profile.root_package {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(&root.id);
                            if let Some(version) = &root.version {
                                ui.weak(version);
                            }
                        });
                    } else {
                        ui.weak("No composition selected");
                    }
                    status_text(ui, status, tone);
                });
            }
        });
        if let Some(index) = requested_profile {
            self.select_profile(index);
        }
    }
}

impl StudioApp {
    pub(super) fn draw_profile_management_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
        profile_name: &str,
    ) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong("Profile settings");
                ui.weak("Name and launch configuration");
            });
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.profile.name_draft);
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && self.profile.name_draft.trim() != profile_name,
                        egui::Button::new("Rename"),
                    )
                    .clicked()
                {
                    self.rename_active_profile();
                }
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_context_switch_edits()
                            && self.profile.items.len() > 1,
                        egui::Button::new("Remove profile"),
                    )
                    .on_disabled_hover_text(
                        "Discard the Create draft or save/discard workspace edits before removing this profile.",
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
                        profile_name
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
