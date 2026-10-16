use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_profile_launch_panel(&mut self, ui: &mut eframe::egui::Ui) {
        ui.separator();
        ui.label("Launch arguments");
        ui.weak("Each row is passed as one argument to the installed Engine.");
        let active_profile = self.profile.active_index;
        let can_edit = !self.operation.running;
        let mut remove_argument = None;
        for index in 0..self.profile.items[active_profile].launch.arguments.len() {
            ui.horizontal(|ui| {
                ui.label(format!("{}", index + 1));
                ui.add_enabled(
                    can_edit,
                    egui::TextEdit::singleline(
                        &mut self.profile.items[active_profile].launch.arguments[index],
                    )
                    .desired_width(320.0),
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
    }
}
