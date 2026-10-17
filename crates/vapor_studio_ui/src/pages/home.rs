use super::super::components::design::{
    installation_status, page_header, primary_button, status_text,
};
use super::super::operation_center::operation_kind_label;
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
    installed: bool,
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
                installed: matches!(&profile.installation, InstallationState::Installed { .. }),
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
            "Start",
            "Home",
            "Play your selected composition or continue working on content.",
        );
        ui.add_space(8.0);

        let card_style = ui.style().clone();
        if ui.available_width() < 820.0 {
            egui::Frame::group(&card_style)
                .show(ui, |ui| self.draw_home_profile_card(ui, &profile));
            ui.add_space(10.0);
            egui::Frame::group(&card_style).show(ui, |ui| {
                self.draw_home_workspace_card(ui, profile.root_package.is_some())
            });
        } else {
            ui.columns(2, |columns| {
                egui::Frame::group(&card_style).show(&mut columns[0], |ui| {
                    self.draw_home_profile_card(ui, &profile);
                });
                egui::Frame::group(&card_style).show(&mut columns[1], |ui| {
                    self.draw_home_workspace_card(ui, profile.root_package.is_some());
                });
            });
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.heading("Recent activity");
            if self.operation.running {
                ui.spinner();
                ui.label("An operation is in progress. Details and progress stay available below.");
            }
        });
        if self.operation.operation_history.is_empty() {
            ui.weak(
                "Completed installs, builds, validation, and publishing actions will appear here.",
            );
        } else {
            let recent_activity = self
                .operation
                .operation_history
                .iter()
                .take(5)
                .map(|entry| {
                    (
                        entry.event.kind.clone(),
                        entry.event.status.clone(),
                        entry.completed_at_unix,
                    )
                })
                .collect::<Vec<_>>();
            let mut open_destination = None;
            let card_style = ui.style().clone();
            for (kind, status, completed_at) in recent_activity {
                egui::Frame::group(&card_style).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(operation_kind_label(&kind));
                        match &status {
                            vapor_studio_core::OperationStatus::Succeeded { summary } => {
                                ui.colored_label(egui::Color32::from_rgb(120, 210, 150), summary);
                            }
                            vapor_studio_core::OperationStatus::Failed { message } => {
                                ui.colored_label(egui::Color32::LIGHT_RED, message);
                            }
                            vapor_studio_core::OperationStatus::Cancelled => {
                                ui.weak("Cancelled");
                            }
                            vapor_studio_core::OperationStatus::Running => {
                                ui.weak("In progress");
                            }
                        }
                        ui.weak(super::super::operations::elapsed_label(completed_at));
                        if ui
                            .small_button("Open")
                            .on_hover_text("Return to the screen for this operation")
                            .clicked()
                        {
                            open_destination = Some(operation_destination(&kind));
                        }
                    });
                });
            }
            if let Some(destination) = open_destination {
                self.open_operation_destination(destination);
            }
        }
    }

    fn draw_home_profile_card(&mut self, ui: &mut egui::Ui, profile: &HomeProfileSummary) {
        ui.small("SELECTED PROFILE");
        ui.heading(&profile.name);
        if let Some(root) = profile.root_package.as_ref() {
            let selector = content_selector(root);
            ui.label(&selector);
            status_text(
                ui,
                profile.installation_status_text,
                profile.installation_tone,
            );
            let source_ready = !self.has_unsaved_workspace_edits()
                && self.profile_composition.preview().is_some_and(|preview| {
                    preview.selector == selector
                        && preview.ready()
                        && preview.nodes.last().is_some_and(|node| {
                            node.source == vapor_studio_core::CompositionPreviewSource::Workspace
                        })
                });
            let action = if profile.installed {
                "Play composition"
            } else if profile.needs_review {
                "Review installation"
            } else if source_ready {
                "Build and play"
            } else {
                "Set up composition"
            };
            let can_play = !self.operation.running
                && (profile.needs_review
                    || profile.installed
                    || source_ready
                    || !self.has_unsaved_workspace_edits());
            let mut play_clicked = false;
            ui.add_enabled_ui(can_play, |ui| {
                play_clicked = primary_button(ui, action).clicked();
            });
            if play_clicked {
                if profile.needs_review {
                    self.activate_navigation_section(Section::Library);
                } else if profile.installed {
                    self.run_installed_selected();
                } else if source_ready {
                    self.run_selected();
                } else {
                    self.activate_navigation_section(Section::Library);
                    self.preview_selected_composition();
                }
            }
            if profile.needs_review {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "This installation needs review before it can be played. Open Profiles for verification or repair.",
                );
            }
        } else {
            if !self.workspace.unregistered_projects.candidates.is_empty() {
                ui.weak(format!(
                    "{} authored projects found. Adopt a Packagepack or create one to make this profile playable.",
                    self.workspace.unregistered_projects.candidates.len(),
                ));
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Review workspace content"),
                    )
                    .clicked()
                {
                    self.navigate_to_workspace_content();
                }
            } else {
                ui.weak("Choose a Packagepack to create your first launchable composition.");
            }
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Browse content"),
                )
                .clicked()
            {
                self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                self.discover();
            }
        }
        if ui.button("Manage play profiles").clicked() {
            self.activate_navigation_section(Section::Library);
        }
    }

    fn draw_home_workspace_card(&mut self, ui: &mut egui::Ui, has_profile_root: bool) {
        ui.small("CONTINUE CREATING");
        if self.workspace.root.trim().is_empty() {
            ui.heading("Start a workspace");
            ui.label("Create a starter workspace or open existing source to author content.");
            if ui.button("Create a workspace").clicked() {
                self.activate_navigation_section(Section::Create);
            }
            if ui.button("Open a workspace").clicked() {
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
                ui.colored_label(egui::Color32::YELLOW, "Unsaved edits need attention");
            } else {
                ui.weak("Workspace open");
            }
            ui.small(if self.workspace.unregistered_scan_completed {
                format!(
                    "{} registered · {} found to adopt",
                    self.catalog.sources.len(),
                    self.workspace.unregistered_projects.candidates.len(),
                )
            } else {
                "Content discovery has not run yet".to_owned()
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.operation.running,
                        egui::Button::new("Source projects"),
                    )
                    .clicked()
                {
                    self.open_workspace_panel(WorkspacePanel::Sources);
                }
                let content_label = if self.workspace.unregistered_scan_completed {
                    "Workspace content"
                } else {
                    "Find workspace content"
                };
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new(content_label),
                    )
                    .clicked()
                {
                    self.navigate_to_workspace_content();
                }
                if ui
                    .add_enabled(!self.operation.running, egui::Button::new("Open files"))
                    .clicked()
                {
                    self.open_workspace_files();
                }
                if ui
                    .add_enabled(!self.operation.running, egui::Button::new("Build"))
                    .clicked()
                {
                    self.open_workspace_panel(WorkspacePanel::Build);
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
        if has_profile_root
            && ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Browse content"),
                )
                .clicked()
        {
            self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
            self.discover();
        }
    }
}

enum OperationDestination {
    Section(Section),
    LocalDiscover,
    Workshop(WorkshopTab),
    Workspace(WorkspacePanel),
    WorkspaceFiles,
}

impl StudioApp {
    fn open_operation_destination(&mut self, destination: OperationDestination) {
        match destination {
            OperationDestination::Section(section) => {
                self.activate_navigation_section(section);
            }
            OperationDestination::LocalDiscover => {
                self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
            }
            OperationDestination::Workshop(tab) => {
                self.open_workshop_tab(tab);
            }
            OperationDestination::Workspace(panel) => {
                self.open_workspace_panel(panel);
            }
            OperationDestination::WorkspaceFiles => {
                self.open_workspace_files();
            }
        }
    }
}

fn operation_destination(kind: &vapor_studio_core::OperationKind) -> OperationDestination {
    use vapor_studio_core::OperationKind;
    match kind {
        OperationKind::Discover => OperationDestination::LocalDiscover,
        OperationKind::SearchWorkshop => OperationDestination::Workshop(WorkshopTab::Browse),
        OperationKind::LoadWorkshopSubscriptions | OperationKind::ChangeWorkshopSubscription => {
            OperationDestination::Workshop(WorkshopTab::Subscriptions)
        }
        OperationKind::Acquire => OperationDestination::Workshop(WorkshopTab::Imports),
        OperationKind::Resolve
        | OperationKind::Install
        | OperationKind::Verify
        | OperationKind::Update
        | OperationKind::Repair
        | OperationKind::Run
        | OperationKind::RunInstalled => OperationDestination::Section(Section::Library),
        OperationKind::CreateContent => OperationDestination::WorkspaceFiles,
        OperationKind::CreateWorkspace => OperationDestination::Workspace(WorkspacePanel::Sources),
        OperationKind::StageWorkshop
        | OperationKind::PublishWorkshop
        | OperationKind::StageRootDistribution
        | OperationKind::PublishRootDistribution => OperationDestination::Section(Section::Publish),
        OperationKind::LoadComposition | OperationKind::SaveComposition => {
            OperationDestination::Section(Section::Compose)
        }
        OperationKind::LoadWorkshopMetadata
        | OperationKind::SaveWorkshopMetadata
        | OperationKind::LoadRuntimeOutputs
        | OperationKind::SaveRuntimeOutputs
        | OperationKind::AdoptContentProject
        | OperationKind::UnregisterContentProject
        | OperationKind::DiscoverContentProjects
        | OperationKind::Validate => OperationDestination::Workspace(WorkspacePanel::Content),
        OperationKind::CreateWorkspaceFile
        | OperationKind::OpenWorkspaceFile
        | OperationKind::SaveWorkspaceFile => OperationDestination::WorkspaceFiles,
        OperationKind::OpenWorkspace
        | OperationKind::RememberWorkspace
        | OperationKind::DiscoverWorkspace => {
            OperationDestination::Workspace(WorkspacePanel::Sources)
        }
        OperationKind::Fmt
        | OperationKind::Check
        | OperationKind::Test
        | OperationKind::Build
        | OperationKind::Clean => OperationDestination::Workspace(WorkspacePanel::Build),
    }
}
