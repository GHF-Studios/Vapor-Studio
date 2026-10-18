use super::super::StudioApp;
use eframe::egui;
use vapor_client_protocol::{ClientAction, CompositionAction};

impl StudioApp {
    pub(super) fn draw_composition_launch_panel(&mut self, ui: &mut eframe::egui::Ui) {
        let mut persist_arguments = false;
        let argument_count = self.composition.items[self.composition.active_index]
            .launch
            .arguments
            .len();
        let title = if argument_count == 0 {
            "Launch options".to_owned()
        } else {
            format!("Launch options · {argument_count}")
        };
        egui::CollapsingHeader::new(title)
            .default_open(false)
            .show(ui, |ui| {
                ui.weak("Additional arguments passed individually to this composition's Engine.");
                let active_composition = self.composition.active_index;
                let can_edit = !self.operation.running;
                let argument_count = self.composition.items[active_composition]
                    .launch
                    .arguments
                    .len();
                if argument_count == 0 {
                    ui.weak("No additional Engine arguments.");
                }
                let mut remove_argument = None;
                for index in 0..argument_count {
                    ui.horizontal_wrapped(|ui| {
                        let input_width = (ui.available_width() - 100.0).clamp(120.0, 360.0);
                        ui.label(format!("{}", index + 1));
                        let response = ui.add_enabled(
                            can_edit,
                            egui::TextEdit::singleline(
                                &mut self.composition.items[active_composition].launch.arguments
                                    [index],
                            )
                            .char_limit(4096)
                            .desired_width(input_width),
                        );
                        persist_arguments |= response.changed() && response.lost_focus();
                        if ui
                            .add_enabled(can_edit, egui::Button::new("Remove"))
                            .clicked()
                        {
                            remove_argument = Some(index);
                        }
                    });
                }
                if let Some(index) = remove_argument {
                    self.composition.items[active_composition]
                        .launch
                        .arguments
                        .remove(index);
                    persist_arguments = true;
                }
                if ui
                    .add_enabled(
                        can_edit && argument_count < 64,
                        egui::Button::new("Add argument"),
                    )
                    .clicked()
                {
                    self.composition.items[active_composition]
                        .launch
                        .arguments
                        .push(String::new());
                    persist_arguments = true;
                }
            });
        if persist_arguments {
            let composition = &self.composition.items[self.composition.active_index];
            let action = ClientAction::Composition(CompositionAction::SetLaunchArguments {
                composition_id: composition.id.clone(),
                arguments: composition.launch.arguments.clone(),
            });
            if let Err(error) = self.apply_service_action(action) {
                self.status = error;
            }
        }
    }
}
