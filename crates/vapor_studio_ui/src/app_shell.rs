use super::{Section, StudioApp};
use eframe::egui;
use vapor_studio_core::{CURRENT_SCHEMA_VERSION, ProfileDocument};

impl StudioApp {
    pub(super) fn draw_profile_target_selector(&mut self, ui: &mut egui::Ui) {
        ui.label("Play profile");
        let active_profile = self.profile.active_index;
        let mut requested_profile = None;
        egui::ComboBox::from_id_salt("active-profile")
            .selected_text(&self.profile.items[active_profile].name)
            .show_ui(ui, |ui| {
                for (index, profile) in self.profile.items.iter().enumerate() {
                    let selected = index == active_profile;
                    if ui.selectable_label(selected, &profile.name).clicked() && !selected {
                        requested_profile = Some(index);
                    }
                }
            });
        if let Some(requested_profile) = requested_profile {
            self.select_profile(requested_profile);
        }
    }

    fn select_navigation_section(&mut self, ui: &mut egui::Ui, section: Section) {
        if ui
            .selectable_label(self.section == section, section.label())
            .clicked()
        {
            self.section = section;
            if section == Section::Workspace
                && self.workspace.editor.files.is_empty()
                && !self.operation.running
            {
                self.refresh_workspace_files();
            }
        }
    }

    fn draw_navigation(&mut self, ui: &mut egui::Ui) {
        ui.heading("Vapor");
        ui.weak("STUDIO");
        ui.add_space(18.0);
        self.select_navigation_section(ui, Section::Home);
        ui.add_space(10.0);
        ui.small("PLAY");
        self.select_navigation_section(ui, Section::Library);
        ui.add_space(8.0);
        ui.small("CONTENT");
        self.select_navigation_section(ui, Section::Discover);
        ui.add_space(8.0);
        ui.small("AUTHOR");
        self.select_navigation_section(ui, Section::Create);
        self.select_navigation_section(ui, Section::Workspace);
        self.select_navigation_section(ui, Section::Compose);
        ui.add_space(8.0);
        ui.small("SHARE");
        self.select_navigation_section(ui, Section::Publish);
        ui.add_space(12.0);
        ui.separator();
        if self.workspace.root.trim().is_empty() {
            ui.weak("No workspace open");
        } else {
            ui.small("OPEN WORKSPACE");
            ui.label(
                std::path::Path::new(&self.workspace.root)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&self.workspace.root),
            );
            if self.has_unsaved_workspace_edits() {
                ui.colored_label(egui::Color32::YELLOW, "Unsaved changes");
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
            ui.heading(self.section.label());
            if self.profile.recovery.is_some() {
                ui.colored_label(egui::Color32::YELLOW, "Profile recovery needed");
            }
            let play_context = matches!(self.section, Section::Home | Section::Library);
            if play_context {
                ui.separator();
                self.draw_profile_target_selector(ui);
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("+ Profile"),
                    )
                    .clicked()
                {
                    self.profile.new_name = format!("Composition {}", self.profile.items.len() + 1);
                    self.add_profile();
                }
            }
            if self.operation.running {
                ui.separator();
                ui.spinner();
                ui.label("Working…");
            }
        });
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_operation();
        egui::Panel::left("studio-navigation")
            .resizable(false)
            .default_size(220.0)
            .show(ui, |ui| self.draw_navigation(ui));
        egui::Panel::top("studio-header").show(ui, |ui| self.draw_header(ui));
        egui::Panel::bottom("studio-operation-center")
            .show(ui, |ui| self.draw_operation_center(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            let workspace_editor =
                self.section == Section::Workspace && self.workspace.editor.files_open;
            if workspace_editor {
                self.draw_workspace(ui);
            } else {
                egui::ScrollArea::vertical()
                    .id_salt(("studio-page-content", self.section))
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.section {
                        Section::Home => self.draw_home(ui),
                        Section::Discover => self.draw_discover(ui),
                        Section::Library => self.draw_library(ui),
                        Section::Compose => self.draw_composition(ui),
                        Section::Create => self.draw_create(ui),
                        Section::Workspace => self.draw_workspace(ui),
                        Section::Publish => self.draw_publish(ui),
                        Section::Diagnostics => self.diagnostics.show(ui),
                    });
            }
        });
        if self.operation.running {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        } else if self
            .workshop
            .subscription_details
            .values()
            .any(|item| item.downloading)
        {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("workspace_root", self.workspace.root.clone());
        storage.set_string(
            "workspace_editor_drafts",
            self.workspace
                .editor
                .save_editor_drafts(&self.workspace.root)
                .unwrap_or_default(),
        );
        let document = ProfileDocument {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: self.profile.items.clone(),
        };
        if let Ok(encoded) = serde_json::to_string(&document) {
            storage.set_string("profiles", encoded);
        }
        if let Some(original) = &self.profile.recovery_backup {
            storage.set_string("profiles.recovery_backup", original.clone());
        }
        storage.set_string("active_profile", self.profile.active_index.to_string());
        if let Some(profile) = self.profile.items.get(self.profile.active_index) {
            storage.set_string("active_profile_id", profile.id.clone());
        }
        if let Ok(history) = serde_json::to_string(&self.operation.operation_history) {
            storage.set_string("operation_history", history);
        }
    }
}
