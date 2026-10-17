use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_profile_launch_panel(&mut self, ui: &mut eframe::egui::Ui) {
        let argument_count = self.profile.items[self.profile.active_index]
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
                ui.weak("Additional arguments passed individually to this profile's Engine.");
                let active_profile = self.profile.active_index;
                let can_edit = !self.operation.running;
                let argument_count = self.profile.items[active_profile].launch.arguments.len();
                if argument_count == 0 {
                    ui.weak("No additional Engine arguments.");
                }
                let mut remove_argument = None;
                for index in 0..argument_count {
                    ui.horizontal_wrapped(|ui| {
                        let input_width = (ui.available_width() - 100.0).clamp(120.0, 360.0);
                        ui.label(format!("{}", index + 1));
                        ui.add_enabled(
                            can_edit,
                            egui::TextEdit::singleline(
                                &mut self.profile.items[active_profile].launch.arguments[index],
                            )
                            .desired_width(input_width),
                        );
                        if ui
                            .add_enabled(can_edit, egui::Button::new("Remove"))
                            .clicked()
                        {
                            remove_argument = Some(index);
                        }
                    });
                }
                if let Some(index) = remove_argument {
                    self.profile.items[active_profile]
                        .launch
                        .arguments
                        .remove(index);
                }
                if ui
                    .add_enabled(can_edit, egui::Button::new("Add argument"))
                    .clicked()
                {
                    self.profile.items[active_profile]
                        .launch
                        .arguments
                        .push(String::new());
                }
            });
    }
}
