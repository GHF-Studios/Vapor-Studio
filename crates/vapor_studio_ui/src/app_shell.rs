use super::components::design::{ACCENT, MUTED_TEXT, StatusTone, status_text};
use super::directory_picker::{DirectoryPickerPurpose, DirectoryPickerResult};
use super::workshop::WorkshopTab;
use super::workspace_ops::WorkspacePanel;
use super::{Section, StudioApp};
use eframe::egui;
use vapor_studio_core::{CURRENT_SCHEMA_VERSION, ProfileDocument};

impl StudioApp {
    pub(super) fn draw_profile_target_selector(&mut self, ui: &mut egui::Ui) {
        let active_profile = self.profile.active_index;
        let can_switch = !self.operation.running && !self.has_unsaved_context_switch_edits();
        let mut requested_profile = None;
        let selector = ui.add_enabled_ui(can_switch, |ui| {
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
                })
        });
        let hover_text = if !can_switch {
            if self.operation.running {
                "Wait for the current operation to finish before switching profiles."
            } else if self.authoring.pending_composition_role_change.is_some() {
                "Resolve the pending composition role choice before switching profiles."
            } else {
                "Save or discard the Create draft and workspace edits before switching profiles."
            }
        } else {
            "A play profile remembers the game setup and launch settings you want to use."
        };
        let _ = selector.response.on_hover_text(hover_text);
        if let Some(requested_profile) = requested_profile {
            self.select_profile(requested_profile);
        }
    }

    fn navigation_section_label(&self, section: Section) -> String {
        match section {
            Section::Create if self.authoring.create.is_dirty() => {
                format!("{} · draft", Section::Create.label())
            }
            Section::Workspace if self.has_unsaved_workspace_edits() => {
                format!("{} · unsaved", Section::Workspace.label())
            }
            Section::Compose if self.authoring.pending_composition_role_change.is_some() => {
                format!("{} · choice pending", Section::Compose.label())
            }
            Section::Compose if self.authoring.composition_has_unsaved_changes() => {
                format!("{} · unsaved", Section::Compose.label())
            }
            Section::Publish
                if self.workshop.preview.is_some() || self.root_publishing.preview.is_some() =>
            {
                format!("{} · review ready", Section::Publish.label())
            }
            _ => section.label().to_owned(),
        }
    }

    pub(super) fn activate_navigation_section(&mut self, section: Section) {
        self.section = section;
        if section == Section::Workspace
            && self.workspace.editor.files_open
            && !self.workspace.editor.files_loaded
            && !self.operation.running
        {
            self.refresh_workspace_files();
        }
    }

    pub(super) fn open_workshop_tab(&mut self, tab: WorkshopTab) {
        self.open_discover_tab(super::catalog::DiscoverTab::Workshop);
        self.workshop.tab = tab;
    }

    pub(super) fn open_discover_tab(&mut self, tab: super::catalog::DiscoverTab) {
        self.catalog.discover_tab = tab;
        self.activate_navigation_section(Section::Discover);
    }

    pub(super) fn open_workspace_panel(&mut self, panel: WorkspacePanel) {
        self.workspace.overview_panel = panel;
        self.workspace.editor.files_open = false;
        self.activate_navigation_section(Section::Workspace);
    }

    pub(super) fn open_workspace_files(&mut self) {
        self.workspace.editor.files_open = true;
        self.activate_navigation_section(Section::Workspace);
        if !self.workspace.editor.files_loaded && !self.operation.running {
            self.refresh_workspace_files();
        }
    }

    fn select_navigation_section(&mut self, ui: &mut egui::Ui, section: Section) {
        let explanation = match section {
            Section::Home => "Launch your selected game setup.",
            Section::Library => "Manage game profiles and the setups they use.",
            Section::Discover => "Browse content on this device or in Steam Workshop.",
            Section::Create => "Start a new content project from a template.",
            Section::Workspace => "Open and edit your project files and content.",
            Section::Compose => "Choose the game engine, game, and add-ons for a play profile.",
            Section::Publish => "Review your content before sharing it.",
            Section::Diagnostics => "Check the app and its available tools.",
        };
        let response = ui
            .add_sized(
                [ui.available_width(), 32.0],
                egui::Button::selectable(
                    self.section == section,
                    self.navigation_section_label(section),
                ),
            )
            .on_hover_text(explanation);
        if response.clicked() {
            self.activate_navigation_section(section);
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
        if ui
            .button("Search Studio · Ctrl/Cmd+K")
            .on_hover_text("Find a page or common workspace action")
            .clicked()
        {
            self.open_command_palette();
        }
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("studio-navigation-items")
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 150.0).max(100.0))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("PLAY")
                        .small()
                        .strong()
                        .color(MUTED_TEXT),
                );
                self.select_navigation_section(ui, Section::Home);
                self.select_navigation_section(ui, Section::Library);
                self.select_navigation_section(ui, Section::Compose);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("DISCOVER")
                        .small()
                        .strong()
                        .color(MUTED_TEXT),
                );
                self.select_navigation_section(ui, Section::Discover);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("CREATE")
                        .small()
                        .strong()
                        .color(MUTED_TEXT),
                );
                self.select_navigation_section(ui, Section::Create);
                self.select_navigation_section(ui, Section::Workspace);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("SHARE")
                        .small()
                        .strong()
                        .color(MUTED_TEXT),
                );
                self.select_navigation_section(ui, Section::Publish);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("APP")
                        .small()
                        .strong()
                        .color(MUTED_TEXT),
                );
                self.select_navigation_section(ui, Section::Diagnostics);
            });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.small("Vapor Studio");
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
                ui.label(
                    egui::RichText::new("CURRENT PROJECT FOLDER")
                        .small()
                        .strong()
                        .color(MUTED_TEXT),
                );
                ui.label(egui::RichText::new(workspace_name).strong());
                if !self.workspace.root.trim().is_empty() {
                    ui.small(&self.workspace.root);
                }
                if self.has_unsaved_context_switch_edits() {
                    status_text(ui, "Unsaved edits", StatusTone::Caution);
                } else if !self.workspace.root.trim().is_empty() {
                    ui.weak("Project folder open");
                }
                if ui
                    .button(if self.workspace.root.trim().is_empty() {
                        "Open project folder"
                    } else {
                        "Manage project folder"
                    })
                    .clicked()
                {
                    if self.workspace.root.trim().is_empty() {
                        self.open_workspace_picker();
                    } else {
                        self.activate_navigation_section(Section::Workspace);
                    }
                }
            });
        });
    }

    fn draw_compact_navigation(&mut self, ui: &mut egui::Ui) {
        let mut requested_section = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("VAPOR STUDIO").strong().color(ACCENT));
            if ui
                .button("Search · Ctrl/Cmd+K")
                .on_hover_text("Find a page or common workspace action")
                .clicked()
            {
                self.open_command_palette();
            }
            egui::ComboBox::from_id_salt("studio-compact-navigation")
                .selected_text(self.navigation_section_label(self.section))
                .show_ui(ui, |ui| {
                    for section in [
                        Section::Home,
                        Section::Library,
                        Section::Compose,
                        Section::Discover,
                        Section::Create,
                        Section::Workspace,
                        Section::Publish,
                        Section::Diagnostics,
                    ] {
                        if ui
                            .selectable_label(
                                self.section == section,
                                self.navigation_section_label(section),
                            )
                            .clicked()
                        {
                            requested_section = Some(section);
                        }
                    }
                });
            ui.separator();
            let workspace_name = if self.workspace.root.trim().is_empty() {
                "Choose workspace"
            } else {
                std::path::Path::new(&self.workspace.root)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&self.workspace.root)
            };
            if ui
                .button(workspace_name)
                .on_hover_text(&self.workspace.root)
                .clicked()
            {
                if self.workspace.root.trim().is_empty() {
                    self.open_workspace_picker();
                } else {
                    self.activate_navigation_section(Section::Workspace);
                }
            }
        });
        if let Some(section) = requested_section {
            self.activate_navigation_section(section);
        }
    }

    fn draw_header(&mut self, ui: &mut egui::Ui, compact_navigation: bool) {
        let show_workspace = !compact_navigation
            && matches!(
                self.section,
                Section::Discover | Section::Create | Section::Workspace | Section::Compose
            );
        let show_profile_picker = true;
        let show_profile_create = matches!(self.section, Section::Home | Section::Library);
        let active_profile_launch_state = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|profile| {
                (
                    profile.root_package.is_some(),
                    matches!(
                        &profile.installation,
                        vapor_studio_core::InstallationState::OutOfDate { .. }
                    ),
                )
            })
            .unwrap_or_default();
        let show_launch_button = self.section != Section::Home;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("VAPOR STUDIO").small().strong().color(MUTED_TEXT));
            if show_workspace {
                ui.separator();
                ui.label(egui::RichText::new("PROJECT FOLDER").small().strong().color(MUTED_TEXT));
                let workspace_label = if self.workspace.root.trim().is_empty() {
                    "Choose folder"
                } else {
                    std::path::Path::new(&self.workspace.root)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&self.workspace.root)
                };
                if ui.button(workspace_label).on_hover_text(&self.workspace.root).clicked() {
                    if self.workspace.root.trim().is_empty() {
                        self.open_workspace_picker();
                    } else {
                        self.activate_navigation_section(Section::Workspace);
                    }
                }
            }
            if show_profile_picker {
                if self.profile.recovery.is_some() {
                    status_text(ui, "Profile recovery needed", StatusTone::Caution);
                }
                ui.separator();
                ui.label(egui::RichText::new("PLAY PROFILE").small().strong().color(MUTED_TEXT));
                self.draw_profile_target_selector(ui);
            }
            if show_launch_button {
                if active_profile_launch_state.0 {
                    let launch_enabled = !self.operation.running && !active_profile_launch_state.1;
                    if ui
                        .add_enabled(
                            launch_enabled,
                            egui::Button::new(egui::RichText::new("Launch").strong()),
                        )
                        .on_disabled_hover_text(if self.operation.running {
                            "Wait for the current operation to finish."
                        } else {
                            "Review or update this profile's game setup before launching."
                        })
                        .clicked()
                    {
                        self.run_installed_selected();
                    }
                } else if ui.button("Choose game setup").clicked() {
                    self.activate_navigation_section(Section::Library);
                }
            }
            if show_profile_create
                && ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_context_switch_edits(),
                        egui::Button::new("+ Play profile"),
                    )
                    .on_disabled_hover_text(
                        "Discard the Create draft or save/discard workspace edits before creating a profile.",
                    )
                    .clicked()
            {
                    let mut number = self.profile.items.len() + 1;
                    loop {
                        let candidate = format!("Profile {number}");
                        if !self
                            .profile
                            .items
                            .iter()
                            .any(|profile| profile.name.eq_ignore_ascii_case(&candidate))
                        {
                            self.profile.new_name = candidate;
                            break;
                        }
                        number += 1;
                    }
                    self.add_profile();
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
        self.poll_service();
        let command_palette_requested =
            ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K));
        if command_palette_requested {
            self.open_command_palette();
        }
        let quick_open_requested =
            ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::P));
        if quick_open_requested && !self.operation.running {
            if self.workspace.root.trim().is_empty() {
                self.workspace.editor.files_open = false;
                self.activate_navigation_section(Section::Workspace);
            } else {
                self.open_workspace_files();
            }
        }
        let viewport = ui.ctx().content_rect().size();
        let compact_navigation = viewport.x < 1040.0 || viewport.y < 520.0;
        if compact_navigation {
            egui::Panel::top("studio-compact-navigation")
                .show(ui, |ui| self.draw_compact_navigation(ui));
        } else {
            egui::Panel::left("studio-navigation")
                .resizable(false)
                .default_size(220.0)
                .show(ui, |ui| self.draw_navigation(ui));
        }
        egui::Panel::top("studio-header").show(ui, |ui| self.draw_header(ui, compact_navigation));
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
        let picker_result = self
            .directory_picker
            .as_mut()
            .and_then(|picker| picker.show(ui.ctx()));
        if let Some(result) = picker_result {
            self.directory_picker = None;
            if let DirectoryPickerResult::Selected { purpose, path } = result {
                match purpose {
                    DirectoryPickerPurpose::OpenWorkspace => {
                        self.workspace.root_draft = path.display().to_string();
                        self.open_workspace_path(path);
                    }
                    DirectoryPickerPurpose::WorkspaceParent => {
                        self.workspace.starter_path = path.display().to_string();
                    }
                }
            }
        }
        self.draw_command_palette(ui);
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
