use super::{Section, StudioApp};
use super::operations::OperationResult;
use eframe::egui;
use vapor_studio_core::{CURRENT_SCHEMA_VERSION, OperationKind, ProfileDocument};

impl StudioApp {
    fn draw_navigation(&mut self, ui: &mut egui::Ui) {
        ui.heading("VAPOR");
        ui.weak("Studio");
        ui.add_space(14.0);
        for section in Section::ALL {
            if ui
                .selectable_label(self.section == section, section.label())
                .clicked()
            {
                self.section = section;
                if section == Section::Workspace && self.workspace_files.is_empty() && !self.running
                {
                    self.refresh_workspace_files();
                }
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            if ui
                .selectable_label(self.section == Section::Diagnostics, "Diagnostics")
                .clicked()
            {
                self.section = Section::Diagnostics;
            }
            ui.separator();
            ui.small("Vapor Studio");
        });
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let previous_profile = self.active_profile;
            ui.heading(self.section.label());
            ui.separator();
            ui.label("Profile");
            egui::ComboBox::from_id_salt("active-profile")
                .selected_text(&self.profiles[self.active_profile].name)
                .show_ui(ui, |ui| {
                    for (index, profile) in self.profiles.iter().enumerate() {
                        ui.selectable_value(&mut self.active_profile, index, &profile.name);
                    }
                });
            if self.active_profile != previous_profile {
                let requested_profile = self.active_profile;
                self.active_profile = previous_profile;
                self.select_profile(requested_profile);
            }
            ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Search content"))
                .on_hover_text("Search the current catalog");
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty,
                    egui::Button::new("+ Profile"),
                )
                .clicked()
            {
                self.new_profile_name = format!("Composition {}", self.profiles.len() + 1);
                self.add_profile();
            }
            if self.running {
                ui.separator();
                ui.spinner();
                ui.label("Working…");
            }
        });
    }

    fn preview_steam_package(&mut self) {
        self.spawn(OperationKind::StageRootDistribution, |_context| {
            let result = vapor_sdk_core::root::package::root_package(
                &vapor_sdk_core::root::types::RootPackageRequest {
                    plan: true,
                    app_id: vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                    depot_id: vapor_sdk_core::root::types::ROOT_STEAM_DEPOT_ID,
                    description: "Vapor Studio package preview".to_owned(),
                    set_live: None,
                },
            )
            .map(|report| {
                format!(
                    "Steam root preview · App {} · depot {} · {} source roots · {} files · output {}",
                    report.app_id,
                    report.depot_id,
                    report.included_roots.len(),
                    report.copied_files,
                    report.package_root.display(),
                )
            })
            .map_err(|error| error.to_string());
            OperationResult::Message(result)
        });
        self.status = "Preparing a read-only Steam root package plan…".to_owned();
    }

}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_operation();
        egui::Panel::left("studio-navigation")
            .resizable(false)
            .default_size(190.0)
            .show(ui, |ui| self.draw_navigation(ui));
        egui::Panel::top("studio-header").show(ui, |ui| self.draw_header(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            match self.section {
                Section::Home => self.draw_home(ui),
                Section::Discover => self.draw_discover(ui),
                Section::Library => self.draw_library(ui),
                Section::Compose => self.draw_composition(ui),
                Section::Create => self.draw_create(ui),
                Section::Workspace => self.draw_workspace(ui),
                Section::Diagnostics => {
                    self.diagnostics.show(ui);
                    egui::CollapsingHeader::new("Steam root package tools").show(ui, |ui| {
                        ui.label("Root app/depot publishing is separate from Workshop content publishing.");
                        if ui.add_enabled(!self.running, egui::Button::new("Preview root package")).clicked() {
                            self.preview_steam_package();
                        }
                    });
                }
            }
            self.draw_operation_center(ui);
            if let Some(recovery) = &self.profile_recovery {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, recovery);
            }
        });
        if self.running {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("workspace_root", self.workspace_root.clone());
        let document = ProfileDocument {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: self.profiles.clone(),
        };
        if let Ok(encoded) = serde_json::to_string(&document) {
            storage.set_string("profiles", encoded);
        }
        if let Some(original) = &self.profile_recovery_backup {
            storage.set_string("profiles.recovery_backup", original.clone());
        }
        storage.set_string("active_profile", self.active_profile.to_string());
        if let Some(profile) = self.profiles.get(self.active_profile) {
            storage.set_string("active_profile_id", profile.id.clone());
        }
    }
}
