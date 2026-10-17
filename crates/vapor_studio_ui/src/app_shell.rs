use super::catalog::DiscoverTab;
use super::components::design::{ACCENT, MUTED_TEXT};
use super::directory_picker::{DirectoryPickerPurpose, DirectoryPickerResult};
use super::workshop::WorkshopTab;
use super::{Section, StudioApp};
use eframe::egui;
use vapor_studio_core::{CURRENT_SCHEMA_VERSION, ProfileDocument};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandPaletteAction {
    Navigate(Section),
    Workshop(WorkshopTab),
    ValidateWorkspace,
    WorkspaceOperation(vapor_core::DevelopmentOperation),
    OpenWorkspace,
    OpenWorkspaceFiles,
    RefreshLocalContent,
}

fn command_palette_entry_matches(title: &str, keywords: &str, query: &str) -> bool {
    let title = title.to_lowercase();
    query
        .split_whitespace()
        .all(|term| title.contains(term) || keywords.contains(term))
}

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

    fn navigation_section_label(&self, section: Section) -> String {
        match section {
            Section::Create if self.authoring.create.is_dirty() => "Create · draft".to_owned(),
            Section::Workspace if self.has_unsaved_workspace_edits() => {
                "Workspace · unsaved".to_owned()
            }
            Section::Compose if self.authoring.composition_has_unsaved_changes() => {
                "Compose · unsaved".to_owned()
            }
            Section::Publish
                if self.workshop.preview.is_some() || self.root_publishing.preview.is_some() =>
            {
                "Publish · review ready".to_owned()
            }
            _ => section.label().to_owned(),
        }
    }

    fn activate_navigation_section(&mut self, section: Section) {
            self.section = section;
            if section == Section::Workspace
                && self.workspace.editor.files.is_empty()
                && !self.operation.running
            {
                self.refresh_workspace_files();
            }
        }

    fn select_navigation_section(&mut self, ui: &mut egui::Ui, section: Section) {
        let response = ui.add_sized(
            [ui.available_width(), 32.0],
            egui::Button::selectable(
                self.section == section,
                self.navigation_section_label(section),
            ),
        );
        if response.clicked() {
            self.activate_navigation_section(section);
        }
    }

    fn open_command_palette(&mut self) {
        self.command_palette_open = true;
        self.command_palette_search.clear();
        self.command_palette_focus_search = true;
        self.command_palette_selected_index = 0;
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
        self.select_navigation_section(ui, Section::Home);
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("PLAY")
                .small()
                .strong()
                .color(MUTED_TEXT),
        );
        self.select_navigation_section(ui, Section::Library);
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("EXPLORE")
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
        self.select_navigation_section(ui, Section::Compose);
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
                egui::RichText::new("TOOLS")
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
                    egui::RichText::new("CURRENT WORKSPACE")
                        .small()
                        .strong()
                        .color(MUTED_TEXT),
                );
                ui.label(egui::RichText::new(workspace_name).strong());
                if !self.workspace.root.trim().is_empty() {
                    ui.small(&self.workspace.root);
                }
                if self.has_unsaved_context_switch_edits() {
                    ui.colored_label(egui::Color32::from_rgb(242, 193, 91), "Unsaved edits");
                } else if !self.workspace.root.trim().is_empty() {
                    ui.weak("Workspace open");
                }
                if ui
                    .button(if self.workspace.root.trim().is_empty() {
                        "Open workspace"
                    } else {
                        "Manage workspace"
                    })
                    .clicked()
                {
                    self.activate_navigation_section(Section::Workspace);
                }
            });
        });
    }

    fn draw_compact_navigation(&mut self, ui: &mut egui::Ui) {
        let mut requested_section = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("VAPOR STUDIO")
                    .strong()
                    .color(ACCENT),
            );
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
                        Section::Discover,
                        Section::Create,
                        Section::Workspace,
                        Section::Compose,
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
            if ui.button(workspace_name).on_hover_text(&self.workspace.root).clicked() {
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

    fn draw_command_palette(&mut self, ui: &mut egui::Ui) {
        if !self.command_palette_open {
            return;
        }

        let entries = [
            ("Home", "launcher start profiles", CommandPaletteAction::Navigate(Section::Home)),
            ("Profiles", "library play install update", CommandPaletteAction::Navigate(Section::Library)),
            ("Discover", "local content catalog workshop browse", CommandPaletteAction::Navigate(Section::Discover)),
            ("Browse Steam Workshop", "search acquire browse remote", CommandPaletteAction::Workshop(WorkshopTab::Browse)),
            ("Workshop subscriptions", "subscribed download unsubscribe installed", CommandPaletteAction::Workshop(WorkshopTab::Subscriptions)),
            ("Recent Workshop imports", "cache imported acquisition", CommandPaletteAction::Workshop(WorkshopTab::Imports)),
            ("Validate workspace content", "manifest references", CommandPaletteAction::ValidateWorkspace),
            ("Format workspace", "fmt rust source", CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Fmt)),
            ("Check workspace", "compiler type check", CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Check)),
            ("Test workspace", "tests verification", CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Test)),
            ("Build workspace", "compile executable", CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Build)),
            ("Clean workspace", "remove derived artifacts", CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Clean)),
            ("Create content", "new project template author", CommandPaletteAction::Navigate(Section::Create)),
            ("Workspace", "source projects files build", CommandPaletteAction::Navigate(Section::Workspace)),
            ("Compose", "packagepack dependencies graph", CommandPaletteAction::Navigate(Section::Compose)),
            ("Publish", "steam workshop review upload", CommandPaletteAction::Navigate(Section::Publish)),
            ("Diagnostics", "logs telemetry runtime", CommandPaletteAction::Navigate(Section::Diagnostics)),
            ("Open workspace", "folder select source", CommandPaletteAction::OpenWorkspace),
            ("Open workspace files", "project editor source code", CommandPaletteAction::OpenWorkspaceFiles),
            ("Refresh local content", "rescan catalog workspace", CommandPaletteAction::RefreshLocalContent),
        ];
        let mut window_open = true;
        let mut requested_action = None;
        let mut close_requested = false;
        let mut request_focus = self.command_palette_focus_search;
        self.command_palette_focus_search = false;

        egui::Window::new("Command palette")
            .id(egui::Id::new("studio-command-palette"))
            .collapsible(false)
            .resizable(false)
            .default_width(ui.ctx().content_rect().width().min(560.0))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
            .open(&mut window_open)
            .show(ui.ctx(), |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.command_palette_search)
                        .desired_width(ui.available_width())
                        .hint_text("Search pages and actions"),
                );
                if request_focus {
                    response.request_focus();
                    request_focus = false;
                }
                        ui.weak("↑ / ↓ move · Enter open · Esc close");
                let query = self.command_palette_search.trim().to_lowercase();
                if response.changed() {
                    self.command_palette_selected_index = 0;
                }
                let enter_pressed = response.has_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                let move_down = response.has_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::ArrowDown));
                let move_up = response.has_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::ArrowUp));
                let escape_pressed = ui.input(|input| input.key_pressed(egui::Key::Escape));
                let can_switch_workspace = !self.operation.running
                    && !self.has_unsaved_context_switch_edits();
                let can_refresh = !self.operation.running
                    && !self.has_unsaved_workspace_edits();
                let can_run_workspace_action = can_refresh
                    && !self.workspace.root.trim().is_empty();
                let enabled = |action: CommandPaletteAction| match action {
                    CommandPaletteAction::Navigate(_) | CommandPaletteAction::Workshop(_) => true,
                    CommandPaletteAction::ValidateWorkspace
                    | CommandPaletteAction::WorkspaceOperation(_) => can_run_workspace_action,
                    CommandPaletteAction::OpenWorkspace => can_switch_workspace,
                    CommandPaletteAction::OpenWorkspaceFiles => {
                        !self.operation.running && !self.workspace.root.trim().is_empty()
                    }
                    CommandPaletteAction::RefreshLocalContent => can_refresh,
                };
                let matching_entries = entries
                    .iter()
                    .filter(|(title, terms, _)| {
                        command_palette_entry_matches(title, terms, &query)
                    })
                    .collect::<Vec<_>>();
                let enabled_entries = matching_entries
                    .iter()
                    .filter(|(_, _, action)| enabled(*action))
                    .collect::<Vec<_>>();
                if !enabled_entries.is_empty() {
                    let last_index = enabled_entries.len() - 1;
                    if move_down {
                        self.command_palette_selected_index =
                            (self.command_palette_selected_index + 1) % enabled_entries.len();
                    } else if move_up {
                        self.command_palette_selected_index = if self.command_palette_selected_index == 0 {
                            last_index
                        } else {
                            self.command_palette_selected_index - 1
                        };
                    } else {
                        self.command_palette_selected_index =
                            self.command_palette_selected_index.min(last_index);
                    }
                } else {
                    self.command_palette_selected_index = 0;
                }
                let selected_action = enabled_entries
                    .get(self.command_palette_selected_index)
                    .map(|(_, _, action)| *action);
                let mut match_count = 0;
                egui::ScrollArea::vertical()
                    .max_height(340.0)
                    .show(ui, |ui| {
                        for (title, _terms, action) in matching_entries {
                            match_count += 1;
                            let is_selected = enabled(*action) && selected_action == Some(*action);
                            let response = ui.add_enabled(
                                enabled(*action),
                                egui::Button::selectable(
                                    is_selected,
                                    *title,
                                )
                                .min_size(egui::vec2(ui.available_width(), 32.0)),
                            );
                            if response.clicked() {
                                requested_action = Some(*action);
                            }
                            if !enabled(*action) {
                                response.on_hover_text("Finish the current operation or save/discard edits first.");
                            }
                        }
                    });
                if match_count == 0 {
                    ui.weak("No pages or actions match this search.");
                }
                if enter_pressed {
                    requested_action = selected_action;
                }
                if escape_pressed {
                    close_requested = true;
                }
            });

        window_open &= !close_requested && requested_action.is_none();
        self.command_palette_open = window_open;
        if !window_open || requested_action.is_some() {
            self.command_palette_search.clear();
            self.command_palette_focus_search = false;
        }
        match requested_action {
            Some(CommandPaletteAction::Navigate(section)) => {
                self.activate_navigation_section(section);
            }
            Some(CommandPaletteAction::Workshop(tab)) => {
                self.catalog.discover_tab = DiscoverTab::Workshop;
                self.workshop.tab = tab;
                self.activate_navigation_section(Section::Discover);
            }
            Some(CommandPaletteAction::ValidateWorkspace) => self.validate_workspace_source(),
            Some(CommandPaletteAction::WorkspaceOperation(operation)) => {
                self.run_operation(operation);
            }
            Some(CommandPaletteAction::OpenWorkspace) => self.open_workspace_picker(),
            Some(CommandPaletteAction::OpenWorkspaceFiles) => {
                self.activate_navigation_section(Section::Workspace);
                self.workspace.editor.files_open = true;
                if self.workspace.editor.files.is_empty() && !self.operation.running {
                    self.refresh_workspace_files();
                }
            }
            Some(CommandPaletteAction::RefreshLocalContent) => self.discover(),
            None => {}
        }
    }

    fn draw_header(&mut self, ui: &mut egui::Ui, compact_navigation: bool) {
        let show_workspace = !compact_navigation
            && matches!(
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
                    if self.workspace.root.trim().is_empty() {
                        self.open_workspace_picker();
                    } else {
                        self.section = Section::Workspace;
                    }
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
                        !self.operation.running && !self.has_unsaved_context_switch_edits(),
                        egui::Button::new("+ Profile"),
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
        let command_palette_requested =
            ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K));
        if command_palette_requested {
            self.open_command_palette();
        }
        let quick_open_requested =
            ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::P));
        if quick_open_requested && !self.operation.running {
            self.section = Section::Workspace;
            if self.workspace.root.trim().is_empty() {
                self.workspace.editor.files_open = false;
            } else {
                self.workspace.editor.files_open = true;
                if self.workspace.editor.files.is_empty() {
                    self.refresh_workspace_files();
                }
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
        egui::Panel::top("studio-header")
            .show(ui, |ui| self.draw_header(ui, compact_navigation));
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
