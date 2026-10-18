use super::super::StudioApp;
use super::super::components::design::confirmation_card;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_composition_management_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
        composition_name: &str,
    ) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Composition settings");
                ui.weak("Name and launch configuration");
            });
            ui.horizontal_wrapped(|ui| {
                let name_width = (ui.available_width() - 160.0).clamp(140.0, 360.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.composition.name_draft)
                        .desired_width(name_width),
                );
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && self.composition.name_draft.trim() != composition_name,
                        egui::Button::new("Rename"),
                    )
                    .clicked()
                {
                    self.rename_active_composition();
                }
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_context_switch_edits()
                            && self.composition.items.len() > 1,
                        egui::Button::new("Remove composition"),
                    )
                    .on_disabled_hover_text(
                        "Discard the Create draft or save/discard workspace edits before removing this composition.",
                    )
                    .clicked()
                {
                    self.composition.confirm_removal = true;
                }
            });
            if self.composition.confirm_removal {
                let title = format!("Remove {composition_name} from Studio?");
                if let Some(remove) = confirmation_card(
                    ui,
                    &title,
                    "Installed content and workspace files remain untouched.",
                    "Remove composition",
                    "Keep composition",
                    !self.operation.running,
                ) {
                    if remove {
                        self.remove_active_composition();
                    } else {
                        self.composition.confirm_removal = false;
                    }
                }
            }
        });
    }
}
