use super::super::StudioApp;
use super::super::components::design::{installation_status, status_text};
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_composition_selector_panel(&mut self, ui: &mut eframe::egui::Ui) {
        let can_switch = !self.operation.running && !self.has_unsaved_context_switch_edits();
        let mut requested_composition = None;
        let composition_list_height = ui.available_height().clamp(1.0, 560.0);
        egui::ScrollArea::vertical()
            .max_height(composition_list_height)
            .show(ui, |ui| {
                for index in 0..self.composition.items.len() {
                    let composition = &self.composition.items[index];
                    let (status, tone) = installation_status(&composition.installation);
                    let selected = index == self.composition.active_index;
                    ui.horizontal_wrapped(|ui| {
                        let response = ui.add_enabled(
                            can_switch,
                            egui::Button::selectable(
                                selected,
                                egui::RichText::new(&composition.name).strong(),
                            ),
                        );
                        if response
                            .on_disabled_hover_text("Discard the Create draft or save/discard workspace edits before switching compositions.")
                            .clicked()
                        {
                            requested_composition = Some(index);
                        }
                        status_text(ui, status, tone);
                    });
                    if let Some(root) = &composition.root_package {
                        ui.small(format!(
                            "{}{}",
                            root.id,
                            root.version
                                .as_ref()
                                .map(|version| format!(" · {version}"))
                                .unwrap_or_default()
                        ));
                    } else {
                        ui.small("No composition selected");
                    }
                    ui.separator();
                }
            });
        if let Some(index) = requested_composition {
            self.select_composition(index);
        }
    }
}
