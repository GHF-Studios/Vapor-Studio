use super::super::components::design::{
    StatusTone, installation_status, page_header, primary_button, status_text,
};
use super::super::workshop::WorkshopTab;
use super::super::workspace_ops::WorkspacePanel;
use super::super::{Section, StudioApp, content_selector};
use eframe::egui;
use vapor_studio_core::InstallationState;

struct HomeProfileSummary {
    name: String,
    root_package: Option<vapor_studio_core::ContentIdentity>,
    installation_status_text: &'static str,
    installation_tone: super::super::components::design::StatusTone,
    play_blocked_by_out_of_date: bool,
    has_workspace_source: bool,
    needs_review: bool,
}
impl StudioApp {
    pub(in crate::app) fn draw_home(&mut self, ui: &mut egui::Ui) {
        let profile = {
            let profile = &self.profile.items[self.profile.active_index];
            let (status, tone) = installation_status(&profile.installation);
            HomeProfileSummary {
                name: profile.name.clone(),
                root_package: profile.root_package.clone(),
                installation_status_text: status,
                installation_tone: tone,
                play_blocked_by_out_of_date: matches!(
                    &profile.installation,
                    InstallationState::OutOfDate { .. }
                ),
                has_workspace_source: profile.workspace.is_some(),
                needs_review: matches!(
                    &profile.installation,
                    InstallationState::OutOfDate { .. }
                        | InstallationState::Partial { .. }
                        | InstallationState::NeedsRepair { .. }
                ),
            }
        };
        page_header(
            ui,
            "Play",
            "Choose a game setup and launch",
            "Your profile keeps the game setup you want to play. Find a ready-made setup, add it to this profile, then launch it here.",
        );
        ui.add_space(8.0);
        let card_style = ui.style().clone();
        egui::Frame::group(&card_style).show(ui, |ui| self.draw_home_profile_card(ui, &profile));

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            let mut find_clicked = false;
            ui.add_enabled_ui(!self.operation.running, |ui| {
                find_clicked = primary_button(ui, "Find game setups and add-ons").clicked();
            });
            if find_clicked {
                self.open_workshop_tab(WorkshopTab::Browse);
                self.search_workshop();
            }
            if ui
                .add_enabled(
                    !self.operation.running,
                    egui::Button::new("Browse content on this device"),
                )
                .clicked()
            {
                self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                self.discover();
            }
            if ui.button("Create content").clicked() {
                self.activate_navigation_section(Section::Create);
            }
            if ui.button("My projects").clicked() {
                self.activate_navigation_section(Section::Workspace);
            }
        });

        ui.collapsing("Project tools", |ui| {
            self.draw_home_workspace_card(ui);
        });
    }

    fn draw_home_profile_card(&mut self, ui: &mut egui::Ui, profile: &HomeProfileSummary) {
        ui.small("PLAY PROFILE");
        ui.heading(&profile.name);
        if let Some(root) = profile.root_package.as_ref() {
            let selector = content_selector(root);
            ui.small("CURRENT GAME SETUP");
            ui.strong(&root.id);
            if let Some(version) = root.version.as_deref() {
                ui.weak(format!("Version {version}"));
            }
            ui.weak("A play profile remembers the game setup and launch settings you want to use.");
            egui::CollapsingHeader::new("Technical setup details")
                .id_salt("home-profile-setup-details")
                .show(ui, |ui| {
                    ui.monospace(selector.clone());
                    ui.small("Exact content selector");
                });
            status_text(
                ui,
                profile.installation_status_text,
                profile.installation_tone,
            );
            let can_play = !self.operation.running && !profile.play_blocked_by_out_of_date;
            let mut play_clicked = false;
            ui.add_enabled_ui(can_play, |ui| {
                play_clicked = primary_button(ui, "Launch").clicked();
            })
            .response
            .on_disabled_hover_text(if self.operation.running {
                "Wait for the current operation to finish before launching."
            } else {
                "This profile's saved game setup differs from what is installed. Review or update it before launching."
            });
            if play_clicked {
                self.run_installed_selected();
            }
            ui.horizontal_wrapped(|ui| {
                if profile.has_workspace_source
                    && ui
                        .add_enabled(
                            !self.operation.running && !self.has_unsaved_workspace_edits(),
                            egui::Button::new("Build and launch project"),
                        )
                        .on_disabled_hover_text(
                            "Save or discard project edits before building and launching the current files.",
                        )
                        .clicked()
                {
                    self.run_selected();
                }
                if profile.needs_review && ui.button("Review or repair this setup").clicked() {
                    self.activate_navigation_section(Section::Library);
                }
                if ui.small_button("Find more content on Steam Workshop").clicked() {
                    self.open_workshop_tab(WorkshopTab::Browse);
                }
            });
        } else {
            ui.weak("Choose a game setup for this profile to make Launch available.");
            if primary_button(ui, "Find a game setup").clicked() {
                self.open_workshop_tab(WorkshopTab::Browse);
                self.search_workshop();
            }
            if !self.workspace.unregistered_projects.candidates.is_empty() {
                ui.weak(format!(
                    "{} projects found in this folder. Add one to Studio, or create a new project.",
                    self.workspace.unregistered_projects.candidates.len(),
                ));
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Review projects in this folder"),
                    )
                    .clicked()
                {
                    self.navigate_to_workspace_content();
                }
            } else {
                ui.weak("Create or find a game setup, then choose it for this play profile.");
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Browse this device"),
                    )
                    .clicked()
                {
                    self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                    self.discover();
                }
                if ui.button("Browse Steam Workshop").clicked() {
                    self.open_workshop_tab(WorkshopTab::Browse);
                }
            });
        }
        if ui
            .button("Manage profiles")
            .on_hover_text("Each play profile remembers a game setup and its launch settings.")
            .clicked()
        {
            self.activate_navigation_section(Section::Library);
        }
    }

    fn draw_home_workspace_card(&mut self, ui: &mut egui::Ui) {
        ui.small("CONTINUE CREATING");
        if self.workspace.root.trim().is_empty() {
            ui.heading("Create or edit content");
            ui.label("A project folder holds the files for content you make. Start from a template or open an existing project.");
            if primary_button(ui, "Start from a template").clicked() {
                self.activate_navigation_section(Section::Create);
            }
            if ui.button("Open an existing project").clicked() {
                self.open_workspace_picker();
            }
        } else {
            let workspace_name = std::path::Path::new(&self.workspace.root)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&self.workspace.root);
            ui.heading(workspace_name);
            ui.small(&self.workspace.root);
            if self.has_unsaved_workspace_edits() {
                status_text(ui, "Unsaved edits need attention", StatusTone::Caution);
            } else {
                ui.weak("Project folder open");
            }
            ui.small(if self.workspace.unregistered_scan_completed {
                format!(
                    "{} available in Studio · {} projects to add",
                    self.catalog.sources.len(),
                    self.workspace.unregistered_projects.candidates.len(),
                )
            } else {
                "Project list has not been refreshed yet".to_owned()
            });
            let mut project_issues = Vec::new();
            if let Some(error) = self.workspace.workspace_validation_error.as_deref() {
                project_issues.push(format!("Project folder needs repair: {error}"));
            }
            if let Some(error) = self.workspace.unregistered_scan_error.as_deref() {
                project_issues.push(format!(
                    "Project folder scan failed; counts may be stale: {error}"
                ));
            }
            if let Some(error) = self.catalog.source_discovery_error.as_deref() {
                project_issues.push(format!(
                    "The project list could not refresh; {}: {error}",
                    if self.catalog.source_discovery_loaded {
                        "showing the last successful list"
                    } else {
                        "no successful list is available yet"
                    }
                ));
            }
            if let Some(error) = self.catalog.catalog_discovery_error.as_deref() {
                project_issues.push(format!(
                    "The on-device content list could not refresh; {}: {error}",
                    if self.catalog.catalog_discovery_loaded {
                        "showing the last successful list"
                    } else {
                        "no successful list is available yet"
                    }
                ));
            }
            if let Some(error) = self.workspace.editor.files_refresh_error.as_deref() {
                project_issues.push(format!("Project files could not refresh: {error}"));
            }
            if !project_issues.is_empty() {
                let mut review_project = false;
                egui::CollapsingHeader::new(format!(
                    "Project needs attention · {} issue{}",
                    project_issues.len(),
                    if project_issues.len() == 1 { "" } else { "s" }
                ))
                .default_open(false)
                .show(ui, |ui| {
                    for issue in &project_issues {
                        status_text(ui, issue, StatusTone::Caution);
                    }
                    if ui.button("Review project").clicked() {
                        review_project = true;
                    }
                });
                if review_project {
                    self.activate_navigation_section(Section::Workspace);
                }
            }
            let mut open_project_files = false;
            ui.add_enabled_ui(!self.operation.running, |ui| {
                open_project_files = primary_button(ui, "Open project files").clicked();
            });
            if open_project_files {
                self.open_workspace_files();
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Manage projects").clicked() {
                    self.open_workspace_panel(WorkspacePanel::Sources);
                }
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Create content"),
                    )
                    .clicked()
                {
                    self.activate_navigation_section(Section::Create);
                }
            });
        }
    }
}
