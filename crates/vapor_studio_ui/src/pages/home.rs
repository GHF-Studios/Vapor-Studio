use super::super::components::design::{
    StatusTone, installation_status, page_header, primary_button, primary_button_enabled,
    status_text,
};
use super::super::workshop::WorkshopTab;
use super::super::workspace_ops::WorkspacePanel;
use super::super::{Section, StudioApp};
use eframe::egui;
use vapor_client_application::InstallationState;

struct HomeCompositionSummary {
    name: String,
    root_package: Option<vapor_client_application::ContentReference>,
    installation_status_text: &'static str,
    installation_tone: super::super::components::design::StatusTone,
    workspace_source_path: Option<std::path::PathBuf>,
    primary_action: HomePrimaryAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HomePrimaryAction {
    Install,
    OpenProject,
    BuildAndLaunch,
    Launch,
    ReviewUpdate,
    ReviewRepair,
}

fn home_primary_action(
    installation: &InstallationState,
    can_build_from_source: bool,
    source_workspace_is_open: bool,
) -> HomePrimaryAction {
    match installation {
        InstallationState::Installed { .. } => HomePrimaryAction::Launch,
        InstallationState::OutOfDate { .. } => HomePrimaryAction::ReviewUpdate,
        InstallationState::Partial { .. } | InstallationState::NeedsRepair { .. } => {
            HomePrimaryAction::ReviewRepair
        }
        InstallationState::Unknown | InstallationState::NotInstalled
            if can_build_from_source && source_workspace_is_open =>
        {
            HomePrimaryAction::BuildAndLaunch
        }
        InstallationState::Unknown | InstallationState::NotInstalled if can_build_from_source => {
            HomePrimaryAction::OpenProject
        }
        InstallationState::Unknown | InstallationState::NotInstalled => HomePrimaryAction::Install,
    }
}
impl StudioApp {
    pub(in crate::app) fn draw_home(&mut self, ui: &mut egui::Ui) {
        let composition = {
            let composition = &self.composition.items[self.composition.active_index];
            let (status, tone) = installation_status(&composition.installation);
            let can_build_from_source = self.composition_root_is_workspace_source(composition)
                && composition.workspace.is_some();
            let workspace_source_path = composition
                .workspace
                .as_ref()
                .map(|workspace| workspace.root.clone());
            let workspace_source_is_open = self.composition_workspace_root_is_open(composition);
            HomeCompositionSummary {
                name: composition.name.clone(),
                root_package: composition.root_package.clone(),
                installation_status_text: status,
                installation_tone: tone,
                workspace_source_path,
                primary_action: home_primary_action(
                    &composition.installation,
                    can_build_from_source,
                    workspace_source_is_open,
                ),
            }
        };
        page_header(
            ui,
            "Play",
            "Install or launch a composition",
            "Start a complete setup, discover more content, or create your own.",
        );
        ui.add_space(8.0);
        let card_style = ui.style().clone();
        egui::Frame::group(&card_style)
            .show(ui, |ui| self.draw_home_composition_card(ui, &composition));

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            let mut find_clicked = false;
            ui.add_enabled_ui(!self.operation.running, |ui| {
                find_clicked = primary_button(ui, "Discover content").clicked();
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

    fn draw_home_composition_card(
        &mut self,
        ui: &mut egui::Ui,
        composition: &HomeCompositionSummary,
    ) {
        ui.small("ACTIVE COMPOSITION");
        ui.heading(&composition.name);
        if composition.root_package.is_some() {
            status_text(
                ui,
                composition.installation_status_text,
                composition.installation_tone,
            );
            let service_connected = self.service.is_some();
            let (label, enabled) = match composition.primary_action {
                HomePrimaryAction::Install => (
                    "Install composition",
                    service_connected
                        && !self.operation.running
                        && !self.has_unsaved_workspace_edits(),
                ),
                HomePrimaryAction::OpenProject => (
                    "Open project to build and launch",
                    service_connected
                        && !self.operation.running
                        && !self.has_unsaved_context_switch_edits(),
                ),
                HomePrimaryAction::BuildAndLaunch => (
                    "Build and launch project",
                    service_connected
                        && !self.operation.running
                        && self
                            .service_action_enabled("composition.build-and-launch")
                            .unwrap_or(true)
                        && !self.has_unsaved_workspace_edits(),
                ),
                HomePrimaryAction::Launch => (
                    "Launch",
                    service_connected
                        && !self.operation.running
                        && self
                            .service_action_enabled("composition.launch")
                            .unwrap_or(true),
                ),
                HomePrimaryAction::ReviewUpdate => ("Review update", true),
                HomePrimaryAction::ReviewRepair => ("Review or repair composition", true),
            };
            let disabled_reason = if self.service.is_none() {
                "Reconnect to Vapor before starting this action."
            } else if self.operation.running {
                "Wait for the current operation to finish."
            } else if self.has_unsaved_context_switch_edits() {
                "Save or discard current project or composition edits before continuing."
            } else {
                "Review this composition before continuing."
            };
            if primary_button_enabled(ui, label, enabled)
                .on_disabled_hover_text(disabled_reason)
                .clicked()
            {
                match composition.primary_action {
                    HomePrimaryAction::Install => self.install_selected(),
                    HomePrimaryAction::OpenProject => {
                        if let Some(path) = composition.workspace_source_path.as_ref() {
                            self.open_workspace_path_to(path.clone(), Section::Home);
                        }
                    }
                    HomePrimaryAction::BuildAndLaunch => self.run_selected(),
                    HomePrimaryAction::Launch => self.run_installed_selected(),
                    HomePrimaryAction::ReviewUpdate | HomePrimaryAction::ReviewRepair => {
                        self.activate_navigation_section(Section::Compositions);
                    }
                }
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .small_button("Find more content on Steam Workshop")
                    .clicked()
                {
                    self.open_workshop_tab(WorkshopTab::Browse);
                }
            });
        } else {
            ui.weak("Choose a playable composition to enable Launch.");
            if primary_button(ui, "Find a playable composition").clicked() {
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
                ui.weak("Find a playable composition, or create your own.");
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
            .button("Manage compositions")
            .on_hover_text("Each composition has its own content and launch settings.")
            .clicked()
        {
            self.activate_navigation_section(Section::Compositions);
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
#[cfg(test)]
mod home_primary_action_tests {
    use super::{HomePrimaryAction, home_primary_action};
    use vapor_client_application::InstallationState;

    #[test]
    fn uninstalled_composition_offers_install_or_project_build() {
        assert_eq!(
            home_primary_action(&InstallationState::NotInstalled, false, false),
            HomePrimaryAction::Install
        );
        assert_eq!(
            home_primary_action(&InstallationState::NotInstalled, true, false),
            HomePrimaryAction::OpenProject
        );
        assert_eq!(
            home_primary_action(&InstallationState::NotInstalled, true, true),
            HomePrimaryAction::BuildAndLaunch
        );
        assert_eq!(
            home_primary_action(&InstallationState::NotInstalled, false, true),
            HomePrimaryAction::Install
        );
    }

    #[test]
    fn incomplete_or_stale_compositions_route_to_review() {
        assert_eq!(
            home_primary_action(
                &InstallationState::Partial {
                    detail: "partial".into()
                },
                false,
                false,
            ),
            HomePrimaryAction::ReviewRepair
        );
        assert_eq!(
            home_primary_action(
                &InstallationState::NeedsRepair {
                    detail: "repair".into(),
                    artifacts: Vec::new(),
                },
                false,
                false,
            ),
            HomePrimaryAction::ReviewRepair
        );
        assert_eq!(
            home_primary_action(
                &InstallationState::OutOfDate {
                    detail: "changed".into(),
                    actual_graph: vapor_client_application::ResolvedCompositionGraph {
                        root: vapor_client_application::ContentReference {
                            id: "game.pack".into(),
                            version: Some("1.0.0".into()),
                        },
                        effective_engine: vapor_client_application::ContentReference {
                            id: "engine.pack".into(),
                            version: Some("1.0.0".into()),
                        },
                        effective_game: None,
                        nodes: Vec::new(),
                    },
                    artifacts: Vec::new(),
                    verified: true,
                },
                false,
                false,
            ),
            HomePrimaryAction::ReviewUpdate
        );
    }

    #[test]
    fn installed_and_unchecked_compositions_offer_state_appropriate_actions() {
        assert_eq!(
            home_primary_action(
                &InstallationState::Installed {
                    artifacts: Vec::new(),
                    graph_fingerprint: None,
                },
                false,
                false,
            ),
            HomePrimaryAction::Launch
        );
        assert_eq!(
            home_primary_action(&InstallationState::Unknown, false, false),
            HomePrimaryAction::Install
        );
        assert_eq!(
            home_primary_action(&InstallationState::Unknown, true, false),
            HomePrimaryAction::OpenProject
        );
        assert_eq!(
            home_primary_action(&InstallationState::Unknown, true, true),
            HomePrimaryAction::BuildAndLaunch
        );
    }
}
