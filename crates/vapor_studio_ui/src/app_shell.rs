use super::{Section, StudioApp};
use super::components::design::{ACCENT, MUTED_TEXT};
use eframe::egui;
use vapor_studio_core::{CURRENT_SCHEMA_VERSION, ProfileDocument};

impl StudioApp {
    pub(super) fn draw_profile_target_selector(&mut self, ui: &mut egui::Ui) {
        let active_profile = self.profile.active_index;
        let mut requested_profile = None;
        egui::ComboBox::from_id_salt("active-profile")
            .width(190.0)
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
        let response = ui.add_sized(
            [ui.available_width(), 32.0],
            egui::Button::selectable(self.section == section, section.label()),
        );
        if response.clicked() {
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
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("VAPOR")
                    .size(21.0)
                    .strong()
                    .color(ACCENT),
            );
            ui.label(egui::RichText::new("STUDIO").small().strong());
        });
        ui.add_space(14.0);
        self.select_navigation_section(ui, Section::Home);
        ui.add_space(12.0);
        ui.label(egui::RichText::new("PLAY").small().strong().color(MUTED_TEXT));
        self.select_navigation_section(ui, Section::Library);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("EXPLORE").small().strong().color(MUTED_TEXT));
        self.select_navigation_section(ui, Section::Discover);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("CREATE").small().strong().color(MUTED_TEXT));
        self.select_navigation_section(ui, Section::Create);
        self.select_navigation_section(ui, Section::Workspace);
        self.select_navigation_section(ui, Section::Compose);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("SHARE").small().strong().color(MUTED_TEXT));
        self.select_navigation_section(ui, Section::Publish);
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.small("Vapor Studio");
            self.select_navigation_section(ui, Section::Diagnostics);
            ui.label(egui::RichText::new("TOOLS").small().strong().color(MUTED_TEXT));
            ui.separator();
            let workspace_name = if self.workspace.root.trim().is_empty() {
                "No workspace open".to_owned()
            } else {
                std::path::Path::new(&self.workspace.root)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&self.workspace.root)
                    .to_owned()
            };
            let style = ui.style().clone();
            egui::Frame::group(&style).show(ui, |ui| {
                ui.label(egui::RichText::new("CURRENT WORKSPACE").small().strong().color(MUTED_TEXT));
                ui.label(egui::RichText::new(workspace_name).strong());
                if !self.workspace.root.trim().is_empty() {
                    ui.small(&self.workspace.root);
                }
                if self.has_unsaved_workspace_edits() {
                    ui.colored_label(egui::Color32::from_rgb(242, 193, 91), "Unsaved edits");
                } else if !self.workspace.root.trim().is_empty() {
                    ui.weak("Workspace ready");
                }
                if ui.button(if self.workspace.root.trim().is_empty() { "Open workspace" } else { "Manage workspace" }).clicked() {
                    self.section = Section::Workspace;
                }
            });
        });
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        let show_workspace = matches!(
            self.section,
            Section::Discover | Section::Create | Section::Workspace | Section::Compose
        );
        let show_profile_picker = matches!(self.section, Section::Home | Section::Discover);
        let show_profile_create = matches!(self.section, Section::Home | Section::Library);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("VAPOR STUDIO").small().strong().color(MUTED_TEXT));
            if show_workspace {
                ui.separator();
                ui.label(egui::RichText::new("WORKSPACE").small().strong().color(MUTED_TEXT));
                let workspace_label = if self.workspace.root.trim().is_empty() {
                    "Choose workspace"
                } else {
                    std::path::Path::new(&self.workspace.root)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&self.workspace.root)
                };
                if ui.button(workspace_label).on_hover_text(&self.workspace.root).clicked() {
                    self.section = Section::Workspace;
                }
            }
            if show_profile_picker {
                if self.profile.recovery.is_some() {
                    ui.colored_label(egui::Color32::from_rgb(242, 193, 91), "Profile recovery needed");
                }
                ui.separator();
                ui.label(egui::RichText::new("PROFILE").small().strong().color(MUTED_TEXT));
                self.draw_profile_target_selector(ui);
            }
            if show_profile_create {
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
