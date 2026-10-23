use super::components::design::{ACCENT, MUTED_TEXT, StatusTone, status_text};
use super::directory_picker::{DirectoryPickerPurpose, DirectoryPickerResult};
use super::workshop::WorkshopTab;
use super::workspace_ops::WorkspacePanel;
use super::{Section, StudioApp};
use crate::navigation::{NAVIGATION_GROUPS, NavigationArea, NavigationGroup};
use eframe::egui;
use vapor_client_application::{CURRENT_SCHEMA_VERSION, CompositionDocument};
use vapor_client_protocol::ClientAction;

const NAVIGATION_PANEL_WIDTH: f32 = 160.0;
const NAVIGATION_ROW_HEIGHT: f32 = 26.0;

impl StudioApp {
    pub(super) fn draw_composition_target_selector(&mut self, ui: &mut egui::Ui) {
        let active_composition = self.composition.active_index;
        let can_switch = !self.operation.running && !self.has_unsaved_context_switch_edits();
        let mut requested_composition = None;
        let selector = ui.add_enabled_ui(can_switch, |ui| {
            egui::ComboBox::from_id_salt("active-composition")
                .width(190.0)
                .selected_text(&self.composition.items[active_composition].name)
                .show_ui(ui, |ui| {
                    for (index, composition) in self.composition.items.iter().enumerate() {
                        let selected = index == active_composition;
                        if ui.selectable_label(selected, &composition.name).clicked() && !selected {
                            requested_composition = Some(index);
                        }
                    }
                })
        });
        let hover_text = if !can_switch {
            if self.operation.running {
                "Wait for the current operation to finish before switching compositions."
            } else if self.authoring.pending_composition_role_change.is_some() {
                "Resolve the pending composition role choice before switching compositions."
            } else {
                "Save or discard the Create draft and workspace edits before switching compositions."
            }
        } else {
            "Your composition includes its playable content and launch settings."
        };
        let _ = selector.response.on_hover_text(hover_text);
        if let Some(requested_composition) = requested_composition {
            self.select_composition(requested_composition);
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
            Section::Home => "Launch your selected composition.",
            Section::Compositions => "Launch or customize your playable compositions.",
            Section::Library => "Browse downloaded and installed content on this device.",
            Section::Discover => "Browse content on this device or in Steam Workshop.",
            Section::Create => "Start a new content project from a template.",
            Section::Workspace => "Open and edit your project files and content.",
            Section::Compose => "Choose the content included in a composition.",
            Section::Publish => "Review your content before sharing it.",
            Section::Diagnostics => "Check the app and its available tools.",
        };
        let response = ui
            .add_sized(
                [ui.available_width(), NAVIGATION_ROW_HEIGHT],
                egui::Button::selectable(
                    self.section == section,
                    self.navigation_section_label(section),
                )
                .wrap_mode(egui::TextWrapMode::Truncate),
            )
            .on_hover_text(explanation);
        if response.clicked() {
            self.activate_navigation_section(section);
        }
    }

    fn draw_navigation(&mut self, ui: &mut egui::Ui) {
        ui.set_min_height(ui.available_height());
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
        ui.label(
            egui::RichText::new("VAPOR STUDIO")
                .small()
                .strong()
                .color(ACCENT),
        );
        if ui
            .add_sized(
                [ui.available_width(), 25.0],
                egui::Button::new("Search · Ctrl/Cmd+K"),
            )
            .on_hover_text("Find a page or common workspace action")
            .clicked()
        {
            self.open_command_palette();
        }
        ui.add_space(2.0);
        egui::ScrollArea::vertical()
            .id_salt("studio-navigation-items")
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 95.0).max(80.0))
            .show(ui, |ui| {
                for group in NAVIGATION_GROUPS {
                    self.draw_navigation_group(ui, group);
                    ui.add_space(3.0);
                }
            });
        ui.separator();
        let workspace_name = if self.workspace.root.trim().is_empty() {
            "No project open".to_owned()
        } else {
            std::path::Path::new(&self.workspace.root)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&self.workspace.root)
                .to_owned()
        };
        ui.label(
            egui::RichText::new(workspace_name)
                .strong()
                .small()
                .color(MUTED_TEXT),
        )
        .on_hover_text(&self.workspace.root);
        if self.has_unsaved_context_switch_edits() {
            status_text(ui, "Unsaved edits", StatusTone::Caution);
        }
        let action = if self.workspace.root.trim().is_empty() {
            "Open project folder"
        } else {
            "Manage project folder"
        };
        if ui
            .add_sized([ui.available_width(), 25.0], egui::Button::new(action))
            .clicked()
        {
            if self.workspace.root.trim().is_empty() {
                self.open_workspace_picker();
            } else {
                self.activate_navigation_section(Section::Workspace);
            }
        }
    }

    fn draw_navigation_group(&mut self, ui: &mut egui::Ui, group: &NavigationGroup) {
        let draw_items = |ui: &mut egui::Ui, this: &mut Self| {
            let color = match group.area {
                NavigationArea::Creator => egui::Color32::from_rgb(235, 190, 98),
                NavigationArea::Player | NavigationArea::Application => MUTED_TEXT,
            };
            ui.label(
                egui::RichText::new(group.label)
                    .small()
                    .strong()
                    .color(color),
            );
            for section in group.sections {
                this.select_navigation_section(ui, *section);
            }
        };
        if group.area == NavigationArea::Creator {
            let mut frame = egui::Frame::group(ui.style());
            frame.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(186, 132, 48));
            frame.inner_margin = egui::Margin::same(5);
            frame.show(ui, |ui| draw_items(ui, self));
        } else {
            draw_items(ui, self);
        }
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
                    for group in NAVIGATION_GROUPS {
                        ui.separator();
                        let color = if group.area == NavigationArea::Creator {
                            egui::Color32::from_rgb(235, 190, 98)
                        } else {
                            MUTED_TEXT
                        };
                        ui.label(
                            egui::RichText::new(group.label)
                                .small()
                                .strong()
                                .color(color),
                        );
                        for section in group.sections {
                            if ui
                                .selectable_label(
                                    self.section == *section,
                                    self.navigation_section_label(*section),
                                )
                                .clicked()
                            {
                                requested_section = Some(*section);
                            }
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

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        let show_composition_picker = true;
        let show_composition_create = matches!(self.section, Section::Home | Section::Compositions);
        let active_composition_launch_state = self
            .composition
            .items
            .get(self.composition.active_index)
            .is_some_and(|composition| composition.root_package.is_some());
        let show_launch_button = self.section != Section::Home;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("VAPOR STUDIO").small().strong().color(MUTED_TEXT));
            if show_composition_picker {
                if self.composition.recovery.is_some() {
                    status_text(ui, "Composition recovery needed", StatusTone::Caution);
                }
                ui.separator();
                ui.label(egui::RichText::new("ACTIVE COMPOSITION").small().strong().color(MUTED_TEXT));
                self.draw_composition_target_selector(ui);
            }
            if show_launch_button {
                if active_composition_launch_state {
                    let launch_enabled = self
                        .service_action_enabled("composition.launch")
                        .unwrap_or(false);
                    if ui
                        .add_enabled(
                            launch_enabled,
                            egui::Button::new(egui::RichText::new("Launch").strong()),
                        )
                        .on_disabled_hover_text(
                            self.service_action_reason("composition.launch")
                                .unwrap_or("Review or update this composition before launching."),
                        )
                        .clicked()
                    {
                        self.run_installed_selected();
                    }
                } else if ui.button("Choose a composition").clicked() {
                    self.activate_navigation_section(Section::Compositions);
                }
            }
            ui.toggle_value(&mut self.console_open, "Console")
                .on_hover_text("Show the docked command console · F2");
            if show_composition_create
                && ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_context_switch_edits(),
                        egui::Button::new("+ Composition"),
                    )
                    .on_disabled_hover_text(
                        "Discard the Create draft or save/discard workspace edits before creating a composition.",
                    )
                    .clicked()
            {
                    let mut number = self.composition.items.len() + 1;
                    loop {
                        let candidate = format!("Composition {number}");
                        if !self
                            .composition
                            .items
                            .iter()
                            .any(|composition| composition.name.eq_ignore_ascii_case(&candidate))
                        {
                            self.composition.new_name = candidate;
                            break;
                        }
                        number += 1;
                    }
                    self.add_composition();
            }
            if self.operation.running {
                ui.separator();
                ui.spinner();
                ui.label("Working…");
            }
            if self.service_connection_attempt.is_some() {
                ui.separator();
                ui.spinner();
                ui.label("Reconnecting to Vapor…");
            } else if self.service.is_none() {
                ui.separator();
                status_text(ui, "Vapor unavailable", StatusTone::Caution);
                if ui.small_button("Reconnect").clicked() {
                    self.reconnect_service_in_background();
                }
            }
        });
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_service();
        self.poll_workshop_download_progress();
        if ui.input(|input| input.key_pressed(egui::Key::F2)) {
            if !self.console_open {
                self.console_problems_active = false;
            }
            self.console_open = !self.console_open;
        }
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
        let compact_navigation = viewport.x < 960.0 || viewport.y < 520.0;
        if compact_navigation {
            egui::Panel::top("studio-compact-navigation")
                .show(ui, |ui| self.draw_compact_navigation(ui));
        } else {
            egui::Panel::left("studio-navigation")
                .resizable(false)
                .default_size(NAVIGATION_PANEL_WIDTH)
                .show(ui, |ui| self.draw_navigation(ui));
        }
        egui::Panel::top("studio-header").show(ui, |ui| self.draw_header(ui));
        egui::Panel::bottom("studio-operation-center")
            .resizable(self.console_open)
            .default_size(if self.console_open { 320.0 } else { 54.0 })
            .size_range(if self.console_open {
                140.0..=520.0
            } else {
                40.0..=80.0
            })
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
                        Section::Compositions => self.draw_compositions(ui),
                        Section::Library => self.draw_library(ui),
                        Section::Compose => self.draw_composition(ui),
                        Section::Create => self.draw_create(ui),
                        Section::Workspace => self.draw_workspace(ui),
                        Section::Publish => self.draw_publish(ui),
                        Section::Diagnostics => {
                            self.diagnostics.show(ui);
                            for action in self.diagnostics.take_profile_actions() {
                                let status = match self.apply_service_action(
                                    ClientAction::Developer(action.clone()),
                                ) {
                                    Ok(()) => match action {
                                        vapor_client_protocol::DeveloperAction::RefreshProfileTraces => {
                                            "Trace library refreshed.".to_owned()
                                        }
                                        vapor_client_protocol::DeveloperAction::ImportProfileTrace { path } => {
                                            format!("Added {}", std::path::Path::new(&path).display())
                                        }
                                        vapor_client_protocol::DeveloperAction::AnalyzeProfileTrace { .. } => {
                                            "Analyzing saved Tracy capture…".to_owned()
                                        }
                                        _ => String::new(),
                                    },
                                    Err(error) => {
                                        if matches!(
                                            &action,
                                            vapor_client_protocol::DeveloperAction::AnalyzeProfileTrace { .. }
                                        ) {
                                            self.diagnostics.profile_action_failed(error.clone());
                                        }
                                        error
                                    }
                                };
                                self.diagnostics.set_status(status);
                            }
                            if self.diagnostics.take_open_profiler_request() {
                                let result = self.apply_service_action(ClientAction::Developer(
                                    vapor_client_protocol::DeveloperAction::OpenProfiler,
                                ));
                                self.diagnostics.set_status(match result {
                                    Ok(()) => "Opened Tracy collector.".to_owned(),
                                    Err(error) => error,
                                });
                            }
                        }
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
                    DirectoryPickerPurpose::Superworkspace => {
                        let action = vapor_client_protocol::ClientAction::Workspace(
                            vapor_client_protocol::WorkspaceAction::SetSuperworkspaceLocation {
                                root: path.display().to_string(),
                            },
                        );
                        let result = self.apply_service_action(action);
                        self.status = match &result {
                            Ok(()) => "Vapor's shared source folder was updated.".to_owned(),
                            Err(error) => {
                                format!("Could not change the shared source folder: {error}")
                            }
                        };
                        if result.is_ok() {
                            self.discover_superworkspace_projects_if_ready();
                        }
                    }
                }
            }
        }
        self.draw_command_palette(ui);
        if self.operation.running || self.service_connection_attempt.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        } else if self
            .workshop
            .subscription_details
            .values()
            .any(|item| item.steam_downloading)
        {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let editor_drafts = self
            .workspace
            .editor
            .save_editor_drafts(&self.workspace.root);
        let operation_history = serde_json::to_string(&self.operation.operation_history).ok();
        if self.service.is_some() {
            if let Err(error) = self.persist_session_state() {
                self.status = format!("Could not save Vapor Client state: {error}");
                save_legacy_egui_state(self, storage, editor_drafts, operation_history);
            }
        } else {
            save_legacy_egui_state(self, storage, editor_drafts, operation_history);
        }
    }
}

fn save_legacy_egui_state(
    app: &StudioApp,
    storage: &mut dyn eframe::Storage,
    editor_drafts: Option<String>,
    operation_history: Option<String>,
) {
    storage.set_string("workspace_root", app.workspace.root.clone());
    storage.set_string("workspace_editor_drafts", editor_drafts.unwrap_or_default());
    let document = CompositionDocument {
        schema_version: CURRENT_SCHEMA_VERSION,
        compositions: app.composition.items.clone(),
    };
    if let Ok(encoded) = serde_json::to_string(&document) {
        storage.set_string("compositions", encoded);
    }
    if let Some(original) = &app.composition.recovery_backup {
        storage.set_string("compositions.recovery_backup", original.clone());
    }
    storage.set_string(
        "active_composition",
        app.composition.active_index.to_string(),
    );
    if let Some(composition) = app.composition.items.get(app.composition.active_index) {
        storage.set_string("active_composition_id", composition.id.clone());
    }
    if let Some(history) = operation_history {
        storage.set_string("operation_history", history);
    }
}
