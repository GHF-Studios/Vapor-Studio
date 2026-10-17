use super::super::StudioApp;
use super::super::components::design::{confirmation_card, installation_status, status_text};
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_profile_selector_panel(&mut self, ui: &mut eframe::egui::Ui) {
        let can_switch = !self.operation.running && !self.has_unsaved_context_switch_edits();
        let mut requested_profile = None;
        let profile_list_height = ui.available_height().clamp(1.0, 560.0);
        egui::ScrollArea::vertical()
            .max_height(profile_list_height)
            .show(ui, |ui| {
                for index in 0..self.profile.items.len() {
                    let profile = &self.profile.items[index];
                    let (status, tone) = installation_status(&profile.installation);
                    let selected = index == self.profile.active_index;
                    ui.horizontal_wrapped(|ui| {
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
                        status_text(ui, status, tone);
                    });
                    if let Some(root) = &profile.root_package {
                        ui.small(format!(
                            "{}{}",
                            root.id,
                            root.version
                                .as_ref()
                                .map(|version| format!(" · {version}"))
                                .unwrap_or_default()
                        ));
                    } else {
                        ui.small("No game setup selected");
                    }
                    ui.separator();
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
            ui.horizontal_wrapped(|ui| {
                ui.strong("Profile settings");
                ui.weak("Name and launch configuration");
            });
            ui.horizontal_wrapped(|ui| {
                let name_width = (ui.available_width() - 160.0).clamp(140.0, 360.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.profile.name_draft)
                        .desired_width(name_width),
                );
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
                let title = format!("Remove {profile_name} from Studio?");
                if let Some(remove) = confirmation_card(
                    ui,
                    &title,
                    "Installed content and workspace files remain untouched.",
                    "Remove profile",
                    "Keep profile",
                    !self.operation.running,
                ) {
                    if remove {
                        self.remove_active_profile();
                    } else {
                        self.profile.confirm_removal = false;
                    }
                }
            }
        });
    }
}
